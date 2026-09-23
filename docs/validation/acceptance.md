# Acceptance, evidence, and final product gates

**Status:** planned acceptance requirements, not measured results. Start at [START_HERE.md](../START_HERE.md). The numerical GPU thresholds already specified in [the fluid plan](../plans/fluid-gpu-redesign/plan.md) remain the detailed P1 contract.

## Evidence rules

A milestone is validated only when its implementation, independent references, regression tests, applicable CPU/GPU comparisons, and evidence are present. A docs-only update, a rendered scene, or a passing old test suite does not prove new physics.

For each run record commit, working-tree snapshot/hash, source/data/schema versions, toolchain, exact commands, seeds, grid/scale, boundaries, accepted physical time, tolerances, raw outputs, and unsupported/unrun checks. Hardware runs also record OS, browser, adapter/device information available to the app, requested limits/features, warm-up, sample count, resource bytes, and recovery behavior.

Keep new artifacts under a run-specific directory. Use [the evidence template](../templates/phase-evidence.md); link a durable summary from phase status. Historical audit files are immutable. The document-validation script checks this package's structure, not the game.

## Gate matrix

| Gate | Required evidence | Cannot substitute |
| --- | --- | --- |
| GPU-M7 / P1 | M0-M7 evidence; conservative CPU/GPU operators/fluxes; volume/phase closure; integrated supported scenes; ownership, controls, recovery and measured performance | A direct shader port of old parcel swapping or M3 alone |
| WAVE-W4 / P2 | W0-W4 shock/acoustic/reflection references, finite venting, structural load/fracture, toggle and fragment conservation | A radius-based explosion effect or global chamber-pressure jump |
| MAT-C2 / P3 | Unique identities, source/unit/domain schema, amount and isotope-preserving composition, balanced reaction definitions, product closure and migrations | 118 names or a bag of unqualified constants |
| MAT-C6 / P4 | 40 core base-form identities, essential compound scenarios, selected reactions, bounds and new-backend tests | 40 enabled picker buttons |
| MAT-C9 / P5 | Exact extended cohort, 83 base-form identities, enabled shared-mechanism tests, active-component capacity and combined scenes | Copying one metal/gas profile to all elements without labels |
| NUC-N3 / P6 | Evaluated nuclide subset, decay/chain references, radiation energy/escape, 20 additional nuclide-first identities, isotope UI and persistence | A per-element radioactive timer or glow |
| NUC-N6 / P7 | Independent fusion and fission network tests plus a bounded host-coupled source/deposition scene | A hot-contact fusion rule or recursive scripted blast |
| NUC-N8 / P8 | Final 15 identities, source-qualified nuclear behavior, clear missing/predicted/creative bulk data, saved overrides | Invented measured-looking superheavy constants |
| REL-Q2 / P9 | Q0-Q2 combined-feature, product, recovery, hardware and documentation evidence | Isolated subsystem tests with untested combinations enabled |

## Numerical and state invariants

| ID | Fixture or invariant | Required check |
| --- | --- | --- |
| OWN-01 | One authority | No legacy transport mutates a new-backend scene; CPU reference is a separate world |
| OWN-02 | Transactionality | Failed solver, source, allocation or checkpoint stage cannot publish partial state |
| CMD-01 | Ordered commands | Tick/sequence order, paint while paused, single step, repeated setting commands, bounded queues |
| ID-01 | Registry | Exactly 118 unique atomic numbers/symbols; separate material/tool/nuclide/species IDs |
| ID-02 | Migrations | Legacy numeric IDs and units preserved or explicitly converted; unsupported state rejected |
| COMP-01 | Ordinary composition | Move/split/merge/phase/chemical steps preserve each supported nuclide's count and charge accounting |
| COMP-02 | Capacity | Product closure and component-growth transaction; overflow cannot delete trace material |
| DATA-01 | Properties | Required unit, source, status and domain; unknown is not zero; scientific and creative records distinct |
| DATA-02 | Reproducible import | Dataset/version/checksum and transformation script recorded; sampled value checks |
| FLUID-01 | Transport | Shared phase/species/energy face fluxes, bounded amounts, matched momentum transport |
| FLUID-02 | Pressure | Compatible D/G, sealed-component nullspaces, hydrostatic rest and residual-controlled solve |
| THERMO-01 | Phase/EOS | Fixed mass/U/volume reference; partial phases; finite venting; invalid domain reported |
| SOLID-01 | Sand and bodies | Unique accepted ownership, reserved outlet capacity, swept volume, momentum/work budget |
| WAVE-01 | Compressible gas | Shock/acoustic/reflection references and grid/time convergence at equal physical times |
| BREAK-01 | Material failure | Stress/geometry, equal opposing pressure control, elastic/fracture/dissipation budget |
| CHEM-01 | Reaction extent | Limiting reagents, retained excess, explicit products, exact definition-level atom/charge balance |
| CHEM-02 | Chemical energy | One reference convention; formation/storage terms not double-counted; no catalytic free energy |
| ISO-01 | Isotope persistence | Supported heating, freezing, chemistry, alloying, fragmentation and save/recovery retain signatures |
| NUC-01 | Decay | Analytic parent/chain tests, branch closure, source time independence, stable/unknown distinction |
| NUC-02 | Nuclear balance | Nucleon/charge accounting and mass-energy convention, complete product/particle inventory |
| RAD-01 | Radiation | Emission equals remaining plus deposited plus escaped energy; no duplicate deposition |
| NUC-03 | Fusion/fission | Selected channel applicability, depletion, products, rate/data domains, source stiffness |
| NUC-04 | Host coupling | EOS/source compatibility, explicit out-of-domain behavior, no reused blast impulse |
| MODE-01 | Creative overrides | Saved visible stabilization/rate changes, no suppressed-decay heat or unrequested catch-up burst |
| SAVE-01 | Persistence | Versioned catalogue/network, active mapping, ledgers, damage, particles, settings and accepted time |
| GPU-01 | Rendering/readback | Direct rendering; no full-world normal-frame readback; tick-stamped staging probes |
| GPU-02 | Recovery | Absent/lost adapter, bounded pending work, consistent checkpoints, no cross-model hot swap |

These are stable fixture families. Give concrete tests stable IDs underneath them and record the independent reference used. Where a generic pseudo-material lacks an exact chemical composition, exclude it from exact chemical/isotope claims and test only its declared budgets.

## Tolerance policy

P1 uses the detailed precision-specific targets in the fluid plan. Freeze scales, absolute tolerances, relative tolerances, convergence criteria, and test duration before optimization. Treat these as initial project gates, not guaranteed solver performance.

For new deterministic chemical/nuclear population fixtures, proposed starting relative inventory tolerances are 1e-9 for the Float64 reference and 1e-5 for f32 GPU totals, combined with fixture-specific absolute tolerances in declared units. These are design targets to validate through conditioning/refinement, not permission to discard small populations. Definition-level stoichiometric/charge/nucleon checks are exact symbolic/integer checks when the reaction representation permits them.

Track physically relevant released, deposited and escaped energy scales. A huge arbitrary rest-energy or reference-enthalpy offset must not hide an important energy error. Do not impose the old absolute 1e-9 on every GPU quantity with different units. When nuclear mass defect is approximated, expose and quantify the approximation term separately.

For stochastic radiation or reaction tests, predeclare seeds, ensemble size, comparison statistics, confidence/error bounds, and handling of multiple comparisons. Compare mean populations, energy, spectra/groups and spatial observables against the deterministic/reference expectation. Do not require identical random trajectories across floating-point backends, and do not rerun seeds until a failure disappears.

Repeat at smaller dt and/or dx as relevant. Agreement between CPU and GPU is insufficient when both use the same wrong equation; include an analytic relation, independent method, evaluated-data check, or published numerical reference.

## Essential integration scenes

| Scene | Cross-system requirement |
| --- | --- |
| Fixed-wall pool and dam break | Fluid transport, pressure/gravity, visual occupancy and wall geometry |
| Sealed heated water vessel and a vent opening | Mass/U/volume closure, source energy and finite exchange within the water domain |
| Many grains competing for one opening | Conflict resolution, fluid displacement, composition and topology invalidation |
| Oil/water and freezing material | Multiple phases, amount authority and dynamic-solid transition |
| Circuit conductor melts or moves | Thermal/material changes invalidate circuit topology at the correct barrier |
| Gas pressure wave loads an enclosure | Compressible transport, boundary traction, structural failure and fragments |
| Partially neutralised solution | Explicit ions/products, excess reactants, concentration and heat |
| Supported isotope-tagged material cycle | Transport, reactions/form changes and save/load preserve signatures |
| Decaying population with escaping/depositing radiation | Parent/daughters, radiation buffers, source time, energy partition |
| Standalone prescribed fusion box | Finite inventories, external condition-control energy and channel products |
| Selected fission/capture fixture | Competing channel bookkeeping, finite parent inventory and emitted populations |
| Bounded nuclear-heating host scene | Source/transport/EOS coupling without out-of-domain extrapolation |
| Exotic creative-mode save/reload | Predicted/fictional flags, stabilization and no hidden inventory changes |

None of these implies that every possible material combination is supported. Validate model eligibility at scene load and after settings/painting that could change it.

## Performance and memory reporting

Report display-frame p50/p95, input-to-command latency, simulated seconds per wall second, substeps, residual iterations, dispatches, GPU/CPU transfer bytes, active-component count, allocation sizes, and dropped/queued visual readbacks. Include sparse and densely mixed worlds, many cavities/grains, reactive sources, and supported radiation workloads, not just an empty grid.

The P1 provisional target is 480 x 270 at 1x, p95 displayed frame interval no greater than 33.3 ms and at least one simulated second per wall second on the declared baseline device. It is a target, not a result. Acoustic or nuclear stiffness may require slower physical progress; report it instead of changing equations or violating stability constraints. Optional timestamps supplement host measurements and cannot be required for correctness.

Do not promise the same speed for every phase or device. Freeze device-specific promotion criteria and publish unsupported modes separately. Resource ceilings come from the actual requested device limits and measured allocations, not a universal hard-coded assumption.

## P9 / Q0: combined-feature coverage

Freeze a matrix of backend, fluid model, component/form, chemistry network, nuclear network, damage model, domain and creative settings. Run the supported combinations and reject the unsupported ones before simulation. Reconcile exact roster membership and support badges with the runtime catalogue and [the planning manifests](../data/element-roadmap.json).

**Exit:** every promoted scene has an explicit capability row and integration evidence; no feature is enabled solely because another backend or physical model supports it.

## P9 / Q1: product and persistence

Complete curated material search/favorites, full periodic-table reference, forms/compound selection, supported isotope controls, readable probes, accessible explanations of measured/approximate/unknown data, and tutorial scenes. Preserve mouse/touch painting, zoom/pan, pause, single step, speed/slow motion, reset/clear and settings.

Run schema migration, corrupted/unknown snapshot handling, active-component remapping, long sessions, bounded queues, checkpoint/recovery and export/import tests. Audit scientific versus creative mode and all cached/render-only state. Confirm no source/network request is required for normal gameplay with bundled data.

**Exit:** users can create, inspect, save and recover every promoted scenario without losing composition, settings, ledgers or supported data provenance.

## P9 / Q2: release evidence and documentation

Run the actual repository's lint/typecheck/test/build commands, independent numerical suites, browser tests, long-session workloads and real-device GPU measurements. Record any unrun gate explicitly. Review data redistribution status and pin the shipped datasets.

Update the actual repository README, [model](../model.md), [elements](../elements.md), [backlog](../remaining-work.md), capability matrix, source records and manifests to match implemented behavior. Regenerate the GPU HTML view and run the documentation validator. Preserve old audit evidence and provide a release evidence index.

**Exit / REL-Q2:** the release claim is limited to tested numerical domains, device targets, enabled feature combinations and correctly labelled creative behavior. No phase remains marked validated without its evidence.

## Rust/WASM migration gates across all phases

[Backend-migration acceptance](backend-migration.md) defines MIG-01 through MIG-14. Apply them to P1 promotion and again when P2-P9 extend state, data layouts, browser APIs or source stages. They add native/WASM builds, ABI/data generation, browser capability, command epochs, GPU-resident rendering, accepted-time metrics and failure recovery to the numerical gates; none replaces those gates.

[The static source review](source-review-2026-09-21.md) is evidence of source inspection only. All P1-P9 and E00-E14 work starts planned. No result in this package is a new application benchmark, Rust compilation or GPU validation run.
