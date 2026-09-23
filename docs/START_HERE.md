# Start here: developer implementation entrypoint

**Project:** Particle Foundry. **Revision:** 21 September 2026, Rust/WASM migration update. **First task:** P1 / M0 / E00. **Status:** development plan, not delivered implementation.

**Change the simulation engine, not the application.** Keep the current TypeScript UI, DOM, CSS, input and viewport. Build corrected references in Rust, use WASM for browser delivery, and run heavy parallel physics plus direct rendering through Rust wgpu and WGSL. This decision is [ADR-001](architecture/adr-001-rust-wasm-wgpu.md).

Implement all P1-P9 phases in dependency order, beginning with the GPU redesign. The Rust bootstrap and engine boundary are part of P1 M0/M1, not a separate whole-codebase rewrite before the numerical work. M2 is a Rust f64 reference; M3 is the equivalent WGSL/f32 GPU solver.

## Copy-paste developer brief

```text
Start with docs/START_HERE.md and implement the P1-P9 roadmap.
First work item: P1 / M0 / E00 in docs/plans/rust-wasm-migration/plan.md.
Follow ADR-001: retain the TypeScript frontend; migrate the simulation engine
selectively to Rust/WASM with wgpu-managed WGSL compute and direct rendering.
Do not rewrite the UI, adopt Bevy, or move src/ to apps/web/ as a prerequisite.
Inspect the real repository, its instructions, scripts, tests and working changes.
Preserve a recoverable tracked/untracked snapshot before any implementation edits.
Read the source review, then establish a fresh baseline. Historical results are not fresh tests.
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
Continue P2-P9 using the same engine contracts and the 40/83/103/118 element cohorts.
Run the applicable native, WASM, browser, numerical and GPU tests; record unrun gates honestly.
Update evidence and work status only when supported. Continue with the next ready work item.
```

## Read in this order

1. [Stack decision](architecture/adr-001-rust-wasm-wgpu.md), [roadmap](roadmap.md), and [active work](remaining-work.md).
2. [Static source review](validation/source-review-2026-09-21.md) and [project structure](project-structure.md). Confirm the real checkout, which may differ from the upload.
3. [Rust/WASM work packages](plans/rust-wasm-migration/plan.md), [engine boundary](architecture/engine-boundary.md), and [fluid/GPU numerical plan](plans/fluid-gpu-redesign/plan.md).
4. [Matter model](architecture/matter-model.md), [current physical model](model.md), and [acceptance](validation/acceptance.md). Read later phase plans to avoid dead-end contracts, not to enlarge M3.

The Markdown documents are canonical. The GPU [HTML view](plans/fluid-gpu-redesign/plan.html) is generated. ADR-001 controls the stack, the engine-boundary document controls integration, the GPU plan controls its equations/gates, and the phase manifest controls sequence. Older copies under history preserve superseded assumptions and do not override this entrypoint.

## What was inspected and what remains unknown

The supplied `src(3).zip` was statically inspected, including the startup, sandbox facade, state, movement, pressure call graph, renderer, diagnostics and material records. Its 43 file hashes and line references are recorded in [the source manifest](data/source-review-manifest.json). No application source was changed or included in this docs-only update.

The upload does not contain package/lockfiles, repository metadata, tests, benchmark harnesses or CI configuration. Their real status must be checked at M0. No application, Rust/WASM, browser or GPU test was run for this revision. The retained 24-pass/11-fail audit and quoted historical timings remain separate evidence, not fresh measurements.

## First development slice

### M0 / E00: capture the actual baseline

Inspect repository instructions and Git state. Preserve tracked and untracked work in a recoverable snapshot; a patch alone is insufficient. Do not reset to the historical audit commit. Identify the real commands for lint, type checking, tests and build, then run them with the checked-in lockfile.

Earlier docs name `bun run lint`, `bun run check`, `bun run test` and `bun run build`; another historical audit block uses `typecheck`. The supplied source cannot resolve that script-name difference. Record the actual equivalents rather than inventing a passing command. Keep legacy audit results and fixtures, but write new results under `artifacts/validation/p1-m0/<run-id>/`.

Freeze numerical fixtures, accepted physical-time accounting, data/version conventions and reference tolerances. Record browser/OS/device and warm-up/sample conventions for new performance evidence. A missing GPU can leave hardware gates unrun without preventing native-reference work.

### M1 / E01-E04: make the Rust/browser seam concrete

Add the minimal Rust workspace and reproducible WASM/bindings build while retaining the existing TypeScript app structure. Establish a real browser initialization/disposal smoke test. Define units, IDs, active-component mappings, snapshot schema, async observation/command contracts and shader layouts.

Use the existing sandbox composition as the adapter point. Preserve legacy scenes separately. Keep a single authored catalogue with generated projections, not handwritten Rust/TS/WGSL copies. Plan canvas selection before acquiring a context and use epoch-stamped callbacks.

E04 adds the corrected Rust CPU circuit reference as an independent branch; it must pass before M6 integration but does not delay M2's fluid work. Do not first fix and re-port every old TypeScript solver.

### M2 / E05-E06: validate corrected Rust equations

Implement the small Rust f64 CPU reference: compatible MAC operators, boundary conditions, conservative phase/tracer/compatible momentum transport, viscosity and stable substeps. No old cellular fluid movement or hydrostatic relocation operates in this new world. Keep full phase/chemistry physics disabled while validating transport.

### M3 / E07-E08: deliver the GPU demonstrator

Implement the same stage graph in WGSL/f32 through Rust wgpu. Use the same device for compute and rendering, bounded in-flight work, asynchronous small probes/reductions and explicit failure/commit state. Do not call the old Canvas/ImageData renderer through a full-world download.

Test at equal accepted physical times, report actual allocations/transfers and distinguish submission cost from GPU completion. M3 is not the default full sandbox and does not promise a speed multiplier. Continue M4 thermodynamics, M5 solids, M6 integration and M7 promotion before moving to the normal P2 queue.

## Defaults and rules for all phases

Preserve the documented 480 x 270 grid, 0.01 m cell width and represented depth, and 1/60 s outer clock until an explicit versioned model change is justified. Internal substeps follow stability constraints; accepted simulation time, not display FPS, drives physics and sources.

Keep physical model and execution backend separate. `lowMach` and `compressible` are models; CPU and wgpu are execution choices. Legacy scenes stay on their own backend until conversion and feature gates pass. No-adapter or device-loss behavior must not silently change a live scene's equations.

Implement each capability as `planned -> in-progress -> implemented -> validated`, with a specific `blocked` status where needed. Use [the evidence template](templates/phase-evidence.md) and [backend validation](validation/backend-migration.md). Shared formulas and passing parity do not replace independent physical reference tests.

The all-118 roster and 40/83/103/118 delivery cohorts remain unchanged. Registry presence is not validated material support. Preserve element/nuclide/species/material distinctions and isotope composition as later phases extend the engine. No whole-nuclide-table or 118-element dense array is required in every cell.
