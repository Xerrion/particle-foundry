# Active work and delivered-history boundary

**Status date:** 29 September 2026. **Next task in the scoped GPU port:** P1/M2/E06. E04 circuits remain planned. M0/E00/FIRE-W00, E01 bootstrap, E02-E03/FIRE-W01 contracts and the E05 CPU pressure reference are validated; see [baseline evidence](validation/p1-m0.md), [bootstrap evidence](validation/p1-m1.md), [E02 evidence](validation/p1-m1-e02.md) and [E05 evidence](validation/p1-m2-e05.md). M1, M2 and P1 remain incomplete. Follow [START_HERE.md](START_HERE.md) and [the roadmap](roadmap.md).

## Current active queue: P1 only

Complete the [current-sandbox checklist](plans/rust-wasm-migration/current-sandbox-scope.md)
before the default GPU switch. Its current materials, interactions and controls
are mandatory, including the eight existing elemental models. New features and
optional worker deployment are deferred.

| Work item | Phase / milestones | Deliverable and acceptance | Status |
| --- | --- | --- | --- |
| R9 | P1 / M0-M1, E00-E04 | Fresh baseline; Rust/WASM workspace and browser smoke test; async facade, IDs/ABI/data projection, independent Rust circuit reference and invalidation | In progress; E00-E03 validated; E04 circuits planned |
| R10 | P1 / M2-M3, E05-E08 | Rust f64 conservative reference and equivalent Rust/wgpu WGSL demonstrator; direct rendering, async probes, equal-time parity and measured transfers | In progress; E05 validated, E06 in progress, E07-E08 planned |
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
- [ ] E06-E08: freeze remaining fixtures, implement conservative transport and prove the limited browser GPU path.
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

E00-E03 and E05 in [the engine tracker](data/engine-migration-work.json) have validation evidence. E06 is in progress; E04 and E07-E14 remain planned. E03 supplies a mock-tested bounded session contract and one generated candidate catalogue projection. E05 adds a sealed Rust f64 pressure reference. E06 has a conservative phase-transport candidate and detached viscous-shear candidate, while the coupled reference and its release gates remain unfinished. A physical browser GPU scene remains future work. The independent E04 circuit implementation does not block E06 transport. The stack decision is settled by [ADR-001](architecture/adr-001-rust-wasm-wgpu.md); numerical details remain governed by the phase plans. Narrow scope does not authorize promoting unsupported scenes or bypassing fire gates.

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
