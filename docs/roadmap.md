# Development roadmap

**Revision:** 21 September 2026. **All P1-P9 phases are planned.** Start at [START_HERE.md](START_HERE.md).

Build a large falling-sand sandbox with all 118 element identities, a conservative GPU-capable physics core, useful compounds and material forms, and later isotope-specific nuclear behavior. Do not create 118 independent solver functions or claim 118 fully realistic elemental substances.

## Canonical sequence

The phase manifest is [development-phases.json](data/development-phases.json). Old references to delivery phases 1-5 described partial legacy work and are historical, not the order below. GPU milestones M0-M7 and active backlog IDs R9-R14 are retained for continuity.

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

The default execution sequence is P1 -> P2 -> P3 -> P4 -> P5 -> P6 -> P7 -> P8 -> P9. The full P3 data pipeline follows the redesign, but its **minimal identity/storage contract is an M1 prerequisite**. Basic chemistry needed to preserve existing interactions is part of GPU M6, not delayed until P4.

P2's fixed-wall numerical prototype is technically possible after M3, as the original follow-up plan states. The default work queue nevertheless finishes P1 first. P2 fragment coupling also requires M5. Parallel experiments may not enlarge or bypass M3's acceptance scope.

## P1: GPU and conservative-fluid redesign

Follow [the GPU plan](plans/fluid-gpu-redesign/plan.md). Capture the actual source baseline; introduce state ownership, commands, snapshots, identity hooks, and cache invalidation; build a conservative Rust f64 CPU reference; then port the same equations to WGSL under Rust wgpu with direct rendering. WASM delivers that engine to the retained TypeScript frontend. M4-M7 add bounded thermodynamics, phase amounts, finite venting, solids, existing reactions/circuits, controls, recovery, and measured promotion.

The first demonstrator ends at M3 and has one liquid, carrier gas, fixed walls, and passive markers. It does not add all elements or solve shocks. M7 is a gate for the explicitly supported redesign scenes, not for future catalogue expansion. Unsupported legacy scenes remain available as separate scenes in the old backend.

## P2: pressure waves and material breakage

Follow [the pressure-wave plan](plans/pressure-waves-breakage/plan.md), stages W0-W4. Add a separate compressible Rust CPU reference, WGSL/wgpu parity, finite energy-release scenarios, structural stress and failure, and moving fragments. Keep breakage enabled by default in supported scenes, with a persisted toggle that stops new fractures without repairing old damage.

The first supported compressible scenes contain gas, reactive grains, and solids. Liquid-containing compressible scenes require a separate validation gate; low-Mach water support does not imply compressible multiphase support.

## P3: scientific data and composition

Follow materials C0-C2 and [the matter contract](architecture/matter-model.md). Expand the M1 subset into versioned element, nuclide, species, material-form, mixture, and property registries. Import only permitted, version-pinned datasets. Preserve numeric legacy IDs through adapters. Validate units, sources, uncertainties, data domains, conservation, reaction product closure, isotope defaults, and save/load migration.

Deliver a shared CPU/GPU table compiler and explicit per-backend/per-model capability records. There are already 118 element identities; this phase makes them usable without pretending every property is known.

## P4: core 40 plus compounds

Follow materials C3-C6. Port the eight documented legacy elements, then implement the remaining 32 core identities using shared physical systems. Add water/air chemistry, specific reaction products, silica-based sand, glass, selected oxides, salt solutions, and simple fuel behavior alongside the elements.

Each core element needs at least one tested material form in a stated domain, source-qualified properties, a transport/thermal contract, a supported-interaction list, and a visible status. A melting or conductivity model may be the complete initial scope for an element. Advanced magnetic, semiconductor, catalytic, and nuclear claims remain disabled until their own gates pass.

## P5: expansion to 83

Follow materials C7-C9. Add the exact 43-element extended cohort in [the roster](elements.md), not an arbitrary definition of the first 83 atomic numbers. Expand selected alloy, passivation, solution, magnetism, excitation, and semiconductor systems with bounded models and distinct capability badges. Do not invent a gimmick or unsupported property to differentiate every element.

## P6: nuclides, decay, and radiation, covering 103 identities

Follow nuclear N0-N3. Activate versioned nuclide populations, decay branches and daughters, separate radiation transport, deposited/escaped energy accounting, isotope UI, and reproducible population evolution. Add the 20 nuclear-first element identities. Existing elements also gain selected radioactive isotopes.

The 103 target means 83 elemental-material identities plus 20 nuclide-first identities. It is not a claim of 103 ordinary bulk substances or 103 stable elements. Decay products may need reference entries from later cohorts; the data dependency graph is never limited by palette unlock order.

## P7: fusion and fission

Follow nuclear N4-N6. Add explicit, versioned reaction channels rather than a rule that hot elements automatically combine or radioactive material automatically explodes. Begin with selected hydrogen/helium fusion nuclides and a small isotope-specific fission/scattering/capture network. Define energy partition, product bookkeeping, time integration, and supported host-model conditions.

A nuclear reaction network is not a plasma-confinement, reactor-engineering, or universal high-energy fluid solver. A standalone bounded nuclear sandbox is a valid first delivery. Promotion of nuclear heating coupled to flow requires a compatible equation of state and a separately passed coupling gate.

## P8: exotic coverage to 118

Follow nuclear N7-N8. Add the final 15 superheavy identities through evaluated nuclide behavior where available and explicitly labelled predicted or creative-only material representations. Unknown density, phase transition, or bulk chemistry remains unknown. A creative stabilisation toggle is allowed only as saved, visible nonphysical behavior, with no hidden decay energy release.

## P9: whole-product release

Follow Q0-Q2 in [acceptance](validation/acceptance.md). Complete the supported-feature matrix, scene import/export, tutorial scenes, catalogue/probe explanations, long-session stability, CPU/GPU parity, recovery, hardware measurements, and documentation reconciliation. Validate combinations, not just isolated features. Unsupported combinations stay unavailable or clearly experimental.

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

P2-P9 extend the same engine contracts. Each later plan now specifies Rust/reference, GPU and TypeScript presentation ownership. A new chemistry or nuclear feature never receives its own whole-world transport loop. See the [cross-phase ownership table](plans/rust-wasm-migration/plan.md#how-every-later-phase-uses-the-chosen-stack) and [backend acceptance](validation/backend-migration.md).
