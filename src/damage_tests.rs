#![allow(clippy::unwrap_used)]

use super::{PixelRect, damage, rasterise_changes};
use crate::{Page, World, compile_page, rasterise};

const PPP: f32 = 2.0;

/// Lay out `body` on a small page, `width` points wide.
fn page_of(body: &str, width: u32) -> Page {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
    let file = dir.path().join("main.typ");
    let source = format!("#set page(width: {width}pt, height: 80pt, margin: 8pt)\n{body}");
    std::fs::write(&file, source).unwrap();
    compile_page(&World::new(&file).unwrap().with_fonts()).unwrap()
}

/// Bring a raster of `before` up to date with `after` and require the result
/// to equal a fresh raster of `after`, byte for byte.
fn assert_exact(before: &str, after: &str) -> Vec<PixelRect> {
    let (prev, next) = (page_of(before, 120), page_of(after, 120));
    let mut frame = rasterise(&prev, PPP);
    let rects = rasterise_changes(&mut frame, &prev, &next, PPP);
    assert!(
        frame == rasterise(&next, PPP),
        "damaged raster differs: {rects:?}"
    );
    rects
}

const TWO_RECTS: &str = "#place(rect(width: 20pt, height: 20pt, fill: red))\n\
                         #place(dx: 60pt, rect(width: 20pt, height: 20pt, fill: blue))";

#[test]
fn identical_pages_damage_nothing() {
    let page = page_of(TWO_RECTS, 120);
    assert_eq!(
        damage(&page, &page_of(TWO_RECTS, 120), PPP),
        Some(Vec::new())
    );
}

#[test]
fn a_recoloured_shape_damages_only_its_own_pixels() {
    let next = TWO_RECTS.replace("fill: blue", "fill: green");
    let rects = assert_exact(TWO_RECTS, &next);
    assert!(!rects.is_empty());
    // The blue square spans x 68..88pt (136..176 px); the red one 8..28pt.
    for rect in &rects {
        assert!(rect.x >= 130 && rect.x + rect.width <= 182, "{rect:?}");
    }
}

#[test]
fn edited_text_is_exact() {
    assert_exact("Hello, world.", "Hello, wordl.");
}

#[test]
fn a_moved_shape_is_exact() {
    let moved = TWO_RECTS.replace("dx: 60pt", "dx: 40pt");
    assert_exact(TWO_RECTS, &moved);
}

#[test]
fn added_and_removed_items_are_exact() {
    let more = format!("{TWO_RECTS}\n#place(dx: 30pt, dy: 30pt, circle(radius: 8pt, fill: olive))");
    assert_exact(TWO_RECTS, &more);
    assert_exact(&more, TWO_RECTS);
}

#[test]
fn a_rotated_group_is_exact() {
    let before = "#place(dx: 30pt, rotate(30deg, rect(width: 30pt, height: 10pt, fill: red)))";
    assert_exact(before, &before.replace("red", "blue"));
}

#[test]
fn a_clipped_group_is_exact() {
    let before = "#box(clip: true, width: 20pt, height: 20pt, \
                  place(dx: -5pt, circle(radius: 15pt, fill: red)))";
    assert_exact(before, &before.replace("red", "blue"));
}

#[test]
fn a_mitred_stroke_is_exact() {
    let before = "#place(dx: 20pt, dy: 10pt, polygon(stroke: (thickness: 4pt, join: \"miter\"), \
                  (0pt, 0pt), (30pt, 2pt), (0pt, 4pt)))";
    assert_exact(before, &before.replace("4pt, join", "3pt, join"));
}

#[test]
fn a_resized_page_is_rasterised_whole() {
    let (prev, next) = (page_of(TWO_RECTS, 120), page_of(TWO_RECTS, 140));
    assert_eq!(damage(&prev, &next, PPP), None);
    let mut frame = rasterise(&prev, PPP);
    let rects = rasterise_changes(&mut frame, &prev, &next, PPP);
    assert!(frame == rasterise(&next, PPP));
    let whole = PixelRect {
        x: 0,
        y: 0,
        width: frame.width,
        height: frame.height,
    };
    assert_eq!(rects, vec![whole]);
}

/// Painted leaves under `frame`, groups opened.
fn leaves(frame: &typst::layout::Frame) -> usize {
    frame
        .items()
        .map(|(_, item)| match item {
            typst::layout::FrameItem::Group(group) => leaves(&group.frame),
            typst::layout::FrameItem::Link(..) | typst::layout::FrameItem::Tag(_) => 0,
            _ => 1,
        })
        .sum()
}

#[test]
fn pruning_drops_the_items_outside_the_damage() {
    use super::{Grid, Keep, prune};
    use typst::layout::Transform;

    let page = page_of(TWO_RECTS, 120);
    // Around the blue square only (136..176 px on x).
    let rects = [PixelRect {
        x: 130,
        y: 10,
        width: 50,
        height: 50,
    }];
    let keep = Keep {
        grid: Grid::new(&page, PPP),
        rects: &rects,
    };
    let pruned = prune(&page.frame, Transform::identity(), &keep);
    assert_eq!((leaves(&page.frame), leaves(&pruned)), (2, 1));
}
