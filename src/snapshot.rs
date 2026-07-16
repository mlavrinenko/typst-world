//! [`SourceSnapshot`]: a shareable [`Source`] cache across multiple
//! [`crate::World`]s.
//!
//! A batch consumer evaluating many short-lived worlds over one file tree
//! (e.g. a common imported prelude) hands every world the same snapshot via
//! [`crate::World::with_shared_sources`], so each unique file is read and
//! parsed exactly once no matter how many worlds ask for it.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use typst::syntax::{FileId, Source};

/// A cheaply cloneable cache of parsed [`Source`]s, shared across
/// [`crate::World`]s.
///
/// All clones of a `SourceSnapshot` back onto one store: insert through any
/// clone and every other clone (and the original) observes it. Pass one to
/// [`crate::World::with_shared_sources`] on each world in a batch that reads
/// from the same file tree.
///
/// A poisoned lock (a reader or writer panicked while holding it) degrades to
/// a cache miss on read, and a dropped write on insert — never a panic. A
/// world backed by a poisoned snapshot falls through to its file provider on
/// every read instead, which is slower but still correct.
#[derive(Debug, Clone, Default)]
pub struct SourceSnapshot {
    sources: Arc<RwLock<HashMap<FileId, Source>>>,
}

impl SourceSnapshot {
    /// An empty, shareable source cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The cached source for `id`, if present. A poisoned lock is treated as a
    /// miss rather than propagating the panic.
    pub(crate) fn get(&self, id: FileId) -> Option<Source> {
        self.sources.read().ok()?.get(&id).cloned()
    }

    /// Insert `source` under `id`, overwriting any existing entry for the same
    /// id. A poisoned lock silently drops the write; the caller already holds
    /// its own copy of `source` to return, so this is best-effort.
    pub(crate) fn insert(&self, id: FileId, source: Source) {
        if let Ok(mut sources) = self.sources.write() {
            sources.insert(id, source);
        }
    }

    /// Number of distinct files currently cached.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sources.read().map_or(0, |sources| sources.len())
    }

    /// Whether the snapshot currently holds no cached sources.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
