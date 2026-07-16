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

A batch consumer evaluating many short-lived worlds can share work across
them: a `World` with no custom inputs/globals shares one process-wide default
eval library, and `SourceSnapshot` lets several worlds share one parsed-source
cache via `World::with_shared_sources`, so a common imported prelude is parsed
once no matter how many worlds read it.

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
