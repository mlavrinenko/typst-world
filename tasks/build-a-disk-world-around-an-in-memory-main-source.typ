#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "build a disk world around an in-memory main source",
  status: done(2026, 10, 28)[released in 0.5.7],
)

== Summary

A host that evaluates source it generated, such as a probe that imports a
project module and calls one function, wants a disk-backed world whose main
file lives only in memory. There is no constructor for that. `World::new`
and `World::with_root` need the main file on disk, so a host writes
`with_provider(id, root, Box::new(DiskProvider))` and then
`with_source_override(id, source)`, naming the id twice and the provider by
hand. MindTape does this in three places. If the two ids ever differ, the
world reads a main file from disk that isn't there, and nothing says why.

The world is this crate's type; typst-harvest only re-exports it as
`HarvestWorld`, so the constructor belongs here.

== Scope

- `World::with_main_source(main, root, source)` builds a disk-backed world
  whose main file `main` is `source`, read from nothing else.
- Imports from it resolve under `root`, relative to `main`'s virtual path.
- Every builder (`with_package_root`, `with_local_package`,
  `with_shared_sources`) still applies.

== Acceptance

- A world built this way evaluates `source`, imports a file under `root` by
  an absolute and a relative path, and never reads `main` from disk.
- README and CHANGELOG name it; gates green; released.
