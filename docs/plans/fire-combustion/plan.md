# Fire and combustion integration plan

**Revision:** 23 September 2026. **Status:** planned, not implemented. **First task:** P1 / M0 / E00 / FIRE-W00. Start with [START_HERE.md](../../START_HERE.md).

This plan incorporates the [preserved fire audit](../../evidence/fire-review-2026-09-23/FIRE_REVIEW.md) into the existing Rust/WASM + wgpu redesign. It is a bounded game-combustion model, not an implementation of a complete fire-engineering solver. [ADR-001](../../architecture/adr-001-rust-wasm-wgpu.md), the [GPU plan](../fluid-gpu-redesign/plan.md) and the [engine boundary](../../architecture/engine-boundary.md) remain in force.

**Do not port the old fire rules line by line. Do not postpone basic oxygen, product or ignition correctness until P4. Do not add combustion to M3.** Preserve the energy-ledger discipline and useful brush behavior while replacing inconsistent physical rules inside P1/M6.

## 1. Evidence and interpretation

The imported audit records 12 characterization checks against the supplied `src(3).zip`: ten isolated subsystem checks and two complete physics-tick checks, F02 and F08. It confirmed the closed-boundary oxygen and cold-FIRE ignition defects through the full pipeline. Other findings are composition defects, documented scope limitations or simplified gameplay rules. The audit runner asserts those historical observations, including the bugs. Its success is not a pass of the corrected specification.

Read [validation/fire-combustion.md](../../validation/fire-combustion.md) for the finding-to-gate mapping, evidence limits and reproduction policy. The report, runners, results and provenance are retained unchanged; none was rerun for this documentation update. Application source is not modified or bundled. All work and fixture statuses remain planned.

## 2. Scope and milestones

| Existing milestone | Fire work | Required boundary |
| --- | --- | --- |
| P1/M0, E00 | FIRE-W00: preserve observations and freeze corrected specifications | Do not treat expected old bugs as desired regressions |
| P1/M1, E02 | FIRE-W01: component/source/boundary/visual contracts | Small active set only; no working chemistry required |
| P1/M2-M3 | Reuse conservative passive-marker transport and same-device rendering | Demonstrator remains nonreactive |
| P1/M4, E09 | FIRE-W02: finite species ventilation and supported hot-scene thermodynamics | FIRE-PRECONDITIONS before coupled fire scenes |
| P1/M6, E11 | FIRE-W03-FIRE-W05: Rust reference, reduced fuel/suppression model, then WGSL parity | Core combustion and products before visual promotion |
| P1/M6, E12 | FIRE-W06: tools, derived flames and probes | FIRE-M6 for the declared integration scenes |
| P1/M7, E13-E14 | FIRE-W07: persistence, recovery and measured promotion | FIRE-M7 in addition to GPU-M7 |
| P4/C3-C6 | FIRE-W08: additional fuel models and selected mechanisms | Extend an already coherent P1 foundation |

Choose and version a minimal fuel-vapor/O2 channel with explicit products at M0/M1, subject to complete property coverage. One validated channel is enough to establish the architecture; it is not permission to advertise all fuel chemistry. For each wood/oil/plant scene promoted at M7, also implement its declared reduced condensed-fuel release model. Unsupported scenes remain separate legacy or experimental scenes with clear capability records.

Soot is optional for a reaction channel. If it is emitted as physical smoke, its yield and composition must be defined and its inventory transported. Residue/char needed to close a chosen fuel model is mandatory for that model. Rich soot oxidation, deposition, detailed pyrolysis and radiative heat transfer may be later extensions.

## 3. Authoritative matter and energy contracts

Use the existing [matter contract](../../architecture/matter-model.md). Authoritative transported quantities are component masses in kg; derive moles from composition-aware molar masses. Register fuel vapor, O2, nonreacting carrier gas, products and any supported condensed fuel, residue or soot through the scene-local active set and product closure.

Do not maintain a second spendable `oxygenKg` inventory that can reach zero while an independent pure-O2 material retains all its mass. Do not make `FIRE`, `burning`, color, lifetime or shader intensity authoritative evidence of available fuel, ignition or product formation. A legacy material/tool ID may survive through an adapter, not as a new chemical species by accident.

At minimum, define these contracts before the M1 layout freezes:

| Contract | Required meaning |
| --- | --- |
| Reactant/product amounts | One authority for fuel, O2, products, condensed residues and any physical soot |
| Boundary state | One open/closed face contract shared by transport, pressure, heat and chemistry |
| Reaction network | Stable version, balanced composition mapping, supported domains, rate/extinction model and product closure |
| Source transaction | Bounded extent plus simultaneous reactant, product, energy and coupled-volume changes |
| Thermal/chemical ownership | SI J and one reference convention; no duplicate formation-energy and explicit-store release |
| Visual diagnostics | Derived heat-release rate, thermal state, supported luminous fraction and soot concentration; never additional physical stores |
| Persistence | Network/property versions, active mappings, component amounts, residues, source/event state and saved approximation settings |

A pseudo-material such as wood or game oil may be calibrated, but a reacting version needs a declared constituent model and balanced yields. A truly opaque legacy preset cannot claim exact element/isotope balance; reject it from physical chemistry until a mapping exists. Source-qualified properties and visibly calibrated rates are different categories.

## 4. Finite ventilation and one oxidizer model

A closed outer face has zero species/mass flux unless an explicit source is declared. Drawing walls and enabling closed world boundaries must not produce incompatible oxygen semantics. Open boundaries specify reservoir composition and state; the transport solver computes finite signed fluxes, including incoming/outgoing energy. Connectivity can identify topology, not refill every reachable interior cell.

All enabled compatible fuel channels consume O2 from that same transported inventory. Equivalent compositions created with an Air brush and explicit gas brushes must not have different reaction eligibility solely because of a legacy material label. This does not require every substance to burn: chemistry still follows selected channels and documented thermal/mixing conditions.

First validate ventilation with passive species at M4. Then repeat it with combustion at M6. An open domain does not imply instantaneous gas equilibration; a closed chamber can stop burning with O2 remaining if the selected extinction criterion says so.

## 5. Reduced combustion model

### Reaction extent and shared inventory

For each enabled channel, propose a nonnegative reacted extent over the accepted interval. Bound it by the declared kinetics/mixing approximation and every required reactant amount. Use the channel stoichiometry to compute products and residual reactants. Rate constants must have units and a calibrated or sourced status; their values are implementation work, not invented in this plan.

Individual bounds are insufficient when several cells or channels request the same reactant. Calculate proposals from a consistent read-only stage, reserve shared inventories using deterministic ownership or reproducible gather/reduction, then commit accepted extents once. Report any priority scheme and test spatial/order sensitivity. Do not use unordered floating-point scatter as the conservation mechanism.

### Ignition and extinction

Derive reaction eligibility from the supported reactant, thermal and mixing state. A hot region can supply existing thermal energy; an ignition tool supplies a ledgered external input. Any explicitly modeled activation/excitation energy must be transferred from an existing or external account. Do not add an arbitrary permanent energy debit per reaction merely to compensate for the old label shortcut.

Test a cold former flame and stale `burning` flag before subsequent reaction stages. Neither is sufficient to ignite cold H2/O2. A selected self-igniting or catalytic channel, if introduced later, needs its own declared physical conditions rather than an exception based on color or material naming.

Extinction is a result of the reduced model's local state and support limits. Distinguish extinguished combustion from a still-hot, glowing object and from ongoing nonoxidative fuel release.

### Condensed fuel and products

For promoted wood/oil/plant presets, heating releases a bounded amount of defined volatile fuel through a declared pyrolysis or evaporation approximation. Debit condensed mass, create the appropriate vapor and residues, and account for release/phase energy using the chosen convention. Surface/char oxidation can be another channel; it is not the same event as gas-phase flame emission.

Retain unreacted fuel and products continuously. Exhausting an energy store must not turn an entire parcel into unspecified smoke. A simple mass-balanced pseudo-fuel is acceptable; an undeclared chemical recipe or arbitrary soot source is not.

### Water and smoke

Thermal coupling, finite water mass, phase change and supported vapor/mixing effects determine suppression. Remove the physical-mode adjacency kill switch. Dose-response tests must stay within the validated property domain and compare against the zero-dose limit; they do not assume every fire responds monotonically to water in all circumstances.

Smoke is a declared physical mixture or particulate population. It may remain after visible flame ends. Cosmetic fading can reduce opacity, not erase its inventory or replace it with fresh air. Physical deposition, oxidation or removal requires a supported inventory transfer and, where applicable, boundary/source ledger.

## 6. Thermal domain, time and coupled stages

The current provisional water table covers 273.15-623.15 K, approximately 0-350 C. The audit identifies a legacy emitted-flame setting of 450 C. Those settings cannot be treated as supported by that initial water subset. At FIRE-W02, specify the necessary temperature, pressure and composition domains for every reactant, product and phase, then extend validated tables or keep the combination unsupported. This plan does not prescribe an unverified replacement upper limit.

Preflight alone is insufficient: a source can drive a candidate state outside the domain. Retry at a smaller stable interval where that can genuinely resolve numerical overshoot; if the physical state itself is unsupported, reject/pause the scene with a clear reason. Never hide a persistent domain failure by infinite retries, temperature clamps, erased energy or silent switch to legacy physics.

Insert source stages into the existing CPU/GPU predictor/corrector graph, not a second movement or pressure solver. At the source stage, propose release/reaction changes from a documented stage snapshot, arbitrate reactants, derive products and energy, and couple the candidate to thermal/phase/volume closure and stability checks. Commit only the complete accepted substep. If the chosen operator split needs correction/iteration, use the same ordering in Rust and WGSL and measure its temporal error.

The source timestep includes depletion, diffusion/mixing, heat release and phase/volume constraints in addition to fluid CFL. Legacy constants tuned per 1/60-second outer tick are not rates to repeat on every internal substep. Express rates in physical units and compare equal accepted physical time at dt and dt/2. Rejected steps cannot consume fuel or apply an ignition command twice.

Charge chemical energy once. Radiation that is only a cosmetic shader is not a second thermal source or sink. A physical radiative model must debit emitted energy and track absorbed/escaping portions explicitly; advanced radiation is not a hidden prerequisite for the first bounded fire slice.

## 7. GPU and visual integration

Rust owns reference tests, network definitions, resource orchestration and sparse commands. WGSL uses the existing component/energy buffers and a proposal/reservation/gather/commit source pipeline. Reuse transport, boundaries and EOS. Neither chemistry nor the renderer gets a full-world CPU mirror or an independent authoritative simulation.

Flame appearance must be derived from actual reacting/hot material, transported gas and supported soot fields. Preserve the useful particle-based appearance, with shader treatment tied to the simulated field/particles rather than an unrelated decorative fire overlay. Cosmetic particles may sample state, but cannot burn fuel, create soot or inject heat. A luminous hot object with zero heat-release rate is not labeled as active combustion.

Retain the Fire tool as a funded ignition command. Probes return component amounts, thermal state, heat-release rate, ignition/extinction reason and model limits with completed tick/epoch stamps. Unsupported oxygen or product data is not replaced by a misleading single material label. Quality settings and disabling glow/smoke rendering must not alter physical outcomes.

## 8. Work packages

[fire-combustion-work.json](../../data/fire-combustion-work.json) is the machine-readable owner for FIRE-W tasks and FIRE-A fixture definitions. Dependencies supplement E00-E14, not replace their numerical prerequisites. Each item remains planned until implementation and new evidence satisfy its exit.

### FIRE-W00: Capture characterization and corrected fire specifications

**P1 / M0. Engine work:** E00. **Dependencies:** none.

Preserve F01-F12 unchanged, verify their source fingerprint, and reproduce them only in a fresh evidence directory on the real checkout. Add distinct corrected FIRE-A specifications, including full-pipeline closed-boundary and cold-FIRE cases, without first rewriting the legacy TypeScript reactions.

**Exit:** The baseline states which observations still reproduce, distinguishes ten isolated checks from two full-tick checks, and freezes independent oracles, scales and tolerances. Characterization success is never counted as corrected acceptance.

### FIRE-W01: Define inventory, boundary, source and flame contracts

**P1 / M1. Engine work:** E02. **Dependencies:** FIRE-W00.

Extend the small active-component contract with fuel, O2, carrier gas, declared products and supported soot/residue identities; define one energy reference, source transactions, boundary face fluxes, model eligibility and derived flame/smoke outputs. Keep chemistry disabled in M3.

**Exit:** Schema/layout and round-trip tests preserve component amounts, network identity and feature settings. No separate spendable oxygen counter, dense all-118 cell allocation, or FIRE material authority is introduced.

### FIRE-W02: Validate finite species transport and the hot-scene domain

**P1 / M4. Engine work:** E09. **Dependencies:** FIRE-W01.

Use the existing Rust/reference then WGSL transport and EOS work to verify finite ventilation, O2/product tracer budgets and the property coverage required by intended fire/water scenes. Extend high-temperature mixture properties explicitly or reject unsupported scenes.

**Exit:** FIRE-A03 and FIRE-A12 pass for passive/preflight fixtures before fire-scene promotion. Actual gas composition, boundary flux and energy budgets agree; connected interior cells are never reset to ambient composition.

### FIRE-W03: Build the corrected Rust gas-combustion reference

**P1 / M6. Engine work:** E11. **Dependencies:** FIRE-W02.

Implement one declared fuel-vapor/O2 network with complete products, amount-limited reaction extents, compatible ignition/extinction, shared-reactant reservations and exactly-once energy release. Use rates per accepted physical time and retain excess reagent composition.

**Exit:** The referenced FIRE-A fixtures pass in the Rust reference against independent balances and refinement oracles. Cold labels cannot ignite, closed boundaries cannot supply oxidizer, and stoichiometric products reconcile with reactant loss.

### FIRE-W04: Add bounded fuel release and thermal suppression

**P1 / M6. Engine work:** E11. **Dependencies:** FIRE-W03.

Implement the minimum calibrated condensed-fuel release needed by each promoted wood/oil/plant scene, with explicit vapor and residue yields and thermal costs. Add amount-based water/steam suppression and persistent supported smoke/soot composition; preserve the funded ignition-brush contract.

**Exit:** FIRE-A07, FIRE-A08, FIRE-A09 and FIRE-A13 pass in Rust; unsupported presets are excluded visibly. No contact-water kill switch, whole-fuel smoke swap or timed physical smoke erasure survives in the physical model.

### FIRE-W05: Implement WGSL parity and transactional reactant ownership

**P1 / M6. Engine work:** E11. **Dependencies:** FIRE-W04.

Implement the validated reference stages in the existing wgpu-owned stage graph using read-only proposals, shared-inventory reservation, per-cell gathered updates and a single validated commit. Recompute coupled thermal/phase/volume state and retry complete rejected substeps.

**Exit:** FIRE-A01 through FIRE-A13 pass on supported CPU/GPU paths, including alternative execution orders, equal-time refinement and hot-domain rejection. No duplicate oxygen spending, unledgered heat or full-world CPU reaction mirror.

### FIRE-W06: Connect derived flames, tools and composition probes

**P1 / M6. Engine work:** E12. **Dependencies:** FIRE-W05.

Render the actual reacting/hot-gas fields and tracked smoke using the existing GPU state. Keep the Fire brush as a bounded command, preserve supported particle-based presentation, and expose tick/epoch-stamped fuel/O2/product/heat-release probes without making visual labels reaction inputs.

**Exit:** FIRE-M6 includes integrated burner, ignition-only, ventilation, water-dose and smoke scenes. Visual quality changes do not change physical inventories, and physically luminous hot objects are not mistaken for active combustion.

### FIRE-W07: Gate fire-scene promotion on recovery and measured evidence

**P1 / M7. Engine work:** E13, E14. **Dependencies:** FIRE-W06.

Run all FIRE-M7 fixtures plus save/load, failure injection, component capacity, supported-scene checks, browser interaction and transfer/performance measurements on the agreed devices. Retain unsupported legacy fire scenes separately.

**Exit:** FIRE-M7 is evidenced for every promoted fire scene. No characterization result is substituted for a corrected test, and no unsupported thermal range, pseudo-material claim or arcade quench is hidden behind a physical-mode label.

### FIRE-W08: Extend fuel chemistry without reopening foundational defects

**P4 / C5. Engine work:** P4/C3-C6; no new E-work item. **Dependencies:** FIRE-W07.

Extend the same engine in C3-C6 with additional bounded fuel-release profiles, char/surface channels, soot formation/oxidation/deposition and selected radiative heat transfer where supported. Reuse the P3 data compiler and preflight the complete reaction-product closure.

**Exit:** Every newly enabled channel has declared composition, yields, properties and calibration/source status, plus the applicable FIRE-A regression evidence. Core oxygen, product and ignition correctness was already delivered in P1, not deferred to P4.

## 9. Promotion and scope control

[FIRE-PRECONDITIONS, FIRE-M6 and FIRE-M7](../../validation/fire-combustion.md#corrected-gates) supplement, not replace, existing GPU gates. A fire scene needs both the relevant combustion evidence and the parent milestone's fluid, thermal, interaction and recovery evidence. Mark unsupported scene combinations explicitly; a missing fixture must not be recategorized as optional to pass promotion.

P4 extends fuel/soot/char and selected radiation models, rerunning the relevant FIRE-A cases for each enabled channel. P2 reactive pressure-wave sources use the same finite inventory and energy accounting, but require their own compressible-domain and shock gates. Nuclear heat later enters through the engine's separate nuclear source contract; it does not turn a FIRE label into a nuclear reaction rule.

The 40/83/103/118 roster and P1-P9 phase order do not change. Source context comes from the preserved audit and [inherited NIST FDS references](../../sources.md#nist-fds-fire-context); these motivate the reduced model but do not validate it.
