#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "read local packages from the directory Typst reads them from",
  status: done(2026, 10, 19)[released in 0.5.5],
)

== Summary

`DiskProvider` resolves an unoverridden `@local` import under
`dirs::data_local_dir()` joined with `typst/packages`, and its docs call that
what Typst itself uses. It is not. Typst and tinymist read `@local` packages
from `$TYPST_PACKAGE_PATH` (or `--package-path`) when set, else from
`dirs::data_dir()` joined with `typst/packages`. On Linux and macOS the two
`dirs` calls agree; on Windows `data_dir` is `%APPDATA%` and `data_local_dir`
is `%LOCALAPPDATA%`, so a package installed where Typst finds it is a miss
here, and the reverse.

A host that installs a package for its users, as MindTape's `mt install`
does, also has no way to ask this crate where that directory is, so it
computes the path itself and can drift from the reader.

- https://github.com/typst/typst/blob/bd5ebec285505889e8ff1d4f68804ab147a699f3/crates/typst-kit/src/packages.rs#L166-L169
- https://github.com/typst/typst/blob/bd5ebec285505889e8ff1d4f68804ab147a699f3/crates/typst-cli/src/packages.rs#L6-L11
- https://github.com/typst/typst/blob/bd5ebec285505889e8ff1d4f68804ab147a699f3/crates/typst-cli/src/args.rs#L524-L525
- https://github.com/Myriad-Dreamin/tinymist/blob/45b50f0d928e825248fa1c2fcbcd68549e957b5d/crates/tinymist-package/src/registry/http.rs#L78-L81
- https://github.com/Myriad-Dreamin/tinymist/blob/45b50f0d928e825248fa1c2fcbcd68549e957b5d/crates/tinymist-world/src/args.rs#L48-L49

== Scope

- The platform directory is Typst's: a non-empty `$TYPST_PACKAGE_PATH`
  names it whole, else `dirs::data_dir()` joined with `typst/packages`.
- An empty `$TYPST_PACKAGE_PATH` names no directory, so a `@local` read
  fails, as Typst refuses to run with it.
- A public `platform_package_root()` returns that directory, and
  `DiskProvider` reads through it, so a host that writes a package and this
  crate that reads it share one answer.
- `ScopedDiskProvider` and `with_local_package` are unchanged.

== Acceptance

- `DiskProvider` resolves `@local/<name>:<version>` under
  `platform_package_root()`.
- A set `$TYPST_PACKAGE_PATH` wins, an empty one names no directory, and
  an unset one falls through to `dirs::data_dir()`.
- README and CHANGELOG name the directory; gates green; released as a patch.
