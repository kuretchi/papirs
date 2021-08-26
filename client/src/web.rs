//! Web API wrappers.

use crate::{
    common::{Color, OnScreen},
    utils,
};
use anyhow::{anyhow, Result};
use geo::{CoordNum, Coordinate, LineString, Rect};
use itertools::Itertools as _;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, mem};
use wasm_bindgen::{convert::FromWasmAbi, prelude::*, JsCast as _};

thread_local! {
    pub static WINDOW: web_sys::Window = web_sys::window().expect("no window");
    pub static DOCUMENT: web_sys::Document = WINDOW.with(|w| w.document().expect("no document"));
}

macro_rules! bind_elements {
    ($(let $id:ident $(: $ty:ty)?;)*) => {$(
        let id = stringify!($id).replace('_', "-");
        let $id $(: $ty)? = wasm_bindgen::JsCast::dyn_into(crate::web::DOCUMENT.with(|d| {
            d.get_element_by_id(&id)
                .unwrap_or_else(|| std::panic!("no element '{}' found", id))
        }))
        .expect("element type mismatch");
    )*};
}
pub(crate) use bind_elements;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MouseButton {
    Left,
    Middle,
    Other,
}

#[derive(Clone, Debug)]
pub struct MouseEvent {
    pub button: MouseButton,
    pub coord: OnScreen<Coordinate<i32>>,
}

impl From<web_sys::MouseEvent> for MouseEvent {
    fn from(event: web_sys::MouseEvent) -> Self {
        Self {
            button: match event.button() {
                0 => MouseButton::Left,
                1 => MouseButton::Middle,
                _ => MouseButton::Other,
            },
            coord: OnScreen(Coordinate {
                x: event.offset_x(),
                y: event.offset_y(),
            }),
        }
    }
}

#[derive(Clone, Debug)]
pub struct WheelEvent {
    pub delta: OnScreen<Coordinate<i32>>,
}

impl WheelEvent {
    pub fn new(event: web_sys::WheelEvent, page_size: OnScreen<Coordinate<u32>>) -> Self {
        let coef = match event.delta_mode() {
            web_sys::WheelEvent::DOM_DELTA_PIXEL => Coordinate { x: 1., y: 1. },
            web_sys::WheelEvent::DOM_DELTA_LINE => Coordinate { x: 25., y: 25. },
            web_sys::WheelEvent::DOM_DELTA_PAGE => utils::coord_map_scalars(page_size.0, f64::from),
            m => unreachable!("invaild `deltaMode`: {}", m),
        };
        let mut delta = OnScreen(Coordinate {
            x: (event.delta_x() * coef.x) as _,
            y: (event.delta_y() * coef.y) as _,
        });
        if event.shift_key() {
            mem::swap(&mut delta.0.x, &mut delta.0.y);
        }
        Self { delta }
    }
}

#[derive(Clone, Debug)]
pub struct KeyboardEvent {
    pub key: String,
    pub ctrl_key: bool,
}

impl From<web_sys::KeyboardEvent> for KeyboardEvent {
    fn from(event: web_sys::KeyboardEvent) -> Self {
        Self {
            key: event.key(),
            ctrl_key: event.ctrl_key(),
        }
    }
}

pub fn listen_event<E>(
    target: impl AsRef<web_sys::EventTarget>,
    event: &str,
    callback: impl FnMut(E) + 'static,
) where
    E: FromWasmAbi + 'static,
{
    let callback = Closure::wrap(Box::new(callback) as Box<dyn FnMut(_)>)
        .into_js_value()
        .unchecked_into();
    target
        .as_ref()
        .add_event_listener_with_callback(event, &callback)
        .expect("unexpected exception");
}

#[derive(Clone, Debug)]
pub struct Path(web_sys::Path2d);

impl<T> From<&'_ LineString<T>> for Path
where
    T: CoordNum,
    f64: From<T>,
{
    fn from(coords: &LineString<T>) -> Self {
        let obj = web_sys::Path2d::new().expect("unexpected exception");
        for pair in coords.0.iter().copied().tuple_windows().with_position() {
            use itertools::Position;
            match pair {
                Position::Only((c_0, c_1)) => {
                    obj.move_to(c_0.x.into(), c_0.y.into());
                    obj.line_to(c_1.x.into(), c_1.y.into());
                }
                Position::First((c_0, c_1)) => {
                    let c_0 = utils::coord_map_scalars(c_0, f64::from);
                    let c_1 = utils::coord_map_scalars(c_1, f64::from);
                    let mid = (c_0 + c_1) / 2.;
                    obj.move_to(c_0.x, c_0.y);
                    obj.line_to(mid.x, mid.y);
                }
                Position::Middle((c_0, c_1)) => {
                    let c_0 = utils::coord_map_scalars(c_0, f64::from);
                    let c_1 = utils::coord_map_scalars(c_1, f64::from);
                    let mid = (c_0 + c_1) / 2.;
                    obj.quadratic_curve_to(c_0.x, c_0.y, mid.x, mid.y);
                }
                Position::Last((_, c_1)) => {
                    obj.line_to(c_1.x.into(), c_1.y.into());
                }
            }
        }
        Self(obj)
    }
}

impl<T> From<Rect<T>> for Path
where
    T: CoordNum,
    f64: From<T>,
{
    fn from(rect: Rect<T>) -> Self {
        let obj = web_sys::Path2d::new().expect("unexpected exception");
        obj.rect(
            rect.min().x.into(),
            rect.min().y.into(),
            rect.width().into(),
            rect.height().into(),
        );
        Self(obj)
    }
}

#[derive(Clone, Debug)]
pub struct Canvas {
    pub ctx: web_sys::CanvasRenderingContext2d,
}

impl From<web_sys::HtmlCanvasElement> for Canvas {
    fn from(canvas: web_sys::HtmlCanvasElement) -> Self {
        let ctx = canvas
            .get_context("2d")
            .expect("unexpected exception")
            .expect("could not get rendering context")
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .expect("failed to cast");
        Self { ctx }
    }
}

impl Canvas {
    fn canvas(&self) -> web_sys::HtmlCanvasElement {
        self.ctx.canvas().expect("no canvas object associated")
    }

    pub fn size(&self) -> OnScreen<Coordinate<u32>> {
        let canvas = self.canvas();
        OnScreen(Coordinate {
            x: canvas.width(),
            y: canvas.height(),
        })
    }

    pub fn resize(&self, size: OnScreen<Coordinate<u32>>) {
        let canvas = self.canvas();
        let mat = self.ctx.get_transform().expect("unexpected exception");
        canvas.set_width(size.0.x);
        canvas.set_height(size.0.y);
        self.ctx
            .set_transform(mat.a(), mat.b(), mat.c(), mat.d(), mat.e(), mat.f())
            .expect("unexpected exception");
    }

    pub fn translate(&self, delta: Coordinate<i32>) {
        self.ctx
            .translate(delta.x.into(), delta.y.into())
            .expect("unexpected exception");
    }

    pub fn set_stroke_color(&self, color: Color) {
        let (r, g, b) = color.rgb();
        let style = JsValue::from_str(&format!("rgb({},{},{})", r, g, b));
        self.ctx.set_stroke_style(&style);
    }

    pub fn stroke_path_obj(&self, path: &Path) {
        self.ctx.stroke_with_path(&path.0);
    }

    pub fn stroke_curve(
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
        let mat = self.ctx.get_transform().expect("unexpected exception");
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
    pub fn local() -> Option<Self> {
        let storage = WINDOW
            .with(|w| w.local_storage())
            .expect("unexpected exception")?;
        Some(Self(storage))
    }

    pub fn get<T>(&self, key: &str) -> Option<Result<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.0
            .get_item(key)
            .expect("unexpected exception")
            .map(|s| Ok(Self::load_from(&s)?))
    }

    pub fn set<T>(&self, key: &str, val: &T) -> Result<()>
    where
        T: Serialize,
    {
        thread_local! {
            static BUF: RefCell<String> = RefCell::new(String::new());
        }
        BUF.with(|s| {
            let s = &mut *s.borrow_mut();
            s.clear();
            Self::save_to(val, s)?;
            self.0
                .set_item(key, s)
                .map_err(|e| anyhow!("exception (the storage is full?): {:?}", e))
        })
    }

    pub fn remove(&self, key: &str) {
        self.0.remove_item(key).expect("unexpected exception");
    }

    fn load_from<T>(s: &str) -> bincode::Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let mut bytes = s.as_bytes();
        let base64 = base64::read::DecoderReader::new(&mut bytes, base64::STANDARD_NO_PAD);
        let deflate = flate2::read::DeflateDecoder::new(base64);
        bincode::deserialize_from(deflate)
    }

    fn save_to<T>(val: &T, s: &mut String) -> bincode::Result<()>
    where
        T: Serialize,
    {
        let mut base64 = base64::write::EncoderStringWriter::from(s, base64::STANDARD_NO_PAD);
        let deflate = flate2::write::DeflateEncoder::new(&mut base64, flate2::Compression::fast());
        bincode::serialize_into(deflate, val)?;
        base64.into_inner();
        Ok(())
    }
}
