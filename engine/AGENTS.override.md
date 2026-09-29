# Rust simulation engine

This directory owns the Rust/WASM workspace and future WGSL shaders. E01's
lifecycle bootstrap, E02's portable contracts and E05's sealed f64 CPU pressure
reference are implemented. E06 has isolated CPU phase-transport and viscous-shear
candidates. The coupled fluid reference is still in progress. The running
application still uses [`web/src/legacy/`](../web/src/legacy/); GPU rendering
and scene promotion remain planned.

The workspace follows the [migration plan](../docs/plans/rust-wasm-migration/plan.md):

```text
engine/
  Cargo.toml                 Workspace definition
  Cargo.lock                 Locked engine dependencies
  rust-toolchain.toml        Tested Rust toolchain
  crates/
    sim/                     Portable IDs, units and contracts
    sim-cpu/                 Rust references and bounded CPU algorithms
    sim-gpu/                 wgpu state, scheduling and rendering
      src/
      shaders/               Future WGSL; create when a pipeline needs it
    wasm/                    Browser bindings and backend construction
```

`sim-cpu` and `sim-gpu` depend on `sim`; the browser bridge constructs the
selected backend. Keep portable contracts independent of concrete backends and
browser APIs. Put tests in their owning crates and add shader families only when
their milestone needs them.

Generate WASM, JavaScript glue and TypeScript declarations into ignored
`web/generated/wasm/`. The browser consumes these through
[`web/src/engine-client/`](../web/src/engine-client/). Keep the material catalogue
single-source and generate validated projections instead of copying definitions.

Use the root [AGENTS.md](../AGENTS.md), [project structure](../docs/project-structure.md)
and [engine contract](../docs/architecture/engine-boundary.md) for ownership and
validation requirements. Run `mise run rust:test`, `mise run rust:clippy`,
`mise run build:wasm` and `mise run test:browser` from the repository root.
`mise run ci` includes these checks. GPU absence is reported separately;
`mise run test:browser artifacts/validation/browser/<new-run> --require-gpu`
requires a real browser adapter. Rustup must be installed before setup.
