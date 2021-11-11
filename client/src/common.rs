use crate::{
    utils::{self, MapScalars as _},
    web,
};
use derive_more::{Add, Neg, Sub};
use enum_map::Enum;
use geo::{prelude::*, LineString, Rect};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use std::mem;
use uuid::Uuid;

/// A marker that indicates that the wrapped coordinates are the actual screen's ones.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug, Add, Sub, Neg)]
pub struct OnScreen<T>(pub T);

impl<T> OnScreen<T> {
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> OnScreen<U> {
        OnScreen(f(self.0))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PathId(Uuid);

impl PathId {
    /// Generates a unique [`PathId`].
    pub fn gen() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Path {
    pub color: Color,
    pub coords: LineString<i32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct Renderable<T> {
    inner: T,
    #[serde(skip)]
    obj: web::Path,
}

impl<T> Renderable<T> {
    pub fn get(&self) -> &T {
        &self.inner
    }

    pub fn path_obj(&self) -> &web::Path {
        &self.obj
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct RenderablePath {
    path: Renderable<Path>,
    #[serde(skip)]
    bounding_rect: Renderable<Rect<i32>>,
}

impl<'de> Deserialize<'de> for RenderablePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let path = Path::deserialize(deserializer)?;
        Self::new(path).ok_or_else(|| D::Error::custom("empty path"))
    }
}

impl RenderablePath {
    /// Creates a new [`RenderablePath`]. Returns [`None`] when the given path is empty.
    pub fn new(path: Path) -> Option<Self> {
        let bounding_rect = path.coords.bounding_rect()?;
        let bounding_rect_ex1 = utils::expand_rect(bounding_rect.map_scalars(f64::from), 4.5);
        let bounding_rect_ex2 = utils::expand_rect(bounding_rect, 5);

        Some(Self {
            path: Renderable {
                obj: (&path.coords).into(),
                inner: path,
            },
            bounding_rect: Renderable {
                obj: bounding_rect_ex1.into(),
                inner: bounding_rect_ex2,
            },
        })
    }

    pub fn take(&mut self) -> Path {
        Path {
            coords: LineString(mem::take(&mut self.path.inner.coords.0)),
            ..self.path.inner
        }
    }

    pub fn get(&self) -> &Renderable<Path> {
        &self.path
    }

    pub fn bounding_rect(&self) -> &Renderable<Rect<i32>> {
        &self.bounding_rect
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Enum, Serialize, Deserialize)]
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
            Self::Black => (0, 0, 0),
            Self::Red => (255, 75, 0),
            Self::Orange => (246, 170, 0),
            Self::Green => (3, 175, 122),
            Self::Blue => (0, 90, 255),
            Self::SkyBlue => (77, 196, 255),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Enum, Serialize, Deserialize)]
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
