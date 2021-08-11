mod data;
mod history;
mod tiling;

use self::{data::Data, history::History, tiling::Tiling};
use crate::{
    common::{Color, Path, PathId, RenderablePath, Tool},
    view::View,
    web,
};
use delegate_attr::delegate;
use geo::{prelude::*, Coordinate, Line, Rect};
use getset::Getters;
use rustc_hash::FxHashSet;
use std::{iter, mem};

#[derive(Debug)]
enum Command {
    Add {
        path_ids: Vec<PathId>,
    },
    Move {
        path_ids: Vec<PathId>,
        delta: Coordinate<i32>,
    },
    Remove {
        paths: Vec<(PathId, RenderablePath)>,
    },
}

#[derive(Getters)]
pub struct Model {
    #[getset(get = "pub")]
    view: View,
    data: Data,
    tiling: Tiling,
    delta: Coordinate<i32>,
    selected_path_ids: FxHashSet<PathId>,
    hidden_path_ids: FxHashSet<PathId>,
    history: History<Command>,
}

#[delegate(self.data)]
#[rustfmt::skip]
impl Model {
    pub fn offset(&self) -> Coordinate<i32>;
    pub fn tool(&self) -> Tool;
    pub fn pen_color(&self) -> Color;
    pub fn set_pen_color(&mut self, color: Color);
}

#[delegate(self.tiling)]
#[rustfmt::skip]
impl Model {
    pub fn bounding_tile_items(
        &self,
        geo: impl BoundingRect<i32, Output = Rect<i32>> + Intersects<Rect<i32>>,
    ) -> impl Iterator<Item = (PathId, &[Line<i32>])>;
}

impl Model {
    pub fn load_from_storage(mut view: View, storage: web::Storage) -> Self {
        let data = Data::load_from_storage(storage);

        view.set_tool(data.tool());
        view.set_pen_color(data.pen_color());
        view.translate(data.offset());

        let mut tiling = Tiling::default();
        for (id, path) in data.paths() {
            tiling.insert_path(id, &path.line_string);
            view.render_path(path);
        }

        Self {
            view,
            data,
            tiling,
            delta: Coordinate::default(),
            selected_path_ids: FxHashSet::default(),
            hidden_path_ids: FxHashSet::default(),
            history: History::default(),
        }
    }

    pub fn screen(&self) -> Rect<i32> {
        let origin = -self.data.offset() - self.delta;
        Rect::new(origin, origin + self.view.size())
    }

    pub fn fix_scroll(&mut self) {
        self.data.add_offset(self.delta);
        self.delta = Coordinate::default();
    }

    pub fn selected_paths(&self) -> impl Iterator<Item = (PathId, &RenderablePath)> {
        self.selected_path_ids
            .iter()
            .map(move |&id| (id, self.data.path(id)))
    }

    pub fn rerender_main(&self) {
        self.view.clear_main_canvas();
        let ids = self
            .tiling
            .bounding_tile_items(self.screen())
            .map(|(id, _)| id)
            .filter(|id| !self.hidden_path_ids.contains(id))
            .collect::<FxHashSet<_>>(); // remove duplicates to prevent double rendering
        for id in ids {
            let path = self.data.path(id);
            self.view.render_path(path);
        }
    }

    pub fn rerender_sub(&self) {
        self.view.clear_sub_canvas();
        for (_, path) in self
            .selected_paths()
            .filter(|(id, _)| !self.hidden_path_ids.contains(id))
        {
            self.view.render_bounding_rect(path);
        }
    }

    pub fn rerender(&self) {
        self.rerender_main();
        self.rerender_sub();
    }

    pub fn updater(&mut self) -> Updater<'_> {
        Updater::new(self)
    }
}

pub struct Updater<'a> {
    pub model: &'a mut Model,
    should_rerender_main: bool,
    should_rerender_sub: bool,
}

impl Drop for Updater<'_> {
    fn drop(&mut self) {
        if self.should_rerender_main {
            self.model.rerender_main();
        }
        if self.should_rerender_sub {
            self.model.rerender_sub();
        }
    }
}

impl<'a> Updater<'a> {
    fn new(model: &'a mut Model) -> Self {
        Self {
            model,
            should_rerender_main: false,
            should_rerender_sub: false,
        }
    }

    pub fn skip_rerendering(self) {
        mem::forget(self);
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.model.data.set_tool(tool);
        if tool != Tool::Selector {
            self.unselect_all_paths();
            self.should_rerender_sub = true;
        }
    }

    pub fn hide_paths(&mut self, ids: impl IntoIterator<Item = PathId>) {
        self.model.hidden_path_ids.extend(ids);
        self.should_rerender_main = true;
        self.should_rerender_sub = true;
    }

    pub fn unhide_paths(&mut self, ids: impl IntoIterator<Item = PathId>) {
        for id in ids {
            self.model.hidden_path_ids.remove(&id);
        }
        self.should_rerender_main = true;
        self.should_rerender_sub = true;
    }

    pub fn select_paths(&mut self, rect: Rect<i32>, added_part: Rect<i32>) {
        let ids = self
            .model
            .tiling
            .bounding_tile_items(added_part)
            .filter({
                let data = &self.model.data;
                move |&(id, _)| {
                    let lines = &data.path(id).line_string;
                    rect.contains(&lines.bounding_rect().expect("empty path"))
                }
            })
            .map(|(id, _)| id);
        let prev_len = self.model.selected_path_ids.len();
        self.model.selected_path_ids.extend(ids);
        if self.model.selected_path_ids.len() != prev_len {
            self.should_rerender_sub = true;
        }
    }

    pub fn unselect_paths(&mut self, rect: Rect<i32>) {
        let ids = self
            .model
            .tiling
            .bounding_tile_items(rect)
            .filter(|(_, lines)| lines.iter().any(|line| line.intersects(&rect)))
            .map(|(id, _)| id);
        let prev_len = self.model.selected_path_ids.len();
        for id in ids {
            self.model.selected_path_ids.remove(&id);
        }
        if self.model.selected_path_ids.len() != prev_len {
            self.should_rerender_sub = true;
        }
    }

    pub fn unselect_all_paths(&mut self) {
        self.model.selected_path_ids.clear();
    }

    pub fn add_paths(&mut self, paths: impl IntoIterator<Item = (PathId, RenderablePath)>) {
        let paths = paths.into_iter();
        let mut ids = Vec::with_capacity(paths.size_hint().0);
        let paths = paths.inspect({
            let tiling = &mut self.model.tiling;
            let ids = &mut ids;
            move |&(id, ref path)| {
                tiling.insert_path(id, &path.line_string);
                ids.push(id);
            }
        });
        self.model.data.add_paths(paths);
        self.model.history.push(Command::Add { path_ids: ids });
        self.should_rerender_main = true;
    }

    pub fn add_new_path(&mut self, path: Path) {
        let path = (PathId::gen(), RenderablePath::new(path));
        self.add_paths(iter::once(path));
    }

    pub fn move_paths(&mut self, ids: impl IntoIterator<Item = PathId>, delta: Coordinate<i32>) {
        let ids = ids.into_iter().collect::<Vec<_>>();
        self.model.data.move_paths(ids.iter().copied(), delta);
        for &id in &ids {
            self.model.tiling.remove_path(id);
        }
        for &id in &ids {
            self.model
                .tiling
                .insert_path(id, &self.model.data.path(id).line_string);
        }
        self.model.history.push(Command::Move {
            path_ids: ids,
            delta,
        });
        self.should_rerender_main = true;
        self.should_rerender_sub = true;
    }

    pub fn remove_paths(&mut self, ids: impl IntoIterator<Item = PathId>) {
        let paths = self.model.data.remove_paths(ids);
        for &(id, _) in &paths {
            self.model.tiling.remove_path(id);
            self.model.selected_path_ids.remove(&id);
            self.model.hidden_path_ids.remove(&id);
        }
        self.model.history.push(Command::Remove { paths });
        self.should_rerender_main = true;
        self.should_rerender_sub = true;
    }

    pub fn remove_selected_paths(&mut self) {
        let mut ids = mem::take(&mut self.model.selected_path_ids);
        self.remove_paths(ids.drain());
        self.model.selected_path_ids = ids; // restore capacity
    }

    pub fn clear(&mut self) {
        self.model.tiling.clear();
        self.model.selected_path_ids.clear();
        self.model.hidden_path_ids.clear();
        let paths = self.model.data.remove_all_paths();
        self.model.history.push(Command::Remove { paths });
        self.should_rerender_main = true;
        self.should_rerender_sub = true;
    }

    pub fn scroll(&mut self, delta: Coordinate<i32>) {
        self.model.delta = self.model.delta + delta;
        self.model.view.translate(delta);
        self.should_rerender_main = true;
        self.should_rerender_sub = true;
    }

    fn rollback(&mut self, com: Command) {
        match com {
            Command::Add { path_ids } => {
                self.remove_paths(path_ids);
            }
            Command::Move { path_ids, delta } => {
                self.move_paths(path_ids, -delta);
            }
            Command::Remove { paths } => {
                self.add_paths(paths);
            }
        }
    }

    pub fn undo(&mut self) {
        if let Some(com) = self.model.history.start_undo() {
            self.rollback(com);
        }
    }

    pub fn redo(&mut self) {
        if let Some(com) = self.model.history.start_redo() {
            self.rollback(com);
        }
    }
}
