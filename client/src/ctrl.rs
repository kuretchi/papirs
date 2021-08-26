//! A controller, which recieves events from the view and manipulates the model.

use crate::{
    common::{Color, OnScreen, Path, PathId, RenderablePath, Tool},
    model::{self, Model},
    utils::{self, MapScalars},
    web,
};
use enum_dispatch::enum_dispatch;
use geo::{prelude::*, Coordinate, Line, LineString, Rect};
use rustc_hash::FxHashSet;
use std::iter;

#[enum_dispatch]
trait Handler {
    fn move_to(&mut self, model: model::DeferCommit, coord: OnScreen<Coordinate<i32>>);
    fn finish(self, model: model::DeferCommit);
}

#[enum_dispatch(Handler)]
#[derive(Debug)]
enum AnyHandler {
    Scroll(ScrollHandler),
    Select(SelectHandler),
    Shift(ShiftHandler),
    Draw(DrawHandler),
    Erase(EraseHandler),
}

#[derive(Debug)]
struct ScrollHandler {
    prev_coord: OnScreen<Coordinate<i32>>,
}

impl ScrollHandler {
    pub fn new(coord: OnScreen<Coordinate<i32>>) -> Self {
        Self { prev_coord: coord }
    }
}

impl Handler for ScrollHandler {
    fn move_to(&mut self, mut model: model::DeferCommit, coord: OnScreen<Coordinate<i32>>) {
        let delta = model.delta_of(coord - self.prev_coord);
        model.scroll(delta);
        self.prev_coord = coord;
    }

    fn finish(self, _: model::DeferCommit) {}
}

#[derive(Debug)]
struct SelectHandler {
    start_coord: Coordinate<i32>,
    prev_coord: Coordinate<i32>,
}

impl SelectHandler {
    pub fn new(model: &Model, coord: OnScreen<Coordinate<i32>>) -> Self {
        let coord = model.coord_at(coord);
        Self {
            start_coord: coord,
            prev_coord: coord,
        }
    }
}

impl Handler for SelectHandler {
    fn move_to(&mut self, mut model: model::DeferCommit, coord: OnScreen<Coordinate<i32>>) {
        let coord = model.coord_at(coord);
        let whole_rect = Rect::new(self.start_coord, coord);
        let diff = utils::rect_diff(self.start_coord, self.prev_coord, coord);
        for removed_rect in diff.removed {
            model.unselect_paths_with(removed_rect);
        }
        for added_rect in diff.added {
            model.select_paths_with(whole_rect, added_rect);
        }
        model.temp_layer().clear();
        model.temp_layer().render_selection_rect(whole_rect);
        self.prev_coord = coord;
    }

    fn finish(self, model: model::DeferCommit) {
        model.temp_layer().clear();
    }
}

#[derive(Debug)]
struct ShiftHandler {
    shifting_path_ids: FxHashSet<PathId>,
    start_coord: Coordinate<i32>,
    prev_coord: Coordinate<i32>,
}

impl ShiftHandler {
    pub fn new(mut model: model::DeferCommit, coord: OnScreen<Coordinate<i32>>) -> Self {
        let coord = model.coord_at(coord);
        let shifting_path_ids = model
            .selected_paths()
            .map(|(id, _)| id)
            .collect::<FxHashSet<_>>();
        for &id in &shifting_path_ids {
            model.hide_path(id);
        }
        let this = Self {
            shifting_path_ids,
            start_coord: coord,
            prev_coord: coord,
        };
        this.rerender(&*model);
        this
    }

    fn rerender(&self, model: &Model) {
        model.temp_layer().clear();
        for &id in &self.shifting_path_ids {
            let path = model.path(id);
            model.temp_layer().render_path(path);
            model.temp_layer().render_bounding_rect_of(path);
        }
    }
}

impl Handler for ShiftHandler {
    fn move_to(&mut self, model: model::DeferCommit, coord: OnScreen<Coordinate<i32>>) {
        let coord = model.coord_at(coord);
        let delta = coord - self.prev_coord;
        model.temp_layer().translate(delta);
        self.rerender(&*model);
        self.prev_coord = coord;
    }

    fn finish(self, mut model: model::DeferCommit) {
        let delta = self.prev_coord - self.start_coord;
        model.temp_layer().clear();
        model.temp_layer().translate(-delta);
        model.shift_paths(self.shifting_path_ids.iter().copied(), delta);
        for id in self.shifting_path_ids {
            model.unhide_path(id);
        }
    }
}

#[derive(Debug)]
struct DrawHandler {
    coords: Vec<Coordinate<i32>>,
}

impl DrawHandler {
    pub fn new(model: &Model, coord: OnScreen<Coordinate<i32>>) -> Self {
        let coord = model.coord_at(coord);
        Self {
            coords: vec![coord],
        }
    }
}

impl Handler for DrawHandler {
    fn move_to(&mut self, model: model::DeferCommit, coord: OnScreen<Coordinate<i32>>) {
        let coord = model.coord_at(coord);
        let (start, control, end) = {
            let mut it = self.coords.iter().rev();
            let c_0 = coord;
            let c_1 = it
                .next()
                .copied()
                .expect("`self.coords` should not be empty");
            let c_2 = it.next().copied().unwrap_or(c_0);
            let mid_0_1 = (c_0 + c_1) / 2;
            let mid_1_2 = (c_1 + c_2) / 2;
            (mid_0_1, c_1, mid_1_2)
        };
        model
            .temp_layer()
            .render_curve(model.pen_color(), start, control, end);
        self.coords.push(coord);
    }

    fn finish(self, mut model: model::DeferCommit) {
        let coords = LineString::from(self.coords)
            .map_scalars(f64::from)
            .simplify(&0.5)
            .map_scalars(|s| s as _);
        let path = Path {
            color: model.pen_color(),
            coords,
        };
        let path = RenderablePath::new(path).expect("`path` should not be empty");
        model.temp_layer().clear();
        model.insert_paths(iter::once((PathId::gen(), path)));
    }
}

#[derive(Debug)]
struct EraseHandler {
    removing_path_ids: FxHashSet<PathId>,
    prev_coord: Coordinate<i32>,
}

impl EraseHandler {
    pub fn new(model: &Model, coord: OnScreen<Coordinate<i32>>) -> Self {
        let coord = model.coord_at(coord);
        Self {
            removing_path_ids: FxHashSet::default(),
            prev_coord: coord,
        }
    }
}

impl Handler for EraseHandler {
    fn move_to(&mut self, mut model: model::DeferCommit, coord: OnScreen<Coordinate<i32>>) {
        let coord = model.coord_at(coord);
        let eraser_line = Line::new(self.prev_coord, coord);
        let ids = model
            .bounding_tile_items(eraser_line)
            .filter(|(id, lines)| {
                !self.removing_path_ids.contains(id)
                    && lines.iter().any(|line| line.intersects(&eraser_line))
            })
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        for &id in &ids {
            model.hide_path(id);
        }
        self.removing_path_ids.extend(ids.iter().copied());
        self.prev_coord = coord;
    }

    fn finish(self, mut model: model::DeferCommit) {
        model.remove_paths(self.removing_path_ids);
    }
}

#[derive(Debug)]
pub struct Controller {
    active_handler: Option<AnyHandler>,
    model: Model,
}

impl Controller {
    pub fn new(model: Model) -> Self {
        Self {
            active_handler: None,
            model,
        }
    }

    pub fn rerender(&self) {
        self.model.force_rerender();
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.model.defer_commit().set_tool(tool);
    }

    pub fn set_pen_color(&mut self, color: Color) {
        self.model.defer_commit().set_pen_color(color);
    }

    pub fn clear_paths(&mut self) {
        self.model.defer_commit().clear_paths();
    }

    pub fn on_key_down(&mut self, event: web::KeyboardEvent) {
        let mut model = self.model.defer_commit();
        match event.key.as_str() {
            "Delete" => {
                model.remove_selected_paths();
            }
            "z" if event.ctrl_key => {
                model.undo();
            }
            "y" if event.ctrl_key => {
                model.redo();
            }
            _ => {}
        }
    }

    pub fn on_wheel(&mut self, event: web::WheelEvent) {
        let mut model = self.model.defer_commit();
        let delta = model.delta_of(event.delta.map(|d| -d));
        model.scroll(delta);
    }

    pub fn on_pointer_down(&mut self, event: web::MouseEvent) {
        if self.active_handler.is_some() {
            return;
        }
        let mut model = self.model.defer_commit();
        match event.button {
            web::MouseButton::Left => {
                if model.selected_paths().any(|(_, path)| {
                    path.bounding_rect()
                        .get()
                        .contains(&model.coord_at(event.coord))
                }) {
                    self.active_handler = Some(ShiftHandler::new(model, event.coord).into());
                } else {
                    model.unselect_all_paths();
                    self.active_handler = Some(match model.tool() {
                        Tool::Selector => SelectHandler::new(&*model, event.coord).into(),
                        Tool::Pen => DrawHandler::new(&*model, event.coord).into(),
                        Tool::Eraser => EraseHandler::new(&*model, event.coord).into(),
                    });
                }
            }
            web::MouseButton::Middle => {
                self.active_handler = Some(ScrollHandler::new(event.coord).into());
            }
            web::MouseButton::Other => {}
        }
    }

    pub fn on_pointer_move(&mut self, event: web::MouseEvent) {
        if let Some(h) = &mut self.active_handler {
            h.move_to(self.model.defer_commit(), event.coord);
        }
    }

    pub fn on_pointer_up(&mut self) {
        if let Some(h) = self.active_handler.take() {
            h.finish(self.model.defer_commit());
        }
    }
}
