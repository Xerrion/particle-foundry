# Development roadmap

**Status:** 27 September 2026. **P1 is the sole active release goal; P2-P9 are deferred.** M0 baseline evidence is recorded; no redesigned scene is promoted. Start at [START_HERE.md](START_HERE.md).

The active goal is to replace the current sandbox engine with Rust/WASM and GPU simulation/rendering while preserving the [frozen current-sandbox scope](plans/rust-wasm-migration/current-sandbox-scope.md). P1 must prove that the existing materials, interactions, tools and controls work, then pass browser correctness, performance and recovery gates before the GPU backend becomes the default.

The retained P2-P9 designs describe possible expansion to pressure waves, breakage, more materials, chemistry and nuclear behavior. They are deferred backlog, not an execution queue. Do not start any deferred phase without a separate user decision, even after P1 passes. The long-term 118-identity target does not authorize 118 independent solver functions or claims of 118 fully realistic elemental substances.

## Active scope and deferred phase register

The phase manifest is [development-phases.json](data/development-phases.json). Old references to delivery phases 1-5 described partial legacy work and are historical, not the order below. GPU milestones M0-M7 and backlog IDs R9-R14 are retained for continuity.

<!-- BEGIN PHASE TABLE -->
| Phase | Deliverable | Local milestones | Exit gate |
| --- | --- | --- | --- |
| P1 | Rust/WASM + wgpu and conservative-fluid redesign | M0, M1, M2, M3, M4, M5, M6, M7 | GPU-M7 |
| P2 | Compressible pressure waves and material breakage | W0, W1, W2, W3, W4 | WAVE-W4 |
| P3 | Matter identities, composition, and scientific-data pipeline | C0, C1, C2 | MAT-C2 |
| P4 | Core 40 elemental materials and essential compounds | C3, C4, C5, C6 | MAT-C6 |
| P5 | Extended materials and chemistry: 83 elements | C7, C8, C9 | MAT-C9 |
| P6 | Nuclide populations, decay, and radiation: 103 elements | N0, N1, N2, N3 | NUC-N3 |
| P7 | Explicit fusion and fission reaction networks | N4, N5, N6 | NUC-N6 |
| P8 | Superheavy and creative-mode coverage: 118 elements | N7, N8 | NUC-N8 |
| P9 | Whole-product validation and release hardening | Q0, Q1, Q2 | REL-Q2 |
<!-- END PHASE TABLE -->

P1 is the only active phase in this table. P2 -> P3 -> P4 -> P5 -> P6 -> P7 -> P8 -> P9 preserves conditional future dependency order if the user activates later work. A satisfied dependency is not activation. Finishing P1 does not advance the work queue to P2.

The full P3 data/reaction pipeline remains deferred. Its **minimal identity/storage contract is an M1 prerequisite**; only property, composition and product support required by today's sandbox belongs in P1. Existing chemistry, all eight existing elemental models and their bounded properties must be migrated through M4-M6 and accepted by M7. A dependency on those behaviors cannot be discharged by assigning them to P3 or P4.

P2's fixed-wall numerical prototype is technically possible after M3, as the original follow-up plan states, but is deferred with P2. P2 fragment coupling also requires M5. These technical dependencies do not authorize parallel expansion or enlarge M3's acceptance scope.

## P1: GPU and conservative-fluid redesign

Follow [the migration plan](plans/rust-wasm-migration/plan.md), [current-sandbox scope](plans/rust-wasm-migration/current-sandbox-scope.md) and [GPU numerical plan](plans/fluid-gpu-redesign/plan.md). Capture the actual source baseline; introduce state ownership, commands, snapshots, identity hooks, and cache invalidation; build a conservative Rust f64 CPU reference; then port the same equations to WGSL under Rust wgpu with direct rendering. WASM delivers that engine to the retained TypeScript frontend. M4-M6 deliver the bounded thermodynamics, phase amounts, finite venting, solids, existing materials and reactions/circuits needed for the frozen scope. M7 accepts its complete coverage, browser controls, recovery and measured default-backend promotion.

The first demonstrator ends at M3 and has one liquid, carrier gas, fixed walls, and passive markers. It proves the GPU path; it does not complete the migration. M7 requires the whole frozen current-sandbox scope, including H, He, C, N, O, S, Fe and Cu. Each declared domain needs evidence; an unsupported existing feature is an open migration blocker, not permission to omit it. Correct documented defects against accepted requirements rather than reproducing historical bugs. Retain the legacy backend and original scenes while migration and promotion gates remain open.

## P2: pressure waves and material breakage

**Deferred.** Requires a separate user activation after P1 acceptance.

Follow [the pressure-wave plan](plans/pressure-waves-breakage/plan.md), stages W0-W4. Add a separate compressible Rust CPU reference, WGSL/wgpu parity, finite energy-release scenarios, structural stress and failure, and moving fragments. Keep breakage enabled by default in supported scenes, with a persisted toggle that stops new fractures without repairing old damage.

The first supported compressible scenes contain gas, reactive grains, and solids. Liquid-containing compressible scenes require a separate validation gate; low-Mach water support does not imply compressible multiphase support.

## P3: scientific data and composition

**Deferred.** Requires a separate user activation and its recorded phase dependencies. P1 supplies the bounded data and reaction support needed by the existing sandbox before this generalized pipeline starts.

Follow materials C0-C2 and [the matter contract](architecture/matter-model.md). Expand the M1 subset into versioned element, nuclide, species, material-form, mixture, and property registries. Import only permitted, version-pinned datasets. Preserve numeric legacy IDs through adapters. Validate units, sources, uncertainties, data domains, conservation, reaction product closure, isotope defaults, and save/load migration.

Deliver a shared CPU/GPU table compiler and explicit per-backend/per-model capability records. There are already 118 element identities; this phase makes them usable without pretending every property is known.

## P4: core 40 plus compounds

**Deferred.** Requires a separate user activation and its recorded phase dependencies.

Follow materials C3-C6. Revalidate the eight elements already migrated in P1 against the expanded data system, then implement the remaining 32 core identities using shared physical systems. Extend water/air chemistry, specific reaction products, silica-based sand, glass, selected oxides, salt solutions, and simple fuel behavior beyond the accepted P1 scope.

Each core element needs at least one tested material form in a stated domain, source-qualified properties, a transport/thermal contract, a supported-interaction list, and a visible status. A melting or conductivity model may be the complete initial scope for an element. Advanced magnetic, semiconductor, catalytic, and nuclear claims remain disabled until their own gates pass.

## P5: expansion to 83

**Deferred.** Requires a separate user activation and its recorded phase dependencies.

Follow materials C7-C9. Add the exact 43-element extended cohort in [the roster](elements.md), not an arbitrary definition of the first 83 atomic numbers. Expand selected alloy, passivation, solution, magnetism, excitation, and semiconductor systems with bounded models and distinct capability badges. Do not invent a gimmick or unsupported property to differentiate every element.

## P6: nuclides, decay, and radiation, covering 103 identities

**Deferred.** Requires a separate user activation and its recorded phase dependencies.

Follow nuclear N0-N3. Activate versioned nuclide populations, decay branches and daughters, separate radiation transport, deposited/escaped energy accounting, isotope UI, and reproducible population evolution. Add the 20 nuclear-first element identities. Existing elements also gain selected radioactive isotopes.

The 103 target means 83 elemental-material identities plus 20 nuclide-first identities. It is not a claim of 103 ordinary bulk substances or 103 stable elements. Decay products may need reference entries from later cohorts; the data dependency graph is never limited by palette unlock order.

## P7: fusion and fission

**Deferred.** Requires a separate user activation and its recorded phase dependencies.

Follow nuclear N4-N6. Add explicit, versioned reaction channels rather than a rule that hot elements automatically combine or radioactive material automatically explodes. Begin with selected hydrogen/helium fusion nuclides and a small isotope-specific fission/scattering/capture network. Define energy partition, product bookkeeping, time integration, and supported host-model conditions.

A nuclear reaction network is not a plasma-confinement, reactor-engineering, or universal high-energy fluid solver. A standalone bounded nuclear sandbox is a valid first delivery. Promotion of nuclear heating coupled to flow requires a compatible equation of state and a separately passed coupling gate.

## P8: exotic coverage to 118

**Deferred.** Requires a separate user activation and its recorded phase dependencies.

Follow nuclear N7-N8. Add the final 15 superheavy identities through evaluated nuclide behavior where available and explicitly labelled predicted or creative-only material representations. Unknown density, phase transition, or bulk chemistry remains unknown. A creative stabilisation toggle is allowed only as saved, visible nonphysical behavior, with no hidden decay energy release.

## P9: whole-product release

**Deferred.** Requires a separate user activation and its recorded phase dependencies.

Follow Q0-Q2 in [acceptance](validation/acceptance.md) for the expanded P2-P8 product. Extend the supported-feature matrix, scene import/export, tutorial scenes, catalogue/probe explanations, long-session stability, CPU/GPU parity, recovery, hardware measurements, and documentation reconciliation to those later capabilities. P1 must already pass correctness, actual browser execution, performance, controls and loss/reset recovery for the complete current sandbox; those checks are not deferred to P9. Validate combinations, not just isolated features. Unsupported future combinations stay unavailable or clearly experimental.

## What roster counts mean

| Cohort | New identities | Cumulative | Primary representation |
| --- | ---: | ---: | --- |
| Core | 40 | 40 | At least one bounded elemental-material form |
| Extended | 43 | 83 | Additional bounded material forms and shared mechanisms |
| Nuclear | 20 | 103 | Nuclide-first; bulk behavior only when separately justified |
| Exotic | 15 | 118 | Nuclides plus explicit predicted/creative-only material options |

These are project priorities, not scientific classifications. Thorium and uranium are in the core material cohort even though their nuclear behavior comes later. All 118 catalogue identities remain available for reference throughout. Scientific identity and isotope references are in [sources](sources.md#iupac-periodic-table).

## Definition of done

Each phase requires implementation, independent reference tests, conservation checks, backend/model capability declarations, documented domain limits, source provenance, migration/recovery tests where relevant, and a recorded evidence bundle. A passing old regression suite does not demonstrate a new physical model. Use [remaining work](remaining-work.md) to track progress; never infer completion from a phase heading or element count.

## Stack decision applied across P1-P9

[ADR-001](architecture/adr-001-rust-wasm-wgpu.md) selects TypeScript for the existing frontend, Rust for engine contracts and CPU reference/irregular algorithms, WASM for browser delivery and Rust wgpu/WGSL for GPU compute and direct rendering. This supersedes the previous TypeScript reference default without changing phase dependencies or roster cohorts.

P1 E00-E14 in [the migration work plan](plans/rust-wasm-migration/plan.md) implements M0-M7. Rust/bootstrap, generated data and the async command boundary are M1 work; M2 is the Rust f64 reference; M3 is WGSL parity and browser integration. E04's circuit reference is an independent branch required by M6, not a blocker for M2's fluid contract.

If separately activated, P2-P9 extend the same engine contracts. Each later plan specifies Rust/reference, GPU and TypeScript presentation ownership. A new chemistry or nuclear feature never receives its own whole-world transport loop. See the [cross-phase ownership table](plans/rust-wasm-migration/plan.md#deferred-phase-ownership) and [backend acceptance](validation/backend-migration.md).

## Fire-specific integration and phase gates

[The fire integration plan](plans/fire-combustion/plan.md) adds FIRE-W00-W08 within the existing sequence. M0 preserves the 12 prior characterization cases and creates corrected fixtures; M1 reserves fuel/O2/product/boundary/source/visual contracts; M4 passes finite ventilation and hot-domain preconditions; M6 implements coherent Rust then WGSL combustion and derived visuals; M7 requires FIRE-M7 before any fire-scene promotion.

The [FIRE-A gates](validation/fire-combustion.md#corrected-gates) explicitly cover closed-boundary oxygen, cold-FIRE ignition, shared finite O2, product formation, condensed-fuel release, water-dose suppression, smoke persistence, substep rates, contention, CPU/GPU parity and recovery. Passing scalar energy checks is not a substitute for composition correctness. The provisional 350 C water ceiling does not authorize legacy 450 C flame/water scenes.

Deferred P4/C3-C6 extends fuel chemistry, char, soot and selected radiative transfer with FIRE-W08 only if separately activated. P1 oxygen/product/ignition correctness and current-feature coverage remain required. Deferred P2 would use the same finite-reactant/source accounting with independently validated compressible physics. M3 and the conditional 40/83/103/118 delivery cohorts are unchanged. No implementation status is advanced by this documentation update.
