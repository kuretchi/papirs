pub mod utils;

use crate::web;
use geo::{prelude::*, LineString, Rect};
use getset::Getters;
use serde::{Deserialize, Deserializer, Serialize};
use shrinkwraprs::Shrinkwrap;
use std::mem;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Color {
    Black,
    Red,
    Orange,
    Green,
    Blue,
    SkyBlue,
}

impl Default for Color {
    fn default() -> Self {
        Self::Black
    }
}

impl Color {
    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            Color::Black => (0, 0, 0),
            Color::Red => (255, 75, 0),
            Color::Orange => (246, 170, 0),
            Color::Green => (3, 175, 122),
            Color::Blue => (0, 90, 255),
            Color::SkyBlue => (77, 196, 255),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PathId(Uuid);

impl PathId {
    pub fn gen() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Path {
    pub color: Color,
    pub line_string: LineString<i32>,
}

#[derive(Clone, Debug, Serialize, Getters, Shrinkwrap)]
pub struct RenderablePath {
    #[shrinkwrap(main_field)]
    path: Path,
    #[getset(get = "pub")]
    #[serde(skip_serializing)]
    path_obj: web::Path,
    #[getset(get = "pub")]
    #[serde(skip_serializing)]
    bounding_rect: Rect<i32>,
    #[getset(get = "pub")]
    #[serde(skip_serializing)]
    bounding_rect_obj: web::Path,
}

impl<'de> Deserialize<'de> for RenderablePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let path = Path::deserialize(deserializer)?;
        Ok(Self::new(path))
    }
}

impl RenderablePath {
    pub fn new(path: Path) -> Self {
        let path_obj = web::Path::from(&path.line_string);
        let bounding_rect = path.line_string.bounding_rect().expect("empty path");
        let bounding_rect_obj = {
            // use a half-integer to avoid blurring. see https://stackoverflow.com/a/18019686
            const DELTA: f64 = 4.5;
            let bounding_rect = utils::expand_rect(
                bounding_rect.map_coords(|&(x, y)| (x.into(), y.into())),
                DELTA,
            );
            web::Path::from(bounding_rect)
        };
        let bounding_rect = utils::expand_rect(bounding_rect, 5);
        Self {
            path,
            path_obj,
            bounding_rect,
            bounding_rect_obj,
        }
    }

    pub fn update(&mut self, f: impl FnOnce(&mut Path)) {
        f(&mut self.path);
        let path = Path {
            line_string: LineString(mem::take(&mut self.path.line_string.0)),
            ..self.path
        };
        *self = Self::new(path);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Tool {
    Selector,
    Pen,
    Eraser,
}

impl Default for Tool {
    fn default() -> Self {
        Self::Pen
    }
}
