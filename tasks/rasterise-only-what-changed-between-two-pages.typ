#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "rasterise only what changed between two pages",
  status: wip(2026, 10, 17),
)

== Summary

A host that redraws on every input tick rasterises the whole page each time,
even when one key of an on-screen keyboard changed colour: in jmav a full
2000x1200 page costs ~22 ms where the one changed key costs ~0.5 ms. Typst's
memoised layout already hands back unchanged subtrees as identical frames, so
the difference between two laid-out pages is cheap to find, and only the
pixels it covers need drawing again.

== Scope

- `damage(prev, next, pixels_per_point)` returns the pixel rectangles where
  the two pages can differ once rasterised: empty when they are identical,
  `None` when their size or fill differs.
- `rasterise_changes(frame, prev, next, pixels_per_point)` brings a frame of
  `prev` up to date with `next`, drawing only the damaged rectangles, and
  returns them. Byte-identical to `rasterise(next)`: it rasterises `next` at
  the full-page transform, pruned to the items that touch the damage, and
  copies only the damaged rectangles out. A translated sub-page differs from
  the full raster in a few anti-aliased pixels per rectangle.
- Item bounds are conservative: a stroke's joins and caps, a glyph's font-wide
  bounding box, a pixel of anti-aliasing bleed.

== Acceptance

- For a recoloured shape, edited text, a moved, added or removed item, a
  rotated group and a clipped group, `rasterise_changes` equals
  `rasterise(next)` byte for byte.
- A one-shape change damages that shape's pixels only.
- Identical pages damage nothing; a resized page returns `None` and the frame
  is rasterised whole.
- Gates green; CHANGELOG `[Unreleased]` names the additions.
