# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

## [0.3.2] - 2026-09-14

### Fixed

- `convert`'s content projection (`content_str`) no longer flattens a
  multi-paragraph or list `Content` value into one run-on word.
  `Content::plain_text()` only concatenates the leaf text it finds while
  walking the tree, with nothing between a paragraph break or a list item and
  the text before it — a status note's trailing `[…]` block with a blank-line
  paragraph break or a bulleted list came back with the break silently
  dropped, welding two sentences into one. Every `Par`/`ListItem`/`EnumItem`/
  `TermItem` boundary now costs a separating space.

## [0.3.1] - 2026-08-17

### Added

- `World::with_package_root(Option<PathBuf>)` and the `ScopedDiskProvider`
  behind it: resolve `@local/<name>:<version>` under a directory the caller
  names — the one holding `local/<name>/<version>/`, what Typst's own
  `--package-path` names — instead of the platform local-package directory.
  `None` states that no such directory exists, leaving only the explicit
  `with_local_package` overrides; it is an answer, not a fall-through to the
  machine's own. Not calling it keeps the platform lookup, so nothing changes
  for a world that does not ask.

  The platform lookup goes through `dirs`, which reads the environment on Unix
  only — on Windows it calls a known-folder API no variable can move. A host
  that keeps its own data-directory setting therefore could not express it, and
  would install a package into one directory while reading from another.

  Purely additive: `ProviderCtx`, `DiskProvider` and `resolve_path` are
  untouched, and both providers resolve through one shared path so they cannot
  diverge on anything but the package directory.

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
