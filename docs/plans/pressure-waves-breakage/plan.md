# Pressure waves and material breakage after the GPU redesign

Implementation proposal · Particle Foundry · 21 September 2026

Status: planned follow-up; not implemented.

## Goal and placement

Add a follow-up milestone to the existing [GPU redesign plan](../fluid-gpu-redesign/plan.md). Keep its initial fluid demonstrator unchanged.

The new mode should produce expanding gas, travelling pressure waves, reflections, directed flow through openings, and material failure under excessive stress. These effects must come from simulated state and conserved energy.

**Defaults:** material breakage on; physical accuracy takes priority over simulation speed; GPU acceleration targets performance without changing the equations.

## Model and integration

### Compressible gas

- Add a `compressible` model alongside the planned `lowMach` model. Select the model when loading a scene; never run both transport systems over the same state.
- Reuse the planned backend interface, command queue, snapshots, material catalogue, GPU rendering, and diagnostics.
- Store gas species masses, two momentum components, and total energy per cell. Derive temperature and local pressure from composition, internal energy, and available volume.
- Use a conservative finite-volume Euler solver with MUSCL reconstruction, a positivity-preserving HLLE fallback, and acoustic CFL-controlled substeps. Establish a Float64 CPU reference before porting to GPU. [Numerical reference](https://www.clawpack.org/riemann_book/html/Euler_approximate.html).
- Replace chamber-wide pressure equalization, capped pressure impulses, and cellular gas swapping inside this mode. Compression, expansion, venting, and advection share the same face fluxes.
- Reflect flow at intact walls. Open world boundaries exchange finite mass, momentum, and energy with an atmospheric reservoir, recording those transfers.
- Retry invalid substeps with a smaller timestep. If the work budget is exhausted, slow physical progress rather than increasing the timestep or silently clipping pressure.

### Ignition and energy release

- Replace the instantaneous radius-based gunpowder impulse in this mode with finite fuel consumption and local heat/product generation.
- Preserve the existing ignition interaction. Transport heat and reaction products so combustion spreads through the simulated charge.
- Fund all release from the stored chemical inventory. The Blast tool remains an explicitly recorded external energy source.
- Keep reaction parameters documented as sandbox approximations. Do not equate a visually plausible reaction with validated explosive chemistry.

### Material failure

- Calculate pressure traction on exposed surfaces and transmit loads through connected solid material. Failure depends on structural stress, geometry, and material properties—not absolute gas pressure alone.
- Use a bonded solid model with elastic deformation, irreversible damage, and distinct brittle and ductile responses. Start with metal, glass, stone, and wood; unsupported materials must be explicitly identified.
- Catalogue profiles hold stiffness, strength, fracture energy, and temperature dependence. Generic “Metal” receives a documented representative profile rather than an implied universal failure pressure.
- Broken connections release movable fragments. Preserve their mass, temperature, momentum, and remaining energy; account separately for elastic energy, fracture expenditure, and dissipation. Bond damage is an established fracture-model approach. [Reference](https://www.sandia.gov/app/uploads/sites/200/2022/05/3m-sandia-silling-140731.pdf).
- Couple fragment motion back to gas through changing geometry and boundary work, using the redesign’s moving-solid infrastructure.

## Delivery stages and interfaces

1. **Capture the baseline.** Preserve all pre-existing working-tree changes and capture a reproducible baseline. The incomplete `blastEnergyJ` addition from the interrupted implementation attempt was removed during documentation closeout on 21 September 2026; it is not part of this design.
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
