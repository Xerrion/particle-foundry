# Materials and chemistry implementation plan

**Status:** planned. **Roadmap:** P3-P5, after the GPU and pressure/breakage work. The identity subset is required earlier in GPU M1. [Start here](../../START_HERE.md).

## Goal and boundaries

Deliver 40 core elemental-material identities, then 43 more to reach 83, with useful compounds and shared physical mechanisms. Keep all 118 element identities in the reference catalogue throughout. The later 20 nuclear-first and 15 exotic identities belong to the nuclear plan.

This is a bounded sandbox chemistry model, not automatic chemistry for every possible mixture. A material is usable only in its supported phase/domain; an unsupported reaction is not evidence that the real substances are inert. State those limitations in the picker and probes.

The exact cohorts are in [elements.md](../../elements.md) and [element-roadmap.json](../../data/element-roadmap.json). The JSON is a planning manifest, not a database of validated transport constants.

## P3 / C0: registry and property ingestion

**Dependency:** P1 state contract and completed P2 in the default work queue. **Suggested owners:** existing `src/materials/` for legacy/presentation data, `crates/sim/src/matter/` for portable contracts, Rust offline data tooling when required, and generated shared data projections. Numerical references use `crates/sim-cpu/`; GPU execution uses `crates/sim-gpu/` and `shaders/chemistry/`.

Implement the full [matter model](../../architecture/matter-model.md): elements, nuclides, species, material forms, recipes, runtime components, property domains, source status, and capability declarations. Preserve the old numeric IDs. Imported data must have source/version/checksum and redistribution review. Compile immutable CPU/GPU property tables from one source.

Start with the existing eight elements plus water, air, and the compounds already produced by their reactions. Do not turn a missing value into a universal metal/gas profile. Overview sources can guide selection; property curves and nuclear data require the appropriate source type. [Sources](../../sources.md).

**Exit:** complete unique Z=1..118 identity registry; all required properties either sourced, explicitly approximated, or unsupported; invalid units, duplicate IDs, missing form references, and unlabelled predictions fail validation.

## P3 / C1: amount, phase, and isotope-preservation contracts

Implement active component storage, conservative splitting/mixing, recipe composition, selected solution concentrations, and conversion between kg and mol. Derive totals rather than maintaining independent authoritative ledgers of the same inventory. Ensure phase changes and supported chemistry preserve nuclide signatures even though radioactive evolution remains disabled until P6.

Keep molecular forms explicit. H2 is not H, O2 is not O, and water is not an elemental material. Keep graphite and diamond as distinct forms of carbon; do not infer structural transitions from a single temperature threshold. [Carbon reference](../../sources.md#rsc-carbon).

Migrate legacy pseudo-materials with clear labels. Existing generic oil, metal, sand, smoke, and wood remain usable in the legacy engine or a documented approximation; do not invent exact molecular formulas to make the new ledger appear complete.

**Exit:** closed transport, phase-cycle, split/merge, concentration, isotope-tag, and save/load fixtures conserve supported inventories. Legacy import reports unsupported states without modifying their originals.

## P3 / C2: explicit reaction engine and GPU integration

Define a reaction as balanced reactants/products, charge handling, applicability domain, a rate or declared instantaneous limit, energy convention, and supported phases/contact geometry. Compute limiting reaction extent in amounts, preserve excess reactants, and use accepted substep duration rather than a frame-dependent chance. Rate constants are measured/evaluated or visibly calibrated; a periodic-table group does not define kinetics.

For contact reactions, compute proposals from a read-only state, assign unique contact owners, and reserve shared reactants before committing. Multiple neighbors may not each consume the same available mass. Use deterministic gather/reduction ownership; do not make unordered floating-point scatter the correctness mechanism.

Compile the enabled reaction network's product closure into the active-component set. If any required product, phase model, isotope mapping, or storage capacity is unavailable, disable/reject that reaction path before executing it. Do not silently create an unspecified neutral product or discard a residual reagent.

Port a validated CPU implementation to the GPU stage graph. Sparse circuit heat remains governed by GPU M6; ordinary chemistry should not demand full-world CPU readback. Source heat and composition changes must feed EOS/phase closure and the stability test before committing.

**Exit / MAT-C2:** balanced definitions, positive amounts, limiting-reactant tests, geometry isolation, timestep refinement, source energy, product closure, contention, CPU/GPU parity, and snapshot tests pass.

## P4 / C3: migrate the existing eight elements

Port H, He, C, N, O, S, Fe, and Cu from their [documented legacy scope](../../elements.md#legacy-eight-element-models). Preserve tests for H2/O2 water production, carbon/sulfur oxidation, element conduction, and thermal mass scaling. Add the new amount/volume/energy tests instead of treating old cell-swap assertions as continuum references.

The GPU M4 water domain does not cover cryogenic nitrogen/oxygen or very hot metals. Introduce separate bounded property support, or keep those phases unavailable in the new backend with an explanation. A legacy property value is not automatically validated new-model thermodynamics.

**Exit:** the eight elements have backend-specific support records and migrated scenarios; no unsupported phase range is silently clamped. Thermal, chemical, and transport effects meet their new reference tests.

## P4 / C4: core matter and compound foundation

Add nonreactive or tightly bounded forms for the remaining core elements, then enable reactions in small verified batches. Start with representative shared models instead of one update function per element. Each element needs at least one declared initial form; elemental material coverage does not require every phase or reaction.

Recommended mechanism batches:

| Batch | Core identities | Initial work |
| --- | --- | --- |
| Atmosphere and excitation | He, Ne, Ar, Xe, H, N, O | Gas transport/properties; excitation off until energy-funded model exists |
| Reactive nonmetals and alkali metals | F, Cl, Br, I, P, S, Li, Na, K | Specific forms and finite balanced interactions, added one at a time |
| Common solids and structural metals | C, B, Si, Mg, Al, Ca, Ti, Cr, Zn, Fe, Co, Ni | Form properties, conduction, bounded oxidation/surface behavior |
| Conductive and melting-focused metals | Cu, Ag, Au, Ga, Sn, Hg, Bi, W, Pt, Pb | Conductivity and phase behavior within supplied data ranges |
| Nuclear-ready ordinary materials | Th, U | Bounded material behavior only; radioactive evolution remains a P6 feature |

Gallium's melting point near 29.76 degrees Celsius is a useful accessible phase-change fixture. Its other properties still need their own sources and domain checks. [Gallium reference](../../sources.md#rsc-gallium).

Build essential compounds alongside the elements:

| Family | Initial species/materials | Required distinction |
| --- | --- | --- |
| Water and atmosphere | H2O phases, defined air mixture, CO2 | Water phases share inventory; air is a mixture |
| Oxidation and minerals | Selected explicit metal oxides, SO2, silica, calcium carbonate | Products have their own composition and properties |
| Solutions | Water, dissolved ions, selected salts, acid/base equivalents | Concentration and excess reagent survive partial reaction |
| Fuels and particulates | One defined simple fuel, soot, declared smoke mixture | Generic game oil/wood do not silently become pure carbon |
| Structural presets | Silica-based sand and documented glass/stone profiles | Glass and stone are not universal pure chemical species |

**Exit:** every core identity has a bounded base-form implementation or a recorded blocking property gap; the phase cannot be declared complete with unlabelled placeholders. Compounds have explicit composition or a clearly scoped pseudo-material status.

## P4 / C5: selected reactive and solution behavior

Add a deliberately small network covering combustion, oxidation, metal-water behavior, neutralisation, dissolution, and precipitation only where the products and domains are defined. There is no requirement to invent all reactions involving all 40 elements.

Replace whole-cell acid/base cancellation with amount-based partial conversion. Preserve remaining acid/base concentration and identify the selected salt/ion products. Separate mixing, chemical equilibration, and heat release. Do not report accurate pH without an activity/equilibrium model appropriate to the selected solution.

Represent passivation with a surface state or layer model that actually changes access/rates. Oxidation must consume reactants and create products. A protective colour overlay is not a reaction barrier. Early surface kinetics may be calibrated and labelled.

Use one chemical-energy convention and consistent product formation references. NIST provides thermochemical records for many species, but data presence does not validate the reaction solver or establish its rate. [NIST WebBook](../../sources.md#nist-webbook).

**Exit:** independent balanced-equation fixtures, partial concentrations, reagent contention, wall separation, temperature-domain checks, finite energy release, and source-ledger tests pass on both supported backends.

## P4 / C6: core release gate

Build a material support matrix for all 40 core elements, their implemented forms, the compound foundation, and selected reactions. Count element identities separately from forms, compounds, and individual reaction channels. Record unsupported phases and all deliberately simplified behavior.

Add seeded scenes for melting/freezing, conduction, mixed gases, water/steam, oxidation, solutions, and a limited combustion loop. Verify each scene's actual model eligibility, save/load, paint/remove, pause/single-step, and CPU/GPU comparison. Preserve the original eight regression families.

**Exit / MAT-C6:** 40 core identities with tested base forms; essential compound scenarios; no lost composition; no implied full chemistry; source/uncertainty status visible; measured active-component memory and transfers.

## P5 / C7: add the 43 extended elements

Import the exact extended cohort and add bounded material forms using already validated mechanisms. Group the work by missing physical support rather than atomic-number order. Batch rare-earth metadata and common host behavior, but retain element-specific properties and provenance.

Do not infer a universal bulk phase, reactivity, magnetism, or toxicity behavior from a family label. A scientifically uncertain property remains unresolved; a creative approximation gets a separate status. Alloys and semiconductor compounds require their own definitions rather than inheriting a pure element's properties.

**Exit:** 83 total elemental-material identities with base-form coverage, separate from nuclear-first or exotic roster entries. Every addition passes catalogue, transport, thermal-domain, and snapshot checks.

## P5 / C8: richer shared material systems

Implement bounded examples of the following systems, with an independent reference and explicit exclusions for each:

| System | First bounded result | Exclusions until separately validated |
| --- | --- | --- |
| Alloys | Composition-preserving steel, brass, bronze, and one low-melting alloy profile | Universal phase diagrams or linear interpolation assumed for all alloys |
| Magnetism | A simplified field/force model with documented material and temperature dependence | Pure neodymium treated as an Nd-Fe-B magnet; magnetic confinement of fusion plasma |
| Excitation and emission | Energy-funded excitation/light and de-excitation bookkeeping | Permanent neon glow; colour alone used as a radiation model |
| Semiconductor behavior | A small calibrated electrical model with explicit materials and contacts | Arbitrary semiconductor physics inferred solely from periodic-table position |
| Catalysis and surfaces | A named catalyst changes a supported reaction rate without supplying net energy | Infinite reaction energy or universal platinum catalysis |
| Solutions and coatings | Selected concentration transport, corrosion/passivation, and bounded ion chemistry | Universal pH, activity, or electrochemistry |

The examples are proposed engineering scope, not new measured properties. Use a separately scoped feature flag for each system and do not make an unvalidated advanced mechanism a hidden requirement for basic element transport.

**Exit:** each enabled system has conservation, domain, CPU/GPU integration, scene, and regression evidence. Unsupported variants are excluded explicitly rather than implied by the element count.

## P5 / C9: 83-element promotion

Complete supported scene combinations and UI/probes, stress active-component capacity, verify all product closures, and profile densely mixed worlds. Confirm alloy melting, fragmentation, and reaction paths retain full composition. Retire legacy-only material paths only when their declared replacements pass.

**Exit / MAT-C9:** 83 base-form identities, the stated compound network, and the enabled advanced mechanisms have evidence; dataset versions and approximations are visible; the nuclide-preserving state is ready for P6 without another world-state rewrite.

## Scientific and engineering limits

No arbitrary periodic-table transmutation during chemistry. No automatic isotope fractionation, universal chemical equilibrium, unbounded property extrapolation, or exact biology for wood/plants. A game-friendly pseudo-material remains acceptable when its composition/energy claims are limited honestly.

All validation and phase evidence requirements apply from [acceptance](../../validation/acceptance.md). Later nuclear behavior is specified separately in [the nuclear plan](../nuclear-simulation/plan.md).

## Execution ownership under ADR-001

Use [the selected Rust/WASM + wgpu architecture](../../architecture/adr-001-rust-wasm-wgpu.md) for C0-C9. C0 migrates the single authored data pipeline deliberately, keeping the legacy TypeScript catalogue compatible until consumers transition. C1/C2 implement amount/reaction references in Rust and matching table-driven WGSL stages. C3-C9 extend shared forms, mechanisms and data rather than add independent Rust/TS/GPU implementations of every element.

The TypeScript picker, legends and probes consume compact generated metadata and tick-stamped observations. They do not mutate material arrays or require a full-world download to display concentrations. Reuse the [engine boundary](../../architecture/engine-boundary.md), source transactions, active-set capacity handling and versioned snapshots. Verify generated-data freshness and browser WASM/GPU behavior alongside the existing chemistry gates.
