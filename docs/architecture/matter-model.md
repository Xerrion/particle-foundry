# Matter, composition, and GPU state contract

**Status:** target architecture, not implemented. Minimal subset in P1/M1; full data pipeline in P3; nuclear populations activated in P6. Start from [the developer entrypoint](../START_HERE.md).

## 1. Separate identity from behavior

| Concept | Stable identity | Meaning and example |
| --- | --- | --- |
| Element | Atomic number Z | Chemical identity: carbon, hydrogen, uranium |
| Nuclide | Z, mass number A, nuclear-state key | A particular nuclear species, including isomers; do not infer A from rounded atomic weight |
| Chemical species | Explicit species ID | Composition, charge, and bonding model: H2, O2, H2O, Na+, or a declared pseudo-species |
| Material form | Explicit form ID | Structure and constitutive model: graphite, diamond, liquid metal, glass, granular silica |
| Mixture or preset | Explicit recipe ID | Initial composition and settings: air, salt solution, steel, or generic game oil |
| Runtime component | Scene-local index linked to stable definitions | A transported population of a species/form/isotopic signature |
| Local physical state | Backend-owned fields | Component amounts, internal energy, phase volumes, momentum, damage, and supported auxiliary state |

An element's proton number and isotope distinctions are scientific identities. [IUPAC](../sources.md#iupac-periodic-table) and [NNDC](../sources.md#nndc-nudat) are the identity references. The registry boundaries and storage choices above are project architecture decisions.

A material ID must not double as an atomic number. Preserve existing `MaterialId` and `ToolId` values through adapters and snapshot migrations. Multiple forms can reference the same element; a compound references multiple elements. A gas brush may create molecular H2, not atomic hydrogen. Free atomic/ionic species require separate definitions and models.

Graphite and diamond illustrate why one element can have different material properties. Phosphorus also has multiple forms with distinct behavior. Do not copy one phase curve or reaction rule across all forms. [Carbon](../sources.md#rsc-carbon), [phosphorus](../sources.md#rsc-phosphorus).

## 2. What M1 must implement, and what it must not

**Implement now:** distinct typed IDs, stable registry references, a schema version, an explicit `ComponentDefinition` layout, scene-local active-component indices, source-qualified defaults, and round-trip snapshots. Demonstrate one liquid, carrier gas, and passive composition markers. Reserve an optional versioned nuclear extension; do not allocate it when disabled.

**Do not implement now:** a full nuclide database in every cell, a universal chemistry framework, arbitrary isotopologue enumeration, plasma physics, or all 118 material behaviors. M1 only needs a concrete extension point tested with a second component and a future-extension round trip. Reject unknown extension versions explicitly rather than discarding their contents.

The M1 subset also defines the [FIRE-W01 contracts](../plans/fire-combustion/plan.md#3-authoritative-matter-and-energy-contracts): one component-mass authority for fuel/O2/products and any physical soot/residue, unified boundary fluxes, a chemical-energy reference, source transactions and derived visual outputs. This reserves capabilities without enabling chemistry in M3 or allocating full-catalogue fields.

An opaque legacy preset can use a named pseudo-species with an honest status. It must not advertise exact atom or isotope conservation until it has a defined composition. A reacting physical-mode pseudo-fuel must supply a balanced declared constituent/yield model; otherwise its combustion remains unsupported rather than silently passing scalar-mass checks.

## 3. Immutable definitions and source-qualified properties

Keep a single authored catalogue. Build CPU lookup tables and packed GPU tables from the same versioned input, never from separate handwritten constants. Immutable metadata may be shared by backends; mutable simulation state may not.

Every scientific property record needs a value or explicit missing state, unit, material form, temperature/pressure/composition domain, source ID, dataset version, uncertainty when available, and a status:

```text
measured | evaluated | predicted | gameplay-override | unknown
```

A constant property is a declared approximation over a bounded domain, not a universal law. Unknown is represented by a tagged missing value or `null`, never by zero, an invented default, or a NaN passed to a shader. If a required property is absent, reject that mode, retain a reference-only entry, or select an explicitly labelled creative profile.

For half-life use a tagged value such as `stable-in-selected-evaluation`, `finite`, or `unknown`. A finite record carries seconds, uncertainty, and a source; do not encode infinity in JSON. A default terrestrial isotope composition is not a universal pure isotope and may be unavailable for an element.

A nuclide table and a periodic-table overview serve different purposes. Use evaluated nuclear datasets for half-lives, states, branching, and reaction data; do not populate nuclear behavior by scraping rounded overview values. [Nuclear sources](../sources.md#nndc-nudat).

## 4. Amount authority and isotope representation

The fluid redesign's authoritative transported quantities remain **component masses in kg**. Derive species amounts in mol from the component's composition-aware molar mass. Derived component totals, elemental totals, nuclide inventories, density, and display labels are views, not independent stores to evolve or periodically reconcile.

A component definition links a species, a supported physical form/phase, and a nuclide signature for each constituent element. Start with fixed default signatures. Splitting or mixing material creates/resolves component populations without changing their signatures. P6 may add resolved isotopic components or a versioned statistical isotope-pool representation; choose one authority and document the conversion. Do not evolve both a mass field and an unrelated isotope counter.

A statistical isotope-pool approximation need not enumerate every molecule. It must conserve the number of each nuclide during ordinary chemistry. It does not model isotope-selective chemistry or isotope-dependent material properties unless those data and reactions are explicitly supplied. Supported heavy-water or other isotope-sensitive presets need distinct property coverage; a nuclide label alone does not provide it.

Ordinary chemistry redistributes atoms among species. Melting changes phase amounts; an allotrope change changes structure. Neither is permission to reset isotope composition. Nuclear transmutation changes nuclear identity and may invalidate the chemical species or material form. It requires an explicit daughter/product mapping, charge handling, and a new property lookup. Until a mapping exists, that species/reaction combination is unsupported.

In the first nuclear implementation, it is acceptable to restrict active transmutation to supported elemental or atomic populations while transporting isotope tags through other matter. The UI must distinguish `isotope tracked` from `nuclear reactions supported`.

## 5. Phase, mixture, and volume closure

A cell represents a coarse quantity of matter, not one atom. Multiple components may coexist in the fluid-owned volume. Group component masses into phases using the supported equation of state and mixing model. The sum of physical phase volumes and solid occupancy must equal the geometric cell volume within the solver tolerance.

Do not renormalize masses to make volume fractions add to one. Define supported excess-volume or immiscibility approximations where applicable. One material label cannot represent the authority for oil/water phase fractions, a partially evaporated liquid, or a solution's concentration.

Mixtures are not automatically reactive. Recipes define initial amounts; reaction networks define allowed changes. Alloys are composition plus structure, not a new element. `sand`, `wood`, `oil`, `smoke`, and `stone` may remain explicitly simplified presets. Do not silently relabel them as pure Si, C, or another element.

## 6. GPU layout and scaling policy

Use a small scene-local active-component set with structure-of-arrays storage as the initial implementation. Allocate only the components required by the scene and its enabled reaction-product closure. Keep global element/nuclide metadata in compact shared tables. Pack related records into a bounded number of storage buffers rather than binding one buffer per element.

At 480 x 270, one f32 scalar per cell costs 518,400 bytes. Allocating one for each of 118 elements costs 61,171,200 bytes, about 58.34 MiB, for a single quantity and single buffer generation. Ping-pong copies double that before species, phases, energy, momentum, solver scratch, or radiation. These are allocation calculations, not measured memory consumption. Thousands of nuclides make indiscriminate dense storage especially unsuitable.

P3 may introduce tiled sparse component pages when benchmarks justify them. An initial dense **active set**, rather than the complete catalogue, is preferable to premature complexity. Record the active-component capacity in scene capabilities. Expanding the set is an ordered transaction: reserve storage, rebuild mappings and reaction tables, validate, then commit at a safe tick boundary.

If a reaction or brush needs more capacity, grow safely, retry, or reject the transaction with a clear reason. Do not delete a trace species, merge unrelated nuclides, or silently disable a product to meet capacity. A deliberately aggregated unresolved group needs a declared composition/accounting model and cannot pretend to retain isotope-specific behavior.

Inspect adapter/device limits and the actual allocation manifest. Use portable f32 physics and integer indexing/ownership rules. Whole-grid dependencies use separate dispatches, not a workgroup barrier misused as a global barrier. Floating-point atomics and optional features are not baseline requirements. [WGSL](../sources.md#wgsl), [WebGPU limits](../sources.md#webgpu-limits).

Render from the authoritative GPU state. CPU probes use tick-stamped small staging-buffer readbacks; mapping a live simulation buffer is not a substitute for an asynchronous API. [Buffer mapping](../sources.md#webgpu-mapping).

## 7. Model/backend eligibility

Treat execution and physics as independent axes:

```text
backend: legacy | cpu-reference | wgpu
fluid model: legacy-cellular | lowMach | compressible
chemistry: off | selected network ID
nuclear: off | selected network ID
material breakage: off | supported damage model ID
creative overrides: explicit saved settings
```

`fluid-cpu` and `fluid-gpu` in the original GPU plan are compatibility labels for the Rust CPU and Rust/wgpu implementations of the new low-Mach model. The actual wgpu runtime backend is recorded separately; browser execution uses WebGPU. They are not separate definitions of matter. Preserve backward-compatible labels through a mapping if needed.

A capability declaration includes supported components/forms, temperature and pressure domains, chemistry/nuclear channels, boundary types, maximum component counts, and tested combinations. Availability of a feature in isolation does not imply every combination works. Scene loading validates all requirements before changing ownership.

## 8. Source application, energy, and transactional updates

Each accepted substep uses one documented stage graph shared by CPU and GPU. Chemistry and nuclear modules propose local amount and energy changes; the owning backend applies them at a defined split stage. Enabled source stages participate in timestep/error control. Events use stable IDs and an ordered commit; failed steps do not partially consume inventory or replay events twice.

Use SI kg, m, s, K, Pa, J, and mol at module boundaries. Convert legacy Celsius and mixed J/kJ fields explicitly. Legacy parcel enthalpy H is not internal energy U. Use the fluid plan's defined reference-state conversion and volume work; reject unsupported conversions.

Track thermal/internal, chemical, kinetic, gravitational, elastic, and nuclear/radiation energy channels separately. Use one consistent chemical-energy convention: formation energies in the state **or** an explicit stored reaction-energy account, with a documented translation. Do not charge both for the same reaction. Catalysis changes a supported rate model, not the reaction's available energy.

Nuclear Q-values require their own reference and rest-mass convention. Store evaluated/precomputed release values; do not subtract enormous nearly equal rest energies in f32. A reaction's release is partitioned between products, local deposition, transported radiation, and escaping energy. Depositing all Q as heat and then also transporting energetic particles double-counts energy. See [nuclear design](../plans/nuclear-simulation/plan.md).

## 9. Snapshots, checkpoints, and UI

Snapshots include schema, catalogue and data hashes, active-component mappings, model/backend requirements, units/reference convention, all authoritative component quantities, seeds/event state, ledgers, damage, settings, and accepted simulated time. Derived caches may be rebuilt. Component indices alone are not stable save identities.

For nuclear scenes include nuclide populations or the chosen resolved-component representation, the active network/version, radiation inventory, and all nonphysical overrides. A CPU checkpoint is a consistent committed state, not arrays read across different GPU ticks. Recovery replays ordered commands from that checkpoint or reports an explicit restart; it does not hot-swap into unrelated legacy equations.

The picker offers a curated material palette, a full periodic-table reference, and an advanced isotope selector. Show support separately for bulk material, chemistry, nuclear behavior, and creative overrides. Probes display actual composition, form, pressure meaning, isotope summary, and data/model limitations. A render glow is an energy/excitation view, not proof of radiation or a free heat source.

## 10. Required contract tests

Round-trip legacy IDs and new stable identities; move, split, merge, freeze, boil, react, save, reload, and fragment supported matter without losing composition. Check exact stoichiometric atom/charge balance for chemical definitions and population/energy balance for enabled nuclear channels. Test component-capacity overflow, absent properties, invalid domains, unknown data versions, delayed probes, and failed-step rollback.

M1 needs a small subset proving the seam. P3 and P6 progressively expand it. The detailed gates are in [acceptance](../validation/acceptance.md).

## 11. Rust, WASM and WGSL ownership

[ADR-001](adr-001-rust-wasm-wgpu.md) and [the engine contract](engine-boundary.md) select the implementation. Place portable identities, units and schemas under `crates/sim/`; numerical references under `crates/sim-cpu/`; resource/layout/execution under `crates/sim-gpu/`; and generated browser bindings behind `crates/wasm/`. WGSL implements the GPU numerical stages, not ordinary Rust compiled to WASM.

Keep one authored data source with generated Rust/TypeScript/GPU projections. The existing TypeScript catalogue remains the legacy source until the explicit data-pipeline migration. Do not manually copy every material into a Rust match and a separate WGSL switch. Version the GPU layout and test byte offsets, strides, IDs, units and precision conversions. Active fields in a GPU session are not mirrored each frame into the Rust reference.

No chemistry/nuclear phase may add synchronous full-world probes or its own transport authority. Its new state is part of the selected backend and its snapshot/capability contracts. Native f64 references and browser GPU parity remain separate evidence requirements.

## 12. Combustion inventory and visual identity

[The fire plan](../plans/fire-combustion/plan.md) specializes these contracts in P1/M6. O2 is an actual component, not a consumable counter independent from an OXYGEN material mass. Fuel depletion and oxidation update defined products and residuals in the same transaction. All compatible fuel channels use the same transported oxidizer amounts and reserve shared reagents before commit.

`FIRE` remains a legacy/tool identity and a derived visual state, not authority for ignition or fuel. A cooled label cannot trigger a reaction. Smoke/soot visibility is separate from its physical inventory; expiration of a cosmetic particle cannot change component masses or replenish air. Define physical removal/deposition/reaction explicitly when supported.

Persist reaction-network and property versions, approximations, component mappings, condensed residues/soot and source state. Gate hot fire/water combinations against the full supported property domains at load and during source application. P4 extends these same records and gates rather than creating a separate oxygen, fuel or smoke system.
