# Rust/WASM + wgpu validation and promotion

**Status:** planned tests, not results. Applies alongside [existing acceptance gates](acceptance.md). [ADR-001](../architecture/adr-001-rust-wasm-wgpu.md) selects the stack; [work packages](../plans/rust-wasm-migration/plan.md) assign implementation ownership.

## Validation layers

| Layer | What it proves | Required evidence |
| --- | --- | --- |
| Static source inspection | Existing code shape and migration touchpoints | Hashes and exact paths/lines, no inferred timing |
| Rust native reference | Contracts, independent equations and numerical invariants | f64 fixtures, tolerances, refinement and accepted time |
| Rust/WASM browser integration | Bindings, lifecycle, input and generated data actually work in the browser | Built artifacts and real-browser tests |
| WGSL/native wgpu parity | Shader equations, ABI and resource behavior on that adapter | Reference comparison and validation errors |
| WGSL/browser WebGPU parity | Browser backend and its limits work for the tested scenes | Browser/device identification and equal-time results |
| Product and performance | Usable, supported coverage on named devices | Sustained full-pipeline measurements and failure injection |

Native compilation, headless JS tests and online API documentation do not replace a browser GPU run. Agreement between Rust and WGSL is not sufficient without independent physical/reference checks.

## Stable migration fixture families

| ID | Required contract |
| --- | --- |
| MIG-01 | Existing TypeScript application builds and controls work behind the facade; no UI rewrite or direct new-world array access |
| MIG-02 | Native/WASM builds, bindings generation, asset loading, repeated initialization/disposal and generated-data freshness |
| MIG-03 | ABI offsets, padding, strides, usages and integer encodings match in Rust and WGSL; sentinel round trip on device |
| MIG-04 | Exactly one backend state owner; no legacy movement or thermal pass mutates a new session |
| MIG-05 | Queue admission differs from commit; bounded backpressure, exactly-once commands/sources, accepted physical time and retries |
| MIG-06 | Paused painting, held tools, single-step, hidden-tab, speed, reset/load and obsolete-probe epoch races |
| MIG-07 | A renderer is selected before context acquisition; canvas replacement preserves controls, zoom, overlays and display scaling |
| MIG-08 | No full-world GPU-to-host readback for normal render/probe/stats; traffic counters include JS/WASM and GPU staging |
| MIG-09 | GPU completion/residual flags prevent invalid state publication or repeated source consumption |
| MIG-10 | Snapshots capture one epoch/tick; atomic load rejection; device-loss rollback notices and unsupported fallback rejection |
| MIG-11 | CPU graph versioning, source funding, changed-conductor samples, sparse event overflow and dense-network cost |
| MIG-12 | Capability checks cover actual browser context, device limits, data domain, component capacity and physics combinations |
| MIG-13 | f64 reference and WGSL comparisons at equal accepted time, with invariant and observable tolerances rather than bit identity |
| MIG-14 | Reproducible benchmark environment, quality settings, transfers, CPU submission versus completed-GPU timing and coverage |

Existing OWN/CMD/ID/FLUID/etc. families remain valid. Link migration tests to those invariants instead of duplicating inconsistent expected values. Freeze tolerances before optimizing.

## Performance evidence protocol

The user quoted approximately 79 ms ambient, 114 ms pool and 479 ms sand-over-water per headless tick at 129,600 cells. They are historical, rendering-excluded measurements from another environment. Raw logs were not supplied here. The 58.89 percent outlet-search figure likewise remains a historical profile reference. Neither is a measured Rust/GPU comparison.

Capture these execution configurations where available:

| Configuration | Purpose | Comparison caveat |
| --- | --- | --- |
| Current TypeScript legacy, headless | Reproduce baseline algorithm/scene costs | Legacy physical model differs from the redesigned solver |
| Current TypeScript application | UI/input/render baseline on target browser | Includes Canvas and current clock behavior |
| Corrected Rust f64 reference, native | Numerical reference and CPU work instrumentation | Host/runtime differs from WASM and GPU |
| Corrected Rust reference in WASM | Browser CPU fallback feasibility and host costs | Does not measure a Rust-versus-TypeScript language effect with identical algorithms |
| Rust/wgpu GPU solver plus direct renderer | Actual target product performance | Record shader precision, substeps, tolerances and enabled features |

Do not build a production TypeScript WebGPU duplicate just for a comparison. An optional bounded host-submission microbenchmark must submit equivalent shaders/work and be reported separately from solver/product speedups.

For each run freeze grid, seed, supported scene, physical duration, boundary conditions, data hashes, enabled networks, precision, solver tolerances and warm-up. Record substeps/retries, pressure iterations and residuals, visited cells/regions, dispatch count, allocations/peak live bytes, upload/readback bytes, queue depth, input latency and accepted time.

Record at least separate host encoding/submission, completion latency, rendering/presentation, end-to-end frame intervals and accepted simulated-seconds per wall-second. Optional GPU timestamps are supplementary and capability-gated. Awaiting completion is allowed in benchmark/test boundaries; it is not permission to block the production render loop after each dispatch.

Report p50/p95, sample counts, repeat runs, warm-up and cold startup separately. Do not count requested or merely submitted ticks as completed simulation. Pair every performance result with the corresponding numerical-quality result. An algorithmic redesign comparison is not evidence that Rust alone is faster.

## Gates and fallback decisions

**M3:** demonstrate a correct supported GPU scene, same-device direct rendering, asynchronous small observations and measured transfer/allocation data. No fixed speed multiplier or 60 FPS promise.

**M7:** retain the fluid plan's provisional 480 x 270, p95 frame interval at most 33.3 ms and at least 1.0 accepted simulated second per wall-second target on the agreed baseline device at 1x. This is a target, not a result. Include active painting, supported hot/phase scenes, sand/solid contention and dense circuits where these are part of the promoted feature set.

If a performance or hardware gate fails, record it and keep the backend experimental or narrow the declared coverage. Optimize measured algorithms, dispatches and transfers without weakening physical tolerances. A no-adapter case can use legacy mode for legacy scenes or the validated Rust CPU backend for compatible scenes; it cannot silently change the physical model of an already evolved world.

## Recovery tests

Inject device loss before submission, during a batch, during probe readback and during checkpoint capture. Test rejected/failed allocations and pipeline creation. Verify cancellation and no partial commit. A device can fail before its latest state reaches a checkpoint, so recovery must disclose rollback or restart rather than claim lossless restoration.

Test stale responses after clear/load, disposal with pending operations, WASM asset/version mismatch, unsupported worker context, incompatible saved data, and capacity overflow. Test checkpoint plus command replay under the same configuration and tolerance; do not demand cross-device bit-exact chaotic trajectories.

## Documentation-update boundary

The original 23 September documentation revision added fire-plan integration and documentation checks only; it did not execute implementation gates. The preserved fire audit is historical characterization, not corrected acceptance. Subsequent E00-E02 and layout-recovery results are linked from [RESUME](../RESUME.md). Record new implementation evidence using [the template](../templates/phase-evidence.md); this specification alone establishes no pass.

## Combustion migration and hardware checks

E02, E09 and E11-E14 now carry [FIRE-W tasks](../plans/fire-combustion/plan.md#8-work-packages). The [fire validation specification](fire-combustion.md) requires Rust f64 reference results and independent balances before corresponding WGSL/f32 promotion. Native checks do not waive browser device, workgroup, transfer and recovery cases.

Normal frames render from reacting/hot gas and physical soot fields on the same GPU device. No full-world fire/oxygen copy is allowed for CPU chemistry or shader presentation. Probes identify their tick/epoch and report actual composition/heat release, not stale FIRE labels. Render-on/off and quality-level changes cannot change fuel consumption or smoke inventories.

Include FIRE-A14 in the failure matrix: partial reaction rollback, duplicate command/replay prevention, active-product capacity, network/property version validation, soot/residue persistence and checkpoint rollback notices. FIRE-M7 is required for promoted fire scenes; a historical F-check result or screenshot cannot substitute.
