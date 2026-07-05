# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
