#![allow(clippy::unwrap_used)]

use typst::foundations::{Dict, Str, Value};

use super::{Frame, HitTarget, render, render_with_targets};
use crate::World;

/// A small fixed-size page, kept tiny so rasterisation stays fast in tests.
const PAGE: &str = "#set page(width: 120pt, height: 80pt, margin: 8pt)\n";

fn world_for(body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
    let file = dir.path().join("main.typ");
    std::fs::write(&file, format!("{PAGE}{body}")).unwrap();
    (dir, file)
}

#[test]
fn renders_rgba_buffer() {
    let (_dir, file) = world_for("Hello, world.");
    let world = World::new(&file).unwrap().with_fonts();

    let frame = render(&world, 2.0).unwrap();
    assert!(frame.width > 0 && frame.height > 0);
    assert_eq!(frame.rgba.len(), (frame.width * frame.height * 4) as usize);
    // Text was actually drawn: not every pixel is the white page fill.
    assert!(frame.rgba.iter().any(|&byte| byte != 255));
}

#[test]
fn pixels_per_point_scales_dimensions() {
    let (_dir, file) = world_for("scale me");
    let world = World::new(&file).unwrap().with_fonts();

    let small = render(&world, 1.0).unwrap();
    let large = render(&world, 2.0).unwrap();
    assert!(large.width > small.width && large.height > small.height);
}

#[test]
fn inputs_reach_sys_inputs() {
    // Body interpolates a value only present if `sys.inputs.state` resolves.
    let (_dir, file) = world_for("#sys.inputs.state");
    let mut inputs = Dict::new();
    inputs.insert(Str::from("state"), Value::Str(Str::from("ready")));
    let world = World::new(&file).unwrap().with_fonts().with_inputs(inputs);

    assert!(render(&world, 1.0).is_ok());
}

#[test]
fn with_input_sets_a_single_string_entry() {
    let (_dir, file) = world_for("#json(bytes(sys.inputs.state)).at(\"k\")");
    let world = World::new(&file)
        .unwrap()
        .with_fonts()
        .with_input("state", "{\"k\": 1}");

    assert!(render(&world, 1.0).is_ok());
}

#[test]
fn set_inputs_batches_multiple_entries_in_one_rebuild() {
    let (_dir, file) = world_for("#json(bytes(sys.inputs.state)).at(\"k\") #sys.inputs.viewport");
    let mut world = World::new(&file).unwrap().with_fonts();
    world.set_inputs([
        ("state", "{\"k\": 1}".to_owned()),
        ("viewport", "{\"w\":1}".to_owned()),
    ]);

    assert!(render(&world, 1.0).is_ok());
}

#[test]
fn missing_input_is_a_render_error() {
    // Same body, but no inputs set: the dict-key access is a fatal diagnostic.
    let (_dir, file) = world_for("#sys.inputs.state");
    let world = World::new(&file).unwrap().with_fonts();

    assert!(render(&world, 1.0).is_err());
}

/// Three link rectangles inside a `place` inside a `box` inside the `1fr` row of
/// a `grid` — several nested groups and offsets between the page origin and each
/// link. The outer two sit flush side by side and the third straddles them, so
/// document order is the only thing that can tell overlapping targets apart.
///
/// Geometry is font-metric free on purpose (the header row is a sized `box`, not
/// text), so the expected points below are arithmetic, not measurements: the
/// 120x80pt page with an 8pt margin leaves 104x64pt of content, the auto row
/// takes 20pt, and the 40x30pt box centres in the 104x44pt left over — landing
/// its origin at (8 + 32, 8 + 20 + 7) = (40, 35).
const OVERLAPPING: &str = r#"#grid(rows: (auto, 1fr), box(height: 20pt), align(center + horizon, box(width: 40pt, height: 30pt, {
  place(dx: 0pt, link("app:note/60", rect(width: 20pt, height: 30pt, fill: white)))
  place(dx: 20pt, link("app:note/62", rect(width: 20pt, height: 30pt, fill: white)))
  place(dx: 14pt, dy: 6pt, link("app:note/61", rect(width: 12pt, height: 18pt, fill: black)))
})))"#;

/// Tolerance for comparing a rectangle in points. Layout is deterministic, so
/// the only wobble is the `f64` to `f32` narrowing and transform composition; a
/// thousandth of a point is nanometres, i.e. exact for any practical purpose.
const EPS_PT: f32 = 1e-3;

/// How far a painted pixel may fall outside a hit rectangle before the rectangle
/// counts as not covering it. One point is two pixels at the scale these tests
/// render at — enough slack for the antialiased fringe around a rotated edge,
/// tight enough that an actually wrong rectangle still fails.
const SLACK_PT: f32 = 1.0;

fn assert_rect(target: &HitTarget, url: &str, expected: [f32; 4]) {
    let [want_x, want_y, want_w, want_h] = expected;
    assert_eq!(target.url, url);
    for (axis, got, want) in [
        ("x", target.x_pt, want_x),
        ("y", target.y_pt, want_y),
        ("w", target.w_pt, want_w),
        ("h", target.h_pt, want_h),
    ] {
        assert!(
            (got - want).abs() < EPS_PT,
            "{url}: {axis} was {got}pt, expected {want}pt"
        );
    }
}

/// The bounding box of everything the rasteriser painted away from the white
/// page fill, in page points as `[left, top, right, bottom]`. Lets a test check
/// a hit rectangle against the pixels themselves rather than against a
/// hand-derived transform.
fn painted_bounds_pt(frame: &Frame, pixels_per_point: f32) -> [f32; 4] {
    let width = usize::try_from(frame.width).unwrap();
    let (mut left, mut top) = (f32::MAX, f32::MAX);
    let (mut right, mut bottom) = (f32::MIN, f32::MIN);
    for (index, pixel) in frame.rgba.chunks_exact(4).enumerate() {
        if pixel.iter().take(3).all(|&channel| channel == 255) {
            continue;
        }
        let col = (index % width) as f32 / pixels_per_point;
        let row = (index / width) as f32 / pixels_per_point;
        left = left.min(col);
        top = top.min(row);
        right = right.max(col + 1.0 / pixels_per_point);
        bottom = bottom.max(row + 1.0 / pixels_per_point);
    }
    assert!(left <= right && top <= bottom, "nothing was painted");
    [left, top, right, bottom]
}

/// Assert a target's rectangle and the painted pixels agree on all four edges:
/// it must cover what was drawn without ballooning past it.
fn assert_matches_painted(target: &HitTarget, painted: [f32; 4]) {
    let [left, top, right, bottom] = painted;
    for (edge, rect_pt, painted_pt) in [
        ("left", target.x_pt, left),
        ("top", target.y_pt, top),
        ("right", target.x_pt + target.w_pt, right),
        ("bottom", target.y_pt + target.h_pt, bottom),
    ] {
        assert!(
            (rect_pt - painted_pt).abs() <= SLACK_PT,
            "{}: {edge} edge at {rect_pt}pt, painted {painted_pt}pt",
            target.url
        );
    }
}

#[test]
fn overlapping_links_report_exact_rects_in_source_order() {
    let (_dir, file) = world_for(OVERLAPPING);
    let world = World::new(&file).unwrap().with_fonts();

    let (_frame, targets) = render_with_targets(&world, 2.0).unwrap();

    assert_eq!(targets.len(), 3);
    let mut ordered = targets.iter();
    // Source order, not sorted or deduped: the straddling third link — the one a
    // hit test must prefer where it overlaps — comes last because it paints last.
    assert_rect(
        ordered.next().unwrap(),
        "app:note/60",
        [8.0, 35.0, 20.0, 30.0],
    );
    assert_rect(
        ordered.next().unwrap(),
        "app:note/62",
        [28.0, 35.0, 20.0, 30.0],
    );
    assert_rect(
        ordered.next().unwrap(),
        "app:note/61",
        [22.0, 41.0, 12.0, 18.0],
    );
}

#[test]
fn rotated_group_target_covers_the_drawn_region() {
    // A 24x12pt rect turned 45 degrees about its centre: the group transform is
    // neither identity nor a translation, so an implementation that ignored it
    // would report the unrotated 24x12pt rect instead of the (24+12)/sqrt(2)
    // square the page actually shows.
    let (_dir, file) = world_for(
        r#"#place(dx: 20pt, dy: 10pt, rotate(45deg, link("app:rotated", rect(width: 24pt, height: 12pt, fill: black))))"#,
    );
    let world = World::new(&file).unwrap().with_fonts();

    let (frame, targets) = render_with_targets(&world, 2.0).unwrap();

    assert_eq!(targets.len(), 1);
    let target = targets.first().unwrap();
    assert_eq!(target.url, "app:rotated");
    assert!(
        (target.w_pt - target.h_pt).abs() < EPS_PT && target.w_pt > 24.0,
        "expected the rotated bounding square, got {}x{}pt",
        target.w_pt,
        target.h_pt
    );
    assert_matches_painted(target, painted_bounds_pt(&frame, 2.0));
}

#[test]
fn scaled_group_target_covers_the_drawn_region() {
    // Doubling about the top-left corner keeps the origin and doubles the size.
    let (_dir, file) = world_for(
        r#"#place(dx: 20pt, dy: 10pt, scale(200%, origin: top + left, link("app:scaled", rect(width: 24pt, height: 12pt, fill: black))))"#,
    );
    let world = World::new(&file).unwrap().with_fonts();

    let (frame, targets) = render_with_targets(&world, 2.0).unwrap();

    assert_eq!(targets.len(), 1);
    let target = targets.first().unwrap();
    assert_rect(target, "app:scaled", [28.0, 18.0, 48.0, 24.0]);
    assert_matches_painted(target, painted_bounds_pt(&frame, 2.0));
}

#[test]
fn document_without_links_yields_no_targets_and_an_unchanged_frame() {
    let (_dir, file) = world_for("Nothing clickable here.");
    let world = World::new(&file).unwrap().with_fonts();

    let (frame, targets) = render_with_targets(&world, 2.0).unwrap();

    assert!(targets.is_empty());
    assert_eq!(frame, render(&world, 2.0).unwrap());
}

#[test]
fn targets_ride_along_with_the_frame_render_would_produce() {
    // One compile feeds both the rasteriser and the frame walk, so the pixels
    // must be byte-identical to the plain render path at the same scale.
    let (_dir, file) = world_for(OVERLAPPING);
    let world = World::new(&file).unwrap().with_fonts();

    let (with_targets, targets) = render_with_targets(&world, 3.0).unwrap();

    assert_eq!(targets.len(), 3);
    assert_eq!(with_targets, render(&world, 3.0).unwrap());
}

#[test]
fn intra_document_destinations_are_not_targets() {
    // `link(<anchor>)` resolves to a `Destination::Location`/`Position`, which
    // means nothing to a host; only the URL link comes back.
    let (_dir, file) =
        world_for("= Heading <anchor>\n\n#link(<anchor>)[jump] #link(\"app:url\")[go]");
    let world = World::new(&file).unwrap().with_fonts();

    let (_frame, targets) = render_with_targets(&world, 1.0).unwrap();

    assert_eq!(targets.len(), 1);
    assert_eq!(targets.first().unwrap().url, "app:url");
}

#[test]
fn a_missing_page_is_an_error_for_the_target_path_too() {
    let (_dir, file) = world_for("#sys.inputs.state");
    let world = World::new(&file).unwrap().with_fonts();

    assert!(render_with_targets(&world, 1.0).is_err());
}
