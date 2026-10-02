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
Mise caching is disabled. Its GitHub token inputs are empty because the automatic
Forgejo token belongs to this instance.
Checkout uses that token only to read the repository and removes its Git credentials.
Artifact upload uses v3 because Forgejo does not support the stock v4 action.
See the [Forgejo Actions guide](https://forgejo.org/docs/v15.0/user/actions/actions/)
and [artifact support](https://forgejo.org/docs/v15.0/user/actions/advanced-features/).

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
