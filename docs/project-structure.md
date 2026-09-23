# Project structure

**Revision:** 21 September 2026. The 43 files under `src/` were inspected statically in `src(3).zip`; no application code was changed or executed. Root build configuration, tests, benchmarks and CI were not included in that upload. Proposed additions below are implementation work, not existing files. Start at [START_HERE](START_HERE.md).

[ADR-001](architecture/adr-001-rust-wasm-wgpu.md) selects the existing TypeScript frontend plus a Rust/WASM engine, Rust CPU references, and wgpu-managed WGSL compute/rendering. Keep the application in `src/`. Moving it to `apps/web/`, introducing a Rust UI or adopting another game engine is not a prerequisite.

## Inspected source layout and present ownership

```text
src/
  main.ts                   Browser startup, Canvas 2D acquisition, RAF loop
  app/                      DOM controls, material picker, views, viewport, pointer mapping
  materials/                Authored material catalogue, IDs, queries and 118-name reference
  physics/                  Current physical solvers and reaction rules
  rendering/                Canvas image construction, maps and visual waves
  scenes/                   Starter scene construction
  simulation/               World, physics orchestration, sandbox API, clock and diagnostics
  styles/                   Application CSS
  tools/                    Brush tools and their source accounting
```

`simulation/world.ts` owns the current mutable arrays. `simulation/physics.ts` orders the current passes; `simulation/sandbox.ts` composes the synchronous application API. This remains true for the legacy backend only. Do not describe it as the permanent owner of a new GPU scene.

`app/controls.ts` already consumes a focused subset of the Sandbox API. Keep that interaction boundary rather than rebuilding the UI. The material picker, view controls, DOM lookup and pointer conversion retain their presentation responsibilities. Browser and DOM access do not move into the portable Rust core.

`materials/definitions.ts` is the existing authored material catalogue, accessed through `materials/index.ts` and the focused re-export/query interfaces. Eight elemental identities have dedicated legacy definitions; `element-reference.ts` already lists all 118 names. Do not confuse the reference list with 118 implemented materials, or duplicate the catalogue by hand in Rust and WGSL.

`rendering/renderer.ts` reads CPU state to construct an image. `simulation/diagnostics.ts` scans the CPU world for totals. GPU scenes require different implementations behind the same user-facing capabilities: direct rendering from owned GPU fields, small diagnostic reductions and asynchronous probes. The source acquires a Canvas 2D context before sandbox selection, which must change at the host integration seam.

The [source review](validation/source-review-2026-09-21.md) provides exact locations and limitations. The [source manifest](data/source-review-manifest.json) records every inspected file's hash, size and line count.

## Proposed Rust workspace and browser seam

Create the minimal workspace during P1/M1/E01. Names below are proposed, not claims about files in the source upload. Keep the existing frontend build; establish its actual scripts during E00 before editing it.

```text
Cargo.toml                      Workspace definition
Cargo.lock                      Reproducible dependency resolution
rust-toolchain.toml             Tested, pinned Rust toolchain
crates/
  sim/                          Package particle-sim
    src/                        Portable IDs, units, schemas, stage/API contracts
  sim-cpu/                      Package particle-sim-cpu
    src/                        Rust f64 reference solvers and bounded CPU algorithms
    tests/                      Analytical, conservation and failure fixtures
  sim-gpu/                      Package particle-sim-gpu
    src/                        wgpu state, pipelines, scheduling and rendering
    tests/                      ABI and native GPU tests when hardware is available
  wasm/                         Package particle-wasm
    src/                        Browser bindings, async initialization and backend construction
shaders/
  fluid/                        WGSL fluid operators and conservative transport
  render/                       WGSL rendering from the same device/state
  ...                           Add thermal, granular and later domains when required
src/
  main.ts                       Existing browser shell with backend/canvas selection
  simulation/
    sandbox.ts                  Retained legacy integration behind a facade
    engine-client.ts            Proposed thin TypeScript command/observation adapter
  app/, materials/, styles/     Retained frontend responsibilities
```

Keep shader sources version controlled. Put generated WASM, JavaScript bindings and TypeScript declarations in a clearly named build-output location compatible with the existing bundler. Decide that path at E01 after inspecting the full repository. Generate them reproducibly; do not hand-edit generated bindings. Keep a tested wasm-bindgen library/CLI pairing and record the resolved dependency versions in evidence.

### Dependency direction

```text
TypeScript browser shell
  -> generated particle-wasm bindings
       -> particle-sim          portable contracts
       -> particle-sim-cpu      selected CPU/reference algorithms
       -> particle-sim-gpu      wgpu resources and GPU execution

particle-sim-cpu -> particle-sim
particle-sim-gpu -> particle-sim
```

The portable `particle-sim` crate must not depend on its concrete backends. Backend construction belongs to the host/bridge. Shared algorithms/contracts may be factored where genuinely shared, but a circular crate graph or a generic framework without a caller is not a milestone deliverable.

A Rust handle owning wgpu buffers is not a CPU copy of those buffers. The live GPU backend owns committed physical fields; the CPU reference has an independent fixture world. Comparing them does not authorize keeping the full reference current in every browser frame.

## Current-to-target migration map

| Existing surface | Required work | Destination and gate |
| --- | --- | --- |
| `main.ts`, `simulation-clock.ts` | Separate requested ticks from accepted physical progress; select rendering context before acquisition; report completion separately from encoding | Existing TypeScript host plus engine status contract, E03/E08/E12 |
| `sandbox.ts`, `app/controls.ts` | Preserve commands and control behavior; adapt synchronous readings into cached status and async probes; explicit capability errors | Thin TypeScript facade and `particle-wasm`, E03/E08 |
| `world.ts`, `physical-scale.ts` | Define independent Rust state, stable IDs, SI units, phase amounts and snapshot conversion/rejection | `particle-sim` contracts and selected backend state, E02 |
| `physics.ts`, `gas-dynamics.ts`, `fluid-solver.ts`, `hydrostatics.ts` | Replace competing transport/pressure passes with the shared conservative stage graph | `particle-sim-cpu` references, `particle-sim-gpu` and `shaders/fluid/`, E05-E07 |
| `thermal.ts`, `boiling.ts` | Implement supported closure, enthalpy/internal-energy conversion and consistent phase/volume coupling | Rust thermal references then WGSL thermal stages, E09 |
| `motion.ts`, `solid-mechanics.ts` | Replace repeated outlet search with batched proposals, owned writes, reservation/commit and conservative coupling | Rust granular/solid references then WGSL, E10 |
| `electricity.ts` | Correct analytical graph/work/heat behavior; communicate bounded topology/sample events rather than full arrays | Rust CPU circuit reference E04, runtime coupling E11 |
| Reaction modules | Preserve supported source contracts; implement amount-based transactions in the new owner | Shared definitions, Rust references and WGSL chemistry where justified, E11 and P3-P5 |
| `renderer.ts`, field maps, visual waves | Retain presentation behavior without image readback; use one wgpu device for compute and rendering | GPU render pipelines E08; field/overlay completion E12 |
| `diagnostics.ts` | Replace normal-frame O(N) CPU reads for GPU scenes with bounded reductions and dated observations | GPU reductions and async Rust/WASM reports, E08 |
| `brush.ts`, `starter-scene.ts` | Convert edits/initialization to ordered command batches; preserve finite source accounting, especially retries/paused edits | Existing UI intent plus backend transactions, E03/E11/E12 |
| `materials/` | Maintain one authored source and validated generated projections during migration | Minimal projection E03; deliberate data pipeline expansion P3 |

Retire each legacy transport path from a new-backend scene when its replacement owns that subsystem. Do not delete legacy behavior needed for still-unsupported scenes before E14 coverage gates. No source-wide mechanical translation is authorized by this table.

## All-phase ownership

| Phase | Proposed implementation ownership | Validation ownership |
| --- | --- | --- |
| P1 / M0-M1 | Existing host, minimal Rust workspace/bindings, portable contracts and generated projections | Existing regressions after discovery, native contract tests, browser WASM smoke |
| P1 / M2-M3 | Rust fluid reference; wgpu resources; WGSL operators, transport and direct rendering | Native reference fixtures, f32 layout/parity, real browser device/transfer tests |
| P1 / M4-M7 | Rust thermal/phase/solid references, matching WGSL, bounded circuits, host controls and persistence | Closure, contention, sources, recovery and target-device promotion |
| P2 | Rust compressible/solid references, `shaders/waves/` and `shaders/solids/`, TypeScript controls | Equal-time wave/refinement, traction, failure, breakage toggle and fragment fixtures |
| P3-P5 | `particle-sim` matter/reaction contracts, offline data preparation, Rust references, WGSL chemistry/material mechanisms, TypeScript palette/probes | Data provenance/schema, element/charge accounting, concentration, ABI/capacity and mechanism fixtures |
| P6-P8 | Portable nuclide/network contracts, Rust nuclear/radiation references, supported WGSL transport/deposition/reaction stages, TypeScript labels | Decay/population, daughter/energy closure, uncertainty, mode and recovery tests |
| P9 | Existing frontend plus the same engine/backends | Combined native/browser scenarios, long sessions, packaging and named-device release gates |

The [migration plan](plans/rust-wasm-migration/plan.md) is the canonical E00-E14 execution map. The [matter contract](architecture/matter-model.md) governs component storage throughout. Chemistry and nuclear updates attach to the same stage boundaries and state owner, not separate whole-world loops. Create future directories only when their milestone needs them.

## Engineering conventions

Keep descriptive kebab-case TypeScript module names and the existing frontend conventions. Use normal Rust snake_case modules and focused crate APIs; no cross-language naming rule requires renaming the existing application. Keep WGSL names/stage bindings consistent with the tested ABI manifest.

Public operations validate coordinates, sizes, IDs, finite values and capabilities before mutation. Preserve the existing distinction between invalid cell readings and valid outside brush strokes; record intentional API changes. Keep source accounting explicit and never repair physical state invisibly after a failed step.

Mutable hot-path arrays stay private to their backend. Rendering and probes are read-only. Tables, property derivatives and cached operators have versions and invalidation dependencies, not independent authority. A CPU reference and a GPU implementation share equations, units, stage contracts and fixtures, not mutable buffers.

Keep refactors scoped, preserve unrelated working changes and add tests for demonstrated defects. Mark proposed/implemented/validated capabilities separately. See the [engine-boundary contract](architecture/engine-boundary.md) for async lifetimes, commands, epochs, canvas ownership and save/load behavior.

## Checks, fixtures and evidence locations

Previous documentation reports `tests/`, `benchmarks/`, root Vite/TypeScript/Biome configuration and Bun checks. They were not supplied in `src(3).zip`; verify their existence and exact commands in the real checkout before claiming them. Do not delete or move them to fit the proposed tree.

Rust tests belong in the appropriate crate with stable fixture IDs. Keep backend-neutral scene/expected-invariant data in one versioned fixture location chosen after repository inspection. Browser integration tests must exercise the generated WASM and actual wgpu/WebGPU path, not only mock bindings. Native GPU success does not waive browser testing.

The E01 build task must document clean-checkout setup and integrate actual Rust, WASM, TypeScript, shader and browser commands with CI. Separate hardware-dependent checks from hardware-free reference/schema checks; a skipped GPU job is not a pass. No unverified game build command is advertised as executable by this docs-only revision.

Use a run-specific `artifacts/validation/` directory for transient output. Commit durable summaries and manifests, not noisy per-cell logs. Original audit files and prior documentation archives remain unchanged under `docs/`; old source paths and line numbers stay historical. They must not be rewritten to imply that a benchmark or test ran on this upload.

Run documentation-only checks with `python docs/scripts/validate-docs.py`. The optional `--source-archive` flag validates the inspected source fingerprint without building or executing that source.
