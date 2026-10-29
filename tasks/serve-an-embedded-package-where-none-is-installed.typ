#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "serve an embedded package where none is installed",
  status: done(2026, 10, 29)[released in 0.5.8],
)

== Summary

A host that embeds its own Typst package, as MindTape embeds its prelude, wants
a world to read the installed copy when there is one and the embedded copy
otherwise. A CI runner or a fresh clone has no installed copy, so every task
fails to load until someone runs an install step. The provider seam can't
express this today: `with_package_root` installs a `ScopedDiskProvider`, and
nothing swaps a built world's provider or chains two.

== Scope

- `FallbackProvider::new(first, second)` reads through `first`, then through
  `second` where `first` fails, returning `first`'s error when both fail.
- `World::with_file_provider(provider)` replaces a built world's provider and
  keeps its main file, root, source and package overrides.

== Acceptance

- A fallback serves the installed file over the embedded one, the embedded one
  where nothing is installed, and the disk error where neither has it.
- A world built on an in-memory main reads a package through a swapped-in
  provider.
- README and CHANGELOG name both; gates green; released.
