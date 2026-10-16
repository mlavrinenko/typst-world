//! Rasterise only what changed between two laid-out pages.
//!
//! Typst's memoised layout hands an unchanged subtree back as an identical
//! frame, so two consecutive pages of an interactive document differ in a few
//! items. [`damage`] finds them by walking both frame trees in step and maps
//! each differing item's bounds to device pixels. [`rasterise_changes`] then
//! redraws only those pixels of an existing raster.
//!
//! The redraw is exact. Rasterising a translated sub-page would move every
//! item by a fraction of a pixel's rounding and shift anti-aliased edges, so
//! instead the whole page is rasterised at its own transform with every item
//! that touches no damaged rectangle dropped, and only the damaged rectangles
//! are copied out. Inside them the kept items paint in the same order with the
//! same transforms as a full raster, so the pixels match byte for byte.

use typst::layout::{Abs, Frame as LayoutFrame, FrameItem, GroupItem, Point, Rect, Transform};
use typst::text::TextItem;
use typst::utils::hash128;

use crate::render::{Frame, Page, rasterise};

/// A rectangle of device pixels on a rasterised page, top-left origin: the
/// orientation of [`Frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    /// Pixels from the left edge.
    pub x: u32,
    /// Pixels from the top edge.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Pixels added around every item's bounds: anti-aliasing reaches one pixel
/// past the geometry, and the margin absorbs rounding in the bounds math.
const BLEED_PX: f64 = 2.0;

/// Damage covering more than 1/`WHOLE_PAGE_FRACTION` of the page is redrawn
/// whole: the pruned raster saves little there and still pays the full-page
/// fill.
const WHOLE_PAGE_FRACTION: u64 = 2;

/// The pixel rectangles where `next` can differ from `prev` once both are
/// rasterised at `pixels_per_point`, merged so none overlap.
///
/// Empty when the pages are identical. `None` when their size or fill differs:
/// then every pixel can change and the caller rasterises the page whole.
#[must_use]
pub fn damage(prev: &Page, next: &Page, pixels_per_point: f32) -> Option<Vec<PixelRect>> {
    if prev.frame.size() != next.frame.size() || hash128(&prev.fill) != hash128(&next.fill) {
        return None;
    }
    let mut bounds = Vec::new();
    diff(&prev.frame, &next.frame, Transform::identity(), &mut bounds);
    let grid = Grid::new(next, pixels_per_point);
    Some(merge(
        bounds.iter().filter_map(|rect| grid.cover(*rect)).collect(),
    ))
}

/// Bring `frame` — `prev` rasterised at `pixels_per_point` — up to date with
/// `next`, redrawing only the pixels [`damage`] names, and return them.
///
/// Byte-identical to [`rasterise`]`(next, pixels_per_point)`. When the pages
/// differ in size or fill, the frame does not match `prev` at this scale, or
/// the damage covers most of the page, the frame is rasterised whole and the
/// one returned rectangle covers it.
pub fn rasterise_changes(
    frame: &mut Frame,
    prev: &Page,
    next: &Page,
    pixels_per_point: f32,
) -> Vec<PixelRect> {
    let grid = Grid::new(next, pixels_per_point);
    let damaged = damage(prev, next, pixels_per_point)
        .filter(|_| (frame.width, frame.height) == (grid.width, grid.height));
    let Some(rects) = damaged else {
        return redraw_whole(frame, next, pixels_per_point);
    };
    let area: u64 = rects
        .iter()
        .map(|rect| u64::from(rect.width) * u64::from(rect.height))
        .sum();
    if area * WHOLE_PAGE_FRACTION > u64::from(grid.width) * u64::from(grid.height) {
        return redraw_whole(frame, next, pixels_per_point);
    }
    if rects.is_empty() {
        return rects;
    }
    let keep = Keep {
        grid,
        rects: &rects,
    };
    let mut pruned = next.clone();
    pruned.frame = prune(&next.frame, Transform::identity(), &keep);
    let canvas = rasterise(&pruned, pixels_per_point);
    for rect in &rects {
        copy_rect(&canvas, frame, *rect);
    }
    rects
}

/// Replace `frame` with a whole raster of `page` and report it as one rectangle.
fn redraw_whole(frame: &mut Frame, page: &Page, pixels_per_point: f32) -> Vec<PixelRect> {
    *frame = rasterise(page, pixels_per_point);
    vec![PixelRect {
        x: 0,
        y: 0,
        width: frame.width,
        height: frame.height,
    }]
}

/// Whether two positioned items are the same item at the same place.
fn same(old: &(Point, FrameItem), new: &(Point, FrameItem)) -> bool {
    old.0 == new.0 && hash128(&old.1) == hash128(&new.1)
}

/// Collect, in page points, the bounds of every item that differs between two
/// frames drawn under `ts`. Equal leading and trailing runs are skipped, so an
/// inserted or removed item damages itself rather than everything after it; a
/// group that kept its place, transform and clip is compared inside.
fn diff(prev: &LayoutFrame, next: &LayoutFrame, ts: Transform, out: &mut Vec<Rect>) {
    if hash128(prev) == hash128(next) {
        return;
    }
    let (old, new) = (prev.items().as_slice(), next.items().as_slice());
    let head = old.iter().zip(new).take_while(|(a, b)| same(a, b)).count();
    let (old, new) = (
        old.get(head..).unwrap_or_default(),
        new.get(head..).unwrap_or_default(),
    );
    let tail = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take_while(|(a, b)| same(a, b))
        .count();
    let old = old
        .get(..old.len().saturating_sub(tail))
        .unwrap_or_default();
    let new = new
        .get(..new.len().saturating_sub(tail))
        .unwrap_or_default();
    if old.len() == new.len() {
        for (a, b) in old.iter().zip(new) {
            diff_item(a, b, ts, out);
        }
    } else {
        out.extend(
            old.iter()
                .chain(new)
                .filter_map(|item| item_bounds(item, ts)),
        );
    }
}

/// Damage for one pair of items in the same slot of their frames.
fn diff_item(
    old: &(Point, FrameItem),
    new: &(Point, FrameItem),
    ts: Transform,
    out: &mut Vec<Rect>,
) {
    if same(old, new) {
        return;
    }
    if let ((old_pos, FrameItem::Group(old_group)), (new_pos, FrameItem::Group(new_group))) =
        (old, new)
        && old_pos == new_pos
        && old_group.transform == new_group.transform
        && hash128(&old_group.clip) == hash128(&new_group.clip)
    {
        let inner = group_ts(ts, *old_pos, old_group);
        diff(&old_group.frame, &new_group.frame, inner, out);
        return;
    }
    out.extend(item_bounds(old, ts));
    out.extend(item_bounds(new, ts));
}

/// The transform a group's children are drawn under.
fn group_ts(ts: Transform, pos: Point, group: &GroupItem) -> Transform {
    ts.pre_concat(Transform::translate(pos.x, pos.y))
        .pre_concat(group.transform)
}

/// Everything a positioned item paints, as one page-point rectangle; `None` for
/// items that paint nothing (links, tags, an empty group).
fn item_bounds(item: &(Point, FrameItem), ts: Transform) -> Option<Rect> {
    let (pos, item) = item;
    if let FrameItem::Group(group) = item {
        let inner = group_ts(ts, *pos, group);
        return group
            .frame
            .items()
            .filter_map(|child| item_bounds(child, inner))
            .reduce(union);
    }
    leaf_bounds(item, ts.pre_concat(Transform::translate(pos.x, pos.y)))
}

/// A painted leaf's bounds under `ts` (which already includes its position).
fn leaf_bounds(item: &FrameItem, ts: Transform) -> Option<Rect> {
    let local = match item {
        FrameItem::Shape(shape, _) => shape.geometry.bbox(shape.stroke.as_ref()),
        FrameItem::Text(text) => text_bounds(text),
        FrameItem::Image(_, size, _) => Rect::from_pos_size(Point::zero(), *size),
        FrameItem::Group(_) | FrameItem::Link(..) | FrameItem::Tag(_) => return None,
    };
    Some(map_rect(local, ts))
}

/// A text run's bounds from its baseline origin: every glyph gets the font's
/// global bounding box (the union of all its outlines), so no glyph's ink can
/// fall outside, plus the stroke's width when the text is stroked.
fn text_bounds(text: &TextItem) -> Rect {
    let face = text.font.ttf().global_bounding_box();
    let per_unit = text.size / text.font.units_per_em();
    let units = |value: i16| per_unit * f64::from(value);
    let pad = text
        .stroke
        .as_ref()
        .map_or(Abs::zero(), |stroke| stroke.thickness);
    let (mut x, mut y) = (Abs::zero(), Abs::zero());
    let mut bounds: Option<Rect> = None;
    for glyph in &text.glyphs {
        let origin = Point::new(
            x + glyph.x_offset.at(text.size),
            -(y + glyph.y_offset.at(text.size)),
        );
        let ink = Rect::new(
            Point::new(
                origin.x + units(face.x_min) - pad,
                origin.y - units(face.y_max) - pad,
            ),
            Point::new(
                origin.x + units(face.x_max) + pad,
                origin.y - units(face.y_min) + pad,
            ),
        );
        bounds = Some(bounds.map_or(ink, |acc| union(acc, ink)));
        x += glyph.x_advance.at(text.size);
        y += glyph.y_advance.at(text.size);
    }
    bounds.unwrap_or_else(|| Rect::new(Point::zero(), Point::zero()))
}

/// The axis-aligned box around `rect`'s four corners under `ts`.
fn map_rect(rect: Rect, ts: Transform) -> Rect {
    let corners = [
        rect.min,
        Point::new(rect.max.x, rect.min.y),
        Point::new(rect.min.x, rect.max.y),
        rect.max,
    ]
    .map(|corner| corner.transform(ts));
    let min = corners.into_iter().reduce(Point::min).unwrap_or_default();
    let max = corners.into_iter().reduce(Point::max).unwrap_or_default();
    Rect::new(min, max)
}

/// The smallest rectangle holding both.
fn union(one: Rect, other: Rect) -> Rect {
    Rect::new(one.min.min(other.min), one.max.max(other.max))
}

/// A page's device-pixel grid at one scale: the size `typst-render` rasterises
/// it to, and the mapping from page points onto it.
#[derive(Clone, Copy)]
struct Grid {
    width: u32,
    height: u32,
    pixels_per_point: f64,
}

impl Grid {
    /// The grid `typst-render` lays `page` on at `pixels_per_point`, rounding
    /// its size the same way.
    fn new(page: &Page, pixels_per_point: f32) -> Self {
        let size = page.frame.size();
        let to_pixels = |length: Abs| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "typst-render rounds in f32"
            )]
            let points = length.to_pt() as f32;
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "typst-render's own pixel-size formula; at least 1, page-sized"
            )]
            let pixels = (pixels_per_point * points).round().max(1.0) as u32;
            pixels
        };
        Self {
            width: to_pixels(size.x),
            height: to_pixels(size.y),
            pixels_per_point: f64::from(pixels_per_point),
        }
    }

    /// The pixels `rect` (page points) can touch, bleed included, clamped to
    /// the grid; `None` when that leaves nothing.
    fn cover(self, rect: Rect) -> Option<PixelRect> {
        let edge = |length: Abs, round: fn(f64) -> f64, bleed: f64, limit: u32| {
            let pixels = round(length.to_pt() * self.pixels_per_point) + bleed;
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "clamped to 0..=limit first"
            )]
            let clamped = pixels.clamp(0.0, f64::from(limit)) as u32;
            clamped
        };
        let x0 = edge(rect.min.x, f64::floor, -BLEED_PX, self.width);
        let y0 = edge(rect.min.y, f64::floor, -BLEED_PX, self.height);
        let x1 = edge(rect.max.x, f64::ceil, BLEED_PX, self.width);
        let y1 = edge(rect.max.y, f64::ceil, BLEED_PX, self.height);
        (x1 > x0 && y1 > y0).then_some(PixelRect {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        })
    }
}

/// Merge overlapping or touching rectangles until none overlap, so a pixel is
/// redrawn and copied once.
fn merge(mut rects: Vec<PixelRect>) -> Vec<PixelRect> {
    let mut merged = Vec::with_capacity(rects.len());
    while let Some(mut rect) = rects.pop() {
        while let Some(at) = rects.iter().position(|other| touches(rect, *other)) {
            rect = hull(rect, rects.swap_remove(at));
        }
        if let Some(at) = merged.iter().position(|other| touches(rect, *other)) {
            rects.push(hull(rect, merged.swap_remove(at)));
        } else {
            merged.push(rect);
        }
    }
    merged
}

/// Whether two rectangles overlap or share an edge.
const fn touches(one: PixelRect, other: PixelRect) -> bool {
    one.x <= other.x + other.width
        && other.x <= one.x + one.width
        && one.y <= other.y + other.height
        && other.y <= one.y + one.height
}

/// The smallest rectangle holding both.
fn hull(one: PixelRect, other: PixelRect) -> PixelRect {
    let (x, y) = (one.x.min(other.x), one.y.min(other.y));
    let right = (one.x + one.width).max(other.x + other.width);
    let bottom = (one.y + one.height).max(other.y + other.height);
    PixelRect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    }
}

/// The damaged rectangles a pruned raster must reproduce.
struct Keep<'a> {
    grid: Grid,
    rects: &'a [PixelRect],
}

impl Keep<'_> {
    /// Whether a leaf with page-point bounds `rect` paints into any of them.
    fn touches(&self, rect: Rect) -> bool {
        self.grid
            .cover(rect)
            .is_some_and(|px| self.rects.iter().any(|kept| touches(px, *kept)))
    }
}

/// `frame` with every leaf that paints into no damaged rectangle dropped, and
/// every group kept with its transform, clip, size and kind so the leaves left
/// inside it are drawn exactly as before. Links and tags paint nothing and go.
fn prune(frame: &LayoutFrame, ts: Transform, keep: &Keep<'_>) -> LayoutFrame {
    let mut out = LayoutFrame::new(frame.size(), frame.kind());
    for (pos, item) in frame.items() {
        match item {
            FrameItem::Group(group) => {
                let inner = prune(&group.frame, group_ts(ts, *pos, group), keep);
                if !inner.is_empty() {
                    let mut kept = group.clone();
                    kept.frame = inner;
                    out.push(*pos, FrameItem::Group(kept));
                }
            }
            FrameItem::Link(..) | FrameItem::Tag(_) => {}
            leaf => {
                let placed = ts.pre_concat(Transform::translate(pos.x, pos.y));
                if leaf_bounds(leaf, placed).is_some_and(|rect| keep.touches(rect)) {
                    out.push(*pos, leaf.clone());
                }
            }
        }
    }
    out
}

/// Copy `rect` from `src` into `dst`, two rasters of one size.
fn copy_rect(src: &Frame, dst: &mut Frame, rect: PixelRect) {
    let stride = src.width as usize * 4;
    let (start, len) = (rect.x as usize * 4, rect.width as usize * 4);
    let rows = rect.y as usize..(rect.y + rect.height) as usize;
    let src_rows = src
        .rgba
        .chunks_exact(stride)
        .skip(rows.start)
        .take(rows.len());
    let dst_rows = dst
        .rgba
        .chunks_exact_mut(stride)
        .skip(rows.start)
        .take(rows.len());
    for (from, to) in src_rows.zip(dst_rows) {
        if let (Some(from), Some(to)) =
            (from.get(start..start + len), to.get_mut(start..start + len))
        {
            to.copy_from_slice(from);
        }
    }
}

#[cfg(test)]
#[path = "damage_tests.rs"]
mod tests;
