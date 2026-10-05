# Project structure

**Current layout: 5 October 2026.** TypeScript lives in `web/`. `engine/` contains
the E01 Rust/WASM bootstrap, E02 portable contracts and the experimental E07/E08
GPU scene, with
[scoped engine guidance](../engine/AGENTS.override.md). The running application
still uses the existing TypeScript backend. E03 supplies an experimental host
contract; local E08 sustained browser validation passes. E09 has an isolated
saturated and single-phase water CPU reference. E04 circuits and E10-E14 remain
planned.
Start with [the knowledge map](README.md).

## Current checkout

```text
particle-foundry/
  AGENTS.md                         Project development rules
  README.md                         Quick start and navigation
  mise.toml                         Root tool versions and task orchestration
  .github/workflows/                GitHub CI uses the same mise tasks
  web/
    index.html                      Live legacy Vite browser entry
    gpu.html                        Opt-in experimental GPU preview entry
    package.json, bun.lock          Frontend dependencies
    vite.config.ts                  Build configuration; output is web/dist/
    tsconfig*.json, biome.json       Type checks, formatting and import boundaries
    src/
      main.ts                       Live browser startup and frame loop
      gpu-main.ts                   Experimental GPU controls and loop
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

The separate `engine-client/wasm.ts` module is consumed by the E01 browser fixture
and the opt-in `gpu.html` page. It keeps the experimental path out of the normal
application bundle. E03 defines the queued session contract; the current M3 GPU
preview uses a narrower dedicated adapter while that general session remains a
mock-owner contract.

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
acquisition. E07 has a small physical GPU qualification scene. The opt-in E08
browser scene owns one WebGPU canvas, resident solver state and a renderer on
the same device. It supports bounded water/air paint, a stamped one-cell probe
and explicit capture prototype. A local sustained browser run passes under
[the engine boundary](architecture/engine-boundary.md).

## Rust workspace

The Rust implementation lives under `engine/`:

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
      shaders/                      WGSL beside the owning pipeline
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
save/load service. The [E07 ABI sentinel](../engine/crates/sim-gpu/src/abi.rs)
and [WGSL shader](../engine/crates/sim-gpu/shaders/abi_sentinel.wgsl) now verify
selected packed records on native and browser GPU adapters. The private
[E07 coupled scene](../engine/crates/sim-gpu/src/scene/coupled.rs) qualifies one
small two-phase tick and rejected-candidate rollback on both adapters. The
[E08 browser owner](../engine/crates/sim-gpu/src/browser_scene.rs) uses that
stage graph for the persistent 480 x 270 experimental scene. Its
[evidence](validation/p1-m3-e08.md) records the passed sustained browser run
and the remaining limits.

[`particle-sim-cpu`](../engine/crates/sim-cpu/src/lib.rs) and
[`particle-sim-gpu`](../engine/crates/sim-gpu/src/lib.rs) own separate bootstrap
sessions. A separate [`water`](../engine/crates/sim-cpu/src/water/mod.rs) module owns
isolated E09 vessel closure. [Thermodynamics](thermodynamics.md) defines its domain.
The fluid session owns [validated f64 pressure fields](../engine/crates/sim-cpu/src/fluid.rs)
for a staggered MAC grid: cell density and correction pressure, face velocity,
face aperture, and declared outer boundaries. The fields are isolated by session;
the E06 coupled step advances closed small fixtures. [Pressure-field tests](../engine/crates/sim-cpu/tests/pressure_fields.rs)
cover layout and invalid inputs. [MAC operators](../engine/crates/sim-cpu/src/operator.rs)
compute aperture-weighted divergence and matching cell-pressure gradients,
including supplied open-face velocity flux and reservoir pressure at the
half-cell distance; [manufactured tests](../engine/crates/sim-cpu/tests/pressure_operator.rs)
cover their signs and boundary behavior.
The [matrix-free CPU pressure assembly](../engine/crates/sim-cpu/src/assembly.rs)
uses cell-derived face density and the same aperture convention for sealed
components. It rejects incompatible component right-hand sides; [assembly tests](../engine/crates/sim-cpu/tests/pressure_assembly.rs)
cover variable density, disconnected cells, and coefficient failures.
The [bounded f64 solver](../engine/crates/sim-cpu/src/solver.rs) applies that
assembly to correct face velocities, checks both scaled pressure residual and
post-correction divergence, and returns no candidate on failure. [Solver tests](../engine/crates/sim-cpu/tests/pressure_solver.rs)
include manufactured pressure, disconnected gauges, and 1,000 ticks of
stratified hydrostatic rest. Pressure assembly and projection currently support
sealed components only; open-reservoir projection remains planned. The CPU
session keys its pressure assembly cache by boundary
geometry, aperture, and density revisions; [cache tests](../engine/crates/sim-cpu/tests/pressure_cache.rs)
compare reused and forced-rebuild solves after input changes. The GPU session
initializes a wgpu device, validates its ABI and core-grid limit preflight,
and exposes an experimental coupled scene. Its full-size browser scene has
passed a local 3,600-tick sustained run on Chrome with a required GPU adapter.
See [E08 evidence](validation/p1-m3-e08.md) for workload and sampling limits.

The CPU session also owns an optional [phase inventory](../engine/crates/sim-cpu/src/transport.rs)
for one liquid, carrier gas, fixed walls, and phase-associated passive markers.
Its derived liquid fraction and signed mass/marker face ledger belong to a
detached, bounded transport candidate. A separate
[shear candidate](../engine/crates/sim-cpu/src/viscosity.rs) applies tangential
viscous stress to MAC faces and reports mechanical energy diagnostics. Their
tests cover sealed conservation, periodic marker displacement, wall rejection,
and a periodic shear oracle. [Compatible momentum](../engine/crates/sim-cpu/src/momentum.rs)
uses the same phase face ledger, and the [coupled CPU step](../engine/crates/sim-cpu/src/coupled.rs)
stages transport, momentum, optional shear, gravity, and pressure before it
accepts physical time and replaces both owners. Rejected stages leave the
session unchanged. A bounded shared-face correction removes pressure-solver
roundoff from the accepted closed-face velocity before the next transport step.
The [E06 evidence](validation/p1-m2-e06.md) records local named-scene and
dam-refinement results. These paths have no live browser caller yet.

[`Grid`](../engine/crates/sim/src/lib.rs) carries cell width as part of its
identity. The default remains 0.01 m; a CPU reference grid can use another
validated spacing for physical refinement. CPU operators, transport, momentum,
viscosity, and coupled substeps derive scale from that grid. Snapshot v1 and the
current GPU grid uniform carry dimensions but not spacing, so they reject
nondefault grids until their formats are extended.
[`particle-wasm`](../engine/crates/wasm/src/lib.rs) exposes initialization and disposal
through [`engine-client/wasm.ts`](../web/src/engine-client/wasm.ts).
The [browser smoke](../web/tests/browser/engine-smoke.ts) exercises lifecycle,
malformed inputs, independent sessions, missing GPU rejection, the Rust-owned
WGSL ABI sentinel and disposal during its asynchronous readback from a production
bundle under `/engine-smoke/`. Require a real browser adapter when using this
smoke as hardware evidence.
[`test-browser.ts`](../web/scripts/test-browser.ts) owns the isolated server and
browser processes. Its [startup check](../web/scripts/browser-server.ts) gives
each HTTP probe 500 ms within a 15-second startup budget.
A stalled request can therefore retry while the server starts.
[Startup regressions](../web/tests/scripts/browser-server.test.ts) cover stalled
requests, permanent failure, non-success HTTP responses and early server exit.

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
| `legacy/rendering/` | Same-device rendering in `sim-gpu` and browser overlays in `app/`; E08/E12 |
| `legacy/simulation/diagnostics.ts` | Bounded `sim-gpu` reductions and async browser observations; E08 |
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
