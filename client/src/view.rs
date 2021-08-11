use crate::{
    common::{Color, RenderablePath, Tool},
    ctrl::Controller,
    web,
};
use geo::{Coordinate, Rect};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{prelude::*, JsCast as _};

#[derive(Clone)]
pub struct View {
    window: web_sys::Window,
    document: web_sys::Document,
    body: web_sys::HtmlElement,

    radio_selector: web_sys::HtmlInputElement,
    radio_pen: web_sys::HtmlInputElement,
    radio_eraser: web_sys::HtmlInputElement,
    button_clear: web_sys::HtmlButtonElement,

    radio_black: web_sys::HtmlInputElement,
    radio_red: web_sys::HtmlInputElement,
    radio_orange: web_sys::HtmlInputElement,
    radio_green: web_sys::HtmlInputElement,
    radio_blue: web_sys::HtmlInputElement,
    radio_sky_blue: web_sys::HtmlInputElement,

    canvases: web_sys::HtmlDivElement,
    main_renderer: web::Renderer,
    sub_renderer: web::Renderer,

    offset: Coordinate<i32>,
}

impl View {
    pub fn init(window: web_sys::Window) -> Self {
        let document = window.document().expect("document does not exist");
        let body = document.body().expect("body does not exist");

        macro_rules! get {
            ($(let $id:ident: $ty:ty;)*) => {$(
                let id = stringify!($id).replace('_', "-");
                let $id = document
                    .get_element_by_id(&id)
                    .unwrap_or_else(|| panic!("element '{}' does not exist", id))
                    .dyn_into::<$ty>()
                    .expect("element type mismatch");
            )*};
        }
        get! {
            let radio_selector: web_sys::HtmlInputElement;
            let radio_pen: web_sys::HtmlInputElement;
            let radio_eraser: web_sys::HtmlInputElement;
            let button_clear: web_sys::HtmlButtonElement;

            let radio_black: web_sys::HtmlInputElement;
            let radio_red: web_sys::HtmlInputElement;
            let radio_orange: web_sys::HtmlInputElement;
            let radio_green: web_sys::HtmlInputElement;
            let radio_blue: web_sys::HtmlInputElement;
            let radio_sky_blue: web_sys::HtmlInputElement;

            let canvases: web_sys::HtmlDivElement;
            let canvas_main: web_sys::HtmlCanvasElement;
            let canvas_sub: web_sys::HtmlCanvasElement;
        };

        let main_renderer = web::Renderer::new(&canvas_main, |ctx| {
            ctx.set_line_cap("round");
            ctx.set_line_join("round");
            ctx.set_line_width(2.0);
        });

        let sub_renderer = web::Renderer::new(&canvas_sub, |ctx| {
            thread_local! {
                static FILL_STYLE: JsValue = JsValue::from_str("rgba(0, 90, 255, 0.2)");
                static LINE_DASH: JsValue =
                    JsValue::from_serde(&[8, 4]).expect("failed to serialize");
            }
            FILL_STYLE.with(|val| ctx.set_fill_style(val));
            LINE_DASH.with(|val| ctx.set_line_dash(val).expect("exception thrown"));
        });

        let width = body.client_width() as u32;
        let height = body.client_height() as u32;
        main_renderer.resize(width, height);
        sub_renderer.resize(width, height);

        Self {
            window,
            document,
            body,
            radio_selector,
            radio_pen,
            radio_eraser,
            button_clear,
            radio_black,
            radio_red,
            radio_orange,
            radio_green,
            radio_blue,
            radio_sky_blue,
            canvases,
            main_renderer,
            sub_renderer,
            offset: Coordinate::default(),
        }
    }

    pub fn listen_events(self, ctrl: Controller) {
        let mut view = self.clone();
        let mut ctrl = Rc::new(RefCell::new(ctrl));

        macro_rules! listen {
            ($elem:expr, $event:expr, $closure:expr) => {
                let view_clone = view.clone();
                let ctrl_clone = Rc::clone(&ctrl);
                $elem
                    .add_event_listener_with_callback(
                        $event,
                        &Closure::wrap(Box::new($closure) as Box<dyn FnMut(_)>)
                            .into_js_value()
                            .unchecked_into::<js_sys::Function>(),
                    )
                    .expect("exception thrown");
                #[allow(unused_assignments)]
                {
                    view = view_clone;
                    ctrl = ctrl_clone;
                }
            };
        }

        listen!(self.window, "resize", {
            let body = self.body;
            move |_event: web_sys::UiEvent| {
                let width = body.client_width() as u32;
                let height = body.client_height() as u32;
                view.main_renderer.resize(width, height);
                view.sub_renderer.resize(width, height);
                ctrl.borrow().rerender();
            }
        });

        listen!(
            self.document,
            "keydown",
            move |event: web_sys::KeyboardEvent| {
                let key_input = web::KeyInput::from(event);
                ctrl.borrow_mut().on_key_down(key_input);
            }
        );

        listen!(
            self.radio_selector,
            "click",
            move |_: web_sys::MouseEvent| {
                ctrl.borrow_mut().set_tool(Tool::Selector);
            }
        );

        listen!(self.radio_pen, "click", move |_: web_sys::MouseEvent| {
            ctrl.borrow_mut().set_tool(Tool::Pen);
        });

        listen!(self.radio_eraser, "click", move |_: web_sys::MouseEvent| {
            ctrl.borrow_mut().set_tool(Tool::Eraser);
        });

        listen!(self.button_clear, "click", move |_: web_sys::MouseEvent| {
            ctrl.borrow_mut().clear();
        });

        listen!(self.radio_black, "click", move |_: web_sys::MouseEvent| {
            ctrl.borrow_mut().set_pen_color(Color::Black);
        });

        listen!(self.radio_red, "click", move |_: web_sys::MouseEvent| {
            ctrl.borrow_mut().set_pen_color(Color::Red);
        });

        listen!(self.radio_orange, "click", move |_: web_sys::MouseEvent| {
            ctrl.borrow_mut().set_pen_color(Color::Orange);
        });

        listen!(self.radio_green, "click", move |_: web_sys::MouseEvent| {
            ctrl.borrow_mut().set_pen_color(Color::Green);
        });

        listen!(self.radio_blue, "click", move |_: web_sys::MouseEvent| {
            ctrl.borrow_mut().set_pen_color(Color::Blue);
        });

        listen!(
            self.radio_sky_blue,
            "click",
            move |_: web_sys::MouseEvent| {
                ctrl.borrow_mut().set_pen_color(Color::SkyBlue);
            }
        );

        listen!(
            self.canvases,
            "pointerdown",
            move |event: web_sys::MouseEvent| {
                let button = web::mouse_event_button(&event);
                let coord = web::mouse_event_offset(&event);
                ctrl.borrow_mut().on_pointer_down(button, coord);
            }
        );

        listen!(
            self.canvases,
            "pointermove",
            move |event: web_sys::MouseEvent| {
                let coord = web::mouse_event_offset(&event);
                ctrl.borrow_mut().on_pointer_move(coord);
            }
        );

        listen!(
            self.canvases,
            "pointerup",
            move |event: web_sys::MouseEvent| {
                let button = web::mouse_event_button(&event);
                ctrl.borrow_mut().on_pointer_up(button);
            }
        );
    }

    pub fn size(&self) -> Coordinate<i32> {
        let canvas = self.main_renderer.canvas();
        Coordinate {
            x: canvas.width() as _,
            y: canvas.height() as _,
        }
    }

    pub fn set_tool(&self, tool: Tool) {
        let radio = match tool {
            Tool::Selector => &self.radio_selector,
            Tool::Pen => &self.radio_pen,
            Tool::Eraser => &self.radio_eraser,
        };
        radio.set_checked(true);
    }

    pub fn set_pen_color(&self, color: Color) {
        let radio = match color {
            Color::Black => &self.radio_black,
            Color::Red => &self.radio_red,
            Color::Orange => &self.radio_orange,
            Color::Green => &self.radio_green,
            Color::Blue => &self.radio_blue,
            Color::SkyBlue => &self.radio_sky_blue,
        };
        radio.set_checked(true);
    }

    pub fn clear_main_canvas(&self) {
        self.main_renderer.clear();
    }

    pub fn clear_sub_canvas(&self) {
        self.sub_renderer.clear();
    }

    pub fn render_path(&self, path: &RenderablePath) {
        self.main_renderer.set_stroke_color(path.color);
        self.main_renderer.stroke_path(path.path_obj());
    }

    pub fn render_curve(
        &self,
        color: Color,
        start: Coordinate<i32>,
        control: Coordinate<i32>,
        end: Coordinate<i32>,
    ) {
        self.main_renderer.set_stroke_color(color);
        self.main_renderer
            .stroke_quadratic_curve(start, control, end);
    }

    pub fn render_selection_rect(&self, rect: Rect<i32>) {
        self.sub_renderer.fill_rect(rect);
    }

    pub fn render_bounding_rect(&self, path: &RenderablePath) {
        self.sub_renderer.stroke_path(path.bounding_rect_obj());
    }

    pub fn translate(&mut self, delta: Coordinate<i32>) {
        self.main_renderer.translate(delta);
        self.sub_renderer.translate(delta);
        self.offset = self.offset + delta;
        self.body
            .style()
            .set_property(
                "background-position",
                &format!("{}px {}px", self.offset.x, self.offset.y),
            )
            .expect("exception thrown");
    }
}
