mod common;
mod ctrl;
mod model;
mod view;
mod web;

use crate::{ctrl::Controller, model::Model, view::View};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    {
        let level = if cfg!(debug_assertions) {
            log::Level::Trace
        } else {
            log::Level::Info
        };
        console_log::init_with_level(level).expect("failed to initialize log");
    }

    let window = web_sys::window().expect("window does not exist");

    let storage = window
        .local_storage()
        .expect("exception thrown")
        .expect("local storage does not exist");
    let storage = web::Storage::new(storage);

    log::debug!("initalizing the view...");
    let view = View::init(window);

    log::debug!("loading the model...");
    let model = Model::load_from_storage(view.clone(), storage);
    let ctrl = Controller::new(model);

    log::debug!("registering event listeners...");
    view.listen_events(ctrl);

    log::debug!("application started");
}
