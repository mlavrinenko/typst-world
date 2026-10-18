#![allow(clippy::unwrap_used)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use typst::World as _;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Value};
use typst::syntax::{FileId, RootedPath, VirtualPath, VirtualRoot};

use super::{World, find_project_root};
use crate::SourceSnapshot;
use crate::provider::{FileProvider, ProviderCtx, local_package_file_id};

#[test]
fn root_marker_at_start() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
    assert_eq!(find_project_root(dir.path()), dir.path());
}

#[test]
fn root_walks_up_to_git() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".git")).unwrap();
    let child = dir.path().join("a").join("b");
    std::fs::create_dir_all(&child).unwrap();
    assert_eq!(find_project_root(&child), dir.path());
}

#[test]
fn new_reads_main_source() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
    let file = dir.path().join("task.typ");
    std::fs::write(&file, "= Hello").unwrap();

    let world = World::new(&file).unwrap();
    let source = world.source(world.main()).unwrap();
    assert!(source.text().contains("Hello"));
}

#[test]
fn with_root_rejects_outside_file() {
    let outer = tempfile::tempdir().unwrap();
    let inner = tempfile::tempdir().unwrap();
    let file = outer.path().join("task.typ");
    std::fs::write(&file, "= x").unwrap();
    // file is not under `inner`, so construction must fail.
    assert!(World::with_root(&file, inner.path()).is_err());
}

#[test]
fn new_rejects_missing_file() {
    assert!(World::new(Path::new("/nonexistent/x.typ")).is_err());
}

/// A bytes-backed [`FileProvider`] that ignores the disk-oriented context and
/// serves an id from a map — the shape the web target uses.
struct MapProvider(HashMap<FileId, Vec<u8>>);

impl FileProvider for MapProvider {
    fn read(&self, id: FileId, _ctx: &ProviderCtx<'_>) -> FileResult<Bytes> {
        self.0
            .get(&id)
            .map(|bytes| Bytes::new(bytes.clone()))
            .ok_or(FileError::AccessDenied)
    }
}

#[test]
fn dependencies_lists_non_main_project_files_read_during_source_resolution() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
    let main = dir.path().join("main.typ");
    std::fs::write(&main, "= Hello").unwrap();
    std::fs::write(dir.path().join("other.typ"), "= Other").unwrap();

    let world = World::new(&main).unwrap();
    // Reading the main file itself must not show up as a dependency.
    world.source(world.main()).unwrap();
    let other_id = FileId::new(RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new("other.typ").unwrap(),
    ));
    world.source(other_id).unwrap();

    assert_eq!(
        world.dependencies().unwrap(),
        vec![PathBuf::from("other.typ")]
    );
}

/// A disk world plus a package root, end to end: the `@local` source the world
/// hands back is the one under the named root, not the platform directory the
/// machine running the test may also have.
#[test]
fn with_package_root_serves_a_local_package_from_the_named_root() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
    let main = dir.path().join("main.typ");
    std::fs::write(&main, "= Hello").unwrap();

    let packages = tempfile::tempdir().unwrap();
    let lib = packages.path().join("local").join("greet").join("0.1.0");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("lib.typ"), "#let hi = 1").unwrap();

    let world = World::new(&main)
        .unwrap()
        .with_package_root(Some(packages.path().to_path_buf()));
    let id = local_package_file_id("greet", "0.1.0", "lib.typ").unwrap();
    assert!(world.source(id).unwrap().text().contains("hi"));
}

/// The same world with no package root reads no `@local` package at all —
/// which is what makes the assertion above about the root, and not about
/// whatever the host happens to have installed.
#[test]
fn with_package_root_none_reads_no_local_package() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
    let main = dir.path().join("main.typ");
    std::fs::write(&main, "= Hello").unwrap();

    let world = World::new(&main).unwrap().with_package_root(None);
    let id = local_package_file_id("greet", "0.1.0", "lib.typ").unwrap();
    assert!(world.source(id).is_err());
}

#[test]
fn custom_provider_serves_bytes_without_disk() {
    let main_id = FileId::new(RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new("main.typ").unwrap(),
    ));
    let mut map = HashMap::new();
    map.insert(main_id, b"= Embedded".to_vec());

    let world = World::with_provider(
        main_id,
        PathBuf::from("/virtual"),
        Box::new(MapProvider(map)),
    );
    let source = world.source(world.main()).unwrap();
    assert!(source.text().contains("Embedded"));
}

/// A bytes-backed [`FileProvider`] that counts reads, so a test can assert a
/// file was fetched exactly once (shared cache hit) or once per world (no
/// sharing) instead of only comparing the resulting text.
struct CountingProvider {
    files: HashMap<FileId, Vec<u8>>,
    reads: Arc<AtomicUsize>,
}

impl FileProvider for CountingProvider {
    fn read(&self, id: FileId, _ctx: &ProviderCtx<'_>) -> FileResult<Bytes> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.files
            .get(&id)
            .map(|bytes| Bytes::new(bytes.clone()))
            .ok_or(FileError::AccessDenied)
    }
}

fn shared_file_setup() -> (FileId, FileId, HashMap<FileId, Vec<u8>>) {
    let main_id = FileId::new(RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new("main.typ").unwrap(),
    ));
    let shared_id = FileId::new(RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new("shared.typ").unwrap(),
    ));
    let mut files = HashMap::new();
    files.insert(shared_id, b"= Shared".to_vec());
    (main_id, shared_id, files)
}

#[test]
fn shared_snapshot_reads_a_common_file_once_across_worlds() {
    let (main_id, shared_id, files) = shared_file_setup();
    let reads = Arc::new(AtomicUsize::new(0));
    let snapshot = SourceSnapshot::new();

    let world_a = World::with_provider(
        main_id,
        PathBuf::from("/virtual"),
        Box::new(CountingProvider {
            files: files.clone(),
            reads: Arc::clone(&reads),
        }),
    )
    .with_shared_sources(&snapshot);
    let world_b = World::with_provider(
        main_id,
        PathBuf::from("/virtual"),
        Box::new(CountingProvider {
            files,
            reads: Arc::clone(&reads),
        }),
    )
    .with_shared_sources(&snapshot);

    let source_a = world_a.source(shared_id).unwrap();
    let source_b = world_b.source(shared_id).unwrap();

    assert_eq!(source_a.text(), source_b.text());
    assert_eq!(
        reads.load(Ordering::SeqCst),
        1,
        "second world must hit the shared cache"
    );
    assert_eq!(snapshot.len(), 1);
}

#[test]
fn without_shared_snapshot_each_world_caches_privately() {
    let (main_id, shared_id, files) = shared_file_setup();
    let reads = Arc::new(AtomicUsize::new(0));

    // Neither world is given a `SourceSnapshot`, so each keeps reading and
    // caching on its own — today's unchanged default behaviour.
    let world_a = World::with_provider(
        main_id,
        PathBuf::from("/virtual"),
        Box::new(CountingProvider {
            files: files.clone(),
            reads: Arc::clone(&reads),
        }),
    );
    let world_b = World::with_provider(
        main_id,
        PathBuf::from("/virtual"),
        Box::new(CountingProvider {
            files,
            reads: Arc::clone(&reads),
        }),
    );

    world_a.source(shared_id).unwrap();
    world_a.source(shared_id).unwrap();
    world_b.source(shared_id).unwrap();

    assert_eq!(
        reads.load(Ordering::SeqCst),
        2,
        "each world reads its own copy once, then serves its own cache"
    );
}

fn _assert_send_sync<T: Send + Sync>() {}

#[test]
fn world_is_send_sync() {
    _assert_send_sync::<World>();
}

/// A world over `main` and the `lib.typ` it imports, counting reads into
/// `reads`, on `snapshot` with its main kept private.
fn world_with_main(main: &str, reads: &Arc<AtomicUsize>, snapshot: &SourceSnapshot) -> World {
    let (main_id, lib_id) = (id("main.typ"), id("lib.typ"));
    let mut files = HashMap::new();
    files.insert(main_id, main.as_bytes().to_vec());
    files.insert(lib_id, b"#let twice(x) = 2 * x".to_vec());
    World::with_provider(
        main_id,
        PathBuf::from("/virtual"),
        Box::new(CountingProvider {
            files,
            reads: Arc::clone(reads),
        }),
    )
    .with_shared_sources_except_main(snapshot)
}

fn id(vpath: &str) -> FileId {
    FileId::new(RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new(vpath).unwrap(),
    ))
}

/// Evaluate `world`'s main file into its module, or its resolved error.
fn eval_main(world: &World) -> Result<typst::foundations::Module, crate::EvalError> {
    use comemo::Track as _;
    use typst::engine::{Route, Sink, Traced};
    let world_dyn: &dyn typst::World = world;
    let source = world.source(world.main()).unwrap();
    let mut sink = Sink::new();
    let traced = Traced::default();
    let route = Route::default();
    typst_eval::eval(
        world_dyn.track(),
        world.library(),
        traced.track(),
        sink.track_mut(),
        route.track(),
        &source,
    )
    .map_err(|diags| world.eval_error(&diags))
}

fn out_of(world: &World) -> typst::foundations::Value {
    let module = eval_main(world).unwrap();
    let binding = module.scope().get("out").unwrap();
    binding.read().clone()
}

#[test]
fn shared_except_main_serves_each_world_its_own_main_at_one_path() {
    let reads = Arc::new(AtomicUsize::new(0));
    let snapshot = SourceSnapshot::new();
    let left = world_with_main(
        "#import \"lib.typ\": twice\n#let out = twice(2)",
        &reads,
        &snapshot,
    );
    let right = world_with_main(
        "#import \"lib.typ\": twice\n#let out = twice(5)",
        &reads,
        &snapshot,
    );

    assert_eq!(out_of(&left), Value::Int(4));
    assert_eq!(out_of(&right), Value::Int(10));
    // Two mains plus one library read: the library is parsed once, shared.
    assert_eq!(reads.load(Ordering::SeqCst), 3);
    assert_eq!(snapshot.len(), 1);
    assert!(snapshot.get(id("lib.typ")).is_some());
    assert!(snapshot.get(id("main.typ")).is_none());
}

#[test]
fn shared_except_main_reports_the_main_at_the_hosts_path() {
    let reads = Arc::new(AtomicUsize::new(0));
    let snapshot = SourceSnapshot::new();
    let good = world_with_main("#let out = 1", &reads, &snapshot);
    let bad = world_with_main("#import \"lib.typ\": twice\n#twice(", &reads, &snapshot);

    assert_eq!(out_of(&good), Value::Int(1));
    let err = eval_main(&bad).unwrap_err();
    assert_eq!(err.main, "main.typ");
    assert_eq!(err.main_location().unwrap().path, "main.typ");
    assert!(snapshot.get(id("main.typ")).is_none());
}

#[test]
fn shared_sources_still_caches_the_main_in_the_snapshot() {
    let (main_id, _, mut files) = shared_file_setup();
    files.insert(main_id, b"= Main".to_vec());
    let snapshot = SourceSnapshot::new();
    let world = World::with_provider(
        main_id,
        PathBuf::from("/virtual"),
        Box::new(CountingProvider {
            files,
            reads: Arc::new(AtomicUsize::new(0)),
        }),
    )
    .with_shared_sources(&snapshot);
    world.source(main_id).unwrap();
    assert!(snapshot.get(main_id).is_some());
}
