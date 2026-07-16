#![allow(clippy::unwrap_used)]

use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};

use super::SourceSnapshot;

fn file_id(vpath: &str) -> FileId {
    FileId::new(RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new(vpath).unwrap(),
    ))
}

#[test]
fn new_snapshot_is_empty() {
    let snapshot = SourceSnapshot::new();
    assert!(snapshot.is_empty());
    assert_eq!(snapshot.len(), 0);
}

#[test]
fn insert_then_get_round_trips() {
    let snapshot = SourceSnapshot::new();
    let id = file_id("a.typ");
    let source = Source::new(id, "= Hello".to_owned());
    snapshot.insert(id, source);

    let cached = snapshot.get(id).unwrap();
    assert_eq!(cached.text(), "= Hello");
    assert_eq!(snapshot.len(), 1);
    assert!(!snapshot.is_empty());
}

#[test]
fn get_on_miss_returns_none() {
    let snapshot = SourceSnapshot::new();
    assert!(snapshot.get(file_id("missing.typ")).is_none());
}

#[test]
fn clones_share_one_backing_store() {
    let snapshot = SourceSnapshot::new();
    let clone = snapshot.clone();
    let id = file_id("shared.typ");
    clone.insert(id, Source::new(id, "= Shared".to_owned()));

    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot.get(id).unwrap().text(), "= Shared");
}

fn _assert_send_sync<T: Send + Sync>() {}

#[test]
fn source_snapshot_is_send_sync() {
    _assert_send_sync::<SourceSnapshot>();
}
