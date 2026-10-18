//! [`FontSet`]: one loaded set of font faces, shared across [`crate::World`]s.
//!
//! Typst caches a [`Bytes`](typst::foundations::Bytes) value's hash inside the value, so a fresh `Bytes`
//! per world re-hashes the same font data in every world. A set built once
//! and handed to each world via [`crate::World::with_font_set`] keeps one
//! allocation, and one cached hash, per face and one for the font book.

use std::sync::Arc;
#[cfg(feature = "render")]
use std::sync::LazyLock;

#[cfg(feature = "render")]
use typst::foundations::Bytes;
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;

/// The bundled typst-assets faces, loaded once per process.
/// [`crate::World::with_fonts`] and [`FontSet::bundled`] both hand it out.
#[cfg(feature = "render")]
static BUNDLED: LazyLock<FontSet> =
    LazyLock::new(|| typst_assets::fonts().fold(FontSet::new(), FontSet::with_font));

/// A cheaply cloneable font book plus its faces.
///
/// Clones share one book and one [`Font`] per face, so every world given the
/// set serves the same font data rather than equal copies. Adding a face with
/// [`Self::with_font`] builds a new book and leaves other clones untouched.
#[derive(Clone)]
pub struct FontSet {
    book: Arc<LazyHash<FontBook>>,
    fonts: Arc<[Font]>,
}

impl Default for FontSet {
    fn default() -> Self {
        Self {
            book: Arc::new(LazyHash::new(FontBook::new())),
            fonts: Arc::from([]),
        }
    }
}

impl std::fmt::Debug for FontSet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FontSet")
            .field("fonts", &self.fonts)
            .finish()
    }
}

impl FontSet {
    #[cfg(feature = "render")]
    /// An empty set, for a caller who brings every face itself.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(feature = "render")]
    /// The bundled faces (Libertinus Serif, New Computer Modern, `DejaVu` Sans
    /// Mono): one process-wide set, so every call shares the same fonts.
    #[must_use]
    pub fn bundled() -> Self {
        BUNDLED.clone()
    }

    #[cfg(feature = "render")]
    /// Add every font face in `data` beside the set's own. The bytes are
    /// wrapped once here, so worlds given the result share them.
    #[must_use]
    pub fn with_font(self, data: impl AsRef<[u8]> + Send + Sync + 'static) -> Self {
        let mut book = FontBook::clone(&self.book);
        let mut fonts = self.fonts.to_vec();
        for font in Font::iter(Bytes::new(data)) {
            book.push(font.info().clone());
            fonts.push(font);
        }
        Self {
            book: Arc::new(LazyHash::new(book)),
            fonts: Arc::from(fonts),
        }
    }

    #[cfg(feature = "render")]
    /// Number of faces in the set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    #[cfg(feature = "render")]
    /// Whether the set holds no faces.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }

    /// The book describing every face, shared by all clones.
    pub(crate) fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    /// The face at `index`, if any.
    pub(crate) fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }
}

#[cfg(all(test, feature = "render"))]
#[path = "fonts_tests.rs"]
mod tests;
