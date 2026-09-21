# Fluid and GPU redesign

Implementation proposal · Particle Foundry · 21 September 2026

**Brief:** Create an implementation plan for the fluid/GPU redesign, using the supplied physics review and executable checks as evidence.

**Recommendation:** Build a small conservative fluid solver on the CPU, then implement the same equations on WebGPU. Keep the existing sandbox available while the new backend gains thermodynamics, granular coupling, and material coverage. Promote the new backend only after numerical, interaction, and performance gates pass.

This task produces a plan and baseline evidence. It does not implement the redesign. Proposed paths and numerical thresholds below are design decisions, not existing capabilities or measured GPU results.

**Planned follow-up:** [Pressure waves and material breakage](../pressure-waves-breakage/plan.md) adds a separate compressible model after the initial GPU demonstrator, with breakage enabled by default and a user toggle. The demonstrator and low-Mach scope below remain unchanged; resolved shocks belong to that follow-up.

## 1. Baseline verified in this workspace

The review describes an earlier source snapshot. Seventeen of its 39 hashed source files differ from the current working tree. The current checkout has substantial pre-existing changes; its base commit is `dd75dde7ebd7e5cc26eff895654992c9d5b91388`. A future implementation must preserve and capture this working state before starting; branching from that commit alone would omit it.

I ran the supplied 35 checks against the current sources: **24 pass and 11 fail**, compared with the report's 20 pass and 15 fail. All 20 original controls still pass. N03, N04, N07, and N10 now pass: liquid falling acquires velocity, cellular transport accounts for gravitational work, hydrostatic transfers close their tested budget, and the tested solid impact closes its budget. These narrow results are not proof of general physical validity.

| Remaining issue | Current evidence | Planned resolution |
| --- | --- | --- |
| Pressure equilibrium and clipped gradients | N01, N02 fail | Compatible MAC operators and boundary tests, M2 |
| Fluid velocity does not determine transport | N05, N06 fail | Shared conservative face fluxes, M2 |
| Phase cycle and open/sealed volume mismatch | N08, N09 fail | Conserved mass/internal energy/actual volume closure, M4 |
| Electrical convergence, heat allocation, terminal work | N11–N13 fail | Small independent prerequisite, M1 |
| Excess acid loses its identity | N14 fails | Species amounts and reaction extents, M6 |
| Tangential gas shear absent | N15 fails | Explicit viscous stress, M2 |

The current entry points confirm the architectural problem. `src/simulation/world.ts` stores one material ID and parcel per cell. `src/simulation/physics.ts` invokes pressure derivation four times, runs cellular gas/liquid motion, and then hydrostatic relocation. `src/physics/motion.ts` still searches liquid regions per sinking grain. `src/simulation/sandbox.ts` exposes synchronous probes and mutations. `src/main.ts` advances whole synchronous ticks at 480 × 270; `src/rendering/renderer.ts` reads CPU fields into ImageData.

Evidence beside this plan includes the unchanged `audit.cjs`, current output, and JSON results with current source hashes. The harness's source-line labels refer to its original snapshot. The JSON separates that fixture reference from current source provenance. Validation used Bun 1.3.12 for CommonJS bundling and Node v26.7.0 for execution; `bun run typecheck` also passed. The original TypeScript compile command is incompatible with the installed TypeScript 7's removed node10 resolution option.

The supplied Linux timings and 58.89% inclusive outlet-search profile remain historical evidence. I have not rerun the benchmark, measured browser latency, or measured GPU performance for this plan.

## 2. Model and ownership decisions

### Two delivery boundaries

**First demonstrator, through M3:** one liquid plus carrier gas, fixed grid-aligned walls, gravity, viscous momentum exchange, conservative volume and passive thermal/species markers, pressure projection, and direct GPU rendering. No phase change, chemistry, explosions, moving solids, or sand in this experimental scene. An unsupported scene stays in the existing backend; it is never partly simulated by both engines.

**Sandbox replacement, through M7:** partial phase amounts, finite venting and chamber pressure, moving solids/sand, additional liquids, material reactions, electrical heat, all existing controls, diagnostics, recovery, and target-device performance. Passing the demonstrator gate does not authorize replacement.

### Conserved state

Keep the current 0.01 m cell width, 0.01 m represented depth, and 1/60 s outer clock. Introduce a versioned `FluidState` independent of the legacy parcel arrays. Each backend has exactly one authoritative copy of its evolving state; the CPU reference and GPU run separate test worlds.

| Field | Location and meaning | Authority |
| --- | --- | --- |
| Solid occupancy and face aperture | Cell volume blocked by solid; open area at each face | Topology state |
| Liquid phase masses and gas species masses | kg per cell; multiple phases may coexist | Fluid state |
| Phase volume fractions | Fraction of whole geometric cell volume; phase fractions plus solid fraction sum to one | Conserved transport, reconciled with thermodynamic closure |
| Internal energy U | J per cell, with a defined phase/species reference zero | Fluid state |
| Face velocity and compatible momentum | u: (W+1)×H; v: W×(H+1), m/s | MAC state |
| Correction pressure π | Cell-centered Pa for mechanical projection | Derived solve output |
| Thermodynamic pressure p0 | Chamber pressure used by EOS and phase closure | Derived from conserved amounts, U, and available volume |
| Temperature, display material, cell velocity | Derived samples; material ID is a visual/interaction label | Read-only views |

Begin with incompressible phase densities and no phase change. Liquid mass and liquid volume are linked by density; do not independently evolve contradictory copies. Later EOS-dependent phase volumes must close the same geometric volume constraint. Never overwrite a phase mass just to normalize fractions.

The new thermodynamic module uses explicit SI internal energy, enthalpy, density, and pressure relations within its documented domain. Existing catalogue coefficients remain calibrated gameplay properties until individually migrated. Current `world.energy` is parcel enthalpy: it cannot simply be renamed U. Conversion must define the reference state and use H = U + pV where the chosen property model supports it, then establish a new ledger baseline. Unsupported legacy phase states must be rejected with a conversion report; no silent energy correction.

One fluid velocity field transports liquid, carrier air, steam/species, and associated energy. Fluxes carry the same phase masses used for momentum transport. The implementation must specify staggered control volumes and dual mass fluxes before coding momentum advection; interpolating an unrelated velocity field is insufficient at a large density contrast.

Basilisk's [MAC implementation](https://basilisk.fr/src/navier-stokes/mac.h) provides a useful staggered-operator reference. Its [VOF implementation](https://basilisk.fr/src/vof.h) illustrates geometric interface transport and tracers sharing phase fluxes. These are references for the proposed design, not a commitment to import their code; review licensing before reusing any implementation.

### Boundaries and pressure

Use the same face apertures, distances, density coefficients, gradient G, and divergence D in assembly and velocity correction:

```text
D [(1/rho_face) G π] = (D u* - S_volume) / dt
u_next = u* - dt (1/rho_face) G π
```

The solver sign convention must produce an SPD matrix on the constrained subspace. Fixed walls impose zero normal velocity; moving walls impose their normal velocity. Free-slip is the demonstrator default; no-slip becomes an explicit boundary option. Atmospheric faces prescribe pressure and account for outgoing/incoming mass and enthalpy. Periodic faces are test-only initially.

Closed components have a pressure nullspace: check RHS compatibility and remove one constant mode per component. Never pin a pressure cell without also checking the requested volume source is compatible with sealed geometry. Apply gravity and hydrostatic pressure through matching face terms so a resting pool stays at rest.

Initially π describes incompressible mechanics with a fixed pressure reference. M4 introduces p0 and defines precisely which mechanical pressure contribution is used for liquid properties. Chamber p0 and π are distinct fields. A uniform sealed pressure must not accelerate fluid into a wall; a prescribed transient pressure gradient is tested at the operator/predictor level. The old N02 gas-step test is not a valid demand for a persistent gradient in a quasi-static chamber.

### Thermodynamics and chamber closure

M4 first solves isolated pure-water vessels from mass, U, and actual available volume. Resolve temperature, pressure, and liquid/vapor amounts together, allowing partial evaporation. A liquid-only sealed vessel also needs finite compressibility or an explicitly unsupported state; perfectly incompressible water in a full rigid heated cell is not enough to determine pressure.

Generate a bounded property table from [IAPWS IF97](https://iapws.org/technical-guidance/release/IF97-Rev). Proposed initial water domain: 273.15–623.15 K and 0.01–20 MPa, covering liquid/vapor equilibrium below the critical point. Validate interpolation against source properties, including near saturation. This subset deliberately excludes ice, extreme fire temperatures, supercritical water, and the report's 318 MPa state. Extending coverage or keeping a scene on the legacy backend is an explicit gate, not arbitrary clamping.

Then add noncondensable carrier air: define an ideal-mixture approximation, vapor partial pressure, and mixture energy. A pure-water saturation lookup using total air-plus-steam pressure is not a mixture closure. Chambers share p0 but may retain local temperature and composition. Rediscover components after geometry/interface changes and reduce extensive quantities; chamber summaries never own duplicate mass or energy.

Use a bounded nonlinear iteration coupling p0, local energy inversion, phase amounts, and total volume. Include phase/thermal volume production in S_volume and chamber compressibility in the sealed compatibility equation. Advance U with pressure work, conduction, and phase-consistent energy fluxes. Closed rigid-vessel tests conserve U; open flow energy tests include enthalpy flux. Track projection/advection numerical energy error separately rather than debiting arbitrary heat to conceal it.

When a wall opens, keep the same EOS and actual volume. Exchange finite mass and energy through the new opening. Do not immediately classify the entire connected interior as atmospheric or reset its pressure. If the low-Mach vent model leaves its supported regime, reject/retry the step or report that the scene requires a different model. Resolved shocks are outside this redesign.

### One substep and its failure policy

1. Apply ordered commands and source terms at a tick boundary; update dependency versions.
2. Rebuild dirty topology, derive properties, and select a stable dt from face speeds, viscosity/diffusion, phase sources, and obstacle motion.
3. Form compatible momentum fluxes and forces; predict face velocities.
4. Solve pressure to a scaled residual; correct face velocities and verify D u = S_volume.
5. Compute geometric phase face fluxes and compatible mass/species/energy fluxes. Cells gather their incident fluxes into next-state buffers.
6. Resolve local energy/phase updates and chamber constraints. If the coupled volume residual is too large, iterate the coupled stage or retry the complete substep at smaller dt.
7. Commit validated buffers; reduce diagnostics and advance simulated time by the accepted dt only.

The implementation must settle predictor/corrector ordering in M2/M4 and use the same ordering in both backends. Failed stages do not partly commit. A maximum substep/solver budget slows or pauses physical progress and reports the reason; it never increases dt beyond the stability bound.

Start geometric VOF with CFL ≤ 0.5 and verify the actual multidimensional scheme. At 3 m/s, the current outer interval spans five cell widths and needs at least ten equal substeps in the one-dimensional example. Also constrain diffusion and phase-volume change; CFL alone is insufficient. The [Basilisk VOF reference](https://basilisk.fr/src/vof.h) documents this scheme-specific bound.

## 3. Milestones and exit gates

### M0 — Freeze evidence and acceptance fixtures

**Dependency:** none. **Primary surface:** tests and benchmark harnesses.

Capture the dirty working tree as a reproducible patch/snapshot, source hashes, tool versions, and command log. Preserve original audit cases and port the invariant assertions into Bun tests with stable IDs. Classify changed-model tests separately from implementation regressions. Run the full existing checks before any code changes; the planning run only performed typechecking and the external audit.

Create backend-neutral fixtures for uniform pressure, gravity-balanced pools, periodic markers, dam break, disconnected pools, narrow channels, and wall edits. Track actual simulated time, cell visits, connected-component rebuilds, pressure solves, and transfers. Benchmark the current 480×270 scenes on this machine before using the historical Linux timings for prioritization.

**Exit:** reproducible baseline and a pass/fail ledger showing the current 24/11 result; the four newly passing cases and all C01–C20 remain protected.

### M1 — State contract, backend seam, and isolated prerequisites

**Dependency:** M0. **Primary surfaces:** proposed `src/simulation/backend.ts`, `src/fluid/state.ts`, `src/simulation/sandbox.ts`, `docs/model.md`; isolated changes in `src/physics/electricity.ts`.

Define array layouts, units, phase/species references, face indexing, boundary configuration, snapshot schema, and command sequencing. Backend selection is per scene: `legacy`, `fluid-cpu`, or `fluid-gpu`. A supported scene declares its material/feature set. Convert scenes once at load; never reconcile a full legacy World with GPU state every tick.

Make a CPU/worker seam before changing rendering assumptions. Commands include a sequence number and intended tick. Probes and statistics return their completed simulation tick; the UI shows the latest completed result without forcing synchronous field access. Keep deterministic command replay and a seeded counter-based random scheme for future granular/reaction events.

As a small separate change, fix N11–N13 using a residual-controlled resistor solve, resistance-weighted edge heat, and each terminal's V×I work. Include exhausted-source re-solving and unpowered/disconnected components. This prerequisite can progress independently of the fluid equations; it does not justify delaying the minimal fluid demonstrator.

Introduce explicit versions for topology, composition/mass, and thermal state. Reuse connectivity only while its geometry version matches. Recompute EOS when mass or U changes; rebuild operator coefficients when density/apertures change. Audit direct typed-array writes and route mutations through tracked stages. Compare cached and forced-rebuild paths in tests before removing any pressure refresh.

**Exit:** the backend contract can host an isolated CPU test world; no two transport owners operate in one scene; electrical analytical fixtures pass; invalidation tests match unconditional recomputation.

### M2 — Conservative CPU fluid reference

**Dependency:** M1 contract. **Primary surfaces:** proposed `src/fluid/cpu/`, `src/fluid/operators.ts`, `tests/fluid/`.

Implement Float64 cell fields and staggered velocities for one liquid and carrier gas. Build matching D/G operators and a matrix-free diagonally preconditioned conjugate-gradient solve. Use a simple relaxation method only as a tiny-grid oracle. Residuals and post-projection divergence determine success; an iteration cap reports failure. Evaluate geometric multigrid as a preconditioner if iterations grow excessively with grid size; [Basilisk's Poisson solver](https://basilisk.fr/src/poisson.h) is a reference, not a drop-in implementation.

Add geometric VOF, conservative phase-associated tracers, compatible momentum transport, and tangential viscous stress. Test fractional occupancy, horizontal transport, ambient-air markers, disconnected topology, and gravity. Retire cellular fluid fall/spread, gas rise, airflow damping, and hydrostatic relocation inside this backend from its first step.

Keep full energy/phase physics disabled here; markers validate transport, not thermodynamic equilibrium. Instrument kinetic energy and numerical dissipation without claiming the legacy enthalpy ledger proves physical closure.

**Exit:** pressure/operator, volume, tracer, shear, hydrostatic-rest, symmetry, and dam-break/refinement tests pass in Float64. The base fluid core has one transport owner and no per-grain search path.

### M3 — WebGPU demonstrator and direct rendering

**Dependency:** M2. **Primary surfaces:** proposed `src/fluid/gpu/`, WGSL kernels, `src/rendering/gpu-renderer.ts`, browser test page.

Implement the same stage graph in f32, using separate dispatches for global dependencies. Face kernels write one flux each; cell kernels gather into their own output. Pressure iterations use ping-pong buffers and GPU reductions. Do not replace the convergent CPU solver with a fixed-count unconverged shader loop.

Keep residual/CFL reductions on the GPU. A bounded batch can include convergence flags and make later iterations no-ops. Read a small completion status before publishing a failed/accepted tick; avoid per-iteration CPU stalls. Test retry and exhaustion explicitly. Inspect actual adapter limits before allocating buffers or choosing workgroup sizes.

Choose the canvas backend before calling getContext: the current app acquires a 2D context immediately. Use a GPU simulation canvas and a separate DOM/2D overlay for the cursor and controls. Render material fractions, temperature marker, pressure, and velocity directly from GPU resources, retaining zoom/pan and point-sampled appearance.

Use a staging-buffer ring for asynchronous probes/reductions and occasional snapshots, with tick IDs and bounded in-flight requests. Mapping live simulation resources is forbidden by the design. WebGPU mapping is asynchronous and mapped buffers cannot be used by the GPU until unmapped; see [GPUBuffer.mapAsync](https://developer.mozilla.org/en-US/docs/Web/API/GPUBuffer/mapAsync). WGSL's available numeric types and synchronization rules require f32 and separate whole-grid stages; see the [WGSL specification](https://www.w3.org/TR/WGSL/).

One 480×270 f32 cell field is 518,400 bytes; a u/v face pair is 1,039,800 bytes. Twenty cell-equivalent fields use approximately 9.89 MiB before ping-pong, solver, multigrid, species, render, and staging resources. Produce an actual allocation manifest; this estimate is not the final memory budget.

**Exit:** CPU/GPU comparisons pass at the same physical times; rendering requires no full-world frame-loop readback; the demonstrator runs at 480×270 and emits end-to-end timings. This is the first usable GPU artifact, not the default sandbox.

### M4 — Closed/open thermodynamics and partial phase state

**Dependency:** M2 state/transport; GPU parity follows M3. **Primary surfaces:** proposed `src/thermodynamics/`, `src/fluid/chambers.ts`, adapters around `src/physics/thermal.ts` and `src/physics/boiling.ts`.

Build the property-table generator with versioned source data and interpolation tests. Implement isolated pure-water volume/energy closure first, then air/steam mixtures, then flow coupling. Remove material-ID toggling as the phase authority; rendering derives appearance from actual phase fractions. Any later nucleation rule changes resolved phase amounts within the same conservation contract.

Track chamber merge/split and vent events without duplicating inventories. GPU topology labeling and segmented reductions must report convergence; do not quietly read back the whole occupancy field each tick. Begin with correct full rebuilds on relevant changes and profile before adding incremental connectivity.

Port each validated closure stage to GPU and repeat energy/volume/property tests. Define supported behavior for fully liquid cavities and table boundaries before enabling heated scenes. Extend below-freezing/high-temperature coverage separately; the demonstrator's narrow table does not cover the current entire catalogue.

**Exit:** fixed-mass/fixed-U/fixed-volume cases converge without a water/steam cycle; a vent has finite, ledgered exchange; gas/liquid volumes fit available geometry; moving-wall work and phase source terms close their documented budgets.

### M5 — Sand, moving solids, and additional liquids

**Dependency:** M2; M4 for heated/phase-changing coupling. **Primary surfaces:** proposed `src/granular/`; retirement seams in `src/physics/motion.ts`, `src/physics/hydrostatics.ts`, and `src/physics/solid-mechanics.ts`.

Replace per-grain outlet searches with a topology snapshot, shared component/outlet-capacity calculation, movement proposals, conflict resolution, and commit. Outlet capacity must be reserved across all accepted grains, not just each destination. Resolve source, destination, and any routing/path conflicts deterministically. If volume cannot be accommodated, defer the move instead of deleting fluid or teleporting it through walls.

Treat accepted solid motion as swept volume/boundary motion coupled to fluid fluxes and pressure. Limit obstacle displacement per substep. Account for fluid/solid momentum, wall reaction, gravitational work, and dissipated energy. Carry all solid metadata through committed moves. This replaces the old displaced-parcel swaps for fluid-owned cells.

Prototype on CPU before GPU. On GPU use integer winner keys/atomics or deterministic reductions; floating-point atomic accumulation is not a baseline dependency. Commit each state entry from one owner. Topology becomes dirty after the batch; cached paths cannot survive in-place mutations.

Add an oil/water two-liquid fixture with explicit phase fractions, density ordering, compatible fluxes, and nonnegative partial masses. Extend material coverage deliberately; do not collapse an immiscible mixture to one material ID.

**Exit:** many grains competing for one opening neither duplicate occupants nor lose water; sealed/no-outlet, narrow-tube, disconnected-pool, sinking, impact, and oil/water tests pass. Instrumented searches scale by batches/regions, not one overlapping traversal per grain. Publish operation counts and timings, not an unproved linear-time claim.

### M6 — Reactions, circuits, and product integration

**Dependency:** M4 and M5 for full-scene coverage. **Primary surfaces:** `src/physics/reactions.ts`, `src/physics/element-reactions.ts`, `src/physics/neutralization.ts`, `src/tools/brush.ts`, `src/app/controls.ts`, `src/simulation/sandbox.ts`, `src/main.ts`.

Transport reactant amounts; use limiting reaction extent and preserve excess reagent identity. Migrate oxygen, fuel, chemical energy, steam production, extinguishing, freezing, and ignition to the new state contract. Maintain source ledgers for painting/removal and energy-limited gameplay blasts. Shock propagation remains unsupported; document the impulse model and its stability limits.

Keep small circuit graphs on CPU initially. Synchronize only changed conductor topology and required thermal/material samples at a defined tick barrier; upload sparse heat events. Melting or conductor motion invalidates the graph before the next electrical solve. If circuits become dense enough that this transfer is no longer sparse, measure and choose a GPU circuit solve or restrict that experimental mode; do not hide full-world synchronization.

A worker owns the simulation scheduler where supported, with bounded command queues and OffscreenCanvas when available. Check adapter availability in the chosen context: [requestAdapter](https://developer.mozilla.org/en-US/docs/Web/API/GPU/requestAdapter) can return null and is available in workers. Provide a main-thread GPU path if the worker/canvas path is unavailable, and a corrected CPU fallback for supported new-model scenes.

Update probes for partial phases, pressure type, and delayed tick-stamped results. Define particle count as occupied matter cells with a stated threshold, separate from total mass; fractional fluids invalidate the old exact material-count interpretation. Port starter scenes via commands. Preserve pause, single-step behavior, speed controls, paint-while-paused, clear/seed, maps, zoom/pan, and cosmetic waves without modifying occupancy.

**Exit:** all intended starter scenes and tools work with explicit feature coverage, no legacy pass mutates fluid-owned state, and chemistry/circuit/source-ledger controls remain valid.

### M7 — Promotion, recovery, and retirement

**Dependency:** all feature and numerical gates for the scenes being promoted.

Run numerical and browser suites, sustained benchmarks, and device-loss tests on intended integrated and discrete GPUs. Record browser/adapter/driver, grid, scene seed, warm-up, samples, substeps, solver residuals, dispatches, memory, reduction/readback bytes, and physical time advanced. Track p50/p95 end-to-end frame time, input-to-command-application latency, and simulated-seconds per wall-second. Optional GPU timestamps supplement host measurements; support is not required for correctness.

Proposed product gate: 480×270 at 1×, p95 displayed frame interval ≤33.3 ms and at least 1.0 simulated second per wall-second on the agreed baseline device. These are targets to validate, not predicted results. Include painting, worst-case sand pools, boiling/venting, disconnected cavities, and dense circuits; a fast ambient frame is insufficient.

On GPU loss, stop accepting new tick commits, preserve the last valid CPU checkpoint and ordered command log, and recover using the corrected CPU model or restart with a clear notice. Never hot-swap partially evolved GPU fields into the legacy parcel engine. Keep backend selection and versioned snapshots reversible through the experimental period.

Promote supported scenes gradually. Remove the legacy fluid branches only after the full feature matrix is covered. Update `docs/model.md`, `README.md`, `docs/elements.md`, and benchmark documentation to the implemented contracts and measured limits.

**Exit:** default backend change is supported by recorded numerical, interaction, performance, and recovery results; remaining unsupported features are visible rather than silently approximated.

## 4. Acceptance matrix

The following are proposed starting gates, not measurements. Freeze fixture scales and thresholds in M0/M2 before optimization. Any later tolerance change needs a numerical rationale, including grid/time refinement, rather than matching the latest output.

| Invariant | CPU Float64 target | GPU f32 target / comparison |
| --- | --- | --- |
| Closed conservative advection, 1,000 outer ticks | Relative liquid mass and marker-total drift ≤1e-10 | ≤1e-5; compare inventories, not bit identity |
| Fraction boundedness | −1e-12 ≤ alpha ≤ 1+1e-12 | −1e-6 ≤ alpha ≤ 1+1e-6; no unledgered clipping |
| Projection | max(dt × abs(Du−S)) ≤1e-8 | ≤1e-5; also report scaled equation residual |
| Uniform sealed pressure and hydrostatic rest | max speed ≤1e-8 m/s after 1,000 ticks | ≤1e-4 m/s; no systematic wallward drift |
| Marker displacement in periodic prescribed flow | Centroid error ≤0.1 cell after one crossing | ≤0.15 cell; phase/species/energy marker totals share flux accounting |
| CPU/GPU one-tick fixture parity | Reference | Volume-fraction mean absolute error ≤1e-5 and scaled velocity RMS error ≤1e-4; refine long-time gates per fixture |
| Isolated phase equilibrium | No cycle; relative U/mass/volume residual ≤1e-9 within table domain | ≤1e-5, plus property interpolation error budget |
| Coupled energy budget | Report U, kinetic, potential, chemical, boundary work/flux and numerical loss separately | Initial closed-fixture drift target ≤1e-4 of a declared physical energy scale; no negative state or hidden heat repair |
| Dam break and refinement | Bounded/conservative; front and center-of-mass converge as dx/dt decrease | Match CPU envelopes at equal physical times; inspect snapshots |
| Sand contention / topology | Exact unique occupancy; mass conservation | Same winning proposals for fixed seed/input; conservative fluid accommodation |
| Circuit analytics | N11–N13 and C17/C18, plus exhaustion/network edits | CPU circuit path retains these gates |
| Device absent/lost, pause and reset | Correct fallback and command ordering | No stale probe presented as current; no partially committed tick |

For totals near zero, each fixture defines absolute tolerances in the same physical units and a nonzero characteristic scale. Use pairwise/compensated CPU totals and hierarchical GPU reductions. Do not normalize thermal error by a huge arbitrary reference enthalpy that conceals meaningful heating error.

Some old tests must be translated. N01 becomes sealed uniform-pressure rest; N02 becomes a manufactured discrete-gradient/predictor test; N05/N06 become displacement and tracer-advection tests; N08/N09 become volume/energy closure and finite venting. Keep the old harness as historical evidence and retain legacy coverage. Mark a model replacement explicitly rather than deleting an inconvenient assertion.

## 5. Risks and decisions

| Risk | Severity | Mitigation / decision |
| --- | --- | --- |
| Mixing calibrated enthalpy with physical U | HIGH | Version units/reference zeros and reject unsupported conversion; validate energy before enabling phases |
| Incompressibility conflicts with evaporation in sealed cells | HIGH | Couple volume source, compressibility, p0 and phase closure; fully liquid case is a required test |
| Density contrast or tiny fluid volumes destabilize projection | HIGH | Compatible mass/momentum fluxes, density coefficients, substeps and residual gates; inspect degenerate cells |
| Geometric VOF conservation is broken by nonzero phase divergence | HIGH | Derive the source-aware update in M4 and test split-stage volume closure; do not reuse zero-divergence proof blindly |
| GPU iterations or component labeling dominate | MED | Batch reductions, measure dispatch count, evaluate multigrid; retain correct CPU reference |
| CPU circuit coupling becomes dense | MED | Measure transfer footprint and topology events; define a supported boundary before promotion |
| Visual behavior changes despite correct conservation | MED | Seeded browser fixtures for pools, boiling, plumes, oil and sand; preserve controls and material cues |
| Device failure loses authoritative fields | MED | Checkpoints, command replay, explicit recovery path; bounded queues |

Defaults for review: preserve falling-sand visual style; prioritize stable pools, boilers, trapped gas, and thermal transport; defer resolved shocks, surface tension, adaptive meshes, and arbitrary moving cut-cell geometry. Start with grid-aligned geometry and substep solid moves. Surface tension and wetting need their own balanced-force tests before addition.

**Decide with the project owner before M4:** adopt the proposed bounded physical water model, or retain a fully calibrated gameplay thermodynamic model. The recommendation is bounded physical water with explicit unsupported-domain behavior while other materials migrate. This changes some existing boiling/heating behavior and cannot be hidden inside a shader port.

**Decide with the project owner before M7:** name the baseline integrated/discrete GPU and browser, approve the performance target, and confirm the material/scene coverage required for default promotion. These choices do not block the CPU reference or first GPU demonstrator.

## 6. Reproduce this planning baseline

From the repository root, use a fresh temporary build directory. The audit exits 1 when its known failing invariants are reproduced.

```powershell
$auditBuild = Join-Path $env:TEMP ('particle-foundry-audit-' + [guid]::NewGuid().ToString('N'))
$auditModules = [ordered]@{
  'world' = 'src/simulation/world.ts'
  'physics' = 'src/simulation/physics.ts'
  'gas-dynamics' = 'src/physics/gas-dynamics.ts'
  'solid-mechanics' = 'src/physics/solid-mechanics.ts'
  'motion' = 'src/physics/motion.ts'
  'fluid-solver' = 'src/physics/fluid-solver.ts'
  'reactions' = 'src/physics/reactions.ts'
  'electricity' = 'src/physics/electricity.ts'
  'element-reactions' = 'src/physics/element-reactions.ts'
  'neutralization' = 'src/physics/neutralization.ts'
  'airflow' = 'src/physics/airflow.ts'
  'thermal' = 'src/physics/thermal.ts'
  'diagnostics' = 'src/simulation/diagnostics.ts'
  'materials' = 'src/materials/index.ts'
  'physical-scale' = 'src/simulation/physical-scale.ts'
}
bun run typecheck
foreach ($auditModule in $auditModules.GetEnumerator()) {
  bun build $auditModule.Value --target=node --format=cjs --outfile (Join-Path $auditBuild ($auditModule.Key + '.js'))
  if ($LASTEXITCODE -ne 0) { throw "Audit bundle failed: $($auditModule.Key)" }
}
Set-Content -LiteralPath (Join-Path $auditBuild 'package.json') -Value '{"type":"commonjs"}'
node docs/plans/fluid-gpu-redesign/audit.cjs $auditBuild (Join-Path $auditBuild 'results.json')
```

The source tree was reorganized after this baseline was captured. Paths in the narrative and commands above follow the new layout; recorded source hashes, audit locations, and outputs retain their historical provenance.

The raw harness emits historical archive labels even when run against another tree. Use the source hashes and corrected provenance in `current-audit-results.json` for this run; capture fresh hashes for subsequent runs. Full implementation validation additionally requires `bun run check`, relevant lint/build checks, numerical fixtures, and browser/GPU tests on actual hardware.
