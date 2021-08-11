use crate::common::{utils, Color};
use geo::{CoordNum, Coordinate, LineString, Rect};
use itertools::Itertools as _;
use wasm_bindgen::{prelude::*, JsCast as _};

pub fn mouse_event_button(event: &web_sys::MouseEvent) -> MouseButton {
    match event.button() {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        _ => MouseButton::Other,
    }
}

pub fn mouse_event_offset(event: &web_sys::MouseEvent) -> Coordinate<i32> {
    Coordinate {
        x: event.offset_x(),
        y: event.offset_y(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MouseButton {
    Left,
    Middle,
    Other,
}

#[derive(Clone, Debug)]
pub struct KeyInput(web_sys::KeyboardEvent);

impl From<web_sys::KeyboardEvent> for KeyInput {
    fn from(event: web_sys::KeyboardEvent) -> Self {
        Self(event)
    }
}

impl KeyInput {
    pub fn key(&self) -> String {
        self.0.key()
    }

    pub fn with_ctrl(&self) -> bool {
        self.0.ctrl_key()
    }
}

#[derive(Clone, Debug)]
pub struct Path(web_sys::Path2d);

impl<T> From<Rect<T>> for Path
where
    T: CoordNum + Into<f64>,
{
    fn from(rect: Rect<T>) -> Self {
        let obj = web_sys::Path2d::new().expect("exception thrown");
        obj.rect(
            rect.min().x.into(),
            rect.min().y.into(),
            rect.width().into(),
            rect.height().into(),
        );
        Self(obj)
    }
}

impl<T> From<&LineString<T>> for Path
where
    T: CoordNum + Into<f64>,
{
    fn from(lines: &LineString<T>) -> Self {
        let obj = web_sys::Path2d::new().expect_throw("exception thrown");
        for pair in lines.0.iter().copied().tuple_windows().with_position() {
            use itertools::Position::*;
            match pair {
                Only((c_0, c_1)) => {
                    obj.move_to(c_0.x.into(), c_0.y.into());
                    obj.line_to(c_1.x.into(), c_1.y.into());
                }
                First((c_0, c_1)) => {
                    let c_0 = utils::coordinate_map(c_0, T::into);
                    let c_1 = utils::coordinate_map(c_1, T::into);
                    let mid = (c_0 + c_1) / 2.;
                    obj.move_to(c_0.x, c_0.y);
                    obj.line_to(mid.x, mid.y);
                }
                Middle((c_0, c_1)) => {
                    let c_0 = utils::coordinate_map(c_0, T::into);
                    let c_1 = utils::coordinate_map(c_1, T::into);
                    let mid = (c_0 + c_1) / 2.;
                    obj.quadratic_curve_to(c_0.x, c_0.y, mid.x, mid.y);
                }
                Last((_, c_1)) => {
                    obj.line_to(c_1.x.into(), c_1.y.into());
                }
            }
        }
        Self(obj)
    }
}

#[derive(Clone)]
pub struct Renderer {
    ctx: web_sys::CanvasRenderingContext2d,
    init_ctx: fn(&web_sys::CanvasRenderingContext2d),
}

impl Renderer {
    pub fn new(
        canvas: &web_sys::HtmlCanvasElement,
        init_ctx: fn(&web_sys::CanvasRenderingContext2d),
    ) -> Self {
        let ctx = canvas
            .get_context("2d")
            .expect("exception thrown")
            .expect("could not get rendering context")
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .expect("never fail: valid cast");
        init_ctx(&ctx);
        Self { ctx, init_ctx }
    }

    pub fn canvas(&self) -> web_sys::HtmlCanvasElement {
        self.ctx.canvas().expect("no canvas object associated")
    }

    pub fn set_stroke_color(&self, color: Color) {
        let (r, g, b) = color.rgb();
        self.ctx
            .set_stroke_style(&JsValue::from_str(&format!("rgb({}, {}, {})", r, g, b)));
    }

    pub fn resize(&self, width: u32, height: u32) {
        let canvas = self.canvas();
        let mat = self.ctx.get_transform().expect("exception thrown");
        canvas.set_width(width);
        canvas.set_height(height);
        (self.init_ctx)(&self.ctx);
        self.ctx
            .set_transform(mat.a(), mat.b(), mat.c(), mat.d(), mat.e(), mat.f())
            .expect("exception thrown");
    }

    pub fn translate(&self, delta: Coordinate<i32>) {
        self.ctx
            .translate(delta.x.into(), delta.y.into())
            .expect("exception thrown");
    }

    pub fn stroke_quadratic_curve(
        &self,
        start: Coordinate<i32>,
        control: Coordinate<i32>,
        end: Coordinate<i32>,
    ) {
        self.ctx.begin_path();
        self.ctx.move_to(start.x.into(), start.y.into());
        self.ctx.quadratic_curve_to(
            control.x.into(),
            control.y.into(),
            end.x.into(),
            end.y.into(),
        );
        self.ctx.stroke();
    }

    pub fn stroke_path(&self, path: &Path) {
        self.ctx.stroke_with_path(&path.0);
    }

    pub fn fill_rect(&self, rect: Rect<i32>) {
        self.ctx.fill_rect(
            rect.min().x.into(),
            rect.min().y.into(),
            rect.width().into(),
            rect.height().into(),
        );
    }

    pub fn clear(&self) {
        let canvas = self.canvas();
        let mat = self.ctx.get_transform().expect("exception thrown");
        self.ctx.clear_rect(
            -mat.e(),
            -mat.f(),
            canvas.width().into(),
            canvas.height().into(),
        );
    }
}

#[derive(Clone, Debug)]
pub struct Storage(web_sys::Storage);

impl Storage {
    const KEY: &'static str = "papirs";

    pub fn new(storage: web_sys::Storage) -> Self {
        Self(storage)
    }

    pub fn set(&self, val: &str) {
        self.0
            .set_item(Self::KEY, val)
            .expect("exception thrown: the storage may be full");
    }

    pub fn get(&self) -> Option<String> {
        self.0.get_item(Self::KEY).expect("exception thrown")
    }
}
