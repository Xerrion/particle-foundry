# GitHub CI and merge policy

GitHub is the authoritative Git host for Particle Foundry.
[The workflow](workflows/ci.yml) runs on pushes to `main` and on pull requests.
All jobs use GitHub-hosted `ubuntu-latest` runners. The former
[Charles runner configuration](runners/README.md) remains available for recovery.
Keep those services stopped unless a separate decision authorizes their use.

## Checks and merge requirements

The documentation job always runs. It validates source links, manifests,
historical integrity and the complete offline handbook through `mise run docs:check`.
It also uploads the compiled handbook for browser review.
Changes confined to `docs/` skip the web,
Rust and browser jobs. Other changes run all three jobs through `ci:web`,
`ci:rust` and `ci:browser`. The final `verify` job requires each applicable job
and rejects failed or missing results. Run `mise run ci` for the complete local
check before delivery.

The workflow pins mise to the release used by the Dockerfiles. Mise selects
Bun, Node.js, Python and validation tools from the project configuration.
Rustup selects the engine toolchain and WASM target from
[`engine/rust-toolchain.toml`](../engine/rust-toolchain.toml).
The WASM library and wasm-bindgen CLI must use the same version.

Use a feature branch and a conventional commit or PR title. Review the complete
change, applicable checks and resolved review conversations before merging.
Passing checks do not authorize a merge or deployment.
[Deployment configuration](../docs/deployment.md) owns the Coolify setup.

## Browser evidence

The browser job uses Chrome from its hosted runner. Its ordinary smoke test
can pass with `gpu.status: unavailable`. That result proves the executed
browser and WASM checks. It does not prove GPU simulation or rendering.

For hardware evidence, run
`mise run test:browser artifacts/validation/browser/<new-run> --require-gpu`
on a machine with a real adapter. Add `--sustained-gpu` when the assigned gate
requires the sustained fixture. Record hardware results separately from CI.
The workflow retains browser artifacts for one day. No paid GPU runner is
configured.
