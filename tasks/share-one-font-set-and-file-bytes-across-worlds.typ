#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "share one font set and file bytes across worlds",
  status: done(2026, 10, 18),
)

== Summary

`with_fonts` and `with_font` wrap every face in a fresh `Bytes` per world,
and `BytesProvider::with_file` copies its bytes into a fresh `Bytes` too.
Typst caches a `Bytes` hash inside the value, so a host that builds many
worlds over the same fonts and embedded files re-hashes megabytes of
identical data in every one. A host profile put ~40% of a batch run's
samples in `StableHasher::write`.

== Scope

- A cheaply cloneable `FontSet` (render feature): the bundled faces, plus a
  caller's own, built once and handed to any number of worlds.
- `with_fonts` reads one process-wide bundled set instead of a fresh copy.
- `BytesProvider::with_bytes` registers an already-built `Bytes`, so a
  host's process-wide copy is shared, not duplicated.
- Default behaviour and every render unchanged.

== Acceptance

- Two worlds given one `FontSet` serve the same font data, not copies.
- A `Bytes` registered through `with_bytes` is the one `file()` returns.
- Gates green; CHANGELOG `[Unreleased]` names the additions.
