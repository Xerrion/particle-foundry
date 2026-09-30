# Physical model specification

**Evidence boundary:** this overview combines retained documentation, the [static review of src(3).zip](validation/source-review-2026-09-21.md), and the [historical fire characterization](validation/fire-combustion.md). The [combustion article](combustion.md) separately identifies its 27 September 2026 working-tree source review. Historical results are not current verification. Current Rust/GPU slices have separate evidence. Expanded chemistry and nuclear capabilities remain planned.

This is a reproducible coarse-grained sandbox, not a validated engineering simulator. [The knowledge map](README.md) routes current topics; [START_HERE.md](START_HERE.md) is the migration brief and [roadmap.md](roadmap.md) explains phase order. The full original model description is preserved in [the input archive](history/README.md).

## Legacy scale and units

| Quantity | Documented convention | Owner |
| --- | --- | --- |
| Cell width | 0.01 m | `web/src/legacy/simulation/physical-scale.ts` |
| Represented depth | 0.01 m | Same module |
| Cell volume | 0.000001 cubic metres | Derived from geometry |
| Outer physics interval | 1/60 s | Physical scale and simulation clock |
| Mass and parcel volume | kg and cubic metres | `world.massKg`, `world.volumeM3` |
| Velocity and pressure | m/s and Pa | World velocity/pressure fields |
| Thermal parcel enthalpy | J | `world.energy` |
| Chemical, kinetic, potential energy | Legacy kJ convention | World/diagnostics fields |

Legacy thermal state is total parcel enthalpy, `H = mass * specificEnthalpy`. The catalogue's phase curves use a 0.001 kg reference mass; runtime scales sensible and latent terms by actual mass. `cellHeatCapacity` is J/K at that reference mass. `lowerCellEnthalpy` and `upperCellEnthalpy` are reference-mass energies, not per-kg quantities. `heatTransferCoefficient` is a calibrated per-tick transfer coefficient, not conductivity in W/(m K). Displacement density, motion cadence, lifetimes, and generic rates remain gameplay parameters.

The **eight**, not five, documented legacy elemental models are specified in [elements.md](elements.md). Their values and old tests do not automatically validate new thermodynamic/property models.

## Legacy state and ownership

`web/src/legacy/simulation/world.ts` owns the authoritative arrays. A cell contains one material parcel. Mass, volume, enthalpy, chemical inventory, oxygen, velocity, and metadata move together. `swap()` moves state; `transport()` additionally accounts for changed gravitational potential energy. Fluid-column summaries are derived and must not own another copy of the fluid inventory.

A new parcel begins at one geometric cell volume. Phase changes preserve mass and enthalpy while recomputing a reference parcel volume from density. Sealed gas uses occupied geometric volume, while some open non-air gas uses its reference volume. This inconsistency is unresolved. One parcel per cell cannot geometrically represent arbitrary expansion or smooth subcell free surfaces.

## Legacy conservation and source accounting

Thermal diffusion makes paired opposite transfers. Collision/drag losses become thermal energy. Combustion spends a finite chemical inventory; batteries fund Joule heat from stored chemical energy. Painting, erasure, Blast and explicitly modelled biomass growth have external source/sink accounting, and some atmosphere-exchange paths are ledgered. The generic combustion fresh-air shortcut is an exception: the fire audit found oxygen supplied without a matching inventory debit or external flux ledger. Do not describe the legacy ledger as complete. [F01/F02 evidence](validation/fire-combustion.md#finding-to-work-traceability).

Diagnostics measure thermal, chemical, kinetic, and gravitational energies and total mass. `matterMassKg` means non-air matter, while `gasMassKg` includes all gases; these overlap and must not be added. Compare conserved totals after subtracting source-ledger changes. The original absolute/relative tolerances, 1e-9 and 1e-10, apply to their original controlled tests, not every future f32 or stochastic calculation.

These accounting rules do not prove correct forces or transport. The historical audit still finds model and integration failures. New solvers must report numerical energy error rather than hide it by arbitrary thermal corrections.

## Legacy fluid, phase, and gas behavior

The fluid model combines cellular parcel motion with derived column/region equalisation. Geometry blocks cross-wall transfers, supported pools level, and lighter oil can be displaced without becoming water. Viscous damping and velocity fields influence some behavior, but velocity is not the single consistent authority for displacement over time. Per-grain liquid outlet searches remain an architectural/performance issue.

The water family uses a bounded Clausius-Clapeyron approximation around its normal boiling point. Liquid and vapor interpret the same pressure-shifted latent interval; the vapor pressure/temperature coupling is implicit. A phase ID changes only after sufficient latent energy. Seeded boiling nucleation may collect already available latent energy from adjacent same-liquid parcels into an energy-funded bubble; it does not lower latent heat or borrow through walls. Hydrostatic equalisation does not teleport submerged steam bubbles directly to the surface.

Gas pressure is derived from an ideal-gas relation. Sealed connected gas shares a chamber pressure. Open/sealed volume conventions differ, and finite conservative expansion/venting is missing. Pressure gives paired momentum impulses with a legacy acceleration cap of 3 m/s squared per neighboring pair. Hydrostatic pressure is locally approximated from vertical liquid columns, not a compatible global MAC solve. These impulses and cellular gas swaps do not resolve acoustic or shock waves.

Gas rise and diffusion include seeded side drift and intermittent motion to avoid grid artefacts. Newly created gas may move immediately without reacting twice. The documented 2 percent per-step gas drag converts lost kinetic energy to heat; it is a calibrated model, not resolved turbulence.

## Legacy solids and rendering

Painted structural solids are anchored. Explicitly dynamic solid parcels fall, float, collide, and carry subcell displacement; a liquid that freezes remains dynamic rather than becoming unsupported anchored terrain. Collision paths prevent simple tunnelling. Resting contact is not intended to create heat. Subcell work includes a numerical energy bookkeeping convention described in the original model.

These are rigid cell parcels, not connected deformable or breakable bodies. There is no implemented structural fracture model. Cosmetic waves and light variation may change appearance but not occupancy or the physical energy budget; material colour variation follows moving parcels.

## Legacy chemistry, fire, and circuits

[Combustion](combustion.md) owns the current fire behavior, source/test map and
known limitations. Generic fuels spend finite chemical energy; the Fire brush
supplies ledgered external ignition heat. Oxygen shortcuts, incomplete generic
products, cold-FIRE ignition, contact-only quenching and smoke identity loss
prevent interpreting this as validated combustion physics. Scalar conservation
alone is insufficient. Historical measurements remain in [the fire evidence](validation/fire-combustion.md),
and future corrections are specified by [the fire integration plan](plans/fire-combustion/plan.md).

The legacy Gunpowder/Blast behavior releases finite heat and calibrated radial impulses with geometric blocking. It is not a resolved detonation, shock, or gas-product chemistry model, regardless of old naming. Painted metal has no failure threshold. The planned compressible mode must not reuse a visual blast ring as physical wave propagation.

Electricity connects orthogonally adjacent solid conductors using a quasistatic resistor network with Battery, Ground, Wire, Lamp, and Metal. Liquid metals do not conduct in the existing solid-only path. The historical audit identifies remaining convergence, heat-allocation, and terminal-work issues addressed by GPU M1. Capacitance, inductance, sparks, and realistic electrochemistry are not documented as implemented.

Legacy acid/base brushes represent fixed aqueous presets. A matched neighboring pair converts to an unspecified neutralised solution using finite stored reaction energy. Partial concentration, excess reagent identity in general cases, specific salt products, dilution, and corrosion are not adequately represented. GPU M6 and materials C2/C5 replace those rules with explicit amounts and products.

The 118-name reference table does not itself implement material physics. Eight separate legacy elemental models provide the limited behaviors listed in [elements.md](elements.md). Allotropy, isotope-specific chemistry, nuclear reactions, and universal chemical equilibrium are absent.

## Historical verification boundary

The input documentation reports R1-R8 and later targeted regressions as delivered in the legacy hybrid model. The retained audit reports 24 passing and 11 failing checks out of 35 for its recorded source snapshot. Neither that result nor any historical tool version is a new check of a present repository. See [history](history/README.md) and [remaining work](remaining-work.md).

The separate September 23 fire review characterizes 12 cases against the uploaded 43-file source snapshot. Its assertions include undesirable behavior and cannot serve as corrected acceptance. The evidence is preserved unchanged and new FIRE-A results must come from implementation work.

Preserve seeded replay, source accounting, input/control behavior, phase/boiling fixtures, pool/barrier geometry, material metadata transport, and existing circuit/chemistry tests while changing the model. Translate assertions tied to superseded physics openly; do not delete inconvenient references or treat regression success as laboratory validation.

## Planned model: P1 GPU redesign

E09 now implements an isolated saturated-water reference in
[`sim-cpu::water`](../engine/crates/sim-cpu/src/water/mod.rs).
It derives temperature, pressure and partial phase amounts from mass, U and actual
volume. [Thermodynamics](thermodynamics.md) owns its domain and source heat contract.
It does not implement coupled M4 flow or complete the current thermal rows.

Follow [GPU M0-M7](plans/fluid-gpu-redesign/plan.md). Introduce a versioned conservative state with component masses, phase volumes, SI internal energy, compatible face momentum/velocity, and matched divergence/gradient operators. Thermodynamic chamber pressure and projection pressure have different roles. Convert legacy enthalpy explicitly; do not rename it internal energy.

M1 includes only the minimum [matter identity/storage contract](architecture/matter-model.md). M2 is the Rust f64 CPU reference under [ADR-001](architecture/adr-001-rust-wasm-wgpu.md). M3 ports its same stage graph to WGSL/f32 and renders directly from GPU resources. This demonstrator has one liquid, carrier gas, fixed walls, and passive markers. No full-world per-frame readback, legacy transport over the same fields, chemistry, moving solids, phase change, or shocks.

M4 adds bounded water/air thermodynamics, actual-volume closure, phase amounts, pressure work, and finite venting. Its provisional water domain does not cover ice, all legacy fire/metal temperatures, or fusion plasma. M5 adds conservative sand/solid coupling and additional liquids. M6 integrates corrected supported reactions/circuits and controls. Its [FIRE-M6 gate](validation/fire-combustion.md#corrected-gates) requires shared finite O2, amount-based products, admissible ignition, bounded condensed-fuel release, thermal suppression, persistent smoke composition and derived visuals. M4 establishes ventilation/property preconditions; M7 adds FIRE-M7 promotion evidence. M3 remains nonreactive. M7 requires numerical, scene, performance, and device-recovery evidence for the promoted feature set.

A scene has one backend and one transport owner. Unsupported legacy scenes may remain separate. CPU reference worlds are tests, not per-tick mirrors of authoritative GPU state.

## Planned model: P2 pressure waves and breakage

Follow [W0-W4](plans/pressure-waves-breakage/plan.md). Select `lowMach` or `compressible` at scene load; do not run both transport systems over the same inventory. Compressible gas uses conservative species mass, momentum, and total energy, with a CPU reference before GPU parity, acoustic CFL control, local EOS pressure, wall reflection, and finite atmospheric exchange.

Finite reaction source terms drive pressure through the selected equations. Structural loads use stress, material profiles, geometry, elastic storage, damage, and fracture expenditure, not absolute gas pressure alone. Fragments keep mass/composition and couple through swept geometry and boundary work.

Material breakage defaults on for supported scenes. Disabling it prevents new fractures without repairing damage or freezing fragments; clear/reset preserves the setting. Initial compressible scenes support gas, reactive grains, and solids. Compressible liquid/multiphase scenes remain unsupported until independently validated.

## Planned model: P3-P5 matter and chemistry

Use separate elements, nuclides, species, material forms, and mixture recipes. Transport bounded component populations, derive composition/phase displays, preserve isotopic signatures, and compile one data source into CPU/GPU tables. A property is source-qualified per form/domain and explicitly measured, evaluated, predicted, overridden, or unknown.

Grow the material palette to 40, then 83 elemental identities with compounds and selected shared mechanisms. Chemistry uses balanced reaction extents and explicit products, preserves excess reactants, and has one energy-reference convention. Colour changes cannot substitute for concentration, passivation, excitation, magnetism, or material phase behavior. See [materials plan](plans/materials-and-chemistry/plan.md).

## Planned model: P6-P8 nuclear and exotic matter

Nuclide-specific populations, decay branches, radiation, and explicit reaction networks use the existing state/energy contract. Radiation is separate from ordinary matter transport. Nuclear Q, product motion, local deposition, remaining radiation, and escaped energy must not be double-counted. Conservation diagnostics distinguish chemical atom conservation from nuclear charge/nucleon and mass-energy accounting.

Add 20 nuclide-first and 15 exotic identities to cover the full 118, without claiming universal bulk properties. Fusion/fission are selected networks, not a universal hot-contact rule or automatic radioactive blast. Nuclear heat coupling is allowed only within a validated host-model domain. A generic plasma or high-temperature equation of state is not supplied by the periodic table or by the initial water solver.

Creative stabilization/predicted material profiles are saved and visibly labelled. Unknown data never becomes a silent zero or a measured-looking default. See [nuclear plan](plans/nuclear-simulation/plan.md) and [sources](sources.md).

## Promotion and numerical claims

Use [acceptance](validation/acceptance.md) for precision-specific deterministic and statistical tests, source/volume/energy budgets, independent references, domain checks, CPU/GPU equal-time comparison, and recovery. FPS and simulated seconds per wall second are different measurements. All numeric targets in plans are provisional design gates until recorded evidence supports them.

Only update a planned section to implemented when the corresponding source, tests, capability matrix, and evidence are present. P9 reconciles all model documentation with actual supported combinations.

## Planned engine implementation and measurement boundary

The TypeScript frontend remains. Rust owns portable state contracts, CPU reference/irregular algorithms and backend orchestration; WASM exposes the browser bridge; wgpu manages WGSL compute and same-device rendering. A GPU session's authoritative physical state remains in GPU buffers. Rust ownership does not require a synchronous CPU mirror.

[The engine boundary](architecture/engine-boundary.md) replaces fresh synchronous probes, Canvas-only rendering and submission-as-completion assumptions with bounded commands, async tick/epoch-stamped observations, accepted-time tracking and consistent snapshots. No stack or performance claim changes the physical model's validity range.

The supplied call graph has four unconditional pressure derivations per tick, including the nested gas step, with a possible fifth after venting. This is a static observation, not a profiling result; invalidation/reference tests govern which work can be removed. Historical timings and audit outcomes are not new measurements for this source.
