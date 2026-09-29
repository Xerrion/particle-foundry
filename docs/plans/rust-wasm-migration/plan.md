# Rust/WASM + wgpu migration work packages

**Revision:** 29 September 2026, current-sandbox migration scope. **Status:** E00-E03 and E05-E08 validated locally; E04 circuits and E09-E14 remain planned. [Baseline evidence](../../validation/p1-m0.md), [bootstrap evidence](../../validation/p1-m1.md), [E02 evidence](../../validation/p1-m1-e02.md), [E03 evidence](../../validation/p1-m1-e03.md), [E07 evidence](../../validation/p1-m3-e07.md) and [E08 evidence](../../validation/p1-m3-e08.md). P1 is the sole active release scope. P2-P9 are deferred and require a separate user decision to activate. **Selected decision:** [ADR-001](../../architecture/adr-001-rust-wasm-wgpu.md). **Entrypoint:** [START_HERE](../../START_HERE.md).

## Goal

Deliver the current sandbox feature set on the Rust/WASM + wgpu engine, with verified physics, browser interactions, recovery and measured performance, before adding new capabilities.

The [current-sandbox scope](current-sandbox-scope.md) fixes the required materials, interactions, tools and controls. It owns the product coverage for P1. The [fluid/GPU plan](../fluid-gpu-redesign/plan.md) owns equations and numerical exit gates. This document owns work sequencing and runtime integration. The [engine boundary](../../architecture/engine-boundary.md) owns commands, state authority, async behavior and persistence. A subset demonstration cannot satisfy the full replacement gate.

## Scope decisions

| Decision | Required interpretation | Basis |
| --- | --- | --- |
| Current functionality before expansion | M4-M6 includes the existing eight elemental models and their current interactions. Their first migration cannot wait for P4/C3. | [Current-sandbox scope](current-sandbox-scope.md) and the user's 27 September 2026 scope decision |
| Preserve user capabilities with corrected physics | Use legacy behavior/tests as regression context. Record intentional numerical changes and fix known conservation, fire and circuit defects; do not reproduce bugs to obtain pixel parity. | [Baseline evidence](../../validation/p1-m0.md), [fire acceptance](../../validation/fire-combustion.md) |
| Prove the GPU path early | Keep M3 small; validate a complete browser compute/render/input path before extending current feature coverage. | [M3 numerical plan](../fluid-gpu-redesign/plan.md), [backend validation](../../validation/backend-migration.md) |
| Defer unrelated work | P2-P9, FIRE-W08, expanded catalogue/reactions, optional worker deployment, native UI and additional product features are outside this release. Completing P1 does not start them. | [Phase tracker](../../data/development-phases.json) |

## Delivery checkpoints

| Checkpoint | Work | Completion criterion |
| --- | --- | --- |
| 1. Working GPU slice | M2-M3 / E05-E08 | One liquid, carrier gas, fixed walls and passive markers; validated Rust reference, browser GPU simulation/direct rendering, basic interaction and measured transfers. Experimental only. |
| 2. Current sandbox coverage | M4-M6 / E09-E12, with E04 before E11 | Every current-sandbox scope row has an implementation and applicable reference/integration results, including current elemental forms, sources, fire, circuits and controls. |
| 3. Verified default switch | M7 / E13-E14 | Every scope row passes on the declared browser/device targets, including failure/recovery and sustained performance; the full GPU-M7 and FIRE-M7 evidence is recorded. |

E00-E03 remain validated foundation work. E05-E06 have local CPU validation evidence, and E07-E08 have local native and browser GPU evidence. Checkpoint 1 is validated locally; remote CI has not been established. E04 and checkpoint 2 remain planned. Execute only assigned work; this plan does not authorize automatic execution of deferred work.

Freeze scene definitions, expected observables, accepted numerical changes and tolerances for each scope row before implementing it. Missing behavior, insufficient property domains, unrun hardware checks or a failing performance target keep the relevant row and P1 incomplete. An experimental subset may remain available, but cannot be used to narrow the default-switch checklist. Any removal of an existing capability from that checklist requires an explicit user scope decision.

Property tables, generated metadata and reaction products needed to preserve current behavior belong in P1. The generalized P3 data pipeline, new selectable materials, new chemical mechanisms, pressure waves, fracture and nuclear behavior remain deferred. GPU recovery checkpoints and bounded CPU circuit work remain required engine mechanisms; a new save-management UI or moving every irregular algorithm to the GPU is not a P1 requirement.

When all active release work is validated, represent the stopped queue explicitly:
set phase `nextPhase` and `nextMilestone` to `null`, engine `nextWorkItem` to `null`,
and fire `nextWorkItem` to `null` after FIRE-W00-W07 are validated. Deferred phases
and FIRE-W08 retain `deferred`. An empty queue is invalid while applicable active
work remains unfinished. Do not retain a completed item as "next" or select a
deferred item to satisfy the tracker. This is the terminal state after evidence,
not a status change made by the present planning revision.

## Non-negotiable implementation boundaries

Keep the TypeScript application in `web/`, its existing backend in `web/src/legacy/`, and new Rust/WGSL implementation in `engine/`. The browser imports simulation operations through `web/src/engine-client/`. Do not adopt Bevy or introduce a Rust UI as part of P1. Add only the workspace, bridge and modules needed by the current milestone.

New numerical reference code is Rust f64. Production GPU numerical work is WGSL/f32, managed by Rust wgpu. The existing TypeScript simulation remains the legacy backend and regression baseline, not an intermediate implementation to port line by line. Keep one state and one transport owner per scene.

Create the first backend seam and build-system smoke test early. Do not wait until the numerical solver is complete to discover that initialization, bindings or canvas ownership cannot work in the browser. Shader-layout/device smoke tests may precede M2 completion; promoting the actual M3 fluid solver still requires M2 reference gates.

## P1 execution graph

```text
E00 baseline
  -> E01 Rust/WASM bootstrap
  -> E02 contracts
       -> E03 facade/data -> E05 operators -> E06 transport
       -> E04 circuit reference (independent branch)
E06 -> E07 WGSL solver -> E08 GPU scene/render/probes
    -> E09 thermodynamics -> E10 solids/granular
E04 + E10 -> E11 sources/circuits -> E12 browser integration
          -> E13 recovery -> E14 promotion
```

E04 is part of the complete M1 work, but the M2 dependency is the M1 state/facade contract, not the circuit branch. It may run alongside the fluid work and must pass before E11. Do not implement an entire corrected TypeScript circuit path first merely to duplicate it in Rust; only a separately justified legacy hotfix changes that order.

## Work packages and acceptance

The machine-readable tracker is [engine-migration-work.json](../../data/engine-migration-work.json). Each item remains `planned` until implementation/evidence changes its status. The source IDs link to [the static review](../../validation/source-review-2026-09-21.md).

### E00: Capture baseline and legacy regression contracts

**P1 / M0. Dependencies:** none. **Source:** SRC-01, SRC-04, SRC-05, SRC-09.

Inspect the full repository, preserve tracked/untracked changes, identify real scripts/toolchains, reproduce controls/audit in a fresh directory, and establish named browser/device fixtures. Instrument actual pressure calls and outlet visits. Include FIRE-W00: preserve the 12 imported F-check observations, verify their source hash and add distinct corrected FIRE-A specifications without rewriting legacy fire first.

**Exit:** Recorded source/config hashes, original checks, fresh failures, equal-time scene definitions and explicit unrun checks. Quoted timings stay historical. Fire characterization and corrected acceptance are separately named; full-tick bugs and positive controls are retained, and the water edge case remains reaction-only.

### E01: Bootstrap Rust workspace and browser WASM

**P1 / M1. Dependencies:** E00. **Source:** SRC-01, SRC-03.

Create the minimal proposed crates in engine/. Pin Rust/wgpu/wasm-bindgen after native and browser smoke tests. Generate bindings into web/generated/wasm/ and integrate them with the web/ frontend build.

**Exit:** Native core tests and a real browser WASM initialize/dispose cycle pass; generated declarations compile in the existing TypeScript application. No UI rewrite.

### E02: Define state, identity, ABI and snapshot contracts

**P1 / M1. Dependencies:** E01. **Source:** SRC-07, SRC-08.

Define backend/model axes, Rust state schemas, stable identities, SI units, active-component mapping, command/status types, snapshot versions and shader packing. Include FIRE-W01: one fuel/O2/product/physical-soot inventory, shared boundary fluxes, source transactions, chemical reference and derived visual contracts, without activating chemistry in M3.

**Exit:** Legacy-ID round trips, malformed-input rejection, optional-extension no-allocation, layout/offset tests and documented H-to-U conversion/rejection rules. Fire inventory/schema round trips, product-capacity checks and legacy FIRE-as-tool/visual mapping pass without an independent spendable oxygen counter.

### E03: Introduce facade, command lifecycle and one catalogue projection

**P1 / M1. Dependencies:** E02. **Source:** SRC-02, SRC-03, SRC-09, SRC-10.

Adapt the existing sandbox seam with a separate legacy adapter; add queued commands, async observations, epoch cancellation, feature preflight and minimal generated catalogue projections. Prepare canvas ownership selection.

**Exit:** Legacy UI remains usable; no duplicate handwritten catalogue or ticking world; mock/CPU contract tests cover backpressure, paused paint, reset and stale callbacks.

### E04: Build corrected Rust CPU circuit reference

**P1 / M1. Dependencies:** E02. **Source:** SRC-11.

Translate the analytical N11-N13 and C17/C18 contracts into Rust tests and implement the corrected bounded graph solver, work and heat allocation. Integrate into GPU scenes at E11.

**Exit:** Residual, branch heat, terminal work, source exhaustion and topology-change tests pass. This independent branch does not block E05/E06.

### E05: Implement Rust f64 pressure and boundary reference

**P1 / M2. Dependencies:** E03. **Source:** SRC-04, SRC-06.

Build matching MAC D/G operators, density/aperture coefficients, gravity balance, sealed-component compatibility, scaled residuals and cache invalidation in sim-cpu.

**Exit:** Analytic/manufactured operator fixtures and hydrostatic rest pass; forced-rebuild comparisons validate cache correctness.

### E06: Implement conservative Rust transport and substeps

**P1 / M2. Dependencies:** E05. **Source:** SRC-05, SRC-06, SRC-08.

Add bounded VOF, shared mass/tracer/compatible momentum fluxes, viscosity and rollback. Keep phase chemistry off and thermal transport passive.

**Exit:** M2 conservation, marker displacement, shear, symmetry, dam-break/refinement and failure tests pass before the equivalent solver is ported.

### E07: Implement WGSL stage graph under Rust wgpu

**P1 / M3. Dependencies:** E06. **Source:** SRC-06, SRC-07.

Implement matching f32 operators/transport with owned writes, reductions, convergence flags, candidate/committed generations and bounded submission. Validate Rust/WGSL ABI on hardware.

**Exit:** Small-fixture CPU/GPU comparisons at equal time, failure/no-partial-commit tests and adapter-limit checks pass. Native testing does not waive browser testing.

### E08: Deliver direct GPU rendering and asynchronous observations

**P1 / M3. Dependencies:** E07. **Source:** SRC-02, SRC-03, SRC-09, SRC-12.

Render from the same wgpu device/state. Add tick/epoch-stamped probes/reductions, explicit startup capability handling, viewport/overlay behavior, async checkpoint prototype and completed-work telemetry. Exercise painting, pause and reset on the limited M3 scene in the actual browser, plus controlled stepping through the test harness. A new single-step UI control is deferred.

**Exit:** 480 x 270 experimental scene works with the stated basic controls; no full-state normal-frame readback, no reused 2D context, no stale-as-current observations, measured transfer bytes and completed-time throughput. This subset does not satisfy current-sandbox coverage or authorize a default switch.

### E09: Add full supported thermodynamics and phase closure

**P1 / M4. Dependencies:** E08. **Source:** SRC-08.

Implement Rust reference then GPU parity for the current-sandbox thermal and phase scope: bounded property tables, partial phases, available-volume closure, chamber pressure and finite venting. Include existing elemental phase forms and the domains required by current tools and scenes. Execute FIRE-W02 for passive O2/product transport, finite ventilation and explicit reactant/product/water domain coverage.

**Exit:** M4 closed/open energy/mass/volume fixtures and the applicable current-sandbox thermal rows pass; table provenance and conversion limits are recorded. FIRE-PRECONDITIONS (FIRE-A03 and FIRE-A12) pass before coupled fire scenes. Invalid domains reject explicitly during development and remain blockers for required coverage; no connected-interior O2 refill, silent hot-state clamp or removal of a required row to claim completion.

### E10: Add granular and solid-fluid coupling

**P1 / M5. Dependencies:** E09. **Source:** SRC-05, SRC-06.

Implement current sand, anchored/dynamic solid and multi-liquid behavior with the M5 batched proposal/reservation/commit design. Preserve mass, composition, swept volume and work across all current material families. New fracture mechanics and additional materials remain deferred.

**Exit:** Current-sandbox motion rows and contention, sealed/no-outlet, sinking, impact, narrow-channel and oil/water fixtures pass. Publish visit counts and transfers instead of asserting a complexity gain. Current liquid and solid families cannot remain legacy-only at the final default switch.

### E11: Integrate chemistry and bounded CPU circuits

**P1 / M6. Dependencies:** E04, E10. **Source:** SRC-10, SRC-11.

Migrate the current-sandbox interactions, including all eight existing elemental models, their reaction products, acid/base presets, funded plant growth, Fire, gunpowder and the bounded Blast interaction. Use only the property/composition/reaction support needed for those capabilities. Couple the E04 Rust CPU circuit reference through bounded topology/sample events and funded heat transactions. Execute FIRE-W03-FIRE-W05 in Rust then WGSL with shared-reactant reservations and exactly-once commits. New palette entries, generalized chemistry, resolved shocks and structural fracture remain deferred.

**Exit:** Every applicable current-sandbox interaction row has reference and GPU integration evidence, including excess reactants, finite sources, phase/domain limits and source accounting. Circuit conductor changes, graph overflow and dense-cost fixtures pass without a whole-world mirror. FIRE-A01-FIRE-A13 pass for the required existing combustion coverage; no cold-label ignition, unledgered O2, missing products, contact-only water kill switch or per-substep replay of per-tick rates.

### E12: Complete current browser controls and presentation

**P1 / M6. Dependencies:** E11. **Source:** SRC-01, SRC-03, SRC-09.

Connect all current-sandbox palette/reference, held-tool, map/probe, zoom/pan, paused-edit, speed, reset/clear and starter-scene behavior to the new engine. Retain the TypeScript UI and main-thread asynchronous wgpu deployment. Execute FIRE-W06 with funded Fire commands, stamped composition/heat-release probes and state-derived particle/shader flames. Worker/OffscreenCanvas deployment and new product controls remain deferred.

**Exit:** All current-sandbox interaction and presentation rows pass on the actual browser path with one live state owner and bounded queues. FIRE-M6 integrated scenes and render-on/off inventory equivalence pass. Unsupported GPU startup is explicit; current legacy access remains available during migration, but missing required controls or scenes block full replacement.

### E13: Validate snapshots, device loss and reversible rollout

**P1 / M7. Dependencies:** E12. **Source:** SRC-03, SRC-07.

Complete engine checkpoint/save-load contracts, component/version preflight, rollback/recovery notices, teardown, reset races and checkpoint replay policy for the frozen current-sandbox scope. Include fire network/property versions, products, physical smoke/residue and source/approximation settings in FIRE-W07. New user-facing save-management workflows remain deferred.

**Exit:** Failure injection never publishes partial state; late callbacks cannot affect a new epoch; checkpoint round trips cover all required current components and settings. Rollback limits are visible and no incompatible legacy hot swap occurs. FIRE-A14 rejects unsupported data or rolls back explicitly without duplicate burning or lost products.

### E14: Validate current-sandbox coverage and default GPU rollout

**P1 / M7. Dependencies:** E13. **Source:** SRC-09, SRC-12.

Run the complete current-sandbox scope matrix through native references, actual browser GPU scenes, regressions, memory/readback, sustained performance and input-latency checks on declared target devices. Complete FIRE-W07/FIRE-M7 for all required current combustion scenarios. Record per-row evidence and all intentional numerical corrections before changing the default backend.

**Exit:** GPU-M7 and FIRE-M7 pass and every required current-sandbox scope row has evidence on its declared targets. No current feature remains legacy-only, silently removed or relabelled optional to pass this gate. Report actual completed simulation time, latency and transfer costs; unrun or failed required gates keep P1 incomplete. Retain reversible legacy access until rollout is proven. P2-P9 remain deferred after completion.

## Proposed repository additions

The existing frontend now lives in `web/`. Proposed package names remain `particle-sim`, `particle-sim-cpu`, `particle-sim-gpu` and `particle-wasm`; create them under `engine/` at E01. `sim` owns portable contracts and generic orchestration, not a duplicate GPU-world mirror. Concrete backend construction belongs to the host/bridge. The current UI uses the synchronous legacy adapter; E03 adds a separate experimental queued session contract tested with mock owners. No physical Rust scene consumes it yet.

```text
engine/
  Cargo.toml                   Workspace, added at E01
  Cargo.lock                   Pinned application dependencies
  rust-toolchain.toml          Pinned validated Rust toolchain
  crates/
    sim/                       Units, IDs, schemas, commands, stages, snapshots
    sim-cpu/                   Rust references and bounded CPU algorithms
    sim-gpu/                   wgpu state, execution, reductions, rendering
      shaders/                 Fluid/render first; other families at their milestones
    wasm/                      Browser bindings and backend selection
web/
  src/app/                     Existing browser controls and lifecycle
  src/engine-client/           Existing legacy entrypoint; async facade at E03
  src/legacy/                  Existing TypeScript backend
  generated/wasm/              Generated WASM/bindings at E01, ignored
  tests/legacy/fixtures/       Existing TypeScript fixtures
```

Keep backend-neutral fixtures in one versioned location when both implementations consume them. Current TypeScript fixtures are executable helpers, not a language-neutral data format.

Do not create empty future shader trees or placeholder nuclear engines solely to match this diagram. Add a module when its milestone has a concrete caller. Ensure shader assets are embedded/packaged reproducibly by `sim-gpu`; a native run must not depend on an accidental working directory.

## Build and continuous integration work

At E00 inspect the current checkout's `mise.toml`, `web/package.json` and `web/bun.lock`, and run its existing checks. Include uncommitted changes in the baseline; no source archive is needed. At E01 pin a compatible Rust/wgpu/bindings toolchain, install the WASM target and establish a reproducible command that produces the `.wasm`, JavaScript glue and declarations consumed by the existing build. Pin the `wasm-bindgen` CLI to the library's compatible version. Do not select an untested version simply because it appears in current online docs. [Build reference](../../sources.md#wgpu-web).

Add separate checks for Rust formatting/lint, native core/reference tests, a WASM target build, generated-output freshness, WGSL parsing/layout and actual browser initialization. Keep the original frontend lint/typecheck/test/build checks. A native-only test job cannot validate the browser backend. Optional hardware tests must explicitly report skipped/unavailable runs rather than pass them.

Run these reference commands from `engine/` after E01 creates the proposed packages; verify actual package/features first:

```sh
rustup target add wasm32-unknown-unknown
cargo fmt --all -- --check
cargo test --locked -p particle-sim -p particle-sim-cpu
cargo build --locked -p particle-wasm --target wasm32-unknown-unknown --release
```

E01 must also add and document the matching glue-generation/frontend-build commands, based on the actual package configuration. Configure web/native GPU features explicitly and prevent native-only blocking/thread/window dependencies leaking into the WASM build. Keep a native headless harness for references and GPU comparisons; a native GUI is out of scope.

Verify deployment MIME, WASM loading paths, caching/version compatibility, secure-context requirements and initialization failures in a real browser. Shared-memory/thread deployments need their own headers/support tests if introduced; baseline WASM + WebGPU does not justify assuming those optional features.

## Legacy-to-new migration policy

Migrate complete scenes/subsystems with one state owner. An initial supported scene converts once on load, then the new engine owns it. Existing complex scenes stay entirely in the legacy backend during development until their gates pass. The current-sandbox checklist remains mandatory for P1 completion; keeping a required feature in legacy mode is a migration state, not a completed replacement.

Preserve old IDs, tool validation, coordinate rules, seed behavior and source accounting through explicit adapters. Record intentional numerical/model changes rather than preserve physically incorrect behavior just to match pixels. Keep input/visual regression tests alongside new reference tests. Retire old branches only after the promoted coverage no longer depends on them.

Fresh synchronous cell reads, Canvas-only renderer signatures and submitted-time timing cannot survive unchanged across the GPU boundary. These are explicit API migrations, not compatibility promised by an identical method name. Saves/loads are new backend contracts: the supplied Sandbox interface does not expose them, so inspect any persistence code in the full repository before inventing an existing save format.

## Deferred phase ownership

| Phase | Implementation ownership | Existing detailed milestones |
| --- | --- | --- |
| P1 | Rust reference -> Rust/wgpu + WGSL backend; existing TypeScript host | M0-M7 |
| P2 | sim-cpu compressible/solid references; sim-gpu and shaders/waves, shaders/solids; TypeScript breakage controls | W0-W4 |
| P3 | sim matter/schema and offline data compiler; generated TS/Rust/WGSL projections; sim-cpu reference reactions | C0-C2 |
| P4 | shared data/forms and sim-cpu/sim-gpu chemistry; TypeScript palette and probes | C3-C6 |
| P5 | same engine modules with measured active-set capacity; advanced mechanism kernels only when justified | C7-C9 |
| P6 | sim nuclide/network contracts; Rust decay/radiation reference and WGSL transport/deposition | N0-N3 |
| P7 | Rust selected nuclear reaction references and shared tables; GPU population/source stages in supported domains | N4-N6 |
| P8 | shared catalogue/network/override persistence; TypeScript labels; no special second physics engine | N7-N8 |
| P9 | Rust/native plus browser WASM/GPU matrix, TypeScript product regressions and package/recovery evidence | Q0-Q2 |

P2-P9 are deferred. The table preserves conditional ownership if a phase is separately activated; it is not an execution queue. Completing P1 does not activate P2 or any later phase. New physics sources always apply through the owning backend's stage/transaction rules. P2 remains a distinct compressible model. P3-P5 retain the exact 40/83-element milestones. P6-P8 retain nuclide/radiation/reaction validation and 103/118-identity coverage; Rust/WASM does not change those scientific limits. P9 validates combined features, including browser execution and loss recovery.

Use [the roadmap](../../roadmap.md) for deferred dependencies. If later work is explicitly activated, apply [backend-migration acceptance](../../validation/backend-migration.md) alongside its numerical gates and reuse the existing state owner. Do not build future modules during P1.

## Fire integration inside the selected migration

[FIRE-W00-W08](../fire-combustion/plan.md#8-work-packages) specialize the existing E-work; they do not add a prerequisite whole-engine rewrite or a second simulation. E00 captures the two full-pipeline bug regressions and useful controls. E02 reserves small inventory/source/visual contracts. E09 validates finite ventilation and hot property domains. E11 builds corrected Rust combustion then equivalent WGSL, E12 connects state-derived visuals, and E13-E14 require persistence and FIRE-M7 before promotion.

Do not copy oxygen flood fills, cold-FIRE ignition, missing product transformations, contact-water extinction or timed smoke erasure into Rust. Preserve ledgered ignition and conservation controls. M3 remains nonreactive. FIRE-W08 and its P4 extensions are deferred; they require separate activation and are not P1 dependencies.

The [fire work manifest](../../data/fire-combustion-work.json) and [acceptance specification](../../validation/fire-combustion.md) separate historical F-characterization records from planned corrected FIRE-A evidence. The E00-E14 dependency graph remains unchanged.
