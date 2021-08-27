//! A model, which manages the application data and requests the view to render objects.

mod compat;
mod history;
mod recorder;
mod tiling;

use self::{history::History, recorder::Recorder, tiling::Tiling};
use crate::{
    common::{Color, OnScreen, PathId, RenderablePath, Tool},
    utils,
    view::{Layer, LayerHandle, View},
    web,
};
use derive_more::{Deref, DerefMut};
use geo::{prelude::*, Coordinate, Line, Rect};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use std::mem;

#[derive(Debug)]
enum Command {
    Insert {
        path_ids: Vec<PathId>,
    },
    Shift {
        path_ids: Vec<PathId>,
        delta: Coordinate<i32>,
    },
    Remove {
        paths: Vec<(PathId, RenderablePath)>,
    },
}

#[derive(Debug)]
pub struct Model {
    paths: Recorder<FxHashMap<PathId, RenderablePath>>,
    tiling: Tiling,
    history: History<Command>,

    selected_path_ids: Recorder<FxHashSet<PathId>>,
    hidden_path_ids: Recorder<FxHashSet<PathId>>,
    offset: Recorder<Coordinate<i32>>,
    tool: Recorder<Tool>,
    pen_color: Recorder<Color>,

    storage: web::Storage,
    view: View,
}

impl Model {
    fn load_field<T>(storage: &web::Storage, key: &str) -> T
    where
        T: for<'de> Deserialize<'de> + Default,
    {
        storage
            .get(&format!("papirs:{}", key))
            .transpose()
            .unwrap_or_else(|err| {
                log::error!("`{}` found in storage but failed to load: {}", key, err);
                None
            })
            .unwrap_or_default()
    }

    fn save_field<T>(storage: &web::Storage, key: &str, value: &T)
    where
        T: Serialize,
    {
        if let Err(err) = storage.set(&format!("papirs:{}", key), value) {
            log::error!("failed to save `{}`: {}", key, err);
        }
    }

    pub fn load(storage: web::Storage, mut view: View) -> Self {
        macro_rules! load {
            ($field:ident) => {
                Self::load_field(&storage, stringify!($field))
            };
        }

        let old_data = compat::Data::load_and_remove(&storage);
        let needs_to_save = old_data.is_some();

        let (paths, offset, tool, pen_color) = old_data.map_or_else(
            || (load!(paths), load!(offset), load!(tool), load!(pen_color)),
            |data| (data.paths, data.offset, data.tool, data.pen_color),
        );
        let tiling = (paths.get().iter())
            .map(|(&id, path)| (id, &path.get().get().coords))
            .collect();

        view.translate(*offset.get());
        for path in paths.get().values() {
            view.layers[Layer::Main].render_path(path);
        }
        view.select_tool(*tool.get());
        view.select_pen_color(*pen_color.get());

        let model = Self {
            paths,
            tiling,
            history: Default::default(),

            selected_path_ids: Default::default(),
            hidden_path_ids: Default::default(),
            offset,
            tool,
            pen_color,

            storage,
            view,
        };
        if needs_to_save {
            model.force_save();
        }
        model
    }

    fn force_save(&self) {
        macro_rules! save {
            ($field:ident) => {
                Self::save_field(&self.storage, stringify!($field), &self.$field);
            };
        }
        save!(paths);
        save!(offset);
        save!(tool);
        save!(pen_color);
    }

    fn save(&self) {
        macro_rules! save {
            ($field:ident) => {
                if self.$field.is_updated() {
                    Self::save_field(&self.storage, stringify!($field), &self.$field);
                }
            };
        }
        save!(paths);
        save!(offset);
        save!(tool);
        save!(pen_color);
    }

    pub fn bounding_tile_items(
        &self,
        geo: impl BoundingRect<i32, Output = Rect<i32>> + Intersects<Rect<i32>>,
    ) -> impl Iterator<Item = (PathId, &[Line<i32>])> {
        self.tiling.bounding_tile_items(geo)
    }

    pub fn path(&self, id: PathId) -> &RenderablePath {
        self.paths.get().get(&id).expect("path not found")
    }

    pub fn contains_path(&self, id: PathId) -> bool {
        self.paths.get().contains_key(&id)
    }

    pub fn insert_paths(&mut self, paths: impl IntoIterator<Item = (PathId, RenderablePath)>) {
        let paths = paths.into_iter();
        self.paths.update(|p| {
            p.reserve(paths.size_hint().0);
            false
        });
        let ids = paths
            .map(|(id, path)| {
                self.tiling.insert_path(id, &path.get().get().coords);
                let old = self.paths.get_mut().insert(id, path);
                assert!(old.is_none(), "path already exists");
                id
            })
            .collect();
        self.history.push(Command::Insert { path_ids: ids });
    }

    pub fn shift_paths(&mut self, ids: impl IntoIterator<Item = PathId>, delta: Coordinate<i32>) {
        let ids = (ids.into_iter())
            .map(|id| {
                let path = self.paths.get_mut().get_mut(&id).expect("path not found");
                *path = {
                    let mut path = path.take();
                    path.coords.translate_inplace(delta.x, delta.y);
                    RenderablePath::new(path).expect("`path` should not be empty")
                };
                self.tiling.remove_path(id);
                self.tiling.insert_path(id, &path.get().get().coords);
                id
            })
            .collect();
        self.history.push(Command::Shift {
            path_ids: ids,
            delta,
        });
    }

    pub fn remove_paths(&mut self, ids: impl IntoIterator<Item = PathId>) {
        let paths = (ids.into_iter())
            .map(|id| {
                self.tiling.remove_path(id);
                self.selected_path_ids.update(|s| s.remove(&id));
                self.hidden_path_ids.update(|h| h.remove(&id));
                let path = self.paths.get_mut().remove(&id).expect("path not found");
                (id, path)
            })
            .collect();
        self.history.push(Command::Remove { paths });
    }

    pub fn remove_selected_paths(&mut self) {
        if self.selected_path_ids.get().is_empty() {
            return;
        }
        let mut ids = mem::take(self.selected_path_ids.get_mut());
        self.remove_paths(ids.drain());
        *self.selected_path_ids.get_mut() = ids; // restore capacity
    }

    pub fn clear_paths(&mut self) {
        if self.paths.get().is_empty() {
            return;
        }
        let paths = self.paths.get_mut().drain().collect();
        self.tiling.clear();
        self.selected_path_ids.get_mut().clear();
        self.hidden_path_ids.get_mut().clear();
        self.history.push(Command::Remove { paths });
    }

    pub fn selected_paths(&self) -> impl Iterator<Item = (PathId, &RenderablePath)> {
        self.selected_path_ids
            .get()
            .iter()
            .map(move |&id| (id, self.paths.get().get(&id).expect("path not found")))
    }

    /// Select paths which intersect `rect`, contained by `whole_rect`.
    pub fn select_paths_with(&mut self, whole_rect: Rect<i32>, rect: Rect<i32>) {
        let ids = (self.tiling.bounding_tile_items(rect))
            .filter({
                let paths = &self.paths;
                move |&(id, _)| {
                    let path = paths.get().get(&id).expect("path not found");
                    let coords = &path.get().get().coords;
                    whole_rect.contains(&coords.bounding_rect().expect("empty path"))
                }
            })
            .map(|(id, _)| id);
        self.selected_path_ids.update(|s| {
            let prev_len = s.len();
            s.extend(ids);
            s.len() != prev_len
        });
    }

    /// Unselect paths which intersect `rect`.
    pub fn unselect_paths_with(&mut self, rect: Rect<i32>) {
        let ids = (self.tiling.bounding_tile_items(rect))
            .filter(|(_, lines)| lines.iter().any(|line| line.intersects(&rect)))
            .map(|(id, _)| id);
        for id in ids {
            self.selected_path_ids.update(|s| s.remove(&id));
        }
    }

    pub fn unselect_all_paths(&mut self) {
        if self.selected_path_ids.get().is_empty() {
            return;
        }
        self.selected_path_ids.get_mut().clear();
    }

    pub fn hide_path(&mut self, id: PathId) {
        assert!(self.contains_path(id), "path not found");
        self.hidden_path_ids.update(|h| h.insert(id));
    }

    pub fn unhide_path(&mut self, id: PathId) {
        assert!(self.contains_path(id), "path not found");
        self.hidden_path_ids.update(|h| h.remove(&id));
    }

    fn rollback(&mut self, com: Command) {
        match com {
            Command::Insert { path_ids } => {
                self.remove_paths(path_ids);
            }
            Command::Shift { path_ids, delta } => {
                self.shift_paths(path_ids, -delta);
            }
            Command::Remove { paths } => {
                self.insert_paths(paths);
            }
        }
    }

    pub fn undo(&mut self) {
        if let Some(com) = self.history.start_undo() {
            self.rollback(com);
        }
    }

    pub fn redo(&mut self) {
        if let Some(com) = self.history.start_redo() {
            self.rollback(com);
        }
    }

    pub fn coord_at(&self, coord: OnScreen<Coordinate<i32>>) -> Coordinate<i32> {
        coord.0 - *self.offset.get()
    }

    pub fn delta_of(&self, delta: OnScreen<Coordinate<i32>>) -> Coordinate<i32> {
        // consider scaling in the future
        delta.0
    }

    fn board_rect(&self) -> Rect<i32> {
        let origin = OnScreen(Coordinate::zero());
        let diagonal = self
            .view
            .size()
            .map(|size| utils::coord_map_scalars(size, |s| s as i32));
        Rect::new(self.coord_at(origin), self.coord_at(diagonal))
    }

    pub fn scroll(&mut self, delta: Coordinate<i32>) {
        *self.offset.get_mut() = *self.offset.get() + delta;
        self.view.translate(delta);
    }

    pub fn tool(&self) -> Tool {
        *self.tool.get()
    }

    pub fn set_tool(&mut self, tool: Tool) {
        *self.tool.get_mut() = tool;
        if tool != Tool::Selector {
            self.unselect_all_paths();
        }
        self.view.select_tool(tool);
    }

    pub fn pen_color(&self) -> Color {
        *self.pen_color.get()
    }

    pub fn set_pen_color(&mut self, color: Color) {
        *self.pen_color.get_mut() = color;
        self.view.select_pen_color(color);
    }

    fn rerender_main_layer(&self) {
        self.view.layers[Layer::Main].clear();
        let ids = self
            .tiling
            .bounding_tile_items(self.board_rect())
            .map(|(id, _)| id)
            .filter(|id| !self.hidden_path_ids.get().contains(id))
            .collect::<FxHashSet<_>>(); // remove duplicates to prevent double rendering
        for id in ids {
            let path = self.paths.get().get(&id).expect("path not found");
            self.view.layers[Layer::Main].render_path(path);
        }
    }

    fn rerender_sub_layer(&self) {
        self.view.layers[Layer::Sub].clear();
        for (_, path) in self
            .selected_paths()
            .filter(|(id, _)| !self.hidden_path_ids.get().contains(id))
        {
            self.view.layers[Layer::Sub].render_bounding_rect_of(path);
        }
    }

    pub fn force_rerender(&self) {
        self.rerender_main_layer();
        self.rerender_sub_layer();
    }

    fn rerender(&mut self) {
        if self.paths.is_updated() || self.offset.is_updated() || self.hidden_path_ids.is_updated()
        {
            self.rerender_main_layer();
        }
        if self.paths.is_updated()
            || self.offset.is_updated()
            || self.selected_path_ids.is_updated()
            || self.hidden_path_ids.is_updated()
        {
            self.rerender_sub_layer();
        }
    }

    /// Performs rerendering and saves the current state to the storage.
    pub fn commit(&mut self) {
        self.rerender();
        self.save();

        self.paths.resolve();
        self.offset.resolve();
        self.tool.resolve();
        self.pen_color.resolve();
        self.selected_path_ids.resolve();
        self.hidden_path_ids.resolve();
    }

    /// Returns a wrapper struct that triggers [`commit`](Self::commit) on scope exit.
    pub fn defer_commit(&mut self) -> DeferCommit<'_> {
        DeferCommit(self)
    }

    /// Lend a layer to the controller for temporary use.
    ///
    /// The model properly handles scrolling on this layer,
    /// but will not perform any (re)rendering on this.
    pub fn temp_layer(&self) -> &LayerHandle {
        &self.view.layers[Layer::Temp]
    }
}

#[derive(Debug, Deref, DerefMut)]
#[deref(forward)]
pub struct DeferCommit<'a>(&'a mut Model);

impl Drop for DeferCommit<'_> {
    fn drop(&mut self) {
        self.0.commit();
    }
}
