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
`project_file_id(root, path)` names a file on disk with `/` on every platform,
so Windows' `tasks\a.typ` is `/tasks/a.typ`, as `World::new` names its main.
`World::with_main_source(main, root, source)` evaluates source you generated as
the file `main` under `root`, importing the rest from disk.

`HVal` is a harvested value with no Typst types in it. `to_typst()` writes one
back as Typst syntax that evaluates to the same value, and the `serde` feature
serializes it, a date as its `YYYY-MM-DD` text:

```rust,ignore
let call = format!("#check({})", value.to_typst()); // ("due": datetime(year: 2026, month: 10, day: 28),)
let json = serde_json::to_string(&value)?;          // {"due":"2026-10-28"}
```

### Where `@local` packages come from

By default an `@local/<name>:<version>` import resolves under the directory
Typst and tinymist read it from. That's `$TYPST_PACKAGE_PATH` when set, else
the platform data dir joined with `typst/packages`.

| Platform | Directory |
| --- | --- |
| Linux | `$XDG_DATA_HOME/typst/packages` or `~/.local/share/typst/packages` |
| macOS | `~/Library/Application Support/typst/packages` |
| Windows | `%APPDATA%\typst\packages` |

`platform_package_root()` returns that directory, so a host that installs a
package for Typst writes it exactly where this crate reads it back.

```rust,ignore
let dir = typst_world::platform_package_root()
    .ok_or("no local package directory")?
    .join("local/my-pkg/0.1.0");
```

Two knobs move the lookup. `World::with_local_package(name, dir)` pins one
package to an in-repo lib directory. `World::with_package_root(root)` moves
the whole thing, where `root` holds `local/<name>/<version>/`, the same
directory Typst's own `--package-path` names.

```rust,ignore
let world = World::new(path)?.with_package_root(Some(my_data_dir.join("typst/packages")));
```

A host with its own data-dir setting wants the second one, since otherwise only
`$TYPST_PACKAGE_PATH` and, on Unix, `XDG_DATA_HOME` move the lookup. Passing
`None` says there's no local-package directory at all, leaving just the
per-package overrides.

A batch of short-lived worlds can share work. A `World` with no custom inputs
or globals shares one process-wide eval library, and a `SourceSnapshot` given
to `World::with_shared_sources` parses a common prelude once for all of them.
When each world brings its own main at the same path,
`World::with_shared_sources_except_main` shares everything but the main:

```rust,ignore
let shared = SourceSnapshot::new();
let left = World::with_provider(main, root.clone(), left_files).with_shared_sources_except_main(&shared);
let right = World::with_provider(main, root, right_files).with_shared_sources_except_main(&shared);
// Each evaluates its own `main.typ`; an import both read is parsed once.
```

Typst hashes a `Bytes` once per copy, so share copies across worlds. With
`render` on, build one `FontSet` and hand it out (`with_fonts` already shares
the bundled set); `BytesProvider::with_bytes` does the same for embedded files.

```rust,ignore
let fonts = FontSet::bundled().with_font(my_font_bytes);
let logo = Bytes::new(logo_png);
let world = World::with_provider(main, root, Box::new(
    BytesProvider::new().with_file(main, doc).with_bytes(logo_id, logo.clone()),
))
.with_font_set(&fonts);
```

### Eval errors

`World::eval_error(&diags)` turns the diagnostics of a failed `typst_eval::eval`
or `typst::compile` into an `EvalError`, and `World::diagnostic(&diag)` resolves
one diagnostic, warnings included. A `Diagnostic` is plain data: its `Severity`,
Typst's message, the `Location` (path, 1-based line and column) its span points
at, Typst's `hints` (each with an optional `Location`), and its `trace`,
innermost first, each `TracePoint` saying what it passed through.
`main_location()` picks the first point inside the main file, walking the trace
when the error was raised in an imported file or package.

`Display` is neutral: an `EvalError`, a `WorldError::Eval` or a `Diagnostic`
prints Typst's message alone, and a `Location` prints `path:line:column`. A
caller renders the rest its own way:

```rust,ignore
// main.typ: #let a = 1 / #a-b
let err = world.eval_error(&diags);
assert_eq!(err.to_string(), "unknown variable: a-b");
for diag in &err.diagnostics {
    let at = diag.first_in(&err.main).map(ToString::to_string).unwrap_or_default();
    eprintln!("{}: {at}: {}", diag.severity, diag.message);
    for hint in &diag.hints {
        eprintln!("hint: {hint}");
    }
}
// error: main.typ:2:2: unknown variable: a-b
// hint: if you meant to use subtraction, try adding spaces around the minus sign …
```

### Hit targets

With the `render` feature, `render_with_targets` returns the raster frame plus
a `HitTarget` for every `#link` on the page: its URL and its rectangle in page
points, with a top-left origin like the frame.

```rust,ignore
let (frame, targets) = typst_world::render_with_targets(&world, 2.0)?;
```

So a document declares its own interactive regions, such as
`#link("app:note/60", key)`, and the host routes pointer input onto them with
no layout math. The rectangles come from the compile that drew the pixels, so
they can't drift. They arrive in paint order, so the last overlapping match
wins, and a link to a label or a position isn't reported.

### Frame loops

`render*` compile, evict comemo's cache, and return pixels in one call. A host
that redraws on every input tick can take the steps apart and evict once the
frame is on screen:

```rust,ignore
let page = typst_world::compile_page(&world)?;
let frame = typst_world::rasterise(&page, 2.0);
let targets = typst_world::hit_targets(&page);
present(&frame);
typst_world::evict(typst_world::EVICT_MAX_AGE);
```

Keep the page and its frame, and the next tick redraws only what changed.
`rasterise_changes` compares the two pages, redraws the damaged pixels, and
returns them; the frame ends up byte-identical to a full `rasterise`:

```rust,ignore
let next = typst_world::compile_page(&world)?;
let damaged = typst_world::rasterise_changes(&mut frame, &page, &next, 2.0);
page = next;
```

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
