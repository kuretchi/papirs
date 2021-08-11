use std::cell::RefCell;

use crate::{
    common::{Color, PathId, RenderablePath, Tool},
    web,
};
use delegate_attr::delegate;
use geo::{prelude::*, Coordinate};
use getset::CopyGetters;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Serialize, Deserialize, CopyGetters)]
struct DataInner {
    paths: FxHashMap<PathId, RenderablePath>,
    #[getset(get_copy)]
    offset: Coordinate<i32>,
    #[getset(get_copy)]
    tool: Tool,
    #[getset(get_copy)]
    pen_color: Color,
}

impl DataInner {
    fn load_from(s: &str) -> bincode::Result<Self> {
        let mut bytes = s.as_bytes();
        let base64 = base64::read::DecoderReader::new(&mut bytes, base64::STANDARD_NO_PAD);
        let deflate = flate2::read::DeflateDecoder::new(base64);
        bincode::deserialize_from(deflate)
    }

    fn save_to(&self, s: &mut String) {
        let mut base64 = base64::write::EncoderStringWriter::from(s, base64::STANDARD_NO_PAD);
        let deflate = flate2::write::DeflateEncoder::new(&mut base64, flate2::Compression::fast());
        bincode::serialize_into(deflate, self).expect("failed to serialize");
        base64.into_inner();
    }
}

#[derive(Debug)]
pub(super) struct Data {
    storage: web::Storage,
    inner: DataInner,
}

#[delegate(self.inner)]
#[rustfmt::skip]
impl Data {
    pub fn offset(&self) -> Coordinate<i32>;
    pub fn tool(&self) -> Tool;
    pub fn pen_color(&self) -> Color;
}

impl Data {
    pub fn load_from_storage(storage: web::Storage) -> Self {
        let inner = storage
            .get()
            .map(|s| DataInner::load_from(&s))
            .transpose()
            .unwrap_or_else(|e| {
                log::error!("failed to load stored data: {}", e);
                None
            })
            .unwrap_or_default();
        Self { storage, inner }
    }

    fn save_to_storage(&self) {
        thread_local! {
            static BUF: RefCell<String> = RefCell::new(String::new());
        }
        BUF.with(|s| {
            let s = &mut *s.borrow_mut();
            s.clear();
            self.inner.save_to(s);
            self.storage.set(s);
        });
    }

    pub fn path(&self, id: PathId) -> &RenderablePath {
        &self.inner.paths[&id]
    }

    pub fn paths(&self) -> impl Iterator<Item = (PathId, &RenderablePath)> {
        self.inner.paths.iter().map(|(&id, path)| (id, path))
    }

    pub fn add_paths(&mut self, paths: impl IntoIterator<Item = (PathId, RenderablePath)>) {
        for (id, path) in paths {
            let old = self.inner.paths.insert(id, path);
            assert!(old.is_none(), "path already exists");
        }
        self.save_to_storage();
    }

    pub fn move_paths(&mut self, ids: impl IntoIterator<Item = PathId>, delta: Coordinate<i32>) {
        for id in ids {
            self.inner
                .paths
                .get_mut(&id)
                .expect("path not found")
                .update(|path| {
                    path.line_string.translate_inplace(delta.x, delta.y);
                });
        }
        self.save_to_storage();
    }

    pub fn remove_paths(
        &mut self,
        ids: impl IntoIterator<Item = PathId>,
    ) -> Vec<(PathId, RenderablePath)> {
        let paths = ids
            .into_iter()
            .map(|id| (id, self.inner.paths.remove(&id).expect("path not found")))
            .collect();
        self.save_to_storage();
        paths
    }

    pub fn remove_all_paths(&mut self) -> Vec<(PathId, RenderablePath)> {
        let paths = self.inner.paths.drain().collect();
        self.save_to_storage();
        paths
    }

    pub fn add_offset(&mut self, delta: Coordinate<i32>) {
        self.inner.offset = self.inner.offset + delta;
        self.save_to_storage();
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.inner.tool = tool;
        self.save_to_storage();
    }

    pub fn set_pen_color(&mut self, color: Color) {
        self.inner.pen_color = color;
        self.save_to_storage();
    }
}
