# Nuclear simulation implementation plan

**Status:** planned. **Roadmap:** P6-P8. **Dependencies:** validated matter/composition contracts, selected material coverage, and source/energy accounting. [Developer entrypoint](../../START_HERE.md).

## Purpose and scope

Add isotope-specific decay, radiation, explicit fusion/fission channels, and eventual representation of all 118 element identities. The target is an educational game simulation with declared approximations, not reactor engineering, weapons design, radiation-dose prediction, or universal high-energy physics.

Nuclear behavior belongs to a nuclide and nuclear state, not to one `radioactive` flag on an element. Reference nuclear structure/decay and reaction datasets separately: [NNDC NuDat](../../sources.md#nndc-nudat), [IAEA LiveChart](../../sources.md#iaea-livechart), and [ENDF](../../sources.md#nndc-endf).

No nuclear milestone is satisfied by adding a glowing powder, deleting a material after a lifetime, or reusing the chemical Blast impulse. Nuclear energy and products must participate in the simulation state and ledgers.

## Shared nuclear contract

### Populations, not one atom per pixel

Cells carry macroscopic quantities. Use resolved component/nuclide populations or a documented statistical inventory within the [matter model](../../architecture/matter-model.md). Counts may be weighted real values in a population solver; an on-screen radiation track may represent many particles. Rendering sample counts must not determine the physical rate or total released energy.

A decay/reaction has explicit parent nuclides, daughter/product nuclides, branch information, applicability, energy convention, emissions, and data provenance. Preserve ground/isomer distinctions. Import the complete product/decay closure required by each enabled network, including reference-only identities outside the current palette tier. Missing data is not a stable state.

For active nuclear transformations in compounds, define how the changed nucleus affects chemical identity, charge, remaining atoms, and chemical energy. Restrict the initial active model to supported populations rather than pretending all molecular transmutations are already handled. Isotope tracking and active nuclear support are separate capabilities.

### Energy and mass convention

Use evaluated or precomputed channel Q-values with a documented atomic-mass versus nuclear-mass convention and electron/positron bookkeeping. Compute tables at build time or in the CPU reference with adequate precision. Do not subtract huge nearly equal rest energies in f32 at runtime.

Partition energy among product motion, local deposition, transported radiation, and escaped energy. Count neutrino escape where the selected decay model requires it. A gamma photon later depositing energy transfers it from the radiation inventory to matter; it does not create a second copy of the decay energy.

Ordinary chemistry conserves nuclide inventory. Nuclear reactions do not conserve individual element counts, and matter rest mass can change. Validate nucleon number and electric charge with the selected particle bookkeeping, and close the combined matter/radiation/released-energy account. Do not require exactly unchanged kg while also claiming a physically resolved nuclear mass defect. A negligible-mass-defect approximation must be named and expose its omitted mass term rather than falsify conservation diagnostics.

### Nuclear time and host-model limits

Decay rates must be independent of display FPS. Separate physical time, display slow motion, and any explicitly nonphysical nuclear-rate multiplier. Save all overrides, show them in the UI, and exclude overridden runs from scientific comparison fixtures.

A low-Mach fluid model does not become a high-temperature plasma solver when heat is added. The P1 water table does not cover fusion conditions. A first fusion network may run in a standalone homogeneous reaction fixture with prescribed conditions and accounted external energy. Coupling to flow requires a suitable equation of state, ionization/energy treatment, source stiffness control, and its own acceptance gate.

## P6 / N0: nuclide data and network compiler

**Suggested owners:** proposed `engine/crates/sim/src/nuclear/` for network/schema contracts, Rust offline data preparation, `engine/crates/sim-cpu/src/nuclear/` for references and `engine/crates/sim-gpu/` plus `engine/crates/sim-gpu/shaders/nuclear/` for GPU state/execution. These are work targets, not supplied files.

Import a small, version-pinned evaluated subset. Begin with H-1, H-2, H-3, He-3, He-4, representative carbon isotopes, and selected heavy-element nuclides needed by the first decay fixtures. Extend the subset only with explicit source and daughter closure. Stable test fixtures and synthetic numerical fixtures must be marked separately from evaluated real nuclides.

Validate nuclide keys, units, state identities, finite/stable/unknown half-life tags, branching and normalization conventions, daughter closure, uncertainties, and required emitted species. Keep nuclear quantities out of `MaterialId`. Do not use rounded standard atomic weights to construct isotope masses or mass numbers.

**Exit:** reproducible import/compiler; licensed/distributable input subset; exact schema validation; bad/missing branches and unsupported data domains rejected; CPU and GPU consume the same compiled network.

## P6 / N1: decay and transmutation reference

Build a Rust f64 reference before WGSL GPU implementation. For an isolated first-order parent, use the analytic relation

```text
lambda = ln(2) / halfLifeSeconds
N(t + dt) = N(t) * exp(-lambda * dt)
```

Use a numerically stable equivalent for the decayed fraction at very small `lambda * dt`. Coupled chains require a suitable positive population integrator or analytic/matrix treatment with a controlled error, not a single parent decay that ignores daughters decaying within the same interval. Stable and unknown are different cases.

Begin with selected alpha, beta, and isomeric-transition examples where the full accounting is specified. Add spontaneous-fission channels only with their product model; they are not the same process as neutron-induced fission. Use seeded statistical sampling only where it is appropriate and validate ensembles against a population reference.

Record daughter quantities, emitted particles, deposited and escaped energy, and any material/species conversion. Retry invalid source steps before commit. Parent depletion never creates negative populations; a GPU retry cannot apply the same decay twice.

**Exit:** half-life curves, simple chain references, branch totals, daughter closure, energy partition, timestep refinement, stable controls, and CPU/GPU population parity pass. Tiny and large populations use declared numerical regimes.

## P6 / N2: radiation transport and deposition

Represent photons, neutrons, and supported charged radiation separately from the ordinary element palette. A transported alpha nucleus and neutral helium material may share nuclear identity, but they are not the same physical state or transport object. Conversion/thermalisation needs a defined transfer.

Start with a bounded educational transport model and explicit interaction tables or labelled approximations. Use weighted particles, a coarse group model, or another declared representation. Include species/energy range, units, uncertainty, deposited energy, escaped energy, and validity limits in the contract. An absorber is not a universal shield for every radiation type.

Validate empty-space propagation, declared attenuation/deposition fixtures, source removal, boundary escape, and simultaneous hits. The GPU owns transport/deposition state for a GPU scene. Use owned gathering or staged reductions for deposits; do not require CPU readback of all events or floating-point atomics. Cap visual tracks independently of physical population.

**Exit:** emitted energy equals remaining radiation plus deposited and escaped energy within the declared budget; deposition events are applied once; no negative or duplicated populations; transport refinement and CPU/GPU statistical comparisons pass. Dose or real-world shielding claims remain unsupported.

## P6 / N3: nuclear-first 20 and user controls

Add Tc, Pm, Po, At, Rn, Fr, Ra, Ac, Pa, Np, Pu, Am, Cm, Bk, Cf, Es, Fm, Md, No, and Lr as the exact 20-element nuclear cohort. Their initial supported representation is nuclide-first. Bulk material behavior is a separate per-property/per-domain capability.

Expose selected isotopes for existing elements as well. Uranium material is not one isotope; isotope-dependent behavior cannot be represented by one element-level fission setting. Preserve natural-mixture defaults only when sourced and allow supported explicit isotope populations without silently resetting them on melting or save/load.

Add isotope/decay probes, a nuclear-enabled setting, reaction/decay histories, bounded radiation overlays, and tick-stamped diagnostics. Turning nuclear physics off preserves populations but does not secretly release decay heat. That setting is a visible simulation override, not a statement that the nuclides became stable.

**Exit / NUC-N3:** 83 material-first plus 20 nuclide-first identities; selected decay/radiation scenes; complete source/network closure; persistence/recovery; and per-feature capability badges. This is not yet general fusion or induced fission.

## P7 / N4: explicit fusion network

Begin with hydrogen and helium nuclides, not additional element IDs. One initial channel is deuterium plus tritium producing helium-4, a neutron, and released energy. [ITER](../../sources.md#iter-fusion) provides the conceptual reaction reference; it is not a substitute for sourced rate data or a validated plasma model.

Declare every enabled channel's products, rate-law applicability, energy partition, and numerical domain. A contact check or universal temperature threshold is not an adequate physical fusion model. Keep unsupported channels disabled; do not assume atomic numbers or mass numbers simply add without emitted products and energy bookkeeping.

First validate a prescribed homogeneous reaction box with finite reactants, source accounting, and reaction-network integration. Account for the energy maintaining prescribed conditions. Then port the accepted source stage to the GPU. Any visual containment, magnetic field, or stellar scene remains a separate explicitly simplified feature unless its own transport/field model has been implemented.

**Exit:** reaction-product and charge/nucleon balance, limiting inventories, no reaction outside supported conditions, energy partition, timestep refinement, and CPU/GPU reference comparison. No claim of physical confinement follows from this gate.

## P7 / N5: isotope-specific fission and neutron interactions

Add a small selected network of scattering, capture, and fission channels. Interaction probabilities depend on the supported nuclide and neutron-energy representation. A radioactive element is not automatically an efficient fission fuel, and an absorbed neutron does not always produce fission. [ENDF](../../sources.md#nndc-endf) separates evaluated reaction data from element identity.

Fission produces a distribution of products, not one universal daughter pair. For the first educational model, use explicitly balanced selected channels with declared approximations. If evaluated yields are introduced, distinguish independent from cumulative yields and do not sample two uncorrelated single-product distributions while claiming event-level conservation. Aggregate or untracked products need explicit inventory and decay limitations.

Track emitted and absorbed neutron populations, finite parent depletion, product/delayed-decay energy, and escape/deposition. Avoid a scripted recursive explosion loop. Use controlled numerical fixtures rather than real device geometry, material recipes, or criticality optimization. This phase validates a game reaction model, not an engineering prediction.

**Exit:** capture-without-fission controls, finite inventory, supported energy-group behavior, balanced products, emitted-particle accounting, residual heat without double counting, statistical/timestep convergence, and CPU/GPU parity for the declared network.

## P7 / N6: host coupling and nuclear promotion

Define the operator splitting between transport, nuclear sources, chemical changes, pressure/phase closure, and radiation deposition. Nuclear heat must drive the supported physical energy/pressure model, not a second radial impulse. Source stiffness may require local analytic integration, adaptive substeps, or slower physical progress. Do not exceed CFL or source constraints to maintain display FPS.

Freeze supported coupling domains. A source update that leaves them must retry where that can resolve the numerical issue, then stop/report or move to an explicitly selected model; repeated smaller steps do not make an invalid equation of state valid. Do not silently transfer the state into the legacy engine.

**Exit / NUC-N6:** N4 and N5 networks pass independently; at least one bounded host-coupled nuclear energy/deposition scene passes the full ledger and state-consistency tests; incompatible plasma/high-pressure/multiphase combinations are explicitly unavailable; save/load and device-loss replay preserve inventory and events.

## P8 / N7: final 15 exotic identities

Add Rf, Db, Sg, Bh, Hs, Mt, Ds, Rg, Cn, Nh, Fl, Mc, Lv, Ts, and Og. Import evaluated nuclear information where available; retain unknown properties when it is not. A short-lived or sparsely measured nuclide may be represented by an event/population and daughter chain without claiming a measurable bulk sample.

Do not give every superheavy element a fabricated density, melting point, or reactivity based solely on its periodic-table column. For example, RSC lists oganesson's bulk density and transition temperatures as unknown. Its nuclear lifetime should come from evaluated nuclear data, not an overview table. [Oganesson](../../sources.md#rsc-oganesson).

**Exit:** the full 118-identity roster has explicit representation/support status; enabled nuclide networks have source-qualified closure; missing bulk data is not labelled measured.

## P8 / N8: creative exotic material mode

Permit paintable visible quantities of exotic material through separate creative profiles. Predicted values identify their theoretical source and uncertainty; fictional values are `gameplay-override`, not `predicted`. Neither is labelled measured. A stabilisation setting suppresses decay and its heat while active, records the override in snapshots, and is visible in scene/probe metadata.

When stabilisation is switched off, resume from the stored population under the selected time convention. Do not apply an accumulated retroactive decay burst unless an explicitly separate, documented time-advance command requested it. Data-mode or creative-setting changes are ordered simulation commands.

**Exit / NUC-N8:** 118 reference identities and their supported material/nuclide/creative representations; stable snapshot/recovery of overrides; no hidden energy or population changes; clear scientific-versus-creative badges. Then proceed to [P9 acceptance](../../validation/acceptance.md).

## Execution ownership under ADR-001

N0-N8 extend [the same Rust/WASM + wgpu engine](../../architecture/adr-001-rust-wasm-wgpu.md). Nuclide/network schemas and compiled data are shared; Rust CPU code provides independent population/reference tests and preprocessing; WGSL handles supported bulk population/transport/deposition work through the owning GPU backend. Do not move every irregular event onto the GPU without measurements, but never download the whole world every tick to run a small nuclear callback.

Use precomputed/evaluated release values and adequate CPU preprocessing precision rather than f32 subtraction of large rest energies. Enforce source/commit IDs, active-network product closure and snapshot inclusion of radiation/nuclide state. The TypeScript isotope selector and provenance display use compact metadata and asynchronous observations.

[Backend migration gates](../../validation/backend-migration.md) continue to apply, including ABI, browser support, bounded buffers, recovery and statistical rather than bit-identical parity. The language choice does not validate new nuclear physics, expand an EOS domain or waive the existing NUC/RAD gates.
