use crate::{
    common::{utils, Color, Path, PathId, RenderablePath, Tool},
    model::Model,
    web,
};
use delegate_attr::delegate;
use enum_dispatch::enum_dispatch;
use geo::{prelude::*, Coordinate, Line, LineString, Rect};
use rustc_hash::{FxHashMap, FxHashSet};

#[enum_dispatch]
trait Handler {
    fn move_to(&mut self, model: &mut Model, coord: Coordinate<i32>);
    fn finish(self, model: &mut Model);
}

#[enum_dispatch(Handler)]
#[derive(Debug)]
enum AnyHandler {
    Scroll(ScrollHandler),
    Select(SelectHandler),
    Move(MoveHandler),
    Draw(DrawHandler),
    Erase(EraseHandler),
}

#[derive(Debug)]
struct ScrollHandler {
    prev_coord: Coordinate<i32>,
}

impl ScrollHandler {
    pub fn new(coord: Coordinate<i32>) -> Self {
        Self { prev_coord: coord }
    }
}

impl Handler for ScrollHandler {
    fn move_to(&mut self, model: &mut Model, coord: Coordinate<i32>) {
        let delta = coord - self.prev_coord;
        model.updater().scroll(delta);
        self.prev_coord = coord;
    }

    fn finish(self, model: &mut Model) {
        model.fix_scroll();
    }
}

#[derive(Debug)]
struct SelectHandler {
    start_coord: Coordinate<i32>,
    prev_coord: Coordinate<i32>,
}

impl SelectHandler {
    pub fn new(model: &Model, coord: Coordinate<i32>) -> Self {
        let coord = coord - model.offset();
        Self {
            start_coord: coord,
            prev_coord: coord,
        }
    }
}

impl Handler for SelectHandler {
    fn move_to(&mut self, model: &mut Model, coord: Coordinate<i32>) {
        let coord = coord - model.offset();
        let whole_rect = Rect::new(self.start_coord, coord);
        let diff = utils::rect_diff(self.start_coord, self.prev_coord, coord);
        {
            let mut updater = model.updater();
            for removed_rect in diff.removed {
                updater.unselect_paths(removed_rect);
            }
            for added_rect in diff.added {
                updater.select_paths(whole_rect, added_rect);
            }
            updater.skip_rerendering();
        }
        model.rerender_sub(); // remove old selection rect
        model.view().render_selection_rect(whole_rect);
        self.prev_coord = coord;
    }

    fn finish(self, model: &mut Model) {
        model.rerender_sub(); // remove selection rect
    }
}

#[derive(Debug)]
struct MoveHandler {
    moving_paths: FxHashMap<PathId, RenderablePath>,
    start_coord: Coordinate<i32>,
    prev_coord: Coordinate<i32>,
}

impl MoveHandler {
    pub fn new(model: &mut Model, coord: Coordinate<i32>) -> Self {
        let coord = coord - model.offset();
        let moving_paths = model
            .selected_paths()
            .map(|(id, path)| (id, path.clone()))
            .collect::<FxHashMap<_, _>>();
        let mut updater = model.updater();
        updater.hide_paths(moving_paths.keys().copied());
        updater.skip_rerendering(); // prevent immediately hiding
        Self {
            moving_paths,
            start_coord: coord,
            prev_coord: coord,
        }
    }
}

impl Handler for MoveHandler {
    fn move_to(&mut self, model: &mut Model, coord: Coordinate<i32>) {
        let coord = coord - model.offset();
        let delta = coord - self.prev_coord;
        for path in self.moving_paths.values_mut() {
            path.update(|path| path.line_string.translate_inplace(delta.x, delta.y));
        }
        model.rerender(); // remove previously rendered moving paths
        for path in self.moving_paths.values() {
            model.view().render_path(path);
            model.view().render_bounding_rect(path);
        }
        self.prev_coord = coord;
    }

    fn finish(self, model: &mut Model) {
        let delta = self.prev_coord - self.start_coord;
        let mut updater = model.updater();
        updater.move_paths(self.moving_paths.keys().copied(), delta);
        updater.unhide_paths(self.moving_paths.keys().copied());
    }
}

#[derive(Debug)]
struct DrawHandler {
    coords: Vec<Coordinate<i32>>,
}

impl DrawHandler {
    pub fn new(model: &Model, coord: Coordinate<i32>) -> Self {
        let coord = coord - model.offset();
        Self {
            coords: vec![coord],
        }
    }
}

impl Handler for DrawHandler {
    fn move_to(&mut self, model: &mut Model, coord: Coordinate<i32>) {
        let coord = coord - model.offset();
        let mut it = self.coords.iter().rev();
        let c_0 = coord;
        let c_1 = it.next().copied().expect("`self.coords` should not empty");
        let c_2 = it.next().copied().unwrap_or(c_0);
        let mid_0_1 = (c_0 + c_1) / 2;
        let mid_1_2 = (c_1 + c_2) / 2;
        model
            .view()
            .render_curve(model.pen_color(), mid_0_1, c_1, mid_1_2);
        self.coords.push(coord);
    }

    fn finish(self, model: &mut Model) {
        let line_string = LineString::from(self.coords)
            .map_coords(|&(x, y)| (f64::from(x), f64::from(y)))
            .simplify(&0.5)
            .map_coords(|&(x, y)| (x as _, y as _));
        let path = Path {
            color: model.pen_color(),
            line_string,
        };
        model.updater().add_new_path(path);
    }
}

#[derive(Debug)]
struct EraseHandler {
    removing_path_ids: FxHashSet<PathId>,
    prev_coord: Coordinate<i32>,
}

impl EraseHandler {
    pub fn new(model: &Model, coord: Coordinate<i32>) -> Self {
        let coord = coord - model.offset();
        Self {
            removing_path_ids: FxHashSet::default(),
            prev_coord: coord,
        }
    }
}

impl Handler for EraseHandler {
    fn move_to(&mut self, model: &mut Model, coord: Coordinate<i32>) {
        let coord = coord - model.offset();
        let eraser_line = Line::new(self.prev_coord, coord);
        let ids = model
            .bounding_tile_items(eraser_line)
            .filter(|(id, lines)| {
                !self.removing_path_ids.contains(id)
                    && lines.iter().any(|line| line.intersects(&eraser_line))
            })
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        self.prev_coord = coord;
        self.removing_path_ids.extend(ids.iter().copied());
        model.updater().hide_paths(ids.iter().copied());
    }

    fn finish(self, model: &mut Model) {
        model.updater().remove_paths(self.removing_path_ids);
    }
}

pub struct Controller {
    model: Model,
    handler: Option<AnyHandler>,
}

#[delegate(self.model)]
#[rustfmt::skip]
impl Controller {
    pub fn set_pen_color(&mut self, color: Color);
    pub fn rerender(&self);
}

#[delegate(self.model.updater())]
#[rustfmt::skip]
impl Controller {
    pub fn set_tool(&mut self, tool: Tool);
    pub fn clear(&mut self);
}

impl Controller {
    pub fn new(model: Model) -> Self {
        Self {
            model,
            handler: None,
        }
    }

    pub fn on_key_down(&mut self, key_input: web::KeyInput) {
        match key_input.key().as_str() {
            "Delete" => {
                self.model.updater().remove_selected_paths();
            }
            "z" if key_input.with_ctrl() => {
                self.model.updater().undo();
            }
            "y" if key_input.with_ctrl() => {
                self.model.updater().redo();
            }
            _ => {}
        }
    }

    pub fn on_pointer_down(&mut self, button: web::MouseButton, coord: Coordinate<i32>) {
        if self.handler.is_some() {
            return;
        }
        match button {
            web::MouseButton::Left => {
                if self.model.selected_paths().any(|(_, path)| {
                    path.bounding_rect()
                        .contains(&(coord - self.model.offset()))
                }) {
                    self.handler = Some(MoveHandler::new(&mut self.model, coord).into());
                } else {
                    self.model.view().clear_sub_canvas();
                    self.model.updater().unselect_all_paths();
                    self.handler = Some(match self.model.tool() {
                        Tool::Selector => SelectHandler::new(&self.model, coord).into(),
                        Tool::Pen => DrawHandler::new(&self.model, coord).into(),
                        Tool::Eraser => EraseHandler::new(&self.model, coord).into(),
                    });
                }
            }
            web::MouseButton::Middle => {
                self.handler = Some(ScrollHandler::new(coord).into());
            }
            web::MouseButton::Other => {}
        }
    }

    pub fn on_pointer_move(&mut self, coord: Coordinate<i32>) {
        if let Some(h) = &mut self.handler {
            h.move_to(&mut self.model, coord);
        }
    }

    pub fn on_pointer_up(&mut self, button: web::MouseButton) {
        if let Some(h) = self.handler.take() {
            let target_button = match h {
                AnyHandler::Scroll(_) => web::MouseButton::Middle,
                AnyHandler::Select(_)
                | AnyHandler::Move(_)
                | AnyHandler::Draw(_)
                | AnyHandler::Erase(_) => web::MouseButton::Left,
            };
            if button == target_button {
                h.finish(&mut self.model);
            }
        }
    }
}
