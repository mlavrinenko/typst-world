#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "split compile from rasterise in the render path",
  status: wip(2026, 10, 16),
)

== Summary

`render`, `render_with_targets` and `render_svg` each compile, evict comemo's
cache, and hand back pixels (or SVG). A host that redraws on every input tick
cannot get between those steps: it never sees the laid-out page, so it cannot
re-rasterise only what changed, hand the page to another rasteriser, or keep
it to compare with the next one. And it pays the eviction walk inside every
frame — measured at 4-5 ms of a ~45 ms frame in jmav, where the eviction could
run after presenting instead.

== Scope

- `compile_page(world)` compiles the main file and returns its first laid-out
  `Page`, without evicting.
- `rasterise(page, pixels_per_point)` and `hit_targets(page)` become public:
  the two halves `render_with_targets` already runs over one compile.
- `evict(max_age)` and `EVICT_MAX_AGE` let a `compile_page` caller run the
  eviction itself, when it suits the caller.
- Re-export `Page`, so a caller names it without depending on `typst-layout`.
- `render`, `render_with_targets` and `render_svg` keep their behaviour,
  eviction included.

== Acceptance

- `rasterise(compile_page(w))` is byte-identical to `render(w)` for the same
  world and scale; `hit_targets(compile_page(w))` equals `render_with_targets`'
  targets.
- A failing document returns the same `WorldError` from `compile_page` as from
  `render`.
- Gates green; CHANGELOG `[Unreleased]` names the additions.
