# Project structure

**Current layout: 27 September 2026.** TypeScript lives in `web/`. `engine/` contains
the E01 Rust/WASM bootstrap and E02 portable contracts, with
[scoped engine guidance](../engine/AGENTS.override.md). The running application
still uses the existing TypeScript backend. E03 supplies an experimental host
contract; E04-E14 remain planned.
Start with [the knowledge map](README.md).

## Current checkout

```text
particle-foundry/
  AGENTS.md                         Project development rules
  README.md                         Quick start and navigation
  mise.toml                         Root tool versions and task orchestration
  .github/workflows/                CI uses the same mise tasks
  web/
    index.html                      Vite browser entry
    package.json, bun.lock          Frontend dependencies
    vite.config.ts                  Build configuration; output is web/dist/
    tsconfig*.json, biome.json       Type checks, formatting and import boundaries
    src/
      main.ts                       Browser startup and frame loop
      app/                          DOM, controls, viewport, pointer mapping, clock
      engine-client/index.ts        Public browser access to the legacy backend
      engine-client/session.ts      Bounded experimental command/probe contract
      engine-client/canvas.ts       Context ownership before acquisition
      engine-client/legacy-adapter.ts  Retained synchronous scene adapter
      engine-client/wasm.ts         Experimental WASM lifecycle adapter
      materials/                    Authored catalogue, validation and reference content
      styles/                       CSS
      legacy/
        simulation/                 World, orchestration, sandbox, scale, diagnostics, RNG
        physics/                    Existing solvers and reactions
        rendering/                  Canvas 2D renderer, maps and visual waves
        tools/                      Physical brush edits and source accounting
        scenes/                     Existing world initialization
    tests/
      app/, materials/, types/      Browser behavior and catalogue checks
      legacy/                       Backend tests, integration cases and fixtures
      browser/                      Production-bundled WASM/WebGPU lifecycle smoke
    scripts/                        WASM/catalogue builds, browser and evidence tools
    generated/wasm/                 Ignored generated glue, declarations and WASM
    generated/catalogue/            Ignored candidate material projection
    benchmarks/                     Existing simulation workloads
  engine/                          Cargo workspace, Rust contracts and bootstrap
  docs/                             Current model, plans, validation and history
  artifacts/                        Ignored local evidence and snapshots
```

Keep frontend configuration and tests with the application. Root `mise.toml`
selects `web/` as the working directory for frontend tasks, preserving commands
such as `mise run dev` and `mise run ci`. Documentation tools and their `.venv/`
remain at repository level. There is no second package manager or task framework.

## TypeScript ownership

The browser imports simulation operations, observations and view metadata through
`web/src/engine-client/index.ts`. The active UI reaches the existing synchronous
Sandbox through a separate legacy adapter. The experimental E03 session contract
is also exported but has no physical owner in the running app. Biome rejects UI
imports of private legacy modules and generated bindings.

The separate `engine-client/wasm.ts` module is consumed by the E01 browser fixture.
It keeps the experimental bootstrap out of the normal application bundle until
E03 defines the queued session contract; a Rust scene adapter remains future work.

`web/src/legacy/simulation/world.ts` owns current mutable state;
`physics.ts` orders its passes, and `sandbox.ts` composes state, tools and rendering.
The brush and starter scene remain in the legacy backend because they mutate its
world. `simulation-clock.ts` belongs to `app/` because it converts elapsed browser
time into requested ticks without owning physical state.

`web/src/materials/definitions.ts` remains the only authored legacy catalogue.
Its public imports, validators and compatibility exports stay together. The 118-name
element reference does not imply 118 implemented materials. Generate future
Rust/WGSL projections from the authored catalogue; do not duplicate definitions.

The E03 session contract handles bounded commands, accepted-time receipts, epoch
reset and asynchronous stamped probes with mock owners. The current UI still uses
the synchronous legacy adapter. Canvas ownership is selected before context
acquisition. A physical Rust scene and its GPU rendering remain future work under
[the engine boundary](architecture/engine-boundary.md).

## Rust workspace

The recovered E01/E02 implementation lives under `engine/`:

```text
engine/
  Cargo.toml                        Workspace definition
  Cargo.lock                        Reproducible engine dependencies
  rust-toolchain.toml               Validated Rust toolchain
  crates/
    sim/                            Portable IDs, units, schemas and contracts
    sim-cpu/                        Rust f64 references and bounded CPU algorithms
    sim-gpu/                        wgpu state, scheduling, reductions and rendering
      src/
      shaders/                      Future WGSL, added when a pipeline needs it
    wasm/                           Browser bindings and backend construction
```

Package names remain `particle-sim`, `particle-sim-cpu`, `particle-sim-gpu` and
`particle-wasm`. Keep unit/integration tests in their owning crates. Add shader
families when their milestone needs them; no empty future trees or placeholder
engines are required.

```text
Browser app -> engine-client -> generated WASM bindings -> particle-wasm
particle-wasm -> particle-sim, particle-sim-cpu, particle-sim-gpu
particle-sim-cpu -> particle-sim
particle-sim-gpu -> particle-sim
```

The portable crate does not depend on concrete backends or browser APIs. The host
constructs the selected backend. One live scene has one state/transport owner;
CPU references run independent fixture worlds. A Rust handle to GPU buffers does
not require a ticking CPU mirror or normal-frame whole-world readback.

`mise run build:wasm` generates JS glue, TypeScript declarations and WASM into ignored
`web/generated/wasm/`. Only the engine client imports bindings. Ignore
`engine/target/`; track Cargo and Bun lockfiles. The build verifies that the
wasm-bindgen CLI matches the pinned library version.

### Implemented bootstrap and contracts

[`particle-sim`](../engine/crates/sim/src/lib.rs) owns grid geometry and the
identity, inventory and source types in [`contracts.rs`](../engine/crates/sim/src/contracts.rs).
[`snapshot.rs`](../engine/crates/sim/src/snapshot.rs) encodes detached `PFSN` v1
checkpoints; [`gpu_layout.rs`](../engine/crates/sim/src/gpu_layout.rs) packs scalar
GPU records. [E02 regression tests](../engine/crates/sim/tests/e02_contracts.rs)
cover IDs, malformed checkpoints, capacity and byte layouts. This is not a live
save/load service or a shader ABI test on hardware.

[`particle-sim-cpu`](../engine/crates/sim-cpu/src/lib.rs) and
[`particle-sim-gpu`](../engine/crates/sim-gpu/src/lib.rs) own separate bootstrap
sessions. The latter initializes a wgpu device; neither advances a physical world.
[`particle-wasm`](../engine/crates/wasm/src/lib.rs) exposes initialization and disposal
through [`engine-client/wasm.ts`](../web/src/engine-client/wasm.ts).
The [browser smoke](../web/tests/browser/engine-smoke.ts) exercises lifecycle,
malformed inputs, independent sessions, missing GPU rejection and actual adapter
initialization from a production bundle under `/engine-smoke/`.

[`engine/fixtures/engine-reference-v1.json`](../engine/fixtures/engine-reference-v1.json)
freezes reference conventions for future numerical work. It does not establish
solver correctness. E03's session contract and candidate catalogue projection
are implemented and tested; see [E03 evidence](validation/p1-m1-e03.md).

## Current-to-target migration map

Paths in the first column are relative to `web/src/`.

| Current owner | Migration destination and gate |
| --- | --- |
| `main.ts`, `app/` | Browser lifecycle, controls and requested versus accepted ticks; E03/E08/E12 |
| `engine-client/`, `legacy/simulation/sandbox.ts` | Async facade and separate legacy adapter; E03/E08 |
| `legacy/simulation/world.ts`, `physical-scale.ts` | `particle-sim` contracts and selected-backend state; E02 |
| `legacy/physics/` | Corrected Rust references then matching WGSL stages; E04-E11 |
| `legacy/rendering/` | Same-device GPU rendering and overlays; E08/E12 |
| `legacy/simulation/diagnostics.ts` | Bounded GPU reductions and async observations; E08 |
| `legacy/tools/`, `legacy/scenes/` | Ordered, funded engine commands and initialization; E03/E11/E12 |
| `materials/` | Single-source validated projections at E03, expanded data pipeline in P3 |

The [migration work plan](plans/rust-wasm-migration/plan.md) remains the E00-E14
execution map. P2-P9 and [fire integration](plans/fire-combustion/plan.md) use the
same contracts, crate owners and state boundaries. Retain legacy scenes and their
regressions until conversion and promotion gates permit retirement.

## Verification and documentation

Run `mise run ci` from the repository root for lint, both TypeScript checks,
existing tests, production build and documentation validation, plus Rust formatting,
native tests/Clippy, WASM generation/freshness and real browser initialization.
`mise run bench` is available for performance work. Hardware skips are not passes.

Current fixtures are TypeScript helpers under `web/tests/legacy/fixtures/`.
New numerical algorithms use Rust references and the frozen engine fixture conventions.
Keep transient results under ignored `artifacts/` and durable summaries under
`docs/validation/`.

[docs/README.md](README.md) is the developer index. [START_HERE.md](START_HERE.md)
is the detailed engine migration brief. [The legacy sandbox guide](legacy-sandbox.md)
documents current controls and implementation. Root README stays a short quick start.

The 2026-09-27 layout supersedes earlier instructions to keep root `src/` in place.
Historical archives, source manifests, audit files, source paths and line numbers
remain unchanged. The [source review](validation/source-review-2026-09-21.md) and
[fire audit](evidence/fire-review-2026-09-23/FIRE_REVIEW.md) describe earlier source
snapshots, not the reorganized checkout or fresh migration acceptance.
