#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "add a caller's own font to the world",
  status: done(2026, 10, 17),
)

== Summary

`with_fonts` loads typst-assets' bundled faces and nothing else, so a host
cannot lay out text in a face of its own. jmav draws music glyphs as Leland
text (scorify's native output), and on web and Android there are no system
fonts to fall back on: the face has to come in as bytes.

== Scope

- `World::with_font(data)` adds every face in `data` to the font book beside
  the ones already loaded; it chains after `with_fonts`.
- Render preset only, like `with_fonts`.

== Acceptance

- A document asking for a face with `fallback: false` draws nothing without
  it and draws once `with_font` adds it.
- After `with_fonts().with_font(..)` the bundled faces still resolve and the
  added face sits after them.
- Gates green; CHANGELOG `[Unreleased]` names the addition.
