#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "publish to crates.io through Trusted Publishing",
  status: proposed(2026, 10, 28),
)

== Summary

The release workflow publishes with `CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}`, a repo-level secret set in June to July 2026 that expires. On 2026-10-28 mindtape's release failed at publish because its stored token, set 2026-03-07, had expired: `error: failed to publish mindtape v0.3.0 to registry at https://crates.io ... (status 403 Forbidden): authentication failed`. This repo breaks the same way when its token expires. nix-why-build (9743ef71) and mindtape (b958a586) already switched to Trusted Publishing, which mints a short-lived token per run and keeps no secret.

== Scope

- `.github/workflows/release.yml`: the `publish` job, which already has `environment: crates-io`, gets `permissions` of `contents: read` and `id-token: write`. A step after the toolchain, `Authenticate with crates.io` with id `auth`, uses `rust-lang/crates-io-auth-action@c6f97d42243bad5fab37ca0427f495c86d5b1a18 # v1.0.5`. The publish step sets `CARGO_REGISTRY_TOKEN: ${{ steps.auth.outputs.token }}`, and `secrets.CARGO_REGISTRY_TOKEN` goes.
- Only crate published: `typst-world`.

== Owner steps

- Before the workflow change lands: at https://crates.io/crates/typst-world/settings, under Trusted Publishing, Add, GitHub, set owner `mlavrinenko`, repo `typst-world`, workflow `release.yml`, environment `crates-io`.
- After the first green publish: `gh secret delete CARGO_REGISTRY_TOKEN -R mlavrinenko/typst-world`, and optionally revoke the token on crates.io.
