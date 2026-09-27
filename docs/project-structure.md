# Project structure

**Current layout: 27 September 2026.** TypeScript lives in `web/`. `engine/` owns
future Rust/WGSL implementation and currently contains only [scoped engine guidance](../engine/AGENTS.override.md). The
running application still uses the existing TypeScript backend; this reorganization
does not implement E01-E14. Start with [the knowledge map](README.md).

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
    benchmarks/                     Existing simulation workloads
  engine/AGENTS.override.md         Scoped guidance for the planned Rust backend
  docs/                             Current model, plans, validation and history
  artifacts/                        Ignored local evidence and snapshots
```

Keep frontend configuration and tests with the application. Root `mise.toml`
selects `web/` as the working directory for frontend tasks, preserving commands
such as `mise run dev` and `mise run ci`. Documentation tools and their `.venv/`
remain at repository level. There is no second package manager or task framework.

## TypeScript ownership

The browser imports simulation operations, observations and view metadata through
`web/src/engine-client/index.ts`. This is currently a small re-export of the existing
synchronous Sandbox API. It creates no extra world or wrapper layer. Biome rejects
UI imports of private legacy modules and generated bindings.

`web/src/legacy/simulation/world.ts` owns current mutable state;
`physics.ts` orders its passes, and `sandbox.ts` composes state, tools and rendering.
The brush and starter scene remain in the legacy backend because they mutate its
world. `simulation-clock.ts` belongs to `app/` because it converts elapsed browser
time into requested ticks without owning physical state.

`web/src/materials/definitions.ts` remains the only authored legacy catalogue.
Its public imports, validators and compatibility exports stay together. The 118-name
element reference does not imply 118 implemented materials. Generate future
Rust/WGSL projections from the authored catalogue; do not duplicate definitions.

At E03 the engine client will adapt backend selection, queued commands and async
observations. The current synchronous readings and Canvas 2D render parameter must
not constrain the new GPU contract. Select canvas ownership before acquiring a
context. Keep UI behavior while following [the engine boundary](architecture/engine-boundary.md).

## Planned Rust workspace

Create this workspace at E01, under `engine/`:

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
      shaders/                      Version-controlled WGSL owned by these pipelines
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

Generate JS glue, TypeScript declarations and WASM into ignored
`web/generated/wasm/`. Only the engine client imports bindings. Ignore
`engine/target/`; track Cargo and Bun lockfiles. Pin and verify compatible
wasm-bindgen library/CLI versions when adding the real build.

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
existing tests, production build and documentation validation. `mise run bench`
is available for performance work. At E01 add native Rust tests, WASM generation,
ABI checks and real browser initialization; hardware skips are not passes.

Current fixtures are TypeScript helpers under `web/tests/legacy/fixtures/`.
Introduce shared versioned fixture data only when both backends consume it.
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
