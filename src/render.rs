//! Compile a world's main file and rasterise or vectorise its first page.
//!
//! The render counterpart to the eval/harvest path: where harvesting evaluates
//! (fonts off, no layout), this runs the full `typst::compile` pipeline and hands
//! the caller a ready-to-present frame. The world must carry fonts (see
//! [`World::with_fonts`]); inject per-frame state with [`World::with_inputs`].
//!
//! The laid-out page carries more than pixels: every `#link` leaves a positioned
//! rectangle in the frame Typst just drew. [`render_with_targets`] hands those
//! back alongside the raster as [`HitTarget`]s, so a host can route pointer input
//! onto the rendered document without redoing any layout math.

use typst::layout::{Abs, Frame as LayoutFrame, FrameItem, Point, Size, Transform};
use typst::model::{Destination, Url};
use typst::utils::Scalar;
use typst_layout::{Page, PagedDocument};
use typst_render::RenderOptions;
use typst_svg::SvgOptions;

use crate::WorldError;
use crate::world::{World, format_diagnostics};

/// A rendered raster frame: premultiplied RGBA8 pixels plus dimensions, the
/// channel order the shell presents directly (it owns any channel swap at the
/// present boundary). `rgba.len() == width * height * 4`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Premultiplied RGBA8 bytes, row-major, top-left origin.
    pub rgba: Vec<u8>,
}

/// A laid-out hit target: the rectangle one `#link` occupies on the rendered
/// page.
///
/// Typst's layout leaves a positioned rectangle behind for every `#link`, with
/// the link's URL as an arbitrary payload it never interprets. A document that
/// wraps its interactive regions in `#link("app:action/arg", body)` therefore
/// tells the host exactly where each region landed — straight from the layout
/// that drew the pixels, so a target can never drift from what was drawn.
///
/// Coordinates are page points with a top-left origin, matching [`Frame`]'s
/// orientation: multiply by the `pixels_per_point` the frame was rendered at to
/// reach pixels in its buffer.
#[derive(Debug, Clone, PartialEq)]
pub struct HitTarget {
    /// The link's URL, verbatim. Typst only checks that it is non-empty and
    /// under 8000 bytes, so the consumer is free to encode its own action
    /// vocabulary in it.
    pub url: String,
    /// Points from the page's left edge to the rectangle's left edge.
    pub x_pt: f32,
    /// Points from the page's top edge to the rectangle's top edge.
    pub y_pt: f32,
    /// Rectangle width in points.
    pub w_pt: f32,
    /// Rectangle height in points.
    pub h_pt: f32,
}

/// Compile `world`'s main file and rasterise its first page at
/// `pixels_per_point`.
///
/// Repeated calls with only `sys.inputs` changed stay cheap: comemo memoises the
/// layout work that the unchanged source shares across frames.
///
/// # Errors
/// Returns [`WorldError::Eval`] if compilation produces fatal diagnostics, or
/// [`WorldError::World`] if the document has no pages.
pub fn render(world: &World, pixels_per_point: f32) -> Result<Frame, WorldError> {
    let page = first_page(world)?;
    Ok(rasterise(&page, pixels_per_point))
}

/// Compile `world`'s main file, rasterise its first page at `pixels_per_point`,
/// and report every `#link` rectangle the layout placed on that page.
///
/// The frame is byte-identical to [`render`]'s for the same world and scale:
/// this is [`render`] plus a walk of the very page the rasteriser consumed, over
/// one shared `typst::compile` rather than a second one.
///
/// Targets come back in document order, which is paint order — overlapping
/// targets are drawn in exactly this sequence (a piano's black keys over its
/// whites), so a hit test wants the *last* match, not the first. Nothing is
/// sorted, merged, or deduplicated.
///
/// Only links to a URL yield a target. `Destination::Position`/`Location` are
/// intra-document links with no meaning to a host, so they are skipped.
///
/// # Errors
/// As [`render`].
pub fn render_with_targets(
    world: &World,
    pixels_per_point: f32,
) -> Result<(Frame, Vec<HitTarget>), WorldError> {
    let page = first_page(world)?;
    let targets = hit_targets(&page.frame);
    Ok((rasterise(&page, pixels_per_point), targets))
}

/// Rasterise one laid-out page at `pixels_per_point` into a presentable
/// [`Frame`]. Split out of [`render`] so [`render_with_targets`] produces the
/// same pixels from the same compile instead of issuing a second one.
fn rasterise(page: &Page, pixels_per_point: f32) -> Frame {
    let opts = RenderOptions {
        pixel_per_pt: Scalar::new(f64::from(pixels_per_point)),
        render_bleed: false,
    };
    let pixmap = typst_render::render(page, &opts);
    Frame {
        width: pixmap.width(),
        height: pixmap.height(),
        rgba: pixmap.take(),
    }
}

/// Compile `world`'s main file and vectorise its first page to an SVG string —
/// the vector analog of [`render`], resolution-free so there is no
/// `pixels_per_point`.
///
/// # Errors
/// As [`render`].
pub fn render_svg(world: &World) -> Result<String, WorldError> {
    let page = first_page(world)?;
    Ok(typst_svg::svg(&page, &SvgOptions::default()))
}

/// Collect every URL link rectangle in a laid-out page frame, in document
/// order.
fn hit_targets(frame: &LayoutFrame) -> Vec<HitTarget> {
    let mut targets = Vec::new();
    push_hit_targets(frame, Transform::identity(), &mut targets);
    targets
}

/// Recursive worker for [`hit_targets`], appending in the order the frame
/// stores its items — which is the order they are painted.
///
/// `to_page` maps this frame's local coordinates onto the page. A nested group
/// composes its own offset and [`typst::layout::GroupItem::transform`] onto it
/// the same way `typst_render` does when it draws that group, so a target's
/// rectangle tracks the drawn one through `rotate`, `scale`, and friends instead
/// of assuming every group sits at identity.
fn push_hit_targets(frame: &LayoutFrame, to_page: Transform, out: &mut Vec<HitTarget>) {
    for (pos, item) in frame.items() {
        match item {
            FrameItem::Group(group) => {
                let inner = to_page
                    .pre_concat(Transform::translate(pos.x, pos.y))
                    .pre_concat(group.transform);
                push_hit_targets(&group.frame, inner, out);
            }
            FrameItem::Link(Destination::Url(url), size) => {
                out.push(hit_target(url, *pos, *size, to_page));
            }
            // Text, shapes, images, tags, and intra-document links: not targets.
            _ => {}
        }
    }
}

/// Map one link rectangle from frame-local coordinates onto the page.
///
/// A group may rotate, scale, or skew everything under it, so all four corners
/// are transformed individually and the axis-aligned bounding box of the results
/// is returned — the tightest upright rectangle covering the drawn region. Under
/// the identity and translate-only transforms ordinary layout produces, that
/// bounding box is the original rectangle unchanged.
fn hit_target(url: &Url, pos: Point, size: Size, to_page: Transform) -> HitTarget {
    let corners = [
        (Abs::zero(), Abs::zero()),
        (size.x, Abs::zero()),
        (Abs::zero(), size.y),
        (size.x, size.y),
    ]
    .map(|(dx, dy)| Point::new(pos.x + dx, pos.y + dy).transform(to_page));

    // Both reductions run over four points, so neither can be empty; the
    // default is unreachable in practice and still a sane degenerate rect.
    let origin = corners.into_iter().reduce(Point::min).unwrap_or_default();
    let far_corner = corners.into_iter().reduce(Point::max).unwrap_or_default();

    HitTarget {
        url: String::from(url.clone().into_inner()),
        x_pt: to_pt_f32(origin.x),
        y_pt: to_pt_f32(origin.y),
        w_pt: to_pt_f32(far_corner.x - origin.x),
        h_pt: to_pt_f32(far_corner.y - origin.y),
    }
}

/// A length as `f32` points — the precision the render path already speaks
/// (`pixels_per_point`) and the one a shell's pointer coordinates arrive in.
/// Page geometry is bounded by the page size, orders of magnitude inside `f32`'s
/// range, so the narrowing costs sub-nanometre precision and nothing else.
#[expect(
    clippy::cast_possible_truncation,
    reason = "page-point magnitudes are far inside f32's range"
)]
fn to_pt_f32(length: Abs) -> f32 {
    length.to_pt() as f32
}

/// How many [`comemo::evict`] passes a memoized result may go unused before
/// eviction. comemo's cache is process-global and every `typst::compile` call
/// feeds it, so a session's thousands of re-renders (redraw on every MIDI
/// note/key/resize tick) grow it unbounded unless something evicts. Called once
/// per compile below, so `EVICT_MAX_AGE` frames of the same unreferenced result
/// survive before it is dropped — enough to keep memoized layout warm across a
/// short burst of frames without letting the cache grow without bound.
const EVICT_MAX_AGE: usize = 10;

/// Compile `world`'s main file and hand back its first page, evicting comemo's
/// global memoization cache afterwards (see [`EVICT_MAX_AGE`]) so the cache
/// stays bounded across the many compiles one render loop issues.
fn first_page(world: &World) -> Result<Page, WorldError> {
    let world_dyn: &dyn typst::World = world;
    let output = typst::compile::<PagedDocument>(world_dyn).output;
    comemo::evict(EVICT_MAX_AGE);
    let document = output.map_err(|diags| WorldError::Eval(format_diagnostics(&diags)))?;
    document
        .pages()
        .first()
        .cloned()
        .ok_or_else(|| WorldError::World("document produced no pages".to_owned()))
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
