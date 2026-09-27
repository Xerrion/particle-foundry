# Active work and delivered-history boundary

**Status date:** 27 September 2026. **Next task:** R9, P1/M1/E03. M0/E00/FIRE-W00, E01 bootstrap and E02/FIRE-W01 contracts are validated; see [baseline evidence](validation/p1-m0.md), [bootstrap evidence](validation/p1-m1.md) and [E02 evidence](validation/p1-m1-e02.md). M1 and P1 remain incomplete. Follow [START_HERE.md](START_HERE.md) and [the roadmap](roadmap.md).

## Current active queue

| Work item | Phase / milestones | Deliverable and acceptance | Status |
| --- | --- | --- | --- |
| R9 | P1 / M0-M1, E00-E04 | Fresh baseline; Rust/WASM workspace and browser smoke test; async facade, IDs/ABI/data projection, independent Rust circuit reference and invalidation | In progress; E00-E02 validated; E03 next |
| R10 | P1 / M2-M3, E05-E08 | Rust f64 conservative reference and equivalent Rust/wgpu WGSL demonstrator; direct rendering, async probes, equal-time parity and measured transfers | Planned |
| R11 | P1 / M4-M7, E09-E14 | Rust/GPU thermodynamics, solids and sources; bounded Rust CPU circuits, TypeScript host/worker integration, recovery and measured promotion | Planned |
| R12 | P2 / W0-W2 | Compressible CPU reference and GPU parity; local waves/reflection, finite exchange, stable time integration | Planned |
| R13 | P2 / W3 | Reactive source integration, stress-based failure, fragments, conservative solid/fluid coupling | Planned |
| R14 | P2 / W4 | Breakage toggle, slow motion/substep inspection, supported scenes, browser/recovery/performance evidence | Planned |
| R15 | P3 / C0-C2 | Source-qualified registries, composition/isotope-preservation, product closure, explicit reaction engine, CPU/GPU table compilation | Planned |
| R16 | P4 / C3-C6 | Migrate eight legacy elements; deliver 40 bounded core forms and essential compound/reaction scenarios | Planned |
| R17 | P5 / C7-C9 | Add exact 43-element cohort; 83 total forms, bounded advanced systems, coverage and capacity tests | Planned |
| R18 | P6 / N0-N3 | Nuclide data, decay, radiation and isotope UI; add 20 nuclear-first identities for 103 total | Planned |
| R19 | P7 / N4-N6 | Selected fusion/fission channels, independent references, energy partition, bounded host coupling | Planned |
| R20 | P8 / N7-N8 | Final 15 exotic identities; explicit unknown/predicted/creative behavior and stabilization persistence | Planned |
| R21 | P9 / Q0-Q2 | Combined-feature matrix, scenario/persistence polish, device measurements, source/model/documentation reconciliation | Planned |

Default work order is sequential by phase. P2's fixed-wall prototype has a technical dependency on M3 and fragments additionally on M5, but it must not enlarge the first GPU demonstrator. The full P3 pipeline comes later; its minimal identity/storage seam is already required in M1. Corrected basic chemistry and fire integration remain part of M6. [FIRE-W00-W08](plans/fire-combustion/plan.md#8-work-packages) specialize R9/R11/R16; the separate [fire manifest](data/fire-combustion-work.json) records planned tasks and corrected fixtures.

No phase completion can be inferred from roster counts. [development-phases.json](data/development-phases.json) records planned phases and evidence references; [element-roadmap.json](data/element-roadmap.json) records exact cohort membership. Update both only with supported changes.

## Immediate work checklist

- [x] Inspect actual repository instructions, source, scripts, lockfile, and Git changes.
- [x] Capture a recoverable tracked/untracked snapshot and fresh hashes.
- [x] Run existing checks and the historical audit in a new evidence directory; record differences without overwriting old files.
- [x] Preserve the imported F01-F12 fire characterization; add distinct corrected FIRE-A assertions for full-pipeline boundary/ignition bugs and useful controls. Record new runs separately.
- [x] Freeze initial reference fixtures, precision-specific tolerances, and hardware/performance measurement conventions.
- [ ] Follow [E00-E14](plans/rust-wasm-migration/plan.md): create the Rust/WASM bootstrap, async facade and identity/data contract, then Rust f64 reference before the WGSL solver port.
- [ ] Preserve the TypeScript UI and legacy scenes; do not rewrite every old solver or relocate the app first.
- [ ] Use [source observations](validation/source-review-2026-09-21.md) to cover canvas ownership, nested pressure calls, command timing and mixed energy conventions.

Use [the milestone evidence template](templates/phase-evidence.md) for every completed slice. If hardware is unavailable, record that gate as unrun; a CPU test cannot stand in for browser or GPU validation.

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

E00-E02 in [the engine tracker](data/engine-migration-work.json) are validated by separate baseline, bootstrap and contract evidence. E03-E14 remain planned. E03's bounded facade, async command/probe lifecycle and first generated catalogue projection are next; the independent E04 circuit implementation does not block the fluid port. The stack decision is settled by [ADR-001](architecture/adr-001-rust-wasm-wgpu.md); numerical details remain governed by the phase plans. Narrow scope does not authorize promoting unsupported scenes or bypassing fire gates.

P2 references and later chemistry/nuclear algorithms are implemented in Rust CPU code first where a reference is required, then equivalent WGSL where justified. UI remains TypeScript. See [the execution mapping](plans/rust-wasm-migration/plan.md#how-every-later-phase-uses-the-chosen-stack).

## Fire audit follow-through

| Existing queue | Required fire work | Exit obligation |
| --- | --- | --- |
| R9 / M0-M1 | FIRE-W00-W01 | Baseline versus corrected assertions separated; minimal component/source/boundary/visual contracts |
| R10 / M2-M3 | Passive conservative transport only | No combustion or whole-catalogue allocation added to the demonstrator |
| R11 / M4 | FIRE-W02 | FIRE-PRECONDITIONS: finite ventilation and explicit hot-scene property coverage |
| R11 / M6 | FIRE-W03-W06 | FIRE-M6: Rust reference, WGSL parity, inventory/product correctness and integrated state-derived visuals |
| R11 / M7 | FIRE-W07 | FIRE-M7: supported fire-scene tests, persistence/recovery and measured GPU evidence |
| R16 / P4 | FIRE-W08 | Extend fuel/char/soot/radiation mechanisms and revalidate each newly enabled channel |

FIRE-W00 baseline and FIRE-W01 contracts are validated; FIRE-W02-W08 and corrected FIRE-A gates remain planned. The [12-case audit](evidence/fire-review-2026-09-23/FIRE_REVIEW.md) is immutable characterization, not delivered fixes. Do not port legacy fire rules unchanged, require a full duplicate TypeScript repair, or defer basic oxidizer/product/ignition correctness until the 40-element expansion.
