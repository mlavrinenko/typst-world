# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-08-01

### Added

- `render_with_targets` and `HitTarget` (behind the `render` feature): the
  raster frame plus the laid-out rectangle of every `#link` on the page, in
  page points with a top-left origin. A document declares its own interactive
  regions with `#link("app:action/arg", body)` and the host routes pointer
  input onto them without redoing layout. Targets come from the same single
  `typst::compile` the frame does, in document (paint) order, so the last match
  wins where they overlap; only URL destinations are reported.

## [0.2.0] - 2026-07-16

### Added

- Process-wide shared default eval `Library`: a `World` with no custom
  `inputs`/`globals` now clones a shared `Arc` instead of building its own
  `Library::default()`, so a batch consumer evaluating many short-lived worlds
  pays for one build total.
- `SourceSnapshot`: a shareable, cheaply cloneable source cache. Wire it into
  several worlds via `World::with_shared_sources` so they parse each unique
  file (e.g. a common imported prelude) exactly once instead of once per
  world. The default (no `with_shared_sources`) is unchanged: a world still
  caches sources privately.

## [0.1.0]

Initial extraction from the mindtape workspace (`crates/typst-world`).

### Added

- `World`/`WorldBuilder`: configurable eval-only `typst::World` resolving
  project-local imports against a root directory and `@local/<name>:<version>`
  packages against the platform local-package directory.
- `FileProvider` seam (`DiskProvider`, `BytesProvider`) so byte reads are
  target-agnostic.
- `HVal`/`convert`/`format_date`: a Typst-free value tree for consumers that
  read evaluated Typst values without depending on `typst::foundations`.
- Optional `render` feature (`render`/`render_svg`) for compiling a world's
  main file to a bitmap or SVG frame; off by default so eval-only consumers
  pay nothing for layout, render, or bundled fonts.
