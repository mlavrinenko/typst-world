//! A configurable eval-only [`typst::World`] over a file-provider seam, plus a
//! Typst-free value tree.
//!
//! The world resolves project-local imports against a root directory and
//! `@local/<name>:<version>` packages against the directory Typst itself reads
//! them from ([`platform_package_root`]), or against a caller-supplied in-repo
//! override for one package ([`World::with_local_package`]), or a caller-named
//! directory for all of them ([`World::with_package_root`], for a host that
//! keeps its own data-directory setting). All byte reads go through
//! a [`FileProvider`], so the world is target-agnostic: native reads disk via
//! [`DiskProvider`]; a web build feeds sources, fonts, and packages from
//! embedded bytes behind the same trait ([`BytesProvider`]). A
//! [`FallbackProvider`] chains two, so a host can serve a package it embeds
//! where no package directory holds it ([`World::with_file_provider`]).
//!
//! It evaluates a `.typ` file with `typst_eval` (never layout or render, unless
//! the `render` feature is on) and projects Typst values to a Typst-free [`HVal`]
//! tree. Marker harvesting on top of this substrate lives in the `typst-harvest`
//! crate, so `typst-world` stays a pure World + value substrate.
//!
//! With the `render` feature on, a world's first page compiles to a raster
//! `Frame` or an SVG string, and `render_with_targets` additionally reports
//! where each `#link` landed as a `HitTarget` — pointer routing straight out of
//! Typst's own layout.
//!
//! A batch consumer evaluating many short-lived worlds shares work across them:
//! every world with no custom `inputs`/`globals` shares one process-wide default
//! eval library, and [`SourceSnapshot`] lets several worlds share one parsed-source
//! cache via [`World::with_shared_sources`], so a common imported prelude is
//! parsed once no matter how many worlds read it.
//! [`World::with_shared_sources_except_main`] keeps each world's own main file
//! out of that cache, for worlds with different mains at one path. With `render` on, a `FontSet`
//! built once and handed out via `World::with_font_set` keeps one copy of each
//! face, and [`BytesProvider::with_bytes`] does the same for embedded files.
//!
//! ```no_run
//! use typst_world::World;
//! # fn run() -> Result<(), typst_world::WorldError> {
//! let world = World::new(std::path::Path::new("doc.typ"))?;
//! println!("{}", world.root().display());
//! # Ok(())
//! # }
//! ```

#[cfg(feature = "render")]
mod damage;
mod diagnostic;
mod fonts;
mod provider;
#[cfg(feature = "render")]
mod render;
mod snapshot;
mod value;
#[cfg(feature = "serde")]
mod value_serde;
mod world;

#[cfg(feature = "render")]
pub use damage::{PixelRect, damage, rasterise_changes};
pub use diagnostic::{Diagnostic, EvalError, Hint, Location, Severity, TracePoint};
#[cfg(feature = "render")]
pub use fonts::FontSet;
pub use provider::{
    BytesProvider, DiskProvider, FallbackProvider, FileProvider, ProviderCtx, ScopedDiskProvider,
    local_package_file_id, platform_package_root, project_file_id, resolve_path, root_file_id,
};
#[cfg(feature = "render")]
pub use render::{
    EVICT_MAX_AGE, Frame, HitTarget, Page, compile_page, evict, hit_targets, rasterise, render,
    render_svg, render_with_targets,
};
pub use snapshot::SourceSnapshot;
pub use value::{HVal, convert, format_date};
pub use world::{World, find_project_root};

/// Errors raised while building a world, evaluating, or querying it.
#[derive(Debug, thiserror::Error)]
pub enum WorldError {
    /// The main source file could not be read.
    #[error("file error: {0}")]
    File(String),

    /// Typst evaluation produced diagnostics, resolved to source locations.
    /// Displays as Typst's message alone; render hints and locations from
    /// the [`EvalError`].
    #[error("{0}")]
    Eval(EvalError),

    /// The world could not be constructed or queried.
    #[error("{0}")]
    World(String),
}
