# Rust simulation engine

This directory owns the planned Rust/WASM engine and its WGSL shaders. The
running application currently uses the TypeScript backend in
[`web/src/legacy/`](../web/src/legacy/). No Cargo workspace or Rust/WASM backend
is implemented yet.

Create the workspace during E01 of the [migration plan](../docs/plans/rust-wasm-migration/plan.md):

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
      shaders/               WGSL owned by these pipelines
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
validation requirements. Add real Rust/WASM tasks to root `mise.toml` when the
implementation exists; there are no Rust commands to run at this stage.
