#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use typst::World as _;
use typst::foundations::Bytes;
use typst::text::Font;

use super::FontSet;
use crate::{World, root_file_id};

/// The bytes of one bundled font, by family, to stand in for a caller's own.
fn bundled_font(family: &str) -> &'static [u8] {
    typst_assets::fonts()
        .find(|data| Font::new(Bytes::new(*data), 0).is_some_and(|f| f.info().family == family))
        .unwrap()
}

fn bare_world() -> World {
    World::with_provider(
        root_file_id("main.typ").unwrap(),
        PathBuf::from("/virtual"),
        Box::new(crate::BytesProvider::new()),
    )
}

/// Whether two worlds serve the very same `Font` (one shared allocation, so
/// one cached hash), not two equal copies.
fn same_font(left: &World, right: &World, index: usize) -> bool {
    let (left, right) = (left.font(index).unwrap(), right.font(index).unwrap());
    std::ptr::eq(left.data(), right.data())
}

#[test]
fn a_new_set_is_empty() {
    let set = FontSet::new();
    assert_eq!(set.len(), 0);
    assert!(set.is_empty());
    assert!(bare_world().with_font_set(&set).font(0).is_none());
}

#[test]
fn the_bundled_set_holds_every_typst_assets_face() {
    let set = FontSet::bundled();
    let faces: usize = typst_assets::fonts()
        .map(|data| Font::iter(Bytes::new(data)).count())
        .sum();
    assert_eq!(set.len(), faces);
    assert!(!set.is_empty());
}

#[test]
fn worlds_given_one_set_share_its_owned_font_data() {
    let owned: Vec<u8> = bundled_font("DejaVu Sans Mono").to_vec();
    let set = FontSet::new().with_font(owned);
    let left = bare_world().with_font_set(&set);
    let right = bare_world().with_font_set(&set);

    assert!(same_font(&left, &right, 0));
    let (lf, rf) = (left.font(0).unwrap(), right.font(0).unwrap());
    assert_eq!(lf.data().as_ptr(), rf.data().as_ptr());
    assert!(std::ptr::eq(left.book(), right.book()));
    assert_eq!(left.book().families().count(), 1);
}

#[test]
fn with_fonts_shares_one_process_wide_bundled_set() {
    let left = bare_world().with_fonts();
    let right = bare_world().with_fonts();
    assert!(same_font(&left, &right, 0));
    assert!(std::ptr::eq(left.book(), right.book()));
    assert_eq!(
        left.book().families().count(),
        FontSet::bundled().book().families().count()
    );
}

#[test]
fn with_font_still_chains_after_a_shared_set() {
    let set = FontSet::bundled();
    let base = bare_world().with_font_set(&set);
    let faces = set.len();
    let extended = bare_world()
        .with_font_set(&set)
        .with_font(bundled_font("DejaVu Sans Mono"));
    assert!(extended.font(faces).is_some());
    assert!(base.font(faces).is_none());
    // Extending one world leaves the shared set, and its other worlds, alone.
    assert_eq!(set.len(), faces);
    assert!(same_font(&base, &extended, 0));
}
