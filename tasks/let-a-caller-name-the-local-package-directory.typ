#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "let a caller name the local-package directory",
  status: done(
    2026,
    8,
    17,
  )[ScopedDiskProvider + World::with_package_root; 44 tests green, no API break],
)

== Summary

`@local/<name>:<version>` resolves under `dirs::data_local_dir()`, hardcoded in
`resolve_package`. A host that carries its own "where my data lives" setting
cannot express it: `dirs` reads the environment on Unix only — on Windows it
calls a known-folder API no variable moves — so exporting `XDG_DATA_HOME`
redirects the lookup on Linux and nothing anywhere else.

The failure that surfaced it is an install and a read disagreeing. MindTape's
`$MINDTAPE_DATA_HOME` moves where `mt install` writes the `@local/mindtape`
package, because that path goes through its own `app_dirs`; it does not move
where the loader reads it, because that path goes through this crate. Point the
variable at a tempdir and the package lands there while every task file keeps
importing the machine's own.

== Scope

- Add the seam without breaking the API. `ProviderCtx`'s fields are public, so
  it cannot gain one — a new provider type can, and `DiskProvider` stays exactly
  what it was. Ships as a minor release, so a consumer picks it up on `cargo
  update` with no coordinated release of anything downstream.
- `None` must mean "no such directory", never "fall back to the platform". A
  host whose data directory is configured empty is stating a fact, and quietly
  reading the machine's own is the bug the whole change exists to prevent.
- Only the fallback moves: an explicit `with_local_package` override still
  wins, and non-`local` namespaces still resolve to nothing.
- The root names the directory holding `local/<name>/<version>/` — what Typst's
  own `--package-path` names — not the `local/` subdirectory.

== Acceptance

- A world with a package root reads the `@local` package under that root, on a
  machine that also has one installed platform-side.
- The same world with `None` reads no `@local` package at all, which is what
  makes the first assertion about the root rather than about the host.
- One resolution path serves both providers, so they cannot diverge on anything
  but the package directory.

== Notes

- Found in mindtape, closing
  `resolve-the-typst-package-from-the-checkout-not-a-global-symlink`: its
  `check-examples` gate had to empty the package directory with `XDG_DATA_HOME`
  because `$MINDTAPE_DATA_HOME` had no effect on a read, which left the gate's
  isolation Linux-only.
