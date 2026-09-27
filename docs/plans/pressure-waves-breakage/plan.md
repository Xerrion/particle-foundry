# Pressure waves and material breakage after the GPU redesign

Implementation proposal · Particle Foundry · 21 September 2026

Status: P2, planned follow-up; not implemented. [Start here](../../START_HERE.md). The default queue completes P1 first; the fixed-wall numerical prototype is technically possible after M3 without enlarging that demonstrator.

## Goal and placement

Add a follow-up milestone to the existing [GPU redesign plan](../fluid-gpu-redesign/plan.md). Keep its initial fluid demonstrator unchanged.

The new mode should produce expanding gas, travelling pressure waves, reflections, directed flow through openings, and material failure under excessive stress. These effects must come from simulated state and conserved energy.

**Defaults:** material breakage on; physical accuracy takes priority over simulation speed; GPU acceleration targets performance without changing the equations.

## Model and integration

### Compressible gas

- Add a `compressible` model alongside the planned `lowMach` model. Select the model when loading a scene; never run both transport systems over the same state.
- Reuse the planned backend interface, command queue, snapshots, material catalogue, GPU rendering, and diagnostics.
- Store gas species masses, two momentum components, and total energy per cell. Derive temperature and local pressure from composition, internal energy, and available volume.
- Use a conservative finite-volume Euler solver with MUSCL reconstruction, an HLLE low-order fallback with a separately validated positivity treatment for reconstruction and source terms, and acoustic CFL-controlled substeps. Establish a Rust f64 CPU reference before implementing equivalent WGSL through Rust wgpu. [Numerical reference](https://www.clawpack.org/riemann_book/html/Euler_approximate.html).
- Replace chamber-wide pressure equalization, capped pressure impulses, and cellular gas swapping inside this mode. Compression, expansion, venting, and advection share the same face fluxes.
- Reflect flow at intact walls. Open world boundaries exchange finite mass, momentum, and energy with an atmospheric reservoir, recording those transfers.
- Retry invalid substeps with a smaller timestep. If the work budget is exhausted, slow physical progress rather than increasing the timestep or silently clipping pressure.

### Ignition and energy release

- Replace the instantaneous radius-based gunpowder impulse in this mode with finite fuel consumption and local heat/product generation.
- Preserve the existing ignition interaction. Transport heat and reaction products so combustion spreads through the simulated charge.
- Fund all release from the stored chemical inventory. The Blast tool remains an explicitly recorded external energy source.
- Keep reaction parameters documented as sandbox approximations. Do not equate a visually plausible reaction with validated explosive chemistry.

### Material failure

- Calculate pressure traction on exposed surfaces and transmit loads through connected solid material. Failure depends on structural stress, geometry, and material properties - not absolute gas pressure alone.
- Use a bonded solid model with elastic deformation, irreversible damage, and distinct brittle and ductile responses. Start with metal, glass, stone, and wood; unsupported materials must be explicitly identified.
- Catalogue profiles hold stiffness, strength, fracture energy, and temperature dependence. Generic “Metal” receives a documented representative profile rather than an implied universal failure pressure.
- Broken connections release movable fragments. Preserve their mass, temperature, momentum, and remaining energy; account separately for elastic energy, fracture expenditure, and dissipation. Bond damage is an established fracture-model approach. [Reference](https://www.sandia.gov/app/uploads/sites/200/2022/05/3m-sandia-silling-140731.pdf).
- Couple fragment motion back to gas through changing geometry and boundary work, using the redesign’s moving-solid infrastructure.

## Delivery stages and interfaces

1. **Capture the baseline.** Preserve all pre-existing working-tree changes and capture a reproducible baseline. The earlier documentation reports removal of an incomplete `blastEnergyJ` addition during its earlier closeout; that is historical context, not a source edit performed by this revision. Inspect the actual checkout before implementation.
2. **CPU gas reference, after GPU M3.** Deliver compression, shock propagation, reflection, finite venting, and prescribed energy-release fixtures with fixed walls.
3. **GPU gas parity.** Port the same equations and stage ordering to WGSL. Keep authoritative state on the GPU; render directly and read back only asynchronous probes, reductions, and checkpoints.
4. **Reactive scenes and breakage.** Integrate fuel release and the moving-solid infrastructure from M5. Add structural loading, fracture, fragment coupling, and supported material profiles.
5. **Product integration and promotion.** Enable complete supported scenes after numerical, browser, recovery, and performance gates pass. Initially support gas, reactive grains, and solids; liquid-containing scenes remain on the existing model until compressible multiphase coupling is separately validated.

Add these backend-neutral interfaces:

- `SimulationModel = "lowMach" | "compressible"`.
- `materialBreakage: boolean`, included in scene settings, snapshots, and ordered commands.
- Tick-stamped diagnostics for actual simulated time, boundary transfers, chemical/internal/kinetic/elastic energy, and fracture dissipation.

Add a **Material breakage** toggle in Settings, enabled by default for supported scenes. Turning it off prevents new fractures; existing fragments and damage remain. Clearing or reseeding preserves the selected setting.

Extend speed controls with slow motion and accepted-substep inspection. Pressure and density-gradient views show the evolving wave; normal rendering shows gas, flames, and debris without an independent scripted explosion ring.

## Acceptance and verification

- **Numerical references:** shock-tube solution, acoustic propagation, wall reflection, and grid/time refinement.
- **Containment:** sealed gas retains mass and energy; opening a passage causes finite venting; intact walls prevent transmission through solid cells.
- **User scenario:** ignition inside a metal enclosure creates pressure-driven outflow through its opening and a wave beyond it, without relying on the old blast radius.
- **Failure:** below-threshold structures survive; increased loading causes damage; geometry and material strength affect failure; equal pressure on opposing faces does not create a false rupture.
- **Toggle:** disabling breakage prevents new fractures while preserving pressure dynamics and existing fragment motion.
- **Conservation:** include boundary flux, support reactions, chemical release, elastic storage, fracture energy, and brush edits. No unexplained energy or mass creation.
- **CPU/GPU parity:** compare equal physical times, conserved inventories, wave arrival, pressure histories, and damage patterns using precision-appropriate tolerances.
- **Performance:** benchmark the existing 480×270 target, recording frame latency and simulated seconds per wall-second separately. Do not claim real-time performance before measuring it.
- Run the existing checks, build, relevant browser tests, and GPU recovery tests. Update the model documentation to distinguish validated behavior from remaining approximations.


## Stable work-item IDs and future composition

The five delivery stages above map to the following stable IDs in the [roadmap](../../roadmap.md):

| ID | Stage | Required result |
| --- | --- | --- |
| W0 | Baseline and supported-domain contract | Fresh provenance, fixture definitions and model eligibility; no new blast impulse field masquerading as waves |
| W1 | CPU gas reference | Conservative fixed-wall gas, reflection, finite venting and refinement references |
| W2 | GPU parity | Equivalent equations/stage graph, direct rendering, equal-time comparison and retry tests |
| W3 | Reactive sources and breakage | Energy-funded sources, structural loading, damage/fragments; M5 solid/fluid coupling is required |
| W4 | Product acceptance | Persisted breakage toggle, slow motion/substeps, supported scenes, recovery and measurements |

Carry full composition/nuclide signatures with moving solids and fragments according to [the matter contract](../../architecture/matter-model.md). The P3 chemistry pipeline extends those signatures; it must not later require changing how fragment mass is owned. Breaking a body cannot reset its isotope composition or chemical inventory.

Nuclear release introduced in P7 is a distinct source/energy channel, not the legacy chemical blast radius. An approved nuclear source may couple only to a supported EOS/temperature/composition domain. This compressible-gas stage does not itself provide fusion plasma, high-energy radiation hydrodynamics, or arbitrary compressible liquid mixtures.

The original references and baseline claims above are retained from the input. This documentation revision adds stage IDs and integration requirements; it did not run numerical or fracture tests. Use [acceptance](../../validation/acceptance.md) for the WAVE-W4 evidence gate.

## Execution ownership under ADR-001

W0-W4 reuse the [Rust/WASM + wgpu decision](../../architecture/adr-001-rust-wasm-wgpu.md): model contracts under `engine/crates/sim/`, Rust references under `engine/crates/sim-cpu/`, GPU resources under `engine/crates/sim-gpu/`, and wave/solid WGSL stages under `engine/crates/sim-gpu/shaders/`. Retain TypeScript settings, controls and visual presentation intent. Do not create a separate TypeScript compressible engine or a second world to couple to the low-Mach solver.

The [engine boundary](../../architecture/engine-boundary.md) supplies accepted-time accounting, commands, async diagnostics, renderer ownership and snapshots. W4 includes native-reference and actual browser-WASM/GPU evidence, ABI and loss recovery, in addition to numerical wave/breakage tests. The historical closeout statements above are retained evidence, not new source changes made by this documentation revision.
