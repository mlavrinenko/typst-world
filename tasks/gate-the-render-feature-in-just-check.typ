#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "gate the render feature in just check",
  status: proposed(2026, 7, 31),
)

== Summary

`just check` never compiles the `render` module. Its clippy and test arms run
`--workspace --all-targets` with no `--all-features`, and `render` is default-off
(`default = []`), so every line behind that feature — `render`, `render_svg`,
`render_with_targets`, the hit-target walk, and their tests — is skipped by the
primary gate. Only `just cover`, which does pass `--all-features`, reaches it,
and coverage is a separate opt-in run rather than the gate a change is expected
to pass. A feature-gated regression can land green today.

== Scope

- Add `--all-features` to the `clippy` and test arms of `check`, and to the
  standalone `clippy` / `test` recipes so a local run matches the gate.
- Keep a default-features arm too, or the reverse gap opens: a change that only
  compiles with `render` on would pass while the published default build broke.
  Two arms, not one substitution.
- Check whether `check`'s other arms (unused deps, file size, drift) have the
  same blind spot.
- The arms are memoised through the project's runner; make sure the added arms
  are keyed so a feature-only change still invalidates them.

== Acceptance

- Deliberately breaking something inside `#[cfg(feature = "render")]` — a lint
  violation and a failing test — turns `just check` red.
- Deliberately breaking the default-features build turns `just check` red.
- `just check` green on an unmodified tree.

== Notes

- Found while implementing `expose-laid-out-hit-targets-from-the-render-path`:
  that work needed `cargo test --features render` and
  `cargo clippy --all-features` run by hand on top of a green `just check`,
  which is the tell.
