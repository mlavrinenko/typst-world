#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "push main, wait for its CI, then tag",
  status: done(2026, 10, 7)[adopted cratemplate's push-wait-tag recipe],
)

== Summary

`just release` refused unless main already matched origin/main with a green
`ci.yml` run, so a release took three steps by hand: push main, wait for CI,
run the recipe. The push was easy to forget, and a wait judged by eye could
tag a commit whose CI then went red.

== Scope

- Adopt cratemplate's one-command `just release X.Y.Z [--dry-run]`, keeping
  `just check` as its gate: it checks everything that needs no push, pushes
  main fast-forward only, waits for the `ci.yml` run on that exact commit,
  and tags and pushes the tag only if that run is green.
- `--dry-run` says what it would push and pushes nothing.
- CONTRIBUTING.md's Releasing section says not to push main first.
