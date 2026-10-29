# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.8] - 2026-10-29

### Added

- `FallbackProvider::new(first, second)` reads each file through `first`, then
  through `second` where `first` fails, so a host can serve a package it
  embeds wherever no package directory holds it.
- `World::with_file_provider(provider)` swaps a built world's file provider,
  keeping its main file, root and overrides.

## [0.5.7] - 2026-10-28

### Added

- `World::with_main_source(main, root, source)` builds a disk-backed world
  whose main file is `source`, held in memory, for a host that evaluates
  source it generated.
- `HVal::to_value` is the inverse of `convert`, and `HVal::to_typst` writes an
  `HVal` as Typst syntax that evaluates back to an equal value.
- A `serde` feature serializes and deserializes `HVal`. A date serializes as
  its `YYYY-MM-DD` text and reads back as a string.

## [0.5.6] - 2026-10-28

### Added

- `project_file_id(root, path)` returns the `FileId` of a file on disk under
  `root`, spelled with `/` on every platform.

### Fixed

- `World::new` and `World::with_root` load a main file in a subfolder on
  Windows. They named it by the platform's relative path, `tasks\a.typ`,
  which Typst refuses as a virtual path; they now name it `/tasks/a.typ`
  through `VirtualPath::virtualize`.

## [0.5.5] - 2026-10-19

### Added

- `platform_package_root()` returns the directory `DiskProvider` reads
  `@local` packages from, so a host that installs a package writes it where
  the reader looks.

### Fixed

- `DiskProvider` reads `@local` packages where Typst and tinymist do:
  `$TYPST_PACKAGE_PATH` when set, else `dirs::data_dir()` joined with
  `typst/packages`. On Windows that's `%APPDATA%`; it was `%LOCALAPPDATA%`,
  so a package installed for Typst was a miss here.

## [0.5.4] - 2026-10-18

### Added

- `FontSet` (render feature) is one loaded book and set of faces, cheap to
  clone, and `World::with_font_set` hands it to a world. Worlds given one set
  serve the same font data, so Typst hashes each face once rather than once
  per world. `World::with_fonts` now reads one process-wide bundled set.
- `BytesProvider::with_bytes` registers an already-built `Bytes` without
  copying it, so a host's process-wide copy of an embedded file is shared.
- `World::with_shared_sources_except_main` shares every source but the main
  file through a `SourceSnapshot`; the main stays in the world's own cache.
  Worlds with different main documents at one virtual path share the parses
  of what they import, and diagnostics keep the path the host chose.

## [0.5.3] - 2026-10-17

### Added

- `World::with_font` adds a caller's own font faces (bytes) beside the bundled
  ones, for a face typst-assets lacks, such as a music font, on targets with
  no system fonts.

## [0.5.2] - 2026-10-17

### Added

- `damage` names the pixel rectangles (`PixelRect`) where two laid-out pages
  can differ once rasterised, and `rasterise_changes` redraws only those in a
  raster of the first. The result is byte-identical to rasterising the second
  page whole.

## [0.5.1] - 2026-10-17

### Added

- `compile_page` compiles a world's main file and returns its first laid-out
  `Page` (re-exported) without evicting comemo's cache. `rasterise` and
  `hit_targets` read such a page, and `evict` with `EVICT_MAX_AGE` lets the
  caller trim the cache when it suits it, such as after presenting a frame
  rather than inside it. `render`, `render_with_targets` and `render_svg` still
  evict after every compile.

## [0.5.0] - 2026-10-07

### Changed

- **Breaking:** `WorldError::Eval` displays as Typst's message alone, without
  the `eval error: ` prefix. A caller that wants a prefix adds its own.
- **Breaking:** `Diagnostic` gains `severity` and `hints`, and its `trace` holds
  `TracePoint`s (Typst's description of the step, such as
  ``while calling `check` ``, plus its `Location`) instead of bare `Location`s.
  Code that builds a `Diagnostic` or reads `trace` must adapt.

### Added

- `Severity` (`Error`, `Warning`; displays as `error`/`warning`), `Hint` (Typst's
  hint text and the optional `Location` of the code it is about) and
  `TracePoint`.
- `World::diagnostic` resolves one `SourceDiagnostic`, warnings included.
- `Location` displays as `path:line:column`, and `Diagnostic`, `Hint` and
  `TracePoint` display as their message.

## [0.4.0] - 2026-10-07

### Changed

- **Breaking:** `WorldError::Eval` carries an `EvalError` instead of a
  pre-formatted string, and `format_diagnostics` is gone. An `EvalError` holds
  every `Diagnostic` — Typst's message, the `Location` its span points at, and
  its trace — plus the main file's path. Its `Display` is the old string: the
  messages joined with `; `.

### Added

- `World::eval_error` resolves a failed evaluation's diagnostics against the
  world, and `World::locate` resolves one span to a `Location`: a root-relative
  or package-qualified path with a 1-based line and column.
- `EvalError::main_location` and `Diagnostic::first_in` name the first point of
  an error inside a given file, walking the trace outward when the error was
  raised in an imported file, so a caller can point at the line of the main file
  that led into it.

Releases up to 0.3.2 are in [archive/CHANGELOG.md](archive/CHANGELOG.md).
