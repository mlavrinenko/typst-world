#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "keep the main file out of a shared source snapshot",
  status: proposed(2026, 10, 18),
)

== Summary

`with_shared_sources` caches every source a world reads, the main file
included. Hosts that build many worlds over one set of library files, each
with its own main document, share the library parses but must keep each
main out of the snapshot. Today the only way is to give every main a unique
virtual path (a content hash), so diagnostics name `main-<hash>.typ`
instead of the path the host chose.

== Scope

- An opt-in on the shared snapshot (or on `with_shared_sources`) that
  serves the world's main file from the world's own private cache, while
  every other file still comes from the snapshot.
- Default behaviour unchanged.

== Acceptance

- Two worlds sharing one snapshot, each with a different main at the same
  virtual path, each compile their own main; a library file both import is
  parsed once.
- Diagnostics from the main file carry the host's chosen path.
- Gates green; CHANGELOG `[Unreleased]` names the addition.
