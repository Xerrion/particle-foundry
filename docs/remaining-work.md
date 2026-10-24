# Engineering work and dependencies

Use [the handbook](README.md) to continue project work.
[STATUS.md](STATUS.md) gives the selected action in Danish. Live Linear issues
and the [engine manifest](data/engine-migration-work.json) record work status.
This reference maps backlog IDs to deliverables, dependencies and acceptance checks.

## Assigned migration work

P1 covers the complete [current sandbox](plans/rust-wasm-migration/current-sandbox-scope.md).
The [migration plan](plans/rust-wasm-migration/plan.md) defines E00-E14.
[ADR-001](architecture/adr-001-rust-wasm-wgpu.md) owns the stack decision.
The [roadmap](roadmap.md) explains phase order and the boundary around deferred work.

| Work item | Phase / milestones  | Required deliverable                                                                                                                                                      |
| --------- | ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R9        | P1 / M0-M1, E00-E04 | Fresh baseline, Rust/WASM workspace, browser smoke, bounded asynchronous facade, IDs/ABI/data projection and independent Rust circuit reference with invalidation         |
| R10       | P1 / M2-M3, E05-E08 | Rust f64 conservative reference and equivalent WGSL demonstrator, direct rendering, asynchronous probes, equal-time parity and measured transfers                         |
| R11       | P1 / M4-M7, E09-E14 | Complete current-sandbox thermal, motion, eight-element, reaction and source behavior, bounded Rust CPU circuits, TypeScript controls, recovery and full-matrix promotion |

E04 circuits are an independent foundation item required before E11. They do not
block the fluid reference. M3 covers one liquid, carrier gas, fixed walls and
passive markers with basic controls. E09-E12 must restore every required current
feature, including hydrogen, helium, carbon, nitrogen, oxygen, sulfur, iron and copper.
E13-E14 must prove recovery and the complete default switch.

The [shared gameplay target](plans/fluid-gpu-redesign/plan.md#final-gameplay-target-clarified-on-13-october-2026)
requires 60 displayed FPS, simulation in real time, responsive painting and
physically credible behavior. Full-sandbox qualification belongs to E14.
Retain legacy access until every required coverage and promotion check passes.
Record a proposed scope change for a user decision. A failed row remains required.

### Reference, GPU and evidence procedure

1. Select the assigned E-item and its dependencies. Read only their applicable
   [numerical requirements](plans/fluid-gpu-redesign/plan.md),
   [engine contract](architecture/engine-boundary.md) and
   [backend acceptance checks](validation/backend-migration.md).
2. Freeze named fixtures, supported domains, expected observations and tolerances
   before implementation. Build an independent Rust f64 reference before the corresponding WGSL model.
3. Keep one authoritative scene owner, bounded commands and asynchronous observations
   stamped with tick and epoch. Render from GPU state. Avoid full-world frame transfers and duplicate ticking worlds.
4. Run the assigned native, WASM and browser checks through mise from the repository root.
   Record the source revision, dirty state, commands, versions, numerical differences and unavailable checks.
5. Record results with [the evidence template](templates/phase-evidence.md).
   Keep raw logs in ignored `artifacts/`. Complete the assigned checkpoint before selecting later work.

Keep the 1/60 s outer clock, 0.01 m cell width and 480 x 270 grid unless an approved
contract versions them. Fix device targets and the measurement protocol before
benchmarking. Measure accepted simulated time, completed work, latency and transfers.
A submitted tick or rendered frame does not establish completed simulation.

Use current source, including uncommitted changes, for fresh evidence. Native
device initialization and mock tests do not prove browser GPU simulation.
Record an unavailable hardware check as unrun. Preserve historical evidence,
source archives and audit totals without changing their original scope.

## XER-64 limited-scene performance work

### XER-64 work split on 14 October 2026

[XER-64](https://linear.app/xerrion/issue/XER-64/run-the-experimental-gpu-scene-in-real-time)
owns real-time performance for the limited experimental scene. Its description
contains the task. Comments record progress, measurements and delivery evidence.
The [performance decision](plans/fluid-gpu-redesign/plan.md#interactive-performance-priority-on-6-october-2026)
defines the accepted precision tradeoff and retained correctness requirements.

| Issues                                                                                                   | Responsibility                                                                                                                       |
| -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| XER-81 through XER-95                                                                                    | Fifteen delivery records from PR #17 through PR #30, including retained changes, rejected comparisons, tooling and RTX qualification |
| [XER-80](https://linear.app/xerrion/issue/XER-80/profile-pressure-convergence-and-gpu-completion-timing) | Pressure convergence diagnostics, host/GPU timing, browser tracing and a measured recommendation                                     |
| XER-96                                                                                                   | Displayed-frame and interaction measurements                                                                                         |
| [XER-97](https://linear.app/xerrion/issue/XER-97) | Browser coarse-pressure batch 16 implemented and locally validated. Native eight unchanged. Isolated feature branch created. See XER-97 for live delivery/review status. |
| XER-98                                                                                                   | Final limited-scene qualification of real time and 60 displayed FPS, dependent on XER-96 and XER-97                                  |

[XER-63](https://linear.app/xerrion/issue/XER-63/fix-gpu-pressure-nonconvergence-during-sustained-browser-scene)
owns the pressure-stop defect and remaining production verification.
[XER-38](https://linear.app/xerrion/issue/XER-38/validate-current-sandbox-coverage-and-default-gpu-rollout)
owns complete current-sandbox qualification and default promotion.
An unselected backlog item does not authorize implementation.

The [E08 validation article](validation/p1-m3-e08.md) owns measurements and delivery history.
Its [profiling evidence](validation/p1-m3-e08.md#pressure-convergence-and-completion-profiling-on-14-october-2026)
and [integration record](validation/p1-m3-e08.md#profiling-integration-with-main-on-14-october-2026)
cover PR #31. The
[RTX baseline](validation/p1-m3-e08.md#rtx-qualification-and-baseline-after-reboot-on-14-october-2026)
records Linux/NVIDIA launch requirements and qualification limits.
Throughput and render submissions do not establish displayed FPS or input latency.

The [24 October XER-97 comparison](validation/p1-m3-e08.md#coarse-pressure-completion-batching-on-24-october-2026)
retains browser batch 16 after three interleaved Windows/Chrome rounds, with
11.9836% median paired duration reduction and exact complete fields/progress.
Complete separate pressure journals reconcile the expected transfer reduction.
This is a fresh platform baseline, not a timing comparison with Linux.
Explicit native GPU regressions, the required sustained browser GPU check and
full local CI pass. The final WASM is byte-identical to measured batch 16.
The local implementation and required local validation are complete.
The verified patch was applied in the separate worktree
`E:/Projects/particle-foundry-xer-97`, on `perf/xer-97-pressure-completion-batches`.
The shared `main` checkout and its existing edits remain unchanged.
The guard remains enabled. It blocked the initial `git switch` attempt,
not feature-branch creation through `git worktree add`.
See [XER-97](https://linear.app/xerrion/issue/XER-97) for live PR and review status.
This record claims no merge or deployment. Native builds keep batch eight.
XER-96 is the next unselected
displayed-frame/interaction measurement task. XER-98's final qualification
remains blocked. Neither task is started by this comparison.

Earlier comparisons remain in that article: [pressure/transport](validation/p1-m3-e08.md#rtx-pressure-and-transport-comparison-on-7-october-2026),
[completion batching](validation/p1-m3-e08.md#browser-completion-batch-comparison-on-7-october-2026),
[block-pressure screening](validation/p1-m3-e08.md#local-block-pressure-screening-on-13-october-2026),
[coarse pressure](validation/p1-m3-e08.md#component-aware-coarse-pressure-on-13-october-2026),
[packed transport](validation/p1-m3-e08.md#packed-transport-pair-comparison-on-13-october-2026),
[allocation screening](validation/p1-m3-e08.md#transport-parameter-and-allocation-comparison-on-13-october-2026),
[buffer reuse](validation/p1-m3-e08.md#resident-candidate-buffer-reuse-on-13-october-2026),
[transport activity](validation/p1-m3-e08.md#transport-activity-investigation-on-14-october-2026)
and [continuous brushes](validation/p1-m3-e08.md#continuous-gpu-brush-strokes-on-14-october-2026).
Camera visibility cannot determine which parts of the physical world evolve.

The bounded [legacy Wood/Oil correction](combustion.md#legacy-woodoil-tempo-checkpoint-on-14-october-2026)
under XER-99/E11 has separate behavior and evidence.
Its [playable-tempo requirement](plans/fire-combustion/plan.md#playable-wood-and-oil-burn-tempo)
also applies to migration. The legacy correction does not complete E11 or Rust/GPU combustion.

## Fire dependencies within P1

| Queue / milestone | Required fire work             | Exit obligation                                                                                     |
| ----------------- | ------------------------------ | --------------------------------------------------------------------------------------------------- |
| R9 / M0-M1        | FIRE-W00-W01                   | Separate baseline and corrected assertions, plus minimal component/source/boundary/visual contracts |
| R10 / M2-M3       | Passive conservative transport | Keep combustion and whole-catalogue allocation outside the demonstrator                             |
| R11 / M4          | FIRE-W02                       | FIRE-PRECONDITIONS: finite ventilation and explicit property coverage for hot scenes                |
| R11 / M6          | FIRE-W03-W06                   | FIRE-M6: Rust reference, WGSL parity, inventory/product correctness and visuals derived from state  |
| R11 / M7          | FIRE-W07                       | FIRE-M7: supported fire scenes, persistence/recovery and measured GPU evidence                      |

[The fire manifest](data/fire-combustion-work.json) owns status.
[The fire plan](plans/fire-combustion/plan.md) and
[corrected FIRE-A gates](validation/fire-combustion.md#corrected-gates) own acceptance.
The [12-case audit](evidence/fire-review-2026-09-23/FIRE_REVIEW.md) is immutable characterization.
Correct oxygen, products and ignition within P1. Do not reproduce known legacy
fire defects or postpone those obligations to the broader materials expansion.

## Deferred backlog

P2-P9 and FIRE-W08 require separate activation. Completing P1 does not activate them.
The [roadmap](roadmap.md) owns their explanation and conditional order.
Minimal data and reaction products needed by current features remain P1 dependencies.
Optional worker deployment and new product features also remain deferred.

| Work item | Phase / milestones | Deferred deliverable                                                                                                               |
| --------- | ------------------ | ---------------------------------------------------------------------------------------------------------------------------------- |
| R12       | P2 / W0-W2         | Compressible CPU reference and GPU parity, waves/reflection, finite exchange and stable time integration                           |
| R13       | P2 / W3            | Reactive sources, stress-based failure, fragments and conservative solid/fluid coupling                                            |
| R14       | P2 / W4            | Breakage toggle, slow motion/substeps, supported scenes and browser/recovery/performance evidence                                  |
| R15       | P3 / C0-C2         | Source-qualified registries, composition/isotope preservation, product closure, explicit reactions and CPU/GPU tables              |
| R16       | P4 / C3-C6         | Revalidate P1's eight elements under expanded data, then deliver 40 core forms and compound/reaction scenarios, including FIRE-W08 |
| R17       | P5 / C7-C9         | Exact 43-element extension, 83 total identities, bounded advanced systems, coverage and capacity tests                             |
| R18       | P6 / N0-N3         | Nuclide data, decay, radiation and isotope UI, adding 20 identities for 103 total                                                  |
| R19       | P7 / N4-N6         | Selected fusion/fission, independent references, energy partition and bounded host coupling                                        |
| R20       | P8 / N7-N8         | Final 15 identities, explicit unknown/predicted/creative behavior and persisted stabilization                                      |
| R21       | P9 / Q0-Q2         | Combined-feature matrix, scenario/persistence polish, device evidence and documentation reconciliation                             |

[development-phases.json](data/development-phases.json) owns phase evidence references.
[element-roadmap.json](data/element-roadmap.json) owns exact cohort membership.
Roster counts do not establish implementation or acceptance.

## Legacy delivery and audit records

R1-R8 describe reported legacy work. Their original descriptions, source paths
and visual-regression history remain in [the input archive](history/README.md).

| Historical item | Reported scope                                                    | Model limit                                        |
| --------------- | ----------------------------------------------------------------- | -------------------------------------------------- |
| R1              | Shared scale, parcel inventories and source/loss ledger           | Does not establish continuum transport             |
| R2              | Hybrid column/cellular fluids and cosmetic waves                  | Velocity does not consistently determine transport |
| R3              | Thermal/latent accounting and phase mass preservation             | Not a general internal-energy/EOS model            |
| R4              | Anchored/dynamic solids, collision and buoyancy                   | No deformable or breakable connected solids        |
| R5              | Ideal-gas chamber pressure and paired impulses                    | Open/sealed volume mismatch and no resolved shocks |
| R6              | Finite fuel/oxygen/chemical stores and external growth            | No complete species chemistry or photosynthesis    |
| R7              | Seeded replay, diagnostics, fixtures, controls, benchmarks and CI | Checks establish only their stated invariants      |
| R8              | Material/state/solver ownership and UI documentation              | Does not establish the replacement backend         |

Preserve the existing regression families for continuous brushes, fire, displacement,
pool occupancy/colour, gas plumes, boiling bubbles, temperature maps, catalogue validation
and input coordinates. The owning current behavior articles and tests define their scope.
Older partly delivered explosion, catalogue, circuit and acid/base phases do not replace P1-P9.

The historical audit records 24 passes and 11 failures across 35 checks.
Preserve its N/C identifiers. Classify a replaced model separately from an implementation regression.
Record new results against current source in new evidence. Update [model.md](model.md)
when implementation changes warrant it. [Acceptance](validation/acceptance.md) defines the required checks.
