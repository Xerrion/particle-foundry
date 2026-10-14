# Active work and delivered-history boundary

**Status date:** 30 September 2026. **Most recent scoped work:** P1/M4/E09,
with an isolated [pure-water reference](thermodynamics.md).
E09 is in progress. E04 circuits and E10-E14 remain planned. M0/E00/FIRE-W00,
E01 bootstrap, E02-E03/FIRE-W01 contracts and E05-E08 have local evidence;
see [baseline](validation/p1-m0.md), [bootstrap](validation/p1-m1.md),
[E02](validation/p1-m1-e02.md), [E05](validation/p1-m2-e05.md),
[E06](validation/p1-m2-e06.md), [E07](validation/p1-m3-e07.md) and
[E08](validation/p1-m3-e08.md). M1 and P1 remain incomplete. Follow
[START_HERE.md](START_HERE.md) and [the roadmap](roadmap.md).

## Current active queue: P1 only

The final outcome for this queue is the complete existing sandbox on Rust/GPU
with 60 displayed FPS, simulation in real time, responsive painting and physically
credible behavior. The [shared gameplay target](plans/fluid-gpu-redesign/plan.md#final-gameplay-target-clarified-on-13-october-2026)
owns those acceptance rules. XER-64 advances the limited scene; XER-38/E14
qualifies the full current-sandbox outcome before default promotion.

**GPU follow-up ownership, 6 October 2026:**
[XER-63](https://linear.app/xerrion/issue/XER-63/fix-gpu-pressure-nonconvergence-during-sustained-browser-scene)
retains the pressure-stop defect and remaining production verification.
[XER-64](https://linear.app/xerrion/issue/XER-64/run-the-experimental-gpu-scene-in-real-time)
owns the current experimental scene's real-time optimization and PR #17.
[XER-38](https://linear.app/xerrion/issue/XER-38/validate-current-sandbox-coverage-and-default-gpu-rollout)
retains final full-sandbox coverage and default promotion. Performance has
priority, with the [accepted precision tradeoff](plans/fluid-gpu-redesign/plan.md#interactive-performance-priority-on-6-october-2026).
The user resumed implementation on 7 October 2026 after the existing PRs merged.
The [measured pressure/transport comparison](validation/p1-m3-e08.md#rtx-pressure-and-transport-comparison-on-7-october-2026)
retains only true-residual reuse. The larger trial changes failed the selected
accuracy limit and the real-time target. The
[completion-batch comparison](validation/p1-m3-e08.md#browser-completion-batch-comparison-on-7-october-2026)
then measures browser costs and reduces completion waits with identical fields.
PR [#21](https://github.com/Xerrion/particle-foundry/pull/21) is merged with
passing main CI; see [merged delivery](validation/p1-m3-e08.md#merged-delivery).
Repeated duration improves by 22.0%; real time remains unmet. Next, compare a
pressure or transport method that reduces GPU work within the required limits.
The [13 October block-pressure screening](validation/p1-m3-e08.md#local-block-pressure-screening-on-13-october-2026)
rejects a local polynomial preconditioner after repeatable native full-grid
nonconvergence. The original solver is retained. The next comparison should
address long-range pressure error while keeping the existing accuracy limits.
The [coarse pressure checkpoint](validation/p1-m3-e08.md#component-aware-coarse-pressure-on-13-october-2026)
then passes native, equal-time accuracy and required sustained browser checks
with a component-aware V-cycle and compensated pressure accumulation.
PR [#23](https://github.com/Xerrion/particle-foundry/pull/23) is merged with
passing main CI and public GPU UI checks; see the owning evidence for limits.
The [packed transport comparison](validation/p1-m3-e08.md#packed-transport-pair-comparison-on-13-october-2026)
then reduces dispatched workgroups with identical complete field hashes.
Its paired browser duration falls by 12.63%, from 15.7259 to 13.7391 wall
seconds per model second. Dispatch counts remain fixed.
The [closure and allocation screening](validation/p1-m3-e08.md#transport-parameter-and-allocation-comparison-on-13-october-2026)
rejects exact fixed-point stopping and parameter packing after both fail to
improve repeated browser duration. That screening selected large
transport/momentum scratch allocations at the transaction owner for a reuse
comparison with rejection, queue-order, reset and complete-field checks.
The [resident-buffer checkpoint](validation/p1-m3-e08.md#resident-candidate-buffer-reuse-on-13-october-2026)
then reduces repeated duration by 5.26% with complete field identity. Preview-policy
render submissions improve from 9.47 to 10.12 per second. Native and required
sustained browser checks pass. The [14 October decision](plans/fluid-gpu-redesign/plan.md#realism-and-active-region-investigation-selected-on-14-october-2026)
selected a bounded transport-activity investigation. The
[completed screening](validation/p1-m3-e08.md#transport-activity-investigation-on-14-october-2026)
rejects exact sleeping of 16 x 16 regions: every region containing fluid changes
in the later normal-brush samples. No sparse runtime is retained. Current brush
throughput timing averages 13.6638 wall seconds per model second. The separate
preview-pacing runs average 9.50 render submissions per second. These measurements
establish no new speedup or displayed-FPS result. The full dense comparison was
stopped without a complete timing series;
the separate ten-tick dense screen passes. The
[water-line correction](validation/p1-m3-e08.md#continuous-gpu-brush-strokes-on-14-october-2026)
is merged in [PR #28](https://github.com/Xerrion/particle-foundry/pull/28) at
`8c963f8`. Continuous GPU brush strokes remove the repeated input scallops
while preserving the simulation equations and limits. The measured pacing change
is within variation, so no simulation speedup is established.
Pressure convergence and completion synchronization remain a proposed later
investigation. Frustum culling has lower priority for the current full-grid
renderer. Real time remains unmet.
PR [#26](https://github.com/Xerrion/particle-foundry/pull/26) is merged at main
`360d514` with all six post-merge CI jobs passing. It aligns the gameplay target;
the activity tools and evidence are merged in
[PR #27](https://github.com/Xerrion/particle-foundry/pull/27) at `4274ba5`.
PR [#24](https://github.com/Xerrion/particle-foundry/pull/24) is merged at main
`372d6ff` with passing main CI. Real time remains unmet.
The [clarified final target](plans/fluid-gpu-redesign/plan.md#final-gameplay-target-clarified-on-13-october-2026)
is 60 visible FPS during normal play, accepted simulation time following wall
time and physically credible material behavior. Measure frame pacing and input
latency alongside throughput; the current benchmark does not qualify game FPS.

Complete the [current-sandbox checklist](plans/rust-wasm-migration/current-sandbox-scope.md)
before the default GPU switch. Its current materials, interactions and controls
are mandatory, including the eight existing elemental models. New features and
optional worker deployment are deferred.

| Work item | Phase / milestones | Deliverable and acceptance | Status |
| --- | --- | --- | --- |
| R9 | P1 / M0-M1, E00-E04 | Fresh baseline; Rust/WASM workspace and browser smoke test; async facade, IDs/ABI/data projection, independent Rust circuit reference and invalidation | In progress; E00-E03 validated; E04 circuits planned |
| R10 | P1 / M2-M3, E05-E08 | Rust f64 conservative reference and equivalent Rust/wgpu WGSL demonstrator; direct rendering, async probes, equal-time parity and measured transfers | Validated locally; the experimental GPU scene is not the default backend |
| R11 | P1 / M4-M7, E09-E14 | All required current-sandbox thermal, motion, eight-element, reaction and source behavior; bounded Rust CPU circuits, existing TypeScript controls, recovery and full-matrix promotion | Planned |

## Deferred backlog: separate activation required

P2-P9 do not start automatically after P1. Their IDs and dependencies are retained
for future planning; they do not expand the current release.

| Work item | Phase / milestones | Deferred deliverable | Status |
| --- | --- | --- | --- |
| R12 | P2 / W0-W2 | Compressible CPU reference and GPU parity; local waves/reflection, finite exchange, stable time integration | Deferred |
| R13 | P2 / W3 | Reactive source integration, stress-based failure, fragments, conservative solid/fluid coupling | Deferred |
| R14 | P2 / W4 | Breakage toggle, slow motion/substep inspection, supported scenes, browser/recovery/performance evidence | Deferred |
| R15 | P3 / C0-C2 | Source-qualified registries, composition/isotope-preservation, product closure, explicit reaction engine, CPU/GPU table compilation | Deferred |
| R16 | P4 / C3-C6 | Revalidate the P1-migrated eight elements under expanded data; deliver 40 core forms and expanded compound/reaction scenarios | Deferred |
| R17 | P5 / C7-C9 | Add exact 43-element cohort; 83 total forms, bounded advanced systems, coverage and capacity tests | Deferred |
| R18 | P6 / N0-N3 | Nuclide data, decay, radiation and isotope UI; add 20 nuclear-first identities for 103 total | Deferred |
| R19 | P7 / N4-N6 | Selected fusion/fission channels, independent references, energy partition, bounded host coupling | Deferred |
| R20 | P8 / N7-N8 | Final 15 exotic identities; explicit unknown/predicted/creative behavior and stabilization persistence | Deferred |
| R21 | P9 / Q0-Q2 | Combined-feature matrix, scenario/persistence polish, device measurements, source/model/documentation reconciliation | Deferred |

Only P1 is active. Keep M3 small, then complete current feature coverage in M4-M6 and verify the full default switch in M7. Minimal generated data and reaction products for existing behavior remain P1 dependencies; the generalized P3 pipeline is deferred. FIRE-W00-W07 specialize R9/R11; FIRE-W08 in R16 is deferred. The [fire manifest](data/fire-combustion-work.json) retains corrected fixture requirements.

No phase completion can be inferred from roster counts. [development-phases.json](data/development-phases.json) records planned phases and evidence references; [element-roadmap.json](data/element-roadmap.json) records exact cohort membership. Update both only with supported changes.

## Immediate work checklist

- [x] E00-E03 baseline, Rust/WASM bootstrap and browser/state contracts have recorded validation.
- [x] Freeze the current feature inventory and defer P2-P9; this is a scope decision, not simulation evidence.
- [x] E05: validate the sealed Rust f64 pressure reference and record its limits.
- [x] E07-E08: implement the equivalent GPU solver and prove the limited browser GPU path locally.
- [ ] E04/E09-E12: implement and verify all required current-feature rows, including existing elemental behavior and controls.
- [ ] E13-E14: validate full-matrix recovery, actual-browser behavior and measured performance before the default switch.

Use [the evidence template](templates/phase-evidence.md) for completed work. If a
required hardware check is unavailable, record it as unrun and keep P1 incomplete.
Keep earlier baseline artifacts and historical results unchanged.

## Legacy delivered history

The supplied documentation reports these earlier changes as delivered. The latest uploaded source was inspected statically, but delivery/test claims remain unverified against a runnable full checkout until M0. Their original descriptions, file references, and detailed visual-regression history are preserved in [the complete input archive](history/README.md).

| Historical item | Reported delivered scope | Important limit |
| --- | --- | --- |
| R1 | Shared scale, parcel inventories, source/loss ledger | Does not establish continuum transport |
| R2 | Hybrid columns/cellular fluids and isolated cosmetic waves | Velocity does not consistently determine all transport |
| R3 | Mass-scaled thermal/latent accounting and phase mass preservation | Not a general internal-energy/EOS model |
| R4 | Anchored versus dynamic solid cells, collision and buoyancy | Not deformable/breakable connected solids |
| R5 | Ideal-gas chamber pressure and paired impulses | Open/sealed volume mismatch; no resolved shocks |
| R6 | Finite fuel/oxygen/chemical stores and external growth accounting | Not complete species chemistry or photosynthesis |
| R7 | Seeded replay, diagnostics, reference fixtures, browser controls, benchmarks and CI | Tests establish only their stated invariants |
| R8 | Material/state/solver module responsibilities and UI documentation | New backend and full data model remain future work |

Additional documented work includes continuous brushes, energy-funded Fire behavior, oil/water displacement, stable pool colour/occupancy, gas-plume regressions, asynchronous flame emission, pressure-consistent water latent behavior, rising boiling bubbles, temperature maps, typed/frozen catalogue lookups, grouped modules, and input-coordinate validation. Preserve those regression families during the redesign.

The original roadmap's partly delivered explosion, picker/catalogue, circuit, and acid/base work is legacy history. It is not a different P1-P4 execution sequence. The current documentation consistently identifies eight legacy element models; an earlier reference to five and the implication that no element had any implementation have been corrected.

## Historical audit, not fresh evidence

The unchanged audit files report 24 passes and 11 failures for 35 checks at their recorded source state. New baseline results may differ. Preserve N/C test IDs and classify a replaced physical model separately from an implementation regression. Do not demand the old pass/fail totals from a newer source tree, and do not call the historical source tree the current checkout.

Record new results separately and update [model.md](model.md) only when implementation changes are confirmed. [Acceptance](validation/acceptance.md) defines numerical, functional, statistical, and hardware gates for new work.

## Selected engine migration and status

E00-E03 and E05-E08 in [the engine tracker](data/engine-migration-work.json)
have local validation evidence. E09 has an isolated water reference and remains
in progress. E04 and E10-E14 remain planned. E03 supplies a
mock-tested bounded session contract and one generated candidate catalogue
projection. E05 adds a sealed Rust f64 pressure reference. E06 adds owned phase
and marker transport, compatible momentum, tangential shear, and an atomic
bounded CPU substep. Named closed scenes and dam-break/refinement gates pass
locally. Periodic marker and shear oracles remain component-level tests, not a
periodic coupled session. The experimental browser GPU scene renders committed
state and passed a local 60-tick 480 x 270 Chrome trial under the frozen
pressure gates. Bounded paint, inspection, pause and reset passed isolated
browser checks. The independent E04 circuit implementation remains required
before E11. The stack decision is settled by
[ADR-001](architecture/adr-001-rust-wasm-wgpu.md); numerical details remain
governed by the phase plans. Narrow scope does not authorize promoting
unsupported scenes or bypassing fire gates.

Deferred P2 and later chemistry/nuclear work would use Rust references and equivalent WGSL if activated. The current UI remains TypeScript. See [the execution mapping](plans/rust-wasm-migration/plan.md#deferred-phase-ownership).

## Fire audit follow-through

| Existing queue | Required fire work | Exit obligation |
| --- | --- | --- |
| R9 / M0-M1 | FIRE-W00-W01 | Baseline versus corrected assertions separated; minimal component/source/boundary/visual contracts |
| R10 / M2-M3 | Passive conservative transport only | No combustion or whole-catalogue allocation added to the demonstrator |
| R11 / M4 | FIRE-W02 | FIRE-PRECONDITIONS: finite ventilation and explicit hot-scene property coverage |
| R11 / M6 | FIRE-W03-W06 | FIRE-M6: Rust reference, WGSL parity, inventory/product correctness and integrated state-derived visuals |
| R11 / M7 | FIRE-W07 | FIRE-M7: supported fire-scene tests, persistence/recovery and measured GPU evidence |
| Deferred R16 / P4 | FIRE-W08 | Extend fuel/char/soot/radiation mechanisms and revalidate each newly enabled channel |

FIRE-W00 baseline and FIRE-W01 contracts are validated; FIRE-W02-W07 and corrected FIRE-A gates remain planned; FIRE-W08 is deferred. The [12-case audit](evidence/fire-review-2026-09-23/FIRE_REVIEW.md) is immutable characterization, not delivered fixes. Do not port legacy fire rules unchanged, require a full duplicate TypeScript repair, or defer basic oxidizer/product/ignition correctness until the 40-element expansion.
