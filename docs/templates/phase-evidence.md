# Milestone evidence record

Copy this file into the run's durable summary location and fill in actual results. Empty fields mean unrecorded, not passed. Never replace a missing run with an estimate.

## Identity

```text
phase:
milestone:
gate:
status: planned | in-progress | implemented | validated | blocked
run ID:
run date:
source commit:
working-tree snapshot/hash:
input scene and seed:
catalogue/data/network versions:
checkpoint/schema version:
```

## Scope and contracts

Describe what changed, who owns state, which previous behavior was replaced, supported backend/model/material combinations, property domains, and explicit exclusions. Link the controlling plan and source references. Record any deviation from the plan and its numerical/design rationale.

## Environment and commands

```text
OS and architecture:
Bun/Node/TypeScript or other tool versions:
browser and version:
adapter/device information available:
requested features/limits:
exact commands and exit codes:
raw log paths:
```

Mark hardware fields `not run` for a CPU-only milestone. Do not imply GPU execution from shader compilation or screenshots alone.

## Results

| Test/fixture ID | Independent reference | Physical time / scale | Tolerance or statistical criterion | Measured result | Pass / fail / unrun |
| --- | --- | --- | --- | --- | --- |

Report conserved inventory and each relevant energy channel before/after, boundary/brush/source terms, residuals, refinement results, and maximum invalid/negative states. Explain any explicitly approximated mass-defect or pseudo-material term.

## Performance and resource evidence

Record grid, active components, warm-up, sample count, displayed frame p50/p95, simulated seconds per wall second, latency, substeps, iteration/dispatch counts, allocation manifest, transfer bytes and queue sizes. Distinguish measured values from proposed targets.

## Recovery and compatibility

List save/load, schema migration, device-absence/loss, component growth, command ordering, pause/reset, settings and replay tests. State unsupported combinations and whether their rejection was tested.

## Acceptance decision and next work

Explain which exit criteria passed, which are blocked/unrun, the exact next dependency-ready task, and what documentation/manifests were updated. Link raw evidence rather than replacing it with a general success statement. `Validated` requires all applicable gate evidence.

## Rust/WASM and GPU execution evidence

- Selected ADR revision and E-work-item IDs:
- Rust toolchain, Cargo.lock hash, wgpu and wasm-bindgen versions:
- Frontend lockfile, generated bindings/data hashes and build command:
- Native versus WASM target, actual wgpu backend and browser context:
- ABI/layout and generated-output checks:
- Accepted epoch/tick/substeps/physical time versus requested/submitted work:
- Host encoding/submission, completion, rendering and end-to-end timing:
- Upload/readback bytes, allocations, queue depth and checkpoint cadence:
- Async probe age, reset/cancellation, snapshot consistency and device-loss outcomes:
- GPU/worker/browser runs that were unavailable or explicitly not run:
- Supported-scene scope of any promotion; remaining legacy coverage:

## Fire and combustion evidence when applicable

```text
FIRE-W work IDs and parent E-work IDs:
FIRE-A fixture IDs and FIRE-PRECONDITIONS / FIRE-M6 / FIRE-M7 gate:
historical F-check context (characterization only, not pass evidence):
reference type: isolated source | complete substep | browser/GPU integration
reaction network/property hashes, fuel profile and declared calibration:
active reactants/products, physical soot/residue and constituent mapping:
full temperature/pressure/composition domain and unsupported cases:
initial/final inventories, reaction extent and residual reagents:
signed boundary component/mass/energy fluxes:
chemical/thermal/kinetic/source/radiation energy accounting convention:
accepted dt sequence, rejected attempts and equal-time refinement:
shared-reactant contention and execution-order variation:
water-dose/evaporation and extinction observations:
render-on/off and smoke-persistence results:
command IDs, commit counts, save/recovery and capacity failure:
per-scene promotion scope and applicable gates not run:
```

The old fire harness asserts observations of the legacy source, including bugs. Do not copy its `observed-and-asserted` status into a corrected FIRE-A pass. Record independent balance oracles and both absolute and relative tolerances for small/zero inventories.
