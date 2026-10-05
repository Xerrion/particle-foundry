# Current-sandbox GPU migration: developer entrypoint

**Revision:** 3 October 2026. **Active release:** P1 only. E00-E03 and
E05-E08 have recorded local validation. E09 is in progress with an isolated
[water, carrier-air and finite-vent reference](thermodynamics.md). E04 and E10-E14 remain planned.

E05 supplies a sealed Rust f64 pressure reference. E06 adds conservative phase
and marker transport, compatible momentum, viscous shear and atomic bounded
substeps. Its named CPU scenes and dam refinement pass locally. The live
sandbox still runs TypeScript.
[E05 evidence](validation/p1-m2-e05.md) and
[E06 evidence](validation/p1-m2-e06.md) record the verified CPU slices.
[E07 GPU-stage evidence](validation/p1-m3-e07.md) and
[E08 browser-scene evidence](validation/p1-m3-e08.md) record current local
results and open gates. [RESUME](RESUME.md) preserves the dated recovery handoff;
the [engine tracker](data/engine-migration-work.json) owns status.

## Goal and scope

Deliver the current sandbox on Rust/WASM + wgpu with correct physics, usable
browser controls, bounded resources, failure recovery and measured performance.
The [current-sandbox scope](plans/rust-wasm-migration/current-sandbox-scope.md)
fixes required coverage. Every row must pass before the full default switch.
This includes the eight existing elemental models and all current interactions;
first migration of that coverage is P1 work, not a deferred P4 dependency.

P2-P9 and FIRE-W08 are deferred. Do not start new pressure waves, fracture,
materials, general chemistry, nuclear features, worker deployment or product
expansion. Completing P1 does not activate them. A separate user decision is
required. Dependencies in deferred plans describe a possible future order.

Retain the TypeScript UI and current web/engine placement under
[ADR-001](architecture/adr-001-rust-wasm-wgpu.md). Implement independent Rust f64
references, then equivalent WGSL/f32 compute and direct rendering. Preserve
current user capabilities while correcting documented physics defects; pixel
parity with an invalid legacy calculation is not an acceptance criterion.

## Three delivery checkpoints

| Checkpoint | Work | Required result |
| --- | --- | --- |
| GPU proof | M2-M3 / E05-E08 | Verified Rust fluid reference and a small real-browser GPU scene with one liquid, carrier gas, fixed walls, basic controls and direct rendering. Experimental only. |
| Existing functionality | M4-M6 / E09-E12, plus E04 circuits | All current-sandbox thermal, motion, elemental, reaction, source, circuit and browser rows implemented and tested within their specified domains. |
| Default switch | M7 / E13-E14 | Complete scope matrix, browser/device evidence, sustained performance and recovery; GPU-M7 and FIRE-M7 pass. |

Retain legacy access throughout development. Passing a subset does not permit
removing a failed row, marking it optional or calling P1 complete. Record any
required scope change for a user decision. Necessary property tables and
reaction products belong to P1; broader registries and new palette/reaction
coverage remain deferred. Bounded Rust CPU circuit work is compatible with the
GPU engine; the scene retains one authoritative state owner.

## Copy-paste developer brief

```text
Read docs/README.md, this entrypoint and the assigned E-work package.
The sole active release is P1: migrate the current sandbox to Rust/WASM + wgpu.
Use docs/plans/rust-wasm-migration/current-sandbox-scope.md as required coverage.
E00-E03 and E05-E08 are validated locally. E09 is in progress; E10-E14 remain planned.
E04 circuits remain planned and are required before E11.
Execute only the assigned work item or explicitly authorized sequence.
Preserve the TypeScript UI in web/, Rust/WGSL in engine/, and current legacy access.
Use current source/tests and recorded E00 evidence; preserve unrelated changes.
Build independent Rust references before porting corresponding physics to WGSL.
Keep one live state owner, bounded commands and stamped asynchronous observations.
Render from GPU state; no full-world normal-frame transfers or duplicate ticking world.
Keep M3 to one liquid, carrier gas, fixed walls and passive markers with basic controls.
M4-M6 must cover all current-sandbox rows, including the existing eight element models.
Use only the property, component and product support required for existing functionality.
Retain FIRE-W00-W07/FIRE-A requirements; fix known fire/circuit/numerical defects.
Keep the existing bounded Blast/gunpowder behavior; resolved waves/fracture are deferred.
Keep P2-P9, FIRE-W08, worker deployment and new product features deferred.
P1 completion does not authorize later work.
Run relevant reference/native/WASM/browser/GPU checks and report unavailable checks.
Record per-row evidence and numerical differences; do not reduce scope to pass M7.
Default switch requires the complete current-sandbox matrix plus GPU-M7/FIRE-M7.
```

## Controlling documents

Use the [knowledge map](README.md) for general development. For an assigned
migration item, read only its dependencies and applicable contracts:

1. [Migration work packages](plans/rust-wasm-migration/plan.md) and
   [current-sandbox scope](plans/rust-wasm-migration/current-sandbox-scope.md).
2. [Fluid/GPU equations and gates](plans/fluid-gpu-redesign/plan.md),
   [engine boundary](architecture/engine-boundary.md) and
   [project placement](project-structure.md).
3. [Backend acceptance](validation/backend-migration.md), applicable
   [fire gates](validation/fire-combustion.md) and
   [evidence template](templates/phase-evidence.md).

The phase and work manifests own status. The scope checklist owns current
product coverage; numerical and fire plans own their acceptance requirements.
Historical evidence and deferred proposals cannot expand an assigned task.
The GPU [HTML view](plans/fluid-gpu-redesign/plan.html) is generated from Markdown.

## Verification and evidence

Run commands from the repository root through mise. Use focused tasks for the
assigned work and `mise run ci` for repository checks; browser GPU evidence is
required for the GPU path. A native device smoke or mock session test cannot
prove a physical GPU scene. Capture unavailable devices and failed checks as
unrun/failed, not passed.

Freeze named fixtures, domains, expected observables and tolerances before
implementing each scope row. Fix browser/device targets and the measurement
protocol before M3 benchmarking. Measure accepted simulated time, completed work,
latency and transfers at the current 480 x 270 grid; a rendered frame or submitted
tick is not completed simulation. The 1/60 s outer clock and 0.01 m cell width
remain the defaults unless explicitly versioned.

Use the current checkout, including uncommitted changes, for fresh evidence.
Record commits, commands, versions, numerical differences and limitations in
new evidence summaries; keep raw logs in ignored artifacts/. Historical source
ZIPs, old audit totals and F01-F12 fire observations retain their original scope.
Preserve them byte-for-byte. Do not turn known historical defects into required
new-engine behavior or claim this plan revision executed new simulation tests.
