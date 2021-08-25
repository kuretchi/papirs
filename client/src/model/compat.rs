//! Old format data.

use super::Recorder;
use crate::{
    common::{Color, PathId, RenderablePath, Tool},
    web,
};
use geo::Coordinate;
use rustc_hash::FxHashMap;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct Data {
    pub paths: Recorder<FxHashMap<PathId, RenderablePath>>,
    pub offset: Recorder<Coordinate<i32>>,
    pub tool: Recorder<Tool>,
    pub pen_color: Recorder<Color>,
}

impl Data {
    pub fn load_and_remove(storage: &web::Storage) -> Option<Self> {
        const KEY: &str = "papirs";
        let data = storage.get(KEY).transpose().unwrap_or_else(|err| {
            log::error!("data found in storage but failed to load: {}", err);
            None
        })?;
        storage.remove(KEY);
        Some(data)
    }
}
