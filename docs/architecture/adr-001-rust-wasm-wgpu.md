# ADR-001: change the engine, keep the application

**Date:** 21 September 2026. **Decision:** selected for the implementation plan. **Implementation status:** bootstrap and contracts exist; the [engine tracker](../data/engine-migration-work.json) owns work-item status. **Applies to:** P1-P9. Start with [START_HERE](../START_HERE.md).

## Decision

Keep the existing TypeScript browser application. Introduce a Rust simulation engine, compile its browser bridge to WebAssembly, and use Rust wgpu to own WGSL compute and GPU rendering. Keep small irregular algorithms on the Rust CPU side until measurement justifies moving them.

The GPU redesign is the migration vehicle. Do not create a separate prerequisite project to port every legacy TypeScript solver to Rust, and do not build a new TypeScript reference solver only to rewrite it in Rust afterwards. Implement corrected subsystem references directly in Rust f64, then their equivalent WGSL/f32 stages.

| Layer | Selected ownership | What does not follow from this choice |
| --- | --- | --- |
| DOM, CSS, controls, menus, picker, viewport | Existing TypeScript application | No Rust UI, Bevy, egui, or application rewrite |
| Browser lifecycle and user intent | TypeScript host and a narrow engine facade | No direct access to evolving engine arrays |
| State schemas, identities, units, command/snapshot contracts | Rust `sim` crate with generated browser projections | A Rust struct need not hold a CPU mirror of GPU fields |
| CPU reference and bounded irregular systems | Rust `sim-cpu` | A literal translation of the old cellular rules is not the new reference |
| GPU resources, solver scheduling, rendering | Rust `sim-gpu` using wgpu | No second independent TypeScript WebGPU device for the same world |
| Parallel numerical work | WGSL shaders | Compiling ordinary Rust to WASM does not compile it into a shader |
| Browser bindings | `wasm-bindgen` in `engine/crates/wasm/` | No whole-world JS/WASM copy each tick |
| Future native execution | Reuse the Rust crates with native wgpu | Native application UI/distribution is not a P1 requirement |

wgpu supports browser execution through WASM and native graphics backends. Its browser path uses the browser WebGPU implementation. These facts support portability, not a guaranteed speedup over submitting equivalent WGSL from TypeScript. See [wgpu platform](../sources.md#wgpu-platforms) and [buffer semantics](../sources.md#wgpu-buffers).

## Why this fits the supplied source

The [static source review](../validation/source-review-2026-09-21.md) identifies concrete boundaries: `createSandbox()` already composes state, physics, tools and rendering; controls depend on a narrowed contract. Retain those UI concepts while replacing their backend implementation. The heavy redesign is the fluid/thermal state and pipeline, not the DOM.

The supplied source also contains repeated liquid-region searches and multiple pressure refreshes. Change those algorithms regardless of language. Existing per-cell arrays are not inherently the problem: structure-of-arrays is still appropriate. The problems are competing transport mechanisms, repeated work, incomplete physical closure, and CPU-oriented access assumptions.

Rust is selected for engine ownership, typed contracts, a reusable native reference/test environment and the author's existing Rust experience. It is not selected because a WGSL shader becomes faster when Rust submits it.

## State authority

A live session owns exactly one active backend. A CPU session owns CPU arrays; a GPU session owns GPU buffers and their derived caches. Rust owns the resource handles, metadata and scheduling in either case. A reference test may create separate CPU and GPU sessions from the same fixture.

A conceptual `Simulation<B>` can contain a backend, command ledger and bounded graph metadata. It must not contain both a fully current CPU `World` and a fully current GPU `World` that are reconciled every frame. CPU checkpoints are occasional committed snapshots, not a second ticking simulation.

Keep pressure and temperature derived where required by the model. The example list of GPU fields in the discussion is conceptual, not an instruction to make mass, density, phase fractions, temperature and energy independently authoritative.

## Delivery policy

Retain P1 M0-M7 and P2-P9. Integrate Rust workspace/bootstrap, facade, bindings, data projection and browser capability checks inside M0/M1. M2 implements the Rust reference. M3 brings the same solver to WGSL with direct rendering. M4 adds complete supported thermodynamics and phase closure; M5 adds solids and granular coupling; M6 integrates chemistry/circuits and UI; M7 makes an evidence-based promotion decision.

The initial demonstrator remains one liquid, carrier gas and fixed walls with passive thermal/species markers. Full phase physics, sand, existing chemistry integration, the 40/83-element expansion and nuclear reactions keep their existing gates. Establish regressions before replacement, but do not require every legacy defect to be fixed twice.

See [migration work packages](../plans/rust-wasm-migration/plan.md), [GPU numerical plan](../plans/fluid-gpu-redesign/plan.md), and [engine boundary](engine-boundary.md).

## Browser and fallback policy

Require WebGPU compute for the accelerated backend. WebGL2 cannot run this compute pipeline; it is not a transparent fallback. Keep legacy TypeScript/Canvas for unmigrated scenes, and allow a corrected Rust CPU backend only for scenes it actually supports. Unsupported combinations must be rejected before state replacement. [Compute support](../sources.md#wgpu-downlevel).

Start M3 on the main browser thread with asynchronous GPU work and bounded submission. Validate a worker/OffscreenCanvas deployment in M6 where supported and useful. The Rust core must not depend on a DOM or on a particular scheduler placement. Workers, threads, shared memory and native packaging are separate decisions, not prerequisites for correcting the fluid solver.

## Alternatives and costs

TypeScript plus WebGPU would be the smaller browser-only change. It remains a valid architectural alternative, but is not the selected implementation in this plan. Do not maintain both a production TypeScript WebGPU backend and a Rust/wgpu backend to avoid choosing.

A full Rust shell or Bevy would expand scope into UI, application lifecycle and asset architecture. Reconsider only when there is a concrete native-product requirement, outside the default P1-P9 delivery sequence.

Costs of this decision include a Rust/WASM toolchain, generated bindings, two numerical implementations for reference/parity, browser-specific GPU validation, explicit async lifecycle management and data-layout tests. Numerical correctness and browser availability still require tests. Avoid speculative abstraction and lock tool versions after a real browser smoke test.

## Evidence and performance claims

The quoted approximately 79/114/479 ms per tick measurements are user-supplied historical headless results, not fresh timings for `src(3).zip`. The original raw benchmark logs and profiling artifacts are not present in the supplied packages. The retained 35-check audit is a different historical artifact.

No Rust, WASM or GPU performance multiplier has been established. Compare completed physical progress, numerical quality, host overhead, rendering, and transfer traffic on named devices. A failed performance gate changes the rollout or algorithm, not the truthfulness of the result. See [migration validation](../validation/backend-migration.md).

## Acceptance of this decision

Before default-backend promotion, demonstrate the narrow boundary, single state authority, Rust native reference, browser WASM integration, WGSL parity, direct rendering, bounded readbacks, supported scene coverage and recovery. All work remains planned until its evidence gates pass.
