mod common;
mod ctrl;
mod model;
mod utils;
mod view;
mod web;

use crate::{ctrl::Controller, model::Model, view::View};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();

    let log_level = if cfg!(debug_assertions) {
        log::Level::Trace
    } else {
        log::Level::Info
    };
    console_log::init_with_level(log_level).expect("failed to initialize log");

    let storage = web::Storage::local().expect("no local storage");
    let view = View::init();
    let model = Model::load(storage, view.clone());
    let ctrl = Controller::new(model);

    view.listen_events(ctrl);
}
