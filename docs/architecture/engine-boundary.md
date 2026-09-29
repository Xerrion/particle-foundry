# Engine boundary, execution and state contracts

**Status:** accepted implementation contract under [ADR-001](adr-001-rust-wasm-wgpu.md). E01 lifecycle initialization and E02 portable schemas/checkpoint packing exist. E03 adds a bounded experimental host session tested with mock owners. The E07 Rust/WGSL ABI sentinel and a small coupled fluid fixture run on native and browser GPU adapters. E08 now has an opt-in browser owner, direct rendering, bounded water/air paint, a stamped one-cell probe and explicit capture prototype. A local 60-tick sustained Chrome run and full repository CI pass; M3 is validated locally. [Project structure](../project-structure.md#implemented-bootstrap-and-contracts) links the implemented subset and its tests. This extends the [matter model](matter-model.md), not the numerical equations in the [GPU plan](../plans/fluid-gpu-redesign/plan.md).

## 1. Concrete separation

```text
Existing web/src/app/, web/src/styles/, web/src/main.ts
  TypeScript UI, input, presentation and browser lifecycle
                   |
         commands / versioned observations
                   |
       engine/crates/wasm: wasm-bindgen facade
                   |
       sim contracts and session orchestration
                   |
        one selected authoritative backend
          /                            \
 sim-cpu: Rust f64              sim-gpu: Rust wgpu
 reference / supported CPU     WGSL compute + rendering
                                      |
                         GPU buffers stay resident
```

Keep the browser application in `web/` and the Rust workspace in `engine/`. The existing TypeScript backend is isolated in `web/src/legacy/`, reached through `web/src/engine-client/`. The active UI uses the synchronous legacy adapter; E03 defines a separate experimental queued contract. That general contract is not yet connected to Rust scene execution. The opt-in E08 page reaches its limited Rust scene through a dedicated adapter. Exact paths and source replacements are in [project structure](../project-structure.md).

## 2. Identity and configuration

Use independent axes for execution and physics. New sessions select execution `cpu-reference` or `wgpu` and a supported model such as `lowMach` or, if deferred P2 is separately activated, `compressible`. The retained legacy adapter has model `legacy-cellular`. Record the actual wgpu runtime backend separately, such as browser WebGPU or a native backend. Preserve older `fluid-cpu`/`fluid-gpu` labels through an explicit alias map, not a new schema for every label.

A session owns an epoch, command sequence, accepted tick/substep and physical time. Load, reset and replacement advance the epoch so late callbacks cannot affect a new world. Keep safe integer validation at the JS boundary; wider serialized counters need an explicit string/BigInt encoding rather than precision-losing numeric conversion.

Capability reports include material/forms, property domains, model, source networks, boundary types, precision, active-component capacity, GPU requirements and validated combinations. Device presence is not physics support. Native success is not browser support.

## 3. Proposed browser-facing facade

The following TypeScript is an illustrative interface contract, not a supplied implementation or a drop-in API. Referenced payload types must be defined and generated/tested in M1.

```ts
interface EngineFacade {
  // Accepts a bounded batch. Acceptance into the queue is not physical commit.
  enqueue(commands: readonly EngineCommand[]): QueueReceipt;

  // Requests fixed outer ticks. Returns immediately, without waiting for GPU work.
  advance(requestedTicks: number): AdvanceReceipt;

  // Cached, detached data. Includes epoch, completed tick, time and sample age.
  latestStatus(): EngineStatus;

  // Encodes presentation of committed state, without reading the world into JS.
  render(view: RenderView): void;

  // Resolves a small detached sample or an explicit stale/cancelled error.
  probe(x: number, y: number, signal?: AbortSignal): Promise<ProbeReading>;

  // Full-state transfers are explicit snapshot operations, not frame-loop work.
  save(): Promise<Uint8Array>;
  load(snapshot: Uint8Array): Promise<LoadReport>;

  // Stops scheduling and cancels pending requests; teardown releases resources.
  dispose(): void;
}
```

Initialization is asynchronous and returns an engine plus a capability/load report. The generated WASM API may use lower-level payloads; keep its TypeScript adapter thin and free of physics. Validate public input on both sides. Rust async exports can return JavaScript Promises; use that for initialization and readback operations rather than blocking waits. [Binding reference](../sources.md#wasm-bindgen-async).

Do not preserve `const probe = sim.probe(x, y)` as an always-fresh synchronous GPU read. The UI can synchronously read a cached observation but must show its epoch/tick and staleness. Coalesce pointer probes and discard obsolete results. A `step()` convenience wrapper, if retained, submits a request; the receipt must not claim the GPU finished.

The native reference API may run synchronously in tests. Shared semantics matter more than forcing identical blocking behavior across native and browser surfaces.

## 4. Commands, input, and accepted time

Commands cover brush strokes, material placement, external energy, settings, reset/seed, dynamic-solid edits and pause/single-step requests. Batch pointer samples or a stroke description instead of crossing JS/WASM once per painted cell. Rust validates and rasterizes physical brush edits; TypeScript keeps pointer mapping and UI preview.

A batch contains epoch, monotonic sequence IDs and scheduling intent. Commands commit in order at an accepted control/tick boundary and receive an acknowledgement. Rejected input, stale epochs, unsupported material/domain and capacity exhaustion are explicit results. Duplicate delivery/retry must not apply a source twice.

Preserve paint-while-paused through a control-only commit with no physical-time increment. A held heat/ignition source is scheduled once per accepted tick or by accepted physical duration, not once per RAF, submitted batch, or retry. Freeze the intended legacy interaction semantics in tests before changing them.

The host clock expresses demand. The engine reports actual accepted time. Limit pending work and drop/defer excess demand with telemetry rather than accumulate unbounded catch-up. Pause and hidden-tab transitions stop new physical work; a submitted batch may finish. Expose that distinction. Do not let a UI clock advance simulation state that the solver rejected.

## 5. State authority and transfers

| Data | GPU session owner | CPU/JS visibility |
| --- | --- | --- |
| Component/phase masses, U, compatible momentum and active radiation | GPU state buffers | Explicit checkpoint/export only |
| Derived temperature, pressure, display label | GPU derived state | Small probe or reduction |
| Immutable properties and scene component mapping | Rust metadata plus uploaded tables | Compact read-only UI projection |
| Command queue and source acknowledgements | Rust session / GPU transaction metadata | Receipts and bounded events |
| Small circuit graph | Rust CPU graph owner | Sparse state samples and funded source events |
| Reference world | A separate CPU test session | Never a continuously synchronized GPU mirror |

No normal step, render, probe or statistics refresh may copy the full physical world GPU -> WASM -> JavaScript. Initial import, an explicit save, controlled backend migration and periodic recovery checkpoints are exceptions with measured bytes and duration. In a genuinely CPU-owned fallback, exporting a compact render image is a different documented path; it must not cause GPU full-state readback.

Use a single wgpu device/queue for simulation and renderer resources in a GPU session. Rendering consumes the committed buffers or a GPU-produced display texture. Rust ownership is ownership of those handles and their rules, not proof that the latest data is in WASM linear memory. [Mapping costs](../sources.md#wgpu-buffers).

## 6. Buffer ABI and precision

Rust f64 is the reference default. WGSL uses portable f32 and integer indexing/ownership. Keep units, operator definitions, boundaries, stage order and fixtures shared; do not assume bit-identical arithmetic. A Rust f32 diagnostic run can help separate precision errors from shader errors, but is not a second required production solver.

Define and version the Rust/WGSL ABI explicitly: byte offsets, alignment, strides, padding, bindings, integer encodings and resource usages. `repr(C)` alone does not prove WGSL layout compatibility. Do not transfer a Rust `Vec`, enum or bool by raw struct copy. Use explicit scalar layouts, safe packing and shader sentinel round-trips. Test structure padding and vec3-related alignment deliberately. [WGSL rules](../sources.md#wgsl).

Use distinct stable IDs wider than the legacy byte material field and a compact active-set mapping. Do not allocate one dense field for all 118 elements or every nuclide. Check actual device limits, aggregate allocations and binding counts before committing a scene. Allocation below a stated device maximum can still fail. [Limits](../sources.md#wgpu-limits).

E02's `PFSN` version 1 checkpoint is a detached, little-endian low-Mach contract, not a legacy save importer or a live CPU mirror. It stores compact component-major kg masses, a passive J marker, MAC m/s faces, fixed walls, source/boundary ledgers, catalogue fingerprint, optional declared network identity, and epoch/tick/time. Its chemical reference is explicitly disabled; a declared network does not enable reactions. The optional nuclear payload is bounded and absent without allocation. Unknown versions, malformed values and unsupported energy references are rejected before scene replacement. The scalar GPU records are packed field-by-field as f32/u32. Rust offsets and byte order are tested; an E07 device shader sentinel validates selected grid, cell and component-major values on native and browser hardware. It does not establish a physical GPU scene.

No E02 import converts legacy `world.energy` into thermodynamic internal energy. That field is parcel enthalpy H in J, while legacy chemical/diagnostic totals use kJ; converting the latter to J does not make it available U. A future converter must identify a supported property/reference-state version for every active phase, obtain its consistent pressure and physical volume, then calculate U = H - pV in J and establish a new ledger baseline. It must reject missing domains, unsupported phases or unknown reference zeros with a conversion report. It cannot silently relabel H, infer U from temperature alone, or charge the same chemical release both to formation energies and a stored reaction account. Until that converter and its physical gates pass, snapshots accept only the passive marker and disabled chemistry.

## 7. GPU scheduling and failure

Use separately dispatched stages for whole-grid dependencies. Owned face fluxes, gather updates and reductions avoid conflicting floating-point scatter. Pressure/closure success depends on residuals and invariants, not a fixed arbitrary number of iterations.

M3 should begin with one bounded batch in flight. GPU status/validation records decide whether a candidate state can become committed; later passes must not consume a failed candidate as valid. Read only small completion/diagnostic records asynchronously when needed. Retry from the same committed state and do not consume command/source IDs twice. This synchronization cost is measured, not hidden.

Separate host encoding, submission, GPU completion, solver acceptance and presentation. A `performance.now()` measurement around `advance()` is submission overhead, not completed physics time. No blocking polling loop on the browser event thread and no blocking native executor copied into the WASM adapter.

Keep no mutable borrowed WASM-world view alive across an await or memory growth. An asynchronous readback completion should resolve an owned request via the session queue, not reenter an already borrowed simulation object. Bound staging buffers, callbacks and cancellation state.

## 8. Canvas and renderer migration

The current `main.ts` acquires a 2D context before creating the engine. Choose the renderer before the first context acquisition, or create a fresh/replacement canvas. The HTML canvas contract does not permit acquiring a different context type after one has been selected. [Canvas reference](../sources.md#html-canvas-context).

Keep a Canvas renderer only for the legacy/CPU path. A GPU scene renders with wgpu directly; it does not call the old `render(ctx)` through an ImageData conversion. Share the viewport, pixel-art sampling intent and field-view semantics through small render parameters.

A pointer/brush overlay can use a separate overlay canvas or a small GPU draw. Neither may read full state. If backend switching replaces a canvas, dispose/rebind event listeners and restore focus, pointer capture policy, pan/zoom, device-pixel ratio and accessibility status. Resize display resources independently of the physical grid.

## 9. CPU graph coupling

Keep circuit solving in Rust CPU code initially. For a GPU scene, send topology-change events and only the thermal/material samples the graph needs. Validate graph version at a declared stage barrier before emitting funded heat transactions. A topology edit caused by melting or motion invalidates the relevant graph before the next solve.

Sparse messages are a design target, not an assumption that every circuit scene is sparse. Record event-buffer overflow and dense-circuit transfer costs. On overflow, retry or reject without losing conductor changes. Choose a GPU graph solve or explicit feature limit if measurements justify it; do not hide a full-world download in the circuit adapter.

## 10. Snapshot, loss and recovery

A save captures one committed epoch/tick across every required buffer, schema, catalogue/component mapping, settings, RNG/source state and ledgers. Pause at a barrier or copy all buffers from the same committed generation into staging before asynchronous serialization. Never assemble a snapshot from unrelated ticks. Include nuclear/radiation state only when that extension is active.

Load performs schema/domain/capacity validation and prepares new resources before atomically replacing the session. Failure leaves the current world intact. Mid-scene model/backend switching requires an explicit supported conversion, not an automatic reinterpretation of arrays.

On device loss, stop new work, reject/cancel affected readbacks and report the latest recoverable checkpoint. The newest state in a lost device may be unavailable. Resume a compatible backend from that checkpoint with an explicit rollback notice, or restart with user-visible loss information. Command replay can restore intent, but floating-point/driver differences may change trajectories; do not promise bit-exact recovery across backends.

## 11. Data and tests

Keep `web/src/materials/definitions.ts` as the single legacy catalogue while bootstrapping. M1 adds a reproducible validated export/projection for the small supported scene, not a second handwritten Rust or WGSL catalogue. P1 extends generated properties only for the [required current-sandbox coverage](../plans/rust-wasm-migration/current-sandbox-scope.md). Deferred P3 would migrate scientific source records to a generalized data pipeline if separately activated. Version/hash the outputs and test IDs, units and precision conversion. UI presentation may remain authored in TypeScript when joined by stable IDs.

All phases use the same boundary. Chemistry, phase changes and nuclear stages produce validated amount/energy transactions in the owning backend; none gets its own world transport loop. See [backend acceptance](../validation/backend-migration.md) and the [work-package mapping](../plans/rust-wasm-migration/plan.md).

## 12. Fire source and renderer boundary

The [fire work plan](../plans/fire-combustion/plan.md) is part of E02, E09 and E11-E14. Fire commands provide bounded, ledgered ignition energy, not a replacement material that silently creates fuel or soot. The reference/GPU source stage reserves reactants and commits amounts, products, energy and coupled phase/volume changes together. Rejected substeps or replayed command acknowledgments cannot repeat the release.

Use one boundary configuration for mechanical and species/heat fluxes. Opening a face permits finite transport from a declared reservoir; it does not reset the composition of every reachable cell. Network/property/capacity preflight includes every combustion product before accepting a brush or scene.

The same-device renderer derives flame/smoke appearance from reacting/hot gas and tracked particulate state. Preserve supported particle-based presentation with state-driven shader treatment; visual particles are not a second combustion solver. Visibility, lifetime and a stale `burning` flag never control physical inventory or ignition. Hot-object glow and active combustion are distinct probe meanings.

Async observations expose actual fuel, O2 and product amounts, heat-release rate, supported extinction reason and data limits with the existing tick/epoch contract. Snapshot/recovery includes network versions, physical smoke/residue and approximation settings. FIRE-A07, FIRE-A09 and FIRE-A14 add exact-once source, visual-independence and lifecycle checks to the existing backend gates.
