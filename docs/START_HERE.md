# Engine migration: developer implementation entrypoint

For general development, start at [the knowledge map](README.md). Use this migration
brief only for assigned migration work. Reading it does not authorize the full roadmap.

**Project:** Particle Foundry. **Revision:** 23 September 2026, fire-audit integration update. **First task:** P1 / M0 / E00. **Status:** development plan, not delivered implementation.

**Change the simulation engine, not the application.** Keep the current TypeScript UI, DOM, CSS, input and viewport. Build corrected references in Rust, use WASM for browser delivery, and run heavy parallel physics plus direct rendering through Rust wgpu and WGSL. This decision is [ADR-001](architecture/adr-001-rust-wasm-wgpu.md).

For assigned migration work, follow P1-P9 dependencies beginning with the GPU redesign.
Implement only the requested work item or explicitly authorized sequence. The Rust
bootstrap and engine boundary are part of P1 M0/M1, not a separate whole-codebase
rewrite before the numerical work. M2 is a Rust f64 reference; M3 is the equivalent
WGSL/f32 GPU solver.

## Copy-paste developer brief

```text
Start with docs/README.md, then use docs/START_HERE.md for the P1-P9 migration.
Execute only the assigned work item or explicitly authorized sequence.
First work item: P1 / M0 / E00 in docs/plans/rust-wasm-migration/plan.md.
Follow ADR-001: retain the TypeScript frontend; migrate the simulation engine
selectively to Rust/WASM with wgpu-managed WGSL compute and direct rendering.
Keep the UI in web/ and add Rust/WGSL under engine/. Do not rewrite the UI or adopt Bevy.
Inspect the real repository, its instructions, scripts, tests and working changes.
Preserve a recoverable tracked/untracked snapshot before any implementation edits.
Read the source review and docs/validation/fire-combustion.md, then establish a fresh baseline.
Preserve the imported F01-F12 characterization evidence; it is not corrected acceptance.
Follow FIRE-W00-FIRE-W07 within P1 and FIRE-W08 in P4, alongside E00-E14.
Historical results are not fresh tests.
For P1, follow both the numerical GPU plan and the E00-E14 migration work packages.
Add the Rust/WASM build and command boundary early, then build the Rust f64 reference.
Only port the numerical solver to WGSL after the corresponding reference gates pass.
Do not first rewrite every old TypeScript solver into Rust or duplicate a new TS reference.
Keep one authoritative state and one transport owner per scene.
Use GPU-resident state, bounded commands, asynchronous tick/epoch-stamped probes,
consistent snapshots, and explicit supported-scene fallback. No full-world frame-loop copies.
Keep M3 limited to one liquid, carrier gas, fixed walls and passive markers.
Implement the minimal matter identity/storage seam in M1, not all chemistry/nuclear features.
Complete M4-M7 integration and promotion before the default queue advances to P2.
Require FIRE-PRECONDITIONS in M4, FIRE-M6 integration and FIRE-M7 before fire-scene promotion.
Replace oxygen refill, cold-FIRE ignition, missing products and contact-only water quenching.
Preserve funded ignition, finite inventories and derived visuals; no per-substep replay of old per-tick rates.
Continue P2-P9 using the same engine contracts and the 40/83/103/118 element cohorts.
Run the applicable native, WASM, browser, numerical and GPU tests; record unrun gates honestly.
Update evidence and work status only when supported.
Continue with another work item only when it is within the authorized task scope.
```

## Reading order for migration work

For a bounded work item, read its controlling plan, dependencies and contracts.
Use this broader sequence when establishing the migration baseline; it is not
required reading for unrelated development.

1. [Stack decision](architecture/adr-001-rust-wasm-wgpu.md), [roadmap](roadmap.md), and [active work](remaining-work.md).
2. [Static source review](validation/source-review-2026-09-21.md), [fire evidence and corrected gates](validation/fire-combustion.md), and [project structure](project-structure.md). Confirm the real checkout, which may differ from the upload.
3. [Rust/WASM work packages](plans/rust-wasm-migration/plan.md), [engine boundary](architecture/engine-boundary.md), [fluid/GPU numerical plan](plans/fluid-gpu-redesign/plan.md), and [fire integration plan](plans/fire-combustion/plan.md).
4. [Matter model](architecture/matter-model.md), [current physical model](model.md), and [acceptance](validation/acceptance.md). Read later phase plans to avoid dead-end contracts, not to enlarge M3.

The Markdown documents are canonical. The GPU [HTML view](plans/fluid-gpu-redesign/plan.html) is generated. ADR-001 controls the stack, the engine-boundary document controls integration, the GPU plan controls its equations/gates, and the phase manifest controls sequence. The fire plan and FIRE-A gates refine M0/M1/M4/M6/M7 and P4 without expanding M3. Older copies under history preserve superseded assumptions and do not override this entrypoint.

## What was inspected and what remains unknown

The following describes the historical 23 September review, not the current repository layout or current verification. The supplied `src(3).zip` was statically inspected, including the startup, sandbox facade, state, movement, pressure call graph, renderer, diagnostics and material records. Its 43 file hashes and line references are recorded in [the source manifest](data/source-review-manifest.json). No application source was changed or included in this docs-only update.

The upload does not contain package/lockfiles, repository metadata, tests, benchmark harnesses or CI configuration. Their real status must be checked at M0. No application, Rust/WASM, browser or GPU test was run for this revision. The retained 24-pass/11-fail audit and quoted historical timings remain separate evidence, not fresh measurements. The separately imported fire review records 12 prior characterization checks, including two complete physics-tick cases; none was rerun in this update and none counts as a corrected FIRE-A pass.

## First development slice

### M0 / E00: capture the actual baseline

Inspect repository instructions and Git state. Preserve tracked and untracked work in a recoverable snapshot; a patch alone is insufficient. Do not reset to the historical audit commit. Identify the real commands for lint, type checking, tests and build, then run them with the checked-in lockfile.

The repository now defines commands and tool versions in `mise.toml`: run `mise run ci` for lint, both type checks, tests, build and documentation validation. Individual tasks include `mise run lint`, `mise run check`, `mise run test` and `mise run build`; setup is documented in the root README. Earlier audit commands and results remain historical evidence. Keep legacy audit results and fixtures, but write new results under `artifacts/validation/p1-m0/<run-id>/`.

Include FIRE-W00 in this baseline. Preserve F01-F12 and their source hashes; create separate corrected assertions for the closed-boundary and cold-FIRE bugs, plus the brush and explicit-reaction controls. Keep the tiny-water observation labelled reaction-only. Do not demand a full legacy TypeScript repair before the Rust work.

Freeze numerical fixtures, accepted physical-time accounting, data/version conventions and reference tolerances. Record browser/OS/device and warm-up/sample conventions for new performance evidence. A missing GPU can leave hardware gates unrun without preventing native-reference work.

### M1 / E01-E04: make the Rust/browser seam concrete

Add the minimal Rust workspace and reproducible WASM/bindings build under `engine/` while retaining the TypeScript app in `web/`. Establish a real browser initialization/disposal smoke test. Define units, IDs, active-component mappings, snapshot schema, async observation/command contracts and shader layouts.

Use the existing sandbox composition as the adapter point. Preserve legacy scenes separately. Keep a single authored catalogue with generated projections, not handwritten Rust/TS/WGSL copies. Plan canvas selection before acquiring a context and use epoch-stamped callbacks.

FIRE-W01 adds the minimum shared fuel/O2/product, boundary-flux, source-energy, smoke/soot and derived-visual contracts. These are schema and ownership requirements, not active combustion in the first demonstrator. Do not create another independent oxygen inventory or allocate all 118 components per cell.

E04 adds the corrected Rust CPU circuit reference as an independent branch; it must pass before M6 integration but does not delay M2's fluid work. Do not first fix and re-port every old TypeScript solver.

### M2 / E05-E06: validate corrected Rust equations

Implement the small Rust f64 CPU reference: compatible MAC operators, boundary conditions, conservative phase/tracer/compatible momentum transport, viscosity and stable substeps. No old cellular fluid movement or hydrostatic relocation operates in this new world. Keep full phase/chemistry physics disabled while validating transport.

### M3 / E07-E08: deliver the GPU demonstrator

Implement the same stage graph in WGSL/f32 through Rust wgpu. Use the same device for compute and rendering, bounded in-flight work, asynchronous small probes/reductions and explicit failure/commit state. Do not call the old Canvas/ImageData renderer through a full-world download.

Test at equal accepted physical times, report actual allocations/transfers and distinguish submission cost from GPU completion. M3 is not the default full sandbox and does not promise a speed multiplier. Continue M4 thermodynamics, M5 solids, M6 integration and M7 promotion before moving to the normal P2 queue.

## Fire integration after the demonstrator

At M4/E09, pass finite species-ventilation and property-domain preconditions before fire/water coupling. The provisional 623.15 K water table does not cover the legacy 450 C emitted-flame setting. Extend validated coverage or reject the combination explicitly; do not clamp it into range.

At M6/E11, implement the coherent combustion slice in Rust first, then WGSL with shared-reactant reservations. Consumed fuel and O2 become declared products, ignition/extinction follows the supported local state, and water suppression is amount/thermal based. Use physical-time rates and transactional rollback. For promoted wood/oil/plant scenes, bounded condensed-fuel release is required now, not postponed to P4.

At M6/E12, retain the Fire brush and particle-based presentation while deriving visible flames/smoke from real state. Cosmetic expiration cannot erase matter. At M7, require the [FIRE-M7 evidence](validation/fire-combustion.md#corrected-gates), supported hot-domain coverage, save/recovery and measured GPU behavior before promotion. P4 extends fuel, char, soot and selected radiation mechanisms through FIRE-W08.

## Defaults and rules for all phases

Preserve the documented 480 x 270 grid, 0.01 m cell width and represented depth, and 1/60 s outer clock until an explicit versioned model change is justified. Internal substeps follow stability constraints; accepted simulation time, not display FPS, drives physics and sources.

Keep physical model and execution backend separate. `lowMach` and `compressible` are models; CPU and wgpu are execution choices. Legacy scenes stay on their own backend until conversion and feature gates pass. No-adapter or device-loss behavior must not silently change a live scene's equations.

Implement each capability as `planned -> in-progress -> implemented -> validated`, with a specific `blocked` status where needed. Use [the evidence template](templates/phase-evidence.md) and [backend validation](validation/backend-migration.md). Shared formulas and passing parity do not replace independent physical reference tests.

The all-118 roster and 40/83/103/118 delivery cohorts remain unchanged. Registry presence is not validated material support. Preserve element/nuclide/species/material distinctions and isotope composition as later phases extend the engine. No whole-nuclide-table or 118-element dense array is required in every cell.
