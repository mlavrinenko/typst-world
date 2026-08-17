//! The configurable [`typst::World`]: root + package resolution, source/file
//! caching, dependency tracking, all byte reads delegated to a [`FileProvider`].

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use chrono::Datelike;
use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Dict, Duration, Scope, Str, Value};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt};

use crate::WorldError;
use crate::provider::{DiskProvider, FileProvider, ProviderCtx, ScopedDiskProvider};
use crate::snapshot::SourceSnapshot;

/// Process-wide default eval [`Library`] (no html/render features) — the exact
/// content [`Library::default()`] builds. A [`World`] with no custom
/// `inputs`/`globals` clones this `Arc` instead of rebuilding a `Library` from
/// scratch, so a batch consumer evaluating many short-lived worlds pays for one
/// build total, not one per world. [`World::with_inputs`]/[`World::with_globals`]
/// still build and own a per-world custom `Library`.
static DEFAULT_LIBRARY: LazyLock<Arc<LazyHash<Library>>> =
    LazyLock::new(|| Arc::new(LazyHash::new(Library::builder().build())));

/// A configurable Typst world. Byte reads go through a [`FileProvider`]; the
/// world owns the caches, dependency set, and resolution context above it.
pub struct World {
    root: PathBuf,
    main_id: FileId,
    provider: Box<dyn FileProvider>,
    /// Shared [`DEFAULT_LIBRARY`] until `inputs`/`globals` are customised, at
    /// which point [`Self::rebuild_library`] swaps in a fresh, per-world `Arc`.
    library: Arc<LazyHash<Library>>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    /// Private per-world source cache, used when no [`SourceSnapshot`] is
    /// wired in via [`Self::with_shared_sources`].
    sources: Mutex<HashMap<FileId, Source>>,
    /// When set, `source()` reads and populates through this shared cache
    /// instead of [`Self::sources`]. See [`Self::with_shared_sources`].
    shared_sources: Option<SourceSnapshot>,
    dependencies: Mutex<HashSet<FileId>>,
    /// Package name → in-repo lib directory, for in-tree package development.
    overrides: HashMap<String, PathBuf>,
    /// File id → in-memory source text, consulted before the provider. Lets a
    /// caller back a file with a synthesised source — e.g. a generated DSL
    /// re-export module that never touches disk (a fonts-off package entry).
    source_overrides: HashMap<FileId, String>,
    /// Values exposed through `sys.inputs` (render preset). The host injects
    /// caller-defined state here for the document to read.
    inputs: Dict,
    /// Extra global bindings merged into the evaluation library — a Rust-bound
    /// DSL the document calls without importing. Kept apart from [`Self::inputs`]
    /// so the library can be rebuilt from either independently.
    globals: Scope,
}

impl World {
    /// Build a world from an explicit main id, root, and file provider. A web
    /// target wires a bytes-backed provider here; native uses [`Self::new`].
    #[must_use]
    pub fn with_provider(main_id: FileId, root: PathBuf, provider: Box<dyn FileProvider>) -> Self {
        Self {
            root,
            main_id,
            provider,
            library: Arc::clone(&DEFAULT_LIBRARY),
            book: LazyHash::new(FontBook::new()),
            fonts: Vec::new(),
            sources: Mutex::new(HashMap::new()),
            shared_sources: None,
            dependencies: Mutex::new(HashSet::new()),
            overrides: HashMap::new(),
            source_overrides: HashMap::new(),
            inputs: Dict::new(),
            globals: Scope::new(),
        }
    }

    /// Build a disk-backed world for `file_path`, auto-detecting the project root
    /// by walking up for a `Cargo.toml`, `lib/`, or `.git` marker. A tool with
    /// its own root convention should use [`Self::with_root`] instead.
    ///
    /// # Errors
    /// Returns an error if the path cannot be resolved or canonicalized.
    pub fn new(file_path: &Path) -> Result<Self, WorldError> {
        let abs = absolutize(file_path)?;
        let dir = parent_dir(&abs)?;
        let root = find_project_root(dir);
        Self::disk(&abs, root)
    }

    /// Build a disk-backed world for `file_path` with an explicit project `root`.
    ///
    /// # Errors
    /// Returns an error if the path cannot be resolved or lies outside `root`.
    pub fn with_root(file_path: &Path, root: &Path) -> Result<Self, WorldError> {
        let abs = absolutize(file_path)?;
        let root = root
            .canonicalize()
            .map_err(|err| WorldError::World(format!("bad root {}: {err}", root.display())))?;
        Self::disk(&abs, root)
    }

    fn disk(abs: &Path, root: PathBuf) -> Result<Self, WorldError> {
        let rel = abs.strip_prefix(&root).map_err(|_| {
            WorldError::World(format!(
                "file {} is not under project root {}",
                abs.display(),
                root.display()
            ))
        })?;
        let rel = rel
            .to_str()
            .ok_or_else(|| WorldError::World(format!("non-utf8 path: {}", rel.display())))?;
        let vpath = VirtualPath::new(rel)
            .map_err(|err| WorldError::World(format!("bad virtual path {rel:?}: {err}")))?;
        let main_id = FileId::new(RootedPath::new(VirtualRoot::Project, vpath));
        Ok(Self::with_provider(main_id, root, Box::new(DiskProvider)))
    }

    /// Register an in-repo lib directory for package `name` (builder style).
    #[must_use]
    pub fn with_local_package(mut self, name: &str, lib_dir: PathBuf) -> Self {
        self.overrides.insert(name.to_owned(), lib_dir);
        self
    }

    /// Resolve `@local/<name>:<version>` packages under `root` instead of the
    /// platform local-package directory. `root` is the directory holding
    /// `local/<name>/<version>/` — what Typst's own `--package-path` names —
    /// and `None` says no such directory exists, leaving only the
    /// [`Self::with_local_package`] overrides. Not calling this keeps the
    /// platform lookup, which is what Typst itself does.
    ///
    /// For a host that carries its own data-directory setting: `dirs` reads the
    /// environment on Unix only, so exporting `XDG_DATA_HOME` moves the lookup
    /// on Linux and nothing on Windows. Passing the directory in is the same
    /// answer everywhere, and keeps a host from installing a package into one
    /// directory while reading from another.
    ///
    /// Installs a [`ScopedDiskProvider`], replacing this world's provider — so
    /// it belongs on a disk world ([`Self::new`], [`Self::with_root`]), whose
    /// provider is the equivalent [`crate::DiskProvider`] anyway. A world built
    /// on a custom provider ([`Self::with_provider`]) should redirect packages
    /// inside that provider instead.
    #[must_use]
    pub fn with_package_root(mut self, root: Option<PathBuf>) -> Self {
        self.provider = Box::new(ScopedDiskProvider::new(root));
        self
    }

    /// Back `id` with an in-memory `source`, consulted before the provider.
    #[must_use]
    pub fn with_source_override(mut self, id: FileId, source: impl Into<String>) -> Self {
        self.source_overrides.insert(id, source.into());
        self
    }

    /// Route this world's source cache through `snapshot` instead of a private
    /// per-world cache, so every world sharing `snapshot` parses each unique
    /// file exactly once — the shape a batch consumer wants when many
    /// short-lived worlds read from one common file tree (e.g. a shared
    /// imported prelude). [`Self::with_source_override`] is still consulted
    /// first on a miss, same as the private-cache path.
    #[must_use]
    pub fn with_shared_sources(mut self, snapshot: &SourceSnapshot) -> Self {
        self.shared_sources = Some(snapshot.clone());
        self
    }

    /// Merge `scope`'s bindings into the evaluation library's global scope, so a
    /// document can call host-native functions (a DSL bound in Rust) without
    /// importing them. Later bindings win over the defaults.
    #[must_use]
    pub fn with_globals(mut self, scope: &Scope) -> Self {
        for (name, binding) in scope.iter() {
            self.globals.bind(name.clone(), binding.clone());
        }
        self.rebuild_library();
        self
    }

    /// Set the values exposed through `sys.inputs`, the render preset's channel
    /// for host-injected state. The document reads them with `sys.inputs.<key>`.
    #[must_use]
    pub fn with_inputs(mut self, inputs: Dict) -> Self {
        self.inputs = inputs;
        self.rebuild_library();
        self
    }

    /// Set one string entry in `sys.inputs` (builder form). The common case: a
    /// host passes state as a JSON string the document reads with
    /// `json(bytes(sys.inputs.<key>))`.
    #[must_use]
    pub fn with_input(mut self, key: &str, value: impl Into<String>) -> Self {
        self.set_input(key, value);
        self
    }

    /// Set one string entry in `sys.inputs` in place — the mutable analogue of
    /// [`Self::with_input`], for a renderer that swaps per-frame state while
    /// reusing the loaded sources and fonts.
    pub fn set_input(&mut self, key: &str, value: impl Into<String>) {
        self.set_inputs([(key, value.into())]);
    }

    /// Set several string entries in `sys.inputs` in place, rebuilding the
    /// evaluation library once for the whole batch — the batched analogue of
    /// [`Self::set_input`], which rebuilds per call. A per-frame renderer
    /// setting `state`/`pressed`/`history`/`viewport` (4-6 keys) turns that many
    /// standard-library rebuilds into one.
    pub fn set_inputs<'a, I>(&mut self, entries: I)
    where
        I: IntoIterator<Item = (&'a str, String)>,
    {
        for (key, value) in entries {
            self.inputs
                .insert(Str::from(key), Value::Str(Str::from(value)));
        }
        self.rebuild_library();
    }

    /// Load the bundled fonts (Libertinus Serif, New Computer Modern, `DejaVu`
    /// Sans Mono) so the world can lay out and render text. Embedded bytes, so
    /// the same call works on web where no system fonts exist. Required before
    /// [`crate::render`]; the eval/harvest preset leaves fonts off.
    #[cfg(feature = "render")]
    #[must_use]
    pub fn with_fonts(mut self) -> Self {
        let mut book = FontBook::new();
        let mut fonts = Vec::new();
        for data in typst_assets::fonts() {
            let bytes = Bytes::new(data);
            for font in Font::iter(bytes.clone()) {
                book.push(font.info().clone());
                fonts.push(font);
            }
        }
        self.book = LazyHash::new(book);
        self.fonts = fonts;
        self
    }

    /// Rebuild the evaluation library from the current `inputs`, then re-apply
    /// the accumulated `globals`. Called whenever either changes so the two stay
    /// independent of call order.
    fn rebuild_library(&mut self) {
        let mut library = Library::builder().with_inputs(self.inputs.clone()).build();
        let global = library.global.scope_mut();
        for (name, binding) in self.globals.iter() {
            global.bind(name.clone(), binding.clone());
        }
        self.library = Arc::new(LazyHash::new(library));
    }

    /// The project root all virtual paths resolve against.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn ctx(&self) -> ProviderCtx<'_> {
        ProviderCtx {
            root: &self.root,
            overrides: &self.overrides,
        }
    }

    fn record_dependency(&self, id: FileId) {
        if id != self.main_id
            && let Ok(mut deps) = self.dependencies.lock()
        {
            deps.insert(id);
        }
    }

    /// Read `id`'s text via `source_overrides` (if set) or the provider, and
    /// parse it into a fresh [`Source`]. Shared by [`Self::source_private`] and
    /// [`Self::source_shared`] on a cache miss; each caller owns inserting the
    /// result into whichever cache backs it.
    fn read_source(&self, id: FileId) -> FileResult<Source> {
        let text = if let Some(synthetic) = self.source_overrides.get(&id) {
            synthetic.clone()
        } else {
            let bytes = self.provider.read(id, &self.ctx())?;
            String::from_utf8(bytes.to_vec())
                .map_err(|_| FileError::Other(Some("source is not valid UTF-8".into())))?
        };
        Ok(Source::new(id, text))
    }

    /// `source()` through this world's private per-world cache — the default
    /// when no [`Self::with_shared_sources`] snapshot is set.
    fn source_private(&self, id: FileId) -> FileResult<Source> {
        if let Ok(cache) = self.sources.lock()
            && let Some(source) = cache.get(&id)
        {
            return Ok(source.clone());
        }
        let source = self.read_source(id)?;
        if let Ok(mut cache) = self.sources.lock() {
            cache.insert(id, source.clone());
        }
        Ok(source)
    }

    /// `source()` through a [`SourceSnapshot`] shared with other worlds:
    /// consult it first, then populate it on a miss.
    fn source_shared(&self, id: FileId, snapshot: &SourceSnapshot) -> FileResult<Source> {
        if let Some(source) = snapshot.get(id) {
            return Ok(source);
        }
        let source = self.read_source(id)?;
        snapshot.insert(id, source.clone());
        Ok(source)
    }

    /// Project-local files read during evaluation, relative to the root.
    ///
    /// # Errors
    /// Returns an error if a dependency path cannot be resolved.
    pub fn dependencies(&self) -> Result<Vec<PathBuf>, WorldError> {
        let deps = self
            .dependencies
            .lock()
            .map_err(|_| WorldError::World("dependencies lock poisoned".to_owned()))?;
        let mut paths = Vec::new();
        for id in deps.iter() {
            let abs = crate::provider::resolve_path(*id, &self.ctx())
                .map_err(|err| WorldError::World(format!("resolve dependency: {err}")))?;
            if let Ok(rel) = abs.strip_prefix(&self.root) {
                paths.push(rel.to_path_buf());
            }
        }
        paths.sort();
        Ok(paths)
    }
}

impl typst::World for World {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.record_dependency(id);
        match &self.shared_sources {
            Some(snapshot) => self.source_shared(id, snapshot),
            None => self.source_private(id),
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.record_dependency(id);
        self.provider.read(id, &self.ctx())
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        let now = chrono::Local::now();
        Datetime::from_ymd(
            now.year(),
            u8::try_from(now.month()).unwrap_or(1),
            u8::try_from(now.day()).unwrap_or(1),
        )
    }
}

fn absolutize(file_path: &Path) -> Result<PathBuf, WorldError> {
    let abs = if file_path.is_absolute() {
        file_path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|err| WorldError::World(format!("cwd: {err}")))?
            .join(file_path)
    };
    abs.canonicalize()
        .map_err(|err| WorldError::World(format!("canonicalize {}: {err}", file_path.display())))
}

fn parent_dir(abs: &Path) -> Result<&Path, WorldError> {
    abs.parent()
        .ok_or_else(|| WorldError::World(format!("no parent dir: {}", abs.display())))
}

/// Walk up from `start` for a generic project-root marker, in priority order:
/// `Cargo.toml`, `lib/`, `.git`. Falls back to `start`. Consumer-blind: a tool
/// with its own root convention (e.g. a dotdir) should locate the root itself
/// and pass it to [`World::with_root`].
#[must_use]
pub fn find_project_root(start: &Path) -> PathBuf {
    let mut dir = start.to_path_buf();
    loop {
        if dir.join("Cargo.toml").is_file() || dir.join("lib").is_dir() || dir.join(".git").exists()
        {
            return dir;
        }
        if !dir.pop() {
            return start.to_path_buf();
        }
    }
}

/// Join Typst source diagnostics into one message, falling back to a generic
/// string when the diagnostic list is empty. Shared by the harvest and render
/// paths, which both surface eval/compile diagnostics as an error message.
#[must_use]
pub fn format_diagnostics(diags: &[SourceDiagnostic]) -> String {
    if diags.is_empty() {
        return "unknown evaluation error".to_owned();
    }
    diags
        .iter()
        .map(|diag| diag.message.as_str().to_owned())
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
#[path = "world_tests.rs"]
mod tests;
