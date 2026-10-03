# Forgejo CI and merge policy

The primary repository is
[xerrion/particle-foundry](https://git.xerrion.io/xerrion/particle-foundry).
[CI](workflows/ci.yml) uses the same pinned tools and mise tasks as local development.
The former [GitHub settings](../.github/README.md) are a dated record.
They do not establish the Forgejo branch settings.

## Runner requirements

The `ubuntu-latest` label must select an Ubuntu 24.04 Docker job with root access
and outbound access to the tool and package servers.
The existing Forgejo run uses `ghcr.io/catthehacker/ubuntu:act-24.04` with runner v13.
These containers need explicit compiler, Rust and Chrome setup.
The engine toolchain remains pinned in `engine/rust-toolchain.toml`.

All remote actions use full URLs and commit hashes.
This avoids dependence on the instance's default action server.
Mise is pinned to 2026.9.13 and caches its tools. Its GitHub token inputs are empty
because the automatic Forgejo token belongs to this instance.
Checkout uses that token only to read the repository and removes its Git credentials.
Artifact upload uses v3 because Forgejo does not support the stock v4 action.
See the [Forgejo Actions guide](https://forgejo.org/docs/v15.0/user/actions/actions/)
and [artifact support](https://forgejo.org/docs/v15.0/user/actions/advanced-features/).

## Build cache

Mise restores its tool directory using a key derived from its version, configuration
and the Ubuntu 24.04 amd64 cache prefix. Change the prefix when the runner image
changes or the cache needs to be reset.

The Rust and browser jobs use the Forgejo-hosted `actions/cache` mirror with
Forgejo Runner's cache service. They keep separate caches for the pinned Rust toolchain,
Cargo downloads, installed binaries and `engine/target/`.
The browser cache includes Cargo installation metadata. This lets the existing
exact-version install reuse `wasm-bindgen-cli` without compiling it again.
Cargo credentials and global configuration are outside the cached paths.

Each build key includes the job, runner image, toolchain, lockfile and authored
Rust source or mise configuration. A changed source creates a new cache entry.
Restore prefixes can reuse earlier outputs for the same toolchain, including
when dependencies change. Cargo checks which outputs need to be rebuilt.
Tests and freshness checks still run on every applicable CI run.

The runner cache service must be reachable from job containers.
Each runner stores its cache in `/data/cache` on its existing persistent bind mount.
The runners keep separate entries, so a job can have a cold build on each runner.
A shared Forgejo cache server is needed to reuse entries across runners.
A pull request writes entries isolated to that pull request. The first run on
`main` can therefore be cold after merge.
A missing or evicted entry causes a normal build.
See [Forgejo cache support](https://forgejo.org/docs/v15.0/user/actions/advanced-features/#cache)
and [Cargo cache paths](https://doc.rust-lang.org/cargo/guide/cargo-home.html).

## Checks and merge requirements

CI runs for pull request updates and pushes to `main`.
The documentation check always runs.
Changes confined to `docs/` skip the web, Rust and browser jobs.
Every other change runs all three jobs.
The final `verify` job requires every applicable job to succeed.
`mise run ci` remains the complete local check.

Use a feature branch and a conventional PR title.
Review the complete change before delivery.
Require a successful `verify` result and resolve review findings before merging.
Passing checks do not authorize a merge or deployment.
The Forgejo branch protection settings need separate verification.
Do not infer their configuration from this policy or the old GitHub ruleset.

## Browser evidence

CI installs Chrome and selects a temporary wrapper through `CHROME_BIN`.
The wrapper disables the Chrome sandbox inside the isolated root job container.
Local browser execution keeps its existing settings.

The default browser smoke can pass with `gpu.status: unavailable`.
That result covers the executed browser and WASM checks.
It does not prove GPU simulation or rendering.
Run `mise run test:browser artifacts/validation/browser/<new-run> --require-gpu`
on a machine with a real adapter for that gate.
Record hardware evidence separately.
Browser artifacts have a one-day retention period.
