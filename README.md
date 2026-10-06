# typst-world

[![CI](https://github.com/mlavrinenko/typst-world/actions/workflows/ci.yml/badge.svg)](https://github.com/mlavrinenko/typst-world/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/typst-world.svg)](https://crates.io/crates/typst-world)
[![License: MIT](https://img.shields.io/crates/l/typst-world.svg)](LICENSE-MIT)

Configurable eval-only typst::World over a file-provider trait, plus a Typst-free value tree — a substrate for tools that use Typst as a data medium

## Install

```bash
cargo add typst-world
```

Default features are eval-only: no fonts, no layout, no render, no network.
Enable `render` to compile documents to bitmap/SVG frames (pulls in
`typst-layout`, `typst-render`, `typst-svg`, and bundled fonts via
`typst-assets`):

```bash
cargo add typst-world --features render
```

## Usage

```rust,no_run
use typst_world::World;

let world = World::new(std::path::Path::new("doc.typ"))?;
println!("{}", world.root().display());
# Ok::<(), typst_world::WorldError>(())
```

All byte reads go through a `FileProvider`, so the world is target-agnostic:
`DiskProvider` reads from disk; `BytesProvider` feeds sources, fonts, and
packages from embedded bytes for non-native targets (e.g. web builds).

### Where `@local` packages come from

By default an `@local/<name>:<version>` import resolves under the platform
local-package directory, the same place Typst itself looks. Two knobs move
that: `World::with_local_package(name, dir)` pins one package to an in-repo
lib directory, and `World::with_package_root(root)` moves the whole lookup —
`root` being the directory that holds `local/<name>/<version>/`, which is what
Typst's own `--package-path` names.

```rust,ignore
let world = World::new(path)?.with_package_root(Some(my_data_dir.join("typst/packages")));
```

A host with its own data-directory setting wants the second one. The platform
lookup goes through `dirs`, which reads the environment on Unix only — on
Windows it calls a known-folder API no variable can move — so exporting
`XDG_DATA_HOME` redirects the lookup on Linux and nothing anywhere else, and a
host that installs a package under its own setting would read from a different
directory than it wrote to. Passing `None` says no local-package directory
exists at all, leaving only the explicit per-package overrides: an answer, not
a fall-through to the machine's own.

A batch consumer evaluating many short-lived worlds can share work across
them: a `World` with no custom inputs/globals shares one process-wide default
eval library, and `SourceSnapshot` lets several worlds share one parsed-source
cache via `World::with_shared_sources`, so a common imported prelude is parsed
once no matter how many worlds read it.

### Eval errors

`World::eval_error(&diags)` turns the diagnostics of a failed `typst_eval::eval`
or `typst::compile` into an `EvalError`: each `Diagnostic` keeps Typst's
message, the `Location` (path, 1-based line and column) its span points at,
and its trace, innermost first. `main_location()` picks the first point inside
the main file, walking the trace when the error was raised in an imported
file or package:

```rust,ignore
// main.typ, line 3: #check(0)   — the assert fails inside lib/check.typ
let err = world.eval_error(&diags);
assert_eq!(err.to_string(), "assertion failed: too small");
let at = err.main_location().unwrap();
assert_eq!((at.path.as_str(), at.line, at.column), ("main.typ", 3, 2));
```

### Hit targets

With the `render` feature, `render_with_targets` returns the raster frame plus
a `HitTarget` for every `#link` on the page — the URL verbatim, and the
rectangle the layout put it at in page points (top-left origin, same
orientation as the frame):

```rust,ignore
let (frame, targets) = typst_world::render_with_targets(&world, 2.0)?;
```

So a document can declare its own interactive regions — `#link("app:note/60",
key)` — and the host routes pointer input onto them without repeating any
layout math. Positions come from the very layout that drew the pixels, over
the same single compile, so they cannot drift from the frame.

Targets arrive in document order, which is paint order: for overlapping
targets the last match wins. Only URL destinations are reported; links to a
label or position inside the document are skipped.

## Development

Prerequisites: [Nix](https://nixos.org/) with flakes enabled.

```bash
direnv allow         # or: nix develop

just check           # fmt + clippy + tests + file-size + drift check
just build
just test
just cover           # code coverage (70% minimum)
just fmt             # format code
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for coding conventions.

## License

MIT
