#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "name a disk world's main file with slashes on Windows",
  status: proposed(2026, 10, 28),
)

== Summary

`World::new` and `World::with_root` name the main file by its path relative
to the project root, turned into a string with `Path::to_str` and handed to
`VirtualPath::new`. On Windows that string is `tasks\a.typ`, and Typst 0.15
refuses `\` in a virtual path, so a main file in any subfolder fails with
`bad virtual path "tasks\\a.typ"`. Only a file at the root loads.

MindTape's Windows CI hit it on every task file under `tasks/`, and works
around it with a `slash_path` helper and a `disk_world` of its own. A host
that needs the id of a file it has not built a world for, as MindTape's
validator does, has no call here that takes a platform path, so it
spells the `/` path itself.

- https://github.com/typst/typst/blob/v0.15.1/crates/typst-syntax/src/path.rs

== Scope

- A disk world's main id comes from `VirtualPath::virtualize(root, file)`,
  Typst's one translation from a real path to a virtual one, so it is
  `/`-separated on every platform.
- A public `project_file_id(root, path)` returns that id for any file under
  `root`, and the disk constructors build theirs through it.
- No other place in the crate turns a platform path into a virtual path or
  an id.

== Acceptance

- A world built on `<root>\tasks\a.typ` names its main `/tasks/a.typ` and
  evaluates it, on Windows and Unix.
- A file outside `root` is still refused with the same message.
- README and CHANGELOG name the fix; gates green; released as 0.5.6.
