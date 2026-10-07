#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "finish a half-failed release on its tag",
  status: proposed(2026, 10, 7),
)

== Summary

v0.4.0 was on crates.io before its tag was pushed, so the tag's
`cargo publish` failed on "already exists" and its release run stayed red.
v0.3.2's two runs failed at the check job on a nix fetch 403, and 0.3.2 was
published by hand. Nothing let the workflow finish on the same tag, and
`just release` tagged whatever it was given after `just check`.

== Scope

- Adopt cratemplate's hardened lib release workflow: `cargo publish` is
  skipped for a version crates.io has, and the workflow re-runs on an existing
  tag through `workflow_dispatch`.
- Adopt its `just release X.Y.Z [--dry-run]` preflight, keeping `just check`
  as the gate; document it in CONTRIBUTING.md.
- Re-run v0.4.0 on its tag so its release run goes green.
