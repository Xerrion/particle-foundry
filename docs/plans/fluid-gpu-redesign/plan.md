# Fluid and GPU redesign

Implementation plan · Particle Foundry · scope revised 27 September 2026

**Phase:** P1, the sole active release scope. Start at [the developer entrypoint](../../START_HERE.md). Complete M0-M7 against the mandatory [current-sandbox scope](../rust-wasm-migration/current-sandbox-scope.md). P2-P9 are deferred; completing P1 does not activate them. This document retains the numerical requirements for a correct migration.

**Selected implementation:** retain the TypeScript frontend, implement the conservative CPU reference in Rust f64, compile the browser engine/bridge to WASM, and execute equivalent WGSL/f32 stages plus direct rendering through Rust wgpu. Keep legacy scenes available while the new backend gains thermodynamics, granular coupling and material coverage. Promote only after numerical, interaction and measured performance gates pass. [ADR-001](../../architecture/adr-001-rust-wasm-wgpu.md) supersedes the previous TypeScript-reference default.

The [E00-E14 migration work packages](../rust-wasm-migration/plan.md) execute M0-M7 and add workspace, bindings, catalogue projection, async lifecycle, renderer and recovery work. The [engine boundary](../../architecture/engine-boundary.md) is binding for integration. Do not create a separate line-by-line Rust port before the numerical redesign. The [fire integration plan](../fire-combustion/plan.md) and [FIRE-A acceptance](../../validation/fire-combustion.md) now make combustion requirements explicit inside M0/M1/M4/M6/M7; M3 remains nonreactive.

This is a planning document with preserved historical evidence: the [September 2026 static source review](../../validation/source-review-2026-09-21.md) and the separately executed 12-check [fire audit](../../evidence/fire-review-2026-09-23/FIRE_REVIEW.md). Current implementation work uses the live checkout, including uncommitted changes; no source archive is required. This plan does not implement the redesign or establish current audit or Rust/GPU performance results. Proposed paths and numerical thresholds are design decisions, not existing capabilities.

**Deferred:** [Pressure waves and material breakage](../pressure-waves-breakage/plan.md), new material/chemistry coverage, nuclear behavior and optional platform deployments require separate activation. Technical feasibility after M3 does not authorize experiments on those features. Preserve the existing bounded Blast/gunpowder interaction in P1; resolved shocks and fracture remain deferred.

## 1. Preserved baseline from the supplied documentation

The audit results in this section describe earlier snapshots, not the current checkout. The [source review and hashes](../../validation/source-review-2026-09-21.md) identify a separate historical inspection. Inspect the live repository at M0 and record new evidence against its Git commit and working-tree changes.

The review describes an earlier source snapshot. Seventeen of its 39 hashed source files differ from the audited working tree. The audited checkout had substantial pre-existing changes; its base commit is `dd75dde7ebd7e5cc26eff895654992c9d5b91388`. A future implementation must preserve and capture this working state before starting; branching from that commit alone would omit it.

The supplied audit records 35 checks against its source snapshot: **24 pass and 11 fail**, compared with the earlier report's 20 pass and 15 fail. All 20 original controls still pass. N03, N04, N07, and N10 now pass: liquid falling acquires velocity, cellular transport accounts for gravitational work, hydrostatic transfers close their tested budget, and the tested solid impact closes its budget. These narrow results are not proof of general physical validity.

| Remaining issue | Historical audit evidence | Planned resolution |
| --- | --- | --- |
| Pressure equilibrium and clipped gradients | N01, N02 fail | Compatible MAC operators and boundary tests, M2 |
| Fluid velocity does not determine transport | N05, N06 fail | Shared conservative face fluxes, M2 |
| Phase cycle and open/sealed volume mismatch | N08, N09 fail | Conserved mass/internal energy/actual volume closure, M4 |
| Electrical convergence, heat allocation, terminal work | N11-N13 fail | Small independent prerequisite, M1 |
| Excess acid loses its identity | N14 fails | Species amounts and reaction extents, M6 |
| Tangential gas shear absent | N15 fails | Explicit viscous stress, M2 |

The audited entry points illustrate the architectural problem. `src/simulation/world.ts` stores one material ID and parcel per cell. The supplied source has four unconditional pressure derivations per tick when the nested call inside `gas.step()` is included, plus a possible fifth after venting; it also runs cellular gas/liquid motion and hydrostatic relocation. [Exact call sites](../../validation/source-review-2026-09-21.md#pressure-count-clarification). `src/physics/motion.ts` still searches liquid regions per sinking grain. `src/simulation/sandbox.ts` exposes synchronous probes and mutations. `src/main.ts` advances whole synchronous ticks at 480 × 270; `src/rendering/renderer.ts` reads CPU fields into ImageData.

Evidence beside this plan includes the unchanged `audit.cjs`, `current-audit-output.txt`, and JSON results. Here, filenames containing "current" mean current at the earlier audit, not at this revision. The harness's source-line labels and JSON source provenance belong to that historical snapshot. The earlier documentation reports validation with Bun 1.3.12 for CommonJS bundling and Node v26.7.0 for execution, and a passing `bun run typecheck`. It also reports a TypeScript 7 node10-resolution incompatibility. This revision did not repeat or independently verify those environment-specific claims; inspect the real checkout at E00.

The supplied Linux timings and 58.89% inclusive outlet-search profile remain historical evidence. Neither the original planning evidence nor this documentation revision provides a new browser/GPU benchmark. This revision did not rerun the application checks.

### Additional preserved fire evidence

The [fire review](../../validation/fire-combustion.md) reproduced boundary-O2 replenishment and cold-FIRE ignition through full physics ticks. It also found species/product inconsistency, heated-air flame proxies, timed smoke identity loss, contact-only water quenching and incompatible fuel-specific oxygen rules. F01-F12 are characterization records, not acceptance of those behaviors. Preserve the useful funded brush and conservation controls; replace the faulty contracts through FIRE-W00-W07.

## 2. Model and ownership decisions

### Two delivery boundaries

**First demonstrator, through M3:** one liquid plus carrier gas, fixed grid-aligned walls, gravity, viscous momentum exchange, conservative volume and passive thermal/species markers, pressure projection, and direct GPU rendering. No phase change, chemistry, explosions, moving solids, or sand in this experimental scene. An unsupported scene stays in the existing backend; it is never partly simulated by both engines.

**Sandbox replacement, through M7:** partial phase amounts, finite venting and chamber pressure, moving solids/sand, additional liquids, material reactions, electrical heat, all existing controls, diagnostics, recovery, and target-device performance. Passing the demonstrator gate does not authorize replacement.

### Conserved state

Keep the current 0.01 m cell width, 0.01 m represented depth, and 1/60 s outer clock. Introduce a versioned Rust-owned `FluidState` contract independent of the legacy parcel arrays. A CPU backend owns Rust arrays; a GPU backend owns wgpu buffers, not a fully current duplicate Rust world. Reference and GPU tests use separate instances. Checkpoints are occasional snapshots, not a second ticking authority.

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

Generate a bounded property table from [IAPWS IF97](https://iapws.org/technical-guidance/release/IF97-Rev). Proposed initial water domain: 273.15-623.15 K and 0.01-20 MPa, covering liquid/vapor equilibrium below the critical point. Validate interpolation against source properties, including near saturation. This subset deliberately excludes ice, extreme fire temperatures, supercritical water, and the report's 318 MPa state. Extending coverage or keeping a scene on the legacy backend is an explicit gate, not arbitrary clamping.

The fire audit identifies a 450 C legacy emitted-flame setting, above this table's approximately 350 C ceiling. FIRE-W02 / FIRE-A12 requires explicit hot-scene property support before promoting that fire/water combination. Extending a lookup range requires source/interpolation/domain validation; retaining a number from the legacy catalogue is not validation. Runtime source-driven domain exits must roll back or report unsupported conditions without clamping energy/temperature.

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

Start geometric VOF with CFL ≤ 0.5 and verify the actual multidimensional scheme. At 3 m/s, the current outer interval spans five cell widths and needs at least ten equal substeps in the one-dimensional example. Also constrain diffusion and phase-volume change; CFL alone is insufficient. M6 adds reaction depletion, heat-release and fuel-release/volume-source constraints. Sources use rates per accepted physical time; a 1/60-second legacy per-tick release must not repeat unscaled on every new substep. Source rejection rolls back reactants, products, energy and commands together. The [Basilisk VOF reference](https://basilisk.fr/src/vof.h) documents this scheme-specific bound.

## 3. Milestones and exit gates

### M0  -  Freeze evidence and acceptance fixtures

**Dependency:** none. **Primary surface:** tests and benchmark harnesses.

Capture the current working tree with tracked and untracked content, Git commit, source hashes, tool versions and command log. Preserve the original Bun/TypeScript regression coverage and translate backend-neutral invariant assertions into Rust reference fixtures with stable IDs. Classify changed-model tests separately from regressions. Inspect the current mise tasks, lockfile and test infrastructure before running checks. E00 establishes fresh evidence from the checkout; archived upload metadata does not determine which tools or tests exist today.

Create backend-neutral fixtures for uniform pressure, gravity-balanced pools, periodic markers, dam break, disconnected pools, narrow channels, and wall edits. Track actual simulated time, cell visits, connected-component rebuilds, pressure solves, and transfers. Benchmark the current 480×270 scenes on this machine before using the historical Linux timings for prioritization.

Execute FIRE-W00: preserve the original F01-F12 report/runners/results and record fresh reproduction separately. Translate the two full-pipeline bugs into distinct corrected assertions; keep F03/F11/F12 and the useful F04 scalar-energy check as controls without accepting F04's wrong composition. Freeze FIRE-A fixtures and tolerances, keeping the F06 water observation explicitly reaction-only. Do not require a complete TypeScript fire rewrite first.

**Exit:** reproducible fresh baseline with a pass/fail ledger compared to the historical 24/11 result; protect the controls and record any differences against the actual checkout. Never force a changed source tree to reproduce old totals.

### M1  -  State contract, backend seam, and isolated prerequisites

**Dependency:** M0. **Primary surfaces:** proposed `engine/crates/sim/`, `engine/crates/sim-cpu/`, `engine/crates/wasm/`, the existing `web/src/engine-client/` entrypoint and `web/src/legacy/simulation/sandbox.ts`, generated catalogue/binding outputs, and Rust/build configuration under `engine/`. Execute E01-E04.

Bootstrap the native Rust and browser WASM toolchain, pin compatible dependencies and generate bindings consumed by the existing frontend build. Establish a real browser init/dispose smoke test before the solver port. Keep the TypeScript UI and source layout; a workspace reorganization or Rust GUI is not a prerequisite.

Define array layouts, units, phase/species references, face indexing, boundary configuration, snapshot schema, and command sequencing. Backend selection is per scene. Retain `legacy`, `fluid-cpu` and `fluid-gpu` as UI/compatibility labels; map new sessions to execution `cpu-reference` or `wgpu` plus a separate physical model and actual device backend. A supported scene declares its material/feature set. Convert scenes once at load; never reconcile a full legacy World with GPU state every tick.

Before freezing this layout, implement the minimal [matter identity contract](../../architecture/matter-model.md): distinct element, nuclide, species, material-form and runtime-component IDs; stable snapshot references; active-component indexing; source/domain metadata; and an optional versioned nuclear extension that is not allocated when disabled. Preserve legacy material IDs. Test the seam with the demonstrator's small component set; do not build full chemistry or nuclide populations in M1. Unknown properties must not compile as zero. The later P3 pipeline expands this contract rather than redesigning the world again.

FIRE-W01 defines minimal fuel/O2/product and supported physical soot/residue component identities, one energy-reference convention, shared open/closed face-flux semantics and derived flame/smoke outputs. No independent spendable oxygen counter or authoritative FIRE identity is allowed. Test schema/serialization and component closure now, while active reactions stay disabled in M3.

Make a Rust/WASM command and observation seam before changing rendering assumptions. Use a main-thread browser deployment for the first demonstrator; preserve the scheduler-independent contract while worker deployment remains deferred. Commands include a sequence number and intended tick. Probes and statistics return their completed simulation tick; the UI shows the latest completed result without forcing synchronous field access. Keep deterministic command ordering and a seeded counter-based random scheme for future granular/reaction events. Record replay provenance, but do not promise bit-identical numerical trajectories across CPU/GPU devices or backends.

As an independent M1 branch (E04), implement the corrected Rust CPU circuit reference for N11-N13 using a residual-controlled resistor solve, resistance-weighted edge heat and each terminal's V x I work. Include exhausted-source re-solving and unpowered/disconnected components. Do not require a duplicate TypeScript repair before the Rust implementation. The M1 state-contract subset gates M2; circuit completion gates M6 integration, not the minimal fluid demonstrator.

Introduce explicit versions for topology, composition/mass, and thermal state. Reuse connectivity only while its geometry version matches. Recompute EOS when mass or U changes; rebuild operator coefficients when density/apertures change. Audit direct typed-array writes and route mutations through tracked stages. Compare cached and forced-rebuild paths in tests before removing any pressure refresh.

**Exit:** native/WASM bootstrap and bindings smoke tests pass; the backend contract hosts an isolated Rust CPU test world; async facade/ID/snapshot contracts and invalidation tests pass; no two transport owners operate in one scene. E04 electrical analytical fixtures complete the independent branch and are mandatory before M6. M2 may start after the contract branch without waiting for E04.

### M2  -  Conservative CPU fluid reference

**Dependency:** M1 state/facade contract, E03. **Primary surfaces:** proposed `engine/crates/sim/src/fluid/`, `engine/crates/sim-cpu/src/fluid/`, Rust integration tests and shared fixtures. Execute E05-E06.

Implement Rust f64 cell fields and staggered velocities for one liquid and carrier gas. Do not first implement another TypeScript reference. Build matching D/G operators and a matrix-free diagonally preconditioned conjugate-gradient solve. Use a simple relaxation method only as a tiny-grid oracle. Residuals and post-projection divergence determine success; an iteration cap reports failure. Evaluate geometric multigrid as a preconditioner if iterations grow excessively with grid size; [Basilisk's Poisson solver](https://basilisk.fr/src/poisson.h) is a reference, not a drop-in implementation.

Add geometric VOF, conservative phase-associated tracers, compatible momentum transport, and tangential viscous stress. Test fractional occupancy, horizontal transport, ambient-air markers, disconnected topology, and gravity. Retire cellular fluid fall/spread, gas rise, airflow damping, and hydrostatic relocation inside this backend from its first step.

Keep full energy/phase physics disabled here; markers validate transport, not thermodynamic equilibrium. Instrument kinetic energy and numerical dissipation without claiming the legacy enthalpy ledger proves physical closure.

**Exit:** pressure/operator, volume, tracer, shear, hydrostatic-rest, symmetry, and dam-break/refinement tests pass in Float64. The base fluid core has one transport owner and no per-grain search path.

### M3  -  WebGPU demonstrator and direct rendering

**Dependency:** M2. **Primary surfaces:** proposed `engine/crates/sim-gpu/`, `engine/crates/sim-gpu/shaders/fluid/`, `engine/crates/sim-gpu/shaders/render/`, `engine/crates/wasm/` and the existing TypeScript host adapter. Execute E07-E08.

Use one Rust/wgpu device for compute and direct presentation. Choose the renderer before the app acquires a canvas context, or replace the canvas and rebind controls. The current Canvas-2D-first startup cannot be retained unchanged. [Canvas ownership](../../architecture/engine-boundary.md#8-canvas-and-renderer-migration).

Initialization, probes and snapshots use asynchronous owned requests. Command/advance receipts describe admission/submission, not completed GPU work. Bound in-flight batches, expose accepted tick/time, and prevent failed candidate states from being published. Rust controls resource handles; it does not mirror the full GPU world in WASM memory.

Implement the same stage graph in f32, using separate dispatches for global dependencies. Face kernels write one flux each; cell kernels gather into their own output. Pressure iterations use ping-pong buffers and GPU reductions. Do not replace the convergent CPU solver with a fixed-count unconverged shader loop.

Keep residual/CFL reductions on the GPU. A bounded batch can include convergence flags and make later iterations no-ops. Read a small completion status before publishing a failed/accepted tick; avoid per-iteration CPU stalls. Test retry and exhaustion explicitly. Inspect actual adapter limits before allocating buffers or choosing workgroup sizes.

Choose the canvas backend before calling getContext: the current app acquires a 2D context immediately. Use a GPU simulation canvas and a separate DOM/2D overlay for the cursor and controls. Render material fractions, temperature marker, pressure, and velocity directly from GPU resources, retaining zoom/pan and point-sampled appearance.

Use a staging-buffer ring for asynchronous probes/reductions and occasional snapshots, with tick IDs and bounded in-flight requests. Mapping live simulation resources is forbidden by the design. WebGPU mapping is asynchronous and mapped buffers cannot be used by the GPU until unmapped; see [GPUBuffer.mapAsync](https://developer.mozilla.org/en-US/docs/Web/API/GPUBuffer/mapAsync). This plan selects portable f32 for physics and separate dispatches for whole-grid dependencies; see the [WGSL specification](https://www.w3.org/TR/WGSL/).

One 480×270 f32 cell field is 518,400 bytes; a u/v face pair is 1,039,800 bytes. Twenty cell-equivalent fields use approximately 9.89 MiB before ping-pong, solver, multigrid, species, render, and staging resources. Produce an actual allocation manifest; this estimate is not the final memory budget.

**Exit:** CPU/GPU comparisons pass at the same physical times; rendering requires no full-world frame-loop readback; the demonstrator runs at 480×270 with existing painting, pause and reset controls, controlled API/test-harness stepping, and end-to-end timings. This completes the first experimental checkpoint. All current-sandbox coverage is still required before the default switch.

### M4  -  Closed/open thermodynamics and partial phase state

**Dependency:** M2 state/transport; GPU parity follows M3. **Primary surfaces:** proposed Rust thermal/chamber contracts and references in `engine/crates/sim/` and `engine/crates/sim-cpu/`, wgpu pipelines in `engine/crates/sim-gpu/`, and `engine/crates/sim-gpu/shaders/thermal/`. Legacy `src/physics/thermal.ts` and `src/physics/boiling.ts` supply regression context, not the new state owner. Execute E09.

Build the property-table generator with versioned source data and interpolation tests. Implement isolated pure-water volume/energy closure first, then air/steam mixtures, then flow coupling. Remove material-ID toggling as the phase authority; rendering derives appearance from actual phase fractions. Any later nucleation rule changes resolved phase amounts within the same conservation contract.

Track chamber merge/split and vent events without duplicating inventories. GPU topology labeling and segmented reductions must report convergence; do not quietly read back the whole occupancy field each tick. Begin with correct full rebuilds on relevant changes and profile before adding incremental connectivity.

Port each validated closure stage to GPU and repeat energy/volume/property tests. Define supported behavior for fully liquid cavities and table boundaries before enabling heated scenes. Extend below-freezing/high-temperature coverage as required by the current-sandbox checklist, including existing elemental phase forms, before their M4-M6 rows can pass. The demonstrator's narrow table does not cover the current catalogue. Rejection is required during development but does not complete a required row.

Execute FIRE-W02 alongside E09. Validate passive O2/product transport with a finite vent and closed-boundary controls, and declare the full reactant/product/water property domains for planned fire scenes. Pass FIRE-PRECONDITIONS (FIRE-A03 and FIRE-A12) before their reactive counterparts are enabled. Do not reset interior species by connectivity or claim the narrow water table supports hot flames.

**Exit:** FIRE-PRECONDITIONS passes; fixed-mass/fixed-U/fixed-volume cases converge without a water/steam cycle; a vent has finite, ledgered exchange; gas/liquid volumes fit available geometry; moving-wall work and phase source terms close their documented budgets.

### M5  -  Sand, moving solids, and additional liquids

**Dependency:** M2; M4 for heated/phase-changing coupling. **Primary surfaces:** proposed Rust granular/solid references in `engine/crates/sim-cpu/`, wgpu pipelines and `engine/crates/sim-gpu/shaders/granular/`; retirement seams in legacy `src/physics/motion.ts`, `src/physics/hydrostatics.ts`, and `src/physics/solid-mechanics.ts`. Execute E10.

Replace per-grain outlet searches with a topology snapshot, shared component/outlet-capacity calculation, movement proposals, conflict resolution, and commit. Outlet capacity must be reserved across all accepted grains, not just each destination. Resolve source, destination, and any routing/path conflicts deterministically. If volume cannot be accommodated, defer the move instead of deleting fluid or teleporting it through walls.

Treat accepted solid motion as swept volume/boundary motion coupled to fluid fluxes and pressure. Limit obstacle displacement per substep. Account for fluid/solid momentum, wall reaction, gravitational work, and dissipated energy. Carry all solid metadata through committed moves. This replaces the old displaced-parcel swaps for fluid-owned cells.

Prototype on CPU before GPU. On GPU use integer winner keys/atomics or deterministic reductions; floating-point atomic accumulation is not a baseline dependency. Commit each state entry from one owner. Topology becomes dirty after the batch; cached paths cannot survive in-place mutations.

Add an oil/water two-liquid fixture with explicit phase fractions, density ordering, compatible fluxes, and nonnegative partial masses. Extend material coverage deliberately; do not collapse an immiscible mixture to one material ID.

**Exit:** many grains competing for one opening neither duplicate occupants nor lose water; sealed/no-outlet, narrow-tube, disconnected-pool, sinking, impact, and oil/water tests pass. Instrumented searches scale by batches/regions, not one overlapping traversal per grain. Publish operation counts and timings, not an unproved linear-time claim.

### M6  -  Reactions, circuits, and product integration

**Dependency:** M4 and M5 for full-scene coverage, plus the E04 circuit-reference branch. **Primary surfaces:** Rust reaction/circuit contracts and bounded CPU algorithms, wgpu source pipelines and WGSL chemistry, `engine/crates/wasm/`, and the retained TypeScript tool/control/sandbox/main adapters. Legacy reaction modules are source/regression context, not the GPU scene owner. Execute E11-E12.

Transport reactant amounts; use limiting reaction extent and preserve excess reagent identity. Replace inconsistent legacy combustion rather than migrating its rules unchanged. Follow FIRE-W03-FIRE-W05 in E11: a corrected Rust fuel-vapor/O2 reference with declared products, shared-reactant reservations, admissible ignition/extinction and physical-time rates; bounded condensed-fuel release for every promoted wood/oil/plant scene; thermal/amount-based water suppression; and persistent supported smoke/soot inventories. Implement equivalent WGSL only after the corresponding reference gates pass.

Repeat the M4 ventilation and hot-domain fixtures with reactions enabled. Fuel/O2 debits, products, source energy and phase/volume closure form one accepted transaction; negative inventories, double oxygen spending or partial failed-step commits are not permitted. A cold FIRE label cannot ignite, a connected interior cannot receive free O2, and cosmetic smoke expiry cannot erase composition. Preserve ledgered Fire-brush inputs and correctly funded transfers.

FIRE-W06 in E12 derives visible flames from reacting/hot gas and physical smoke/soot, preserving supported particle-based presentation without a disconnected overlay or second fire simulation. Probes expose actual composition and heat-release state. Quality settings do not affect physical inventories.

Maintain source ledgers for painting/removal and energy-limited gameplay blasts. Freezing/phase behavior follows M4's supported domains. Shock propagation remains unsupported; document the impulse model and its stability limits. FIRE-W08 and the P4 fuel-specific chemistry, char, soot and radiative-transfer extensions are deferred. Foundational correctness for existing fuels remains mandatory in P1.

Keep small circuit graphs in Rust CPU code initially, reusing the corrected E04 reference. Synchronize only changed conductor topology and required thermal/material samples at a defined tick barrier; upload sparse heat events. Melting or conductor motion invalidates the graph before the next electrical solve. If circuits become dense enough that this transfer is no longer sparse, measure and choose a GPU circuit solve or restrict that experimental mode; do not hide full-world synchronization.

Keep the main-thread asynchronous wgpu path through E12. Worker/OffscreenCanvas deployment, threads and shared-memory optimization are deferred. Use a corrected Rust CPU fallback only for compatible, validated scenes; retain legacy TypeScript/Canvas for legacy scenes. WebGL2 cannot execute the WGSL compute backend. [Browser/fallback contract](../../architecture/adr-001-rust-wasm-wgpu.md#browser-and-fallback-policy).

Update probes for partial phases, pressure type, and delayed tick-stamped results. Define particle count as occupied matter cells with a stated threshold, separate from total mass; fractional fluids invalidate the old exact material-count interpretation. Port starter scenes via commands. Preserve pause, controlled API stepping, speed controls, paint-while-paused, clear/seed, maps, zoom/pan, and cosmetic waves without modifying occupancy. The current UI has no single-step button/key; adding one remains deferred.

Migrate all eight current elemental models and their existing products/interactions here, with property/phase prerequisites in M4 and motion in M5. Include current acid/base presets, funded plant growth and the bounded gunpowder/Blast behavior. The deferred P3/P4 general data and expanded reaction work is not a prerequisite; add only the tables and source stages needed for the current-sandbox checklist. P4/C3 later revalidates this migrated coverage under expanded data contracts.

**Exit:** FIRE-M6 (FIRE-A01-FIRE-A13) and the current-sandbox reaction/circuit/source/control rows pass. All current starter-scene, material and tool capabilities have evidence; no legacy pass mutates GPU-owned state. Missing current interactions keep M6 incomplete.

### M7  -  Promotion, recovery, and retirement

**Dependency:** all implementation, feature and numerical gates for every required row of the current-sandbox scope, including M6 and E04 circuits.

Run numerical and browser suites, sustained benchmarks, and device-loss tests on intended integrated and discrete GPUs. Record browser/adapter/driver, grid, scene seed, warm-up, samples, substeps, solver residuals, dispatches, memory, reduction/readback bytes, and physical time advanced. Track p50/p95 end-to-end frame time, input-to-command-application latency, and simulated-seconds per wall-second. Optional GPU timestamps supplement host measurements; support is not required for correctness.

Proposed product gate: 480×270 at 1×, p95 displayed frame interval ≤33.3 ms and at least 1.0 simulated second per wall-second on the agreed baseline device. These are targets to validate, not predicted results. Include painting, worst-case sand pools, boiling/venting, disconnected cavities, and dense circuits; a fast ambient frame is insufficient.

On GPU loss, stop accepting new tick commits, cancel pending readbacks and preserve the last valid committed checkpoint plus command log. The newest device state may be unavailable. Recover only through a compatible validated Rust CPU or restored wgpu backend, with an explicit checkpoint rollback notice, or restart with a clear notice. Replay restores commands, not a promise of bit-identical cross-device trajectories. Never hot-swap partially evolved GPU fields into the legacy parcel engine. Keep backend selection and versioned snapshots reversible through the experimental period.

FIRE-W07 requires FIRE-M7 (FIRE-A01-FIRE-A14), including network/component persistence, failure injection, replay, derived-visual independence and measured transfers for every promoted fire scene. Include closed-O2, vented burning, ignition-only, water-suppression and persistent-smoke coverage where claimed. Historical characterization is not a substitute for corrected hardware/reference evidence.

Expose passing subsets experimentally during development. The default switch requires every current-sandbox scope row; no current capability may be left legacy-only or relabelled optional to pass M7. Retain reversible legacy access until rollout is proven. Remove legacy branches only after their full required feature matrix is covered. Update `docs/model.md`, `README.md`, `docs/elements.md`, and benchmark documentation to the implemented contracts and measured limits.

**Exit / GPU-M7:** every required current-sandbox row has recorded numerical, browser interaction, performance and recovery evidence on declared targets; FIRE-M7 passes for required combustion scenarios. Unrun/failed checks or missing current features keep P1 incomplete. Future expansion remains deferred after this gate.

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
| Circuit analytics | N11-N13 and C17/C18, plus exhaustion/network edits | CPU circuit path retains these gates |
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

**Record before M4:** use the proposed bounded physical water model as the provisional development default; document any explicit project decision to retain a calibrated gameplay model instead. The recommendation is bounded physical water with explicit unsupported-domain behavior while other materials migrate. This changes some existing boiling/heating behavior and cannot be hidden inside a shader port.

**Freeze per-row fixtures before implementation:** the [current-sandbox scope](../rust-wasm-migration/current-sandbox-scope.md) now fixes the replacement feature inventory. Define named scenes, expected observables, tolerances and intentional numerical corrections before implementing each row. Record the baseline browser/device targets and measurement protocol before M3 benchmarking, and keep them fixed for M7 comparisons. Do not reduce coverage or relax thresholds to fit results. Any removal of current functionality requires an explicit user scope decision. Future P2-P9 content is not part of this release.

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
mise run typecheck
foreach ($auditModule in $auditModules.GetEnumerator()) {
  mise exec -- bun build $auditModule.Value --target=node --format=cjs --outfile (Join-Path $auditBuild ($auditModule.Key + '.js'))
  if ($LASTEXITCODE -ne 0) { throw "Audit bundle failed: $($auditModule.Key)" }
}
Set-Content -LiteralPath (Join-Path $auditBuild 'package.json') -Value '{"type":"commonjs"}'
mise exec -- node docs/plans/fluid-gpu-redesign/audit.cjs $auditBuild (Join-Path $auditBuild 'results.json')
```

The source tree was reorganized after this baseline was captured. Paths in the narrative and commands above follow the new layout; recorded source hashes, audit locations, and outputs retain their historical provenance.

The raw harness emits historical archive labels even when run against another tree. Use the source hashes and corrected provenance in `current-audit-results.json` for this run; capture fresh hashes for subsequent runs. Full implementation validation additionally requires `mise run ci`, numerical fixtures, and browser/GPU tests on actual hardware.


## 7. Forward-compatible composition without widening M3

The deferred [full roadmap](../../roadmap.md) retains targets of 40, then 83 elemental-material identities, followed by 20 nuclear-first and 15 exotic identities. Those counts are not per-cell field counts and not M3 deliverables. The registry already contains 118 names; support remains explicit per form, mechanism, backend, and domain.

Use a compact global catalogue and a scene-local active-component layout. Component masses, associated phase volumes, energy and nuclide signatures follow the same accepted face fluxes. Default signatures can remain fixed and radioactive evolution off until P6. Derived display labels and aggregate inventories do not own a second amount store.

At 480 x 270, 118 dense f32 component quantities require 61,171,200 bytes, about 58.34 MiB, for one quantity in one buffer generation. Do not preallocate all elements, phases and nuclides densely by default. Begin with a small active-set SoA; introduce tiled sparse pages only after measurement. Pack tables within queried adapter limits. The chosen maximum component count and any capacity failure are visible capabilities, not a reason to delete material.

Compile required reaction/decay products into the active set before enabling a network. Expansion or remapping is an ordered transaction at a safe boundary with snapshot-version handling. Failed source or allocation updates cannot partly commit. Do not hard-code chemistry, isotope, or phase identity into a rendering colour or material ID.

In M6, amount-based existing reactions use this state contract but do not imply the entire P4/P5 chemistry scope is delivered. Nuclear modules introduced later own source calculations, not a second fluid transport loop. All energy/composition events have stable IDs and are applied exactly once by the owning backend.

Execution labels `fluid-cpu` and `fluid-gpu` map to Rust CPU and Rust/wgpu implementations of the low-Mach model; browser wgpu uses WebGPU. Physical model and actual device backend remain distinct fields. Deferred P2 would add a different `compressible` physical model through the same backend boundary if activated. Do not conflate physical-model selection with CPU/GPU selection.

Save data must include stable component definitions, catalogue/data versions, accepted physical time, source ledgers and optional extension versions. GPU loss recovery uses a consistent committed checkpoint and ordered command replay; unsupported extensions are rejected rather than discarded.

**Additional M1/M3 acceptance:** identity round trip, independent material/element IDs, inactive nuclear extension with no allocation, active-component mapping round trip, explicit overflow behavior, allocation manifest, and zero full-world readback in the normal render loop. Full isotope/reaction correctness is gated in P3/P6, not claimed by a placeholder field.

## 8. Rust/WASM integration gates

The [engine migration acceptance](../../validation/backend-migration.md) adds MIG-01 through MIG-14 to the numerical matrix. Validate native reference tests, WASM/bindings builds, byte layout, browser execution, command epochs/commit semantics, canvas ownership, same-device rendering, bounded transfers, source transactions and checkpoint recovery. Native wgpu performance is not browser performance.

The authoritative P1 work tracker is [E00-E14](../../data/engine-migration-work.json). Later phases are deferred; if activated they reuse the Rust contracts, CPU references, wgpu resources and WGSL patterns rather than introduce another simulation owner. No performance gain is proven by this stack decision alone.

## 9. Fire acceptance is part of this redesign

[The combustion plan](../fire-combustion/plan.md) and [fire acceptance specification](../../validation/fire-combustion.md) retain FIRE-W00-W07 in active P1 and FIRE-W08 in deferred P4 without altering their numerical gates. FIRE-PRECONDITIONS belongs to M4; FIRE-M6 and FIRE-M7 refine chemistry integration and scene promotion. The E-work and fire manifests cross-reference those obligations.

No generic FIRE material port, visually convincing flame or scalar mass/energy check can replace the component, boundary, product, ignition and extinction gates. M3 remains the one-liquid/carrier-gas passive demonstrator. The reduced fire model is not a universal combustion solver or a reason to bypass pressure-wave/high-temperature domain limits.
