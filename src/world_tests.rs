#![allow(clippy::unwrap_used)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use typst::World as _;
use typst::diag::{FileError, FileResult};
use typst::foundations::Bytes;
use typst::syntax::{FileId, RootedPath, VirtualPath, VirtualRoot};

use super::{World, find_project_root};
use crate::SourceSnapshot;
use crate::provider::{FileProvider, ProviderCtx};

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
