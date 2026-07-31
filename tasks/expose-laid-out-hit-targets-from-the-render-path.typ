#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "expose laid-out hit targets from the render path",
  status: proposed(2026, 7, 31),
)

== Summary

`render` compiles to a `PagedDocument`, rasterises the first page, and drops
everything else. The laid-out `typst::layout::Frame` it discards carries
`FrameItem::Link(Destination, Size)` at a `Point` — a positioned rectangle
with an arbitrary URL payload. That is a ready-made hit-target primitive: a
consumer can author `#link("app:action/arg", body)` and, after layout, learn
exactly where that body landed on the page. Nothing else gives a host this;
positions come from Typst's own layout, so they cannot drift from what was
drawn. Expose it so a host can route pointer input into a rendered document
without duplicating any layout math.

== Scope

- Add `HitTarget { url: String, x_pt: f32, y_pt: f32, w_pt: f32, h_pt: f32 }`
  (page points, top-left origin, matching the rasterised frame's orientation).
- Add `render_with_targets(world, pixels_per_point) -> Result<(Frame, Vec<HitTarget>), WorldError>`
  behind the existing `render` feature, sharing one `typst::compile` with
  `render` — never a second compile. Keep `render`/`render_svg` unchanged.
- Walk the page frame recursively, accumulating each child's `Point` and each
  `GroupItem::transform`. Verified by spike: a `place`d, `grid`-nested link
  reports the correct absolute point and size with an identity transform, but
  a rotated/scaled group must still resolve — compose transforms rather than
  assuming identity, and fall back to the transformed bounding box.
- Preserve document order in the returned vector. Paint order is significance
  order for overlapping targets (a piano's black keys are drawn over the
  whites), so the consumer's rule is last-match-wins; do not sort or dedupe.
- Only `Destination::Url` yields a target. `Position`/`Location` destinations
  are internal document links with no host meaning — skip them.

== Acceptance

- A doc with three overlapping `#link` rects inside `place` inside a `grid`
  `1fr` row returns three targets with exact points and sizes, in source
  order.
- A link inside a `rotate`/`scale` group returns a target whose rect covers
  the drawn region.
- A doc with no links returns an empty vector and an unchanged frame.
- `render_with_targets` and `render` produce byte-identical frames for the
  same world and scale.
- Gates green; the new surface is documented and covered like the rest of the
  crate.

== Notes

- Consumer is jmav (`tasks/touch-and-mouse-note-input.typ` there), which needs
  this to route taps and clicks onto a rendered on-screen piano. Until this
  ships to crates.io that repo consumes it through `[patch.crates-io]`.
- `GroupItem` also carries `label: Option<Label>`, a second addressing route.
  Not in scope — `link` wins because its payload is an arbitrary string, so a
  consumer encodes its own action vocabulary with no Typst-side registry.
