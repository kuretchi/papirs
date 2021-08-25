//! A view, which renders objects and notifies the controller of recieved user events.

use crate::{
    common::{Color, OnScreen, RenderablePath, Tool},
    ctrl::Controller,
    web,
};
use enum_map::{enum_map, Enum, EnumMap};
use geo::{Coordinate, Rect};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;

fn adjust_canvas_size<'a>(
    board: &web_sys::HtmlDivElement,
    canvases: impl IntoIterator<Item = &'a web::Canvas>,
) {
    let size = OnScreen(Coordinate {
        x: board.client_width() as u32,
        y: board.client_height() as u32,
    });
    for canvas in canvases {
        canvas.resize(size);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Enum)]
pub enum Layer {
    Main,
    Sub,
    Temp,
}

#[derive(Clone, Debug)]
pub struct LayerHandle {
    canvas: web::Canvas,
}

impl LayerHandle {
    fn new(canvas: web::Canvas) -> Self {
        Self { canvas }
    }

    pub fn translate(&self, delta: Coordinate<i32>) {
        self.canvas.translate(delta);
    }

    pub fn render_path(&self, path: &RenderablePath) {
        self.set_style_for_path();
        self.canvas.set_stroke_color(path.get().get().color);
        self.canvas.stroke_path_obj(path.get().path_obj());
    }

    pub fn render_bounding_rect_of(&self, path: &RenderablePath) {
        self.set_style_for_bounding_rect();
        self.canvas.stroke_path_obj(path.bounding_rect().path_obj());
    }

    pub fn render_curve(
        &self,
        color: Color,
        start: Coordinate<i32>,
        control: Coordinate<i32>,
        end: Coordinate<i32>,
    ) {
        self.set_style_for_path();
        self.canvas.set_stroke_color(color);
        self.canvas.stroke_curve(start, control, end);
    }

    pub fn render_selection_rect(&self, rect: Rect<i32>) {
        self.set_style_for_selection_rect();
        self.canvas.fill_rect(rect);
    }

    pub fn clear(&self) {
        self.canvas.clear();
    }

    fn set_style_for_path(&self) {
        self.canvas
            .ctx
            .set_line_dash(&js_sys::Array::new())
            .expect("unexpected exception");
        self.canvas.ctx.set_line_cap("round");
        self.canvas.ctx.set_line_join("round");
        self.canvas.ctx.set_line_width(2.0);
    }

    fn set_style_for_selection_rect(&self) {
        thread_local! {
            static FILL_STYLE: JsValue = JsValue::from_str("rgba(0,90,255,0.15)");
        }
        FILL_STYLE.with(|val| self.canvas.ctx.set_fill_style(val));
    }

    fn set_style_for_bounding_rect(&self) {
        thread_local! {
            static LINE_DASH: js_sys::Array =
                js_sys::Array::of2(&JsValue::from_f64(8.), &JsValue::from_f64(6.));
        }
        LINE_DASH.with(|val| {
            self.canvas
                .ctx
                .set_line_dash(val)
                .expect("unexpected exception");
        });
        self.canvas.ctx.set_line_cap("butt");
        self.canvas.ctx.set_line_join("butt");
        self.canvas.ctx.set_line_width(1.0);
        self.canvas.set_stroke_color(Color::Black);
    }
}

#[derive(Clone, Debug)]
pub struct View {
    board: web_sys::HtmlDivElement,
    pub layers: EnumMap<Layer, LayerHandle>,
    offset: Coordinate<i32>,

    tool_radios: EnumMap<Tool, web_sys::HtmlInputElement>,
    pen_color_radios: EnumMap<Color, web_sys::HtmlInputElement>,
    button_clear: web_sys::HtmlButtonElement,
}

impl View {
    pub fn init() -> Self {
        web::bind_elements! {
            let board;
            let canvas_main: web_sys::HtmlCanvasElement;
            let canvas_sub: web_sys::HtmlCanvasElement;
            let canvas_temp: web_sys::HtmlCanvasElement;

            let radio_selector: web_sys::HtmlInputElement;
            let radio_pen: web_sys::HtmlInputElement;
            let radio_eraser: web_sys::HtmlInputElement;

            let radio_black: web_sys::HtmlInputElement;
            let radio_red: web_sys::HtmlInputElement;
            let radio_orange: web_sys::HtmlInputElement;
            let radio_green: web_sys::HtmlInputElement;
            let radio_blue: web_sys::HtmlInputElement;
            let radio_sky_blue: web_sys::HtmlInputElement;

            let button_clear;
        }

        let canvas_main = web::Canvas::from(canvas_main);
        let canvas_sub = web::Canvas::from(canvas_sub);
        let canvas_temp = web::Canvas::from(canvas_temp);

        adjust_canvas_size(&board, [&canvas_main, &canvas_sub, &canvas_temp]);

        Self {
            board,
            layers: enum_map! {
                Layer::Main => LayerHandle::new(canvas_main.clone()),
                Layer::Sub => LayerHandle::new(canvas_sub.clone()),
                Layer::Temp => LayerHandle::new(canvas_temp.clone()),
            },
            offset: Coordinate::zero(),

            tool_radios: enum_map! {
                Tool::Selector => radio_selector.clone(),
                Tool::Pen => radio_pen.clone(),
                Tool::Eraser => radio_eraser.clone(),
            },
            pen_color_radios: enum_map! {
                Color::Black => radio_black.clone(),
                Color::Red => radio_red.clone(),
                Color::Orange => radio_orange.clone(),
                Color::Green => radio_green.clone(),
                Color::Blue => radio_blue.clone(),
                Color::SkyBlue => radio_sky_blue.clone(),
            },
            button_clear,
        }
    }

    pub fn listen_events(self, ctrl: Controller) {
        let ctrl = Rc::new(RefCell::new(ctrl));

        web::WINDOW.with({
            let board = self.board.clone();
            let layers = self.layers;
            let ctrl = Rc::clone(&ctrl);
            move |window| {
                web::listen_event(window, "resize", move |_: web_sys::UiEvent| {
                    adjust_canvas_size(&board, layers.values().map(|l| &l.canvas));
                    ctrl.borrow().rerender();
                });
            }
        });

        web::DOCUMENT.with({
            let ctrl = Rc::clone(&ctrl);
            move |document| {
                web::listen_event(document, "keydown", move |event: web_sys::KeyboardEvent| {
                    ctrl.borrow_mut().on_key_down(event.into())
                });
            }
        });

        for (tool, radio) in &self.tool_radios {
            web::listen_event(radio, "click", {
                let ctrl = Rc::clone(&ctrl);
                move |_: web_sys::MouseEvent| ctrl.borrow_mut().set_tool(tool)
            });
        }

        for (color, radio) in &self.pen_color_radios {
            web::listen_event(radio, "click", {
                let ctrl = Rc::clone(&ctrl);
                move |_: web_sys::MouseEvent| ctrl.borrow_mut().set_pen_color(color)
            });
        }

        web::listen_event(&self.button_clear, "click", {
            let ctrl = Rc::clone(&ctrl);
            move |_: web_sys::MouseEvent| ctrl.borrow_mut().clear_paths()
        });

        web::listen_event(&self.board, "pointerdown", {
            let ctrl = Rc::clone(&ctrl);
            move |event: web_sys::MouseEvent| ctrl.borrow_mut().on_pointer_down(event.into())
        });

        web::listen_event(&self.board, "pointermove", {
            let ctrl = Rc::clone(&ctrl);
            move |event: web_sys::MouseEvent| ctrl.borrow_mut().on_pointer_move(event.into())
        });

        web::listen_event(&self.board, "pointerup", {
            let ctrl = Rc::clone(&ctrl);
            move |_: web_sys::MouseEvent| ctrl.borrow_mut().on_pointer_up()
        });
    }

    pub fn size(&self) -> OnScreen<Coordinate<u32>> {
        self.layers[Layer::Main].canvas.size()
    }

    pub fn translate(&mut self, delta: Coordinate<i32>) {
        for layer in self.layers.values() {
            layer.translate(delta);
        }
        self.offset = self.offset + delta;
        self.board
            .style()
            .set_property(
                "background-position",
                &format!("{}px {}px", self.offset.x, self.offset.y),
            )
            .expect("unexpected exception");
    }

    pub fn select_tool(&self, tool: Tool) {
        self.tool_radios[tool].set_checked(true);
    }

    pub fn select_pen_color(&self, color: Color) {
        self.pen_color_radios[color].set_checked(true);
    }
}
