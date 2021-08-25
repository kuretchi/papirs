use serde::{Deserialize, Serialize};

/// A wrapper struct that provides modification detection.
#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub(super) struct Recorder<T> {
    inner: T,
    #[serde(skip)]
    is_updated: bool,
}

impl<T> Recorder<T> {
    pub fn get(&self) -> &T {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.is_updated = true;
        &mut self.inner
    }

    /// Updates the wrapped value by the given function `f`.
    /// This marks the value as updated iff `f` returns `true`.
    pub fn update(&mut self, f: impl FnOnce(&mut T) -> bool) {
        self.is_updated |= f(&mut self.inner);
    }

    pub fn is_updated(&self) -> bool {
        self.is_updated
    }

    pub fn resolve(&mut self) {
        self.is_updated = false;
    }
}
