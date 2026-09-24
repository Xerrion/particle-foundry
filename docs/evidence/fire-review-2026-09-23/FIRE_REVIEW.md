# Fire handling review

Reviewed: 2026-09-23. Source: the uploaded `src(3).zip`. This report reviews the existing implementation, not a newer repository revision. The documentation reference is `particle-foundry-docs-rust-wasm-wgpu.zip`.

## Verdict

The current implementation has useful energy-accounting safeguards, but it is not a physically coherent combustion model. Two correctness defects were reproduced through the actual complete physics tick: closed world boundaries still admit combustion oxygen, and a cold cell labelled FIRE can trigger hydrogen oxidation before it is quenched. Several additional behaviors are intentional game approximations rather than acceptable contracts for the planned species-based engine.

No application source was changed. No existing documentation archive was rewritten. This is an additional review and a proposal for more explicit development gates.

## Method and limits

The uploaded archive contains 43 source files. All extracted source files were compared byte-for-byte with their ZIP entries after the review. The source archive SHA-256 is recorded in `provenance.json`.

The relevant TypeScript modules were compiled to a separate CommonJS output directory using TypeScript 5.8.3. Twelve targeted checks were executed with Node v22.16.0. Ten are isolated subsystem checks, and two invoke `createPhysics(world).step()`.

These are characterization checks: their assertions confirm the reported observations, including undesirable behavior. Successful harness execution does not mean that the implementation passes a corrected combustion specification. No original application test suite, browser rendering, GPU implementation, long-running fire scenario, or application production build was tested.

The water-mass and cold-FIRE checks intentionally construct edge-case states to inspect contracts. The cold-FIRE result was also reproduced through the full pipeline. The tiny-water result is explicitly reaction-only; thermal diffusion or vaporization elsewhere in a complete tick can affect that setup.

## Findings

### 1. Closed boundaries do not seal combustion oxygen

Evidence: F01, F02 and F03. Source: `src/physics/reactions.ts:36-66,107-130`; compare `src/physics/motion.ts:273-290` and `src/simulation/physics.ts:33-54`.

`beginStep()` always flood-fills gas from geometric edges. Every reachable EMPTY cell has `oxygenKg` reset to a default ambient value. Neither this procedure nor `hasFreshAir()` checks `world.boundariesEnabled`. `consumeOxygen()` returns the full requested amount immediately when `hasFreshAir()` is true, without subtracting oxygen or registering an external mass input.

In a 7 by 7 world with `boundariesEnabled: true`, hot wood and initially zero oxygen, the oxygen field was repopulated and the wood released approximately 100 J in one real physics tick. In the isolated reaction check, this combustion did not debit oxygen or change the source ledger. A contrasting explicitly stone-enclosed cavity with zero oxygen correctly did not burn.

Thus the global boundary toggle and actual wall-enclosed cavities have different oxygen semantics. This does not establish that all sealed cavities are broken. Even for open boundaries, global reachability is not a finite ventilation or species-transport model.

Required direction: one boundary-condition contract; finite transported oxygen; explicit boundary species/mass/energy fluxes. No interior oxygen replenishment merely because a gas path reaches an opening.

### 2. Generic combustion preserves scalar totals but not a coherent chemical composition

Evidence: F04. Source: `src/physics/reactions.ts:114-123,160-169,222-232`; `src/simulation/world.ts:187-202`.

Wood/oil/plant combustion decreases `chemicalEnergyKj` and, in an enclosed pocket, `oxygenKg`. It does not progressively move consumed fuel mass into products, consume the mass represented by a pure OXYGEN parcel, or create a corresponding product composition. When the chemical store is exhausted, the whole remaining fuel parcel becomes SMOKE.

The test consumed an OXYGEN cell's full 1.3212e-6 kg oxygen inventory and released approximately 18.8743 J. The cell still had material OXYGEN and the same positive mass. Fuel mass was unchanged. No CO2 or water/steam product was created. Tracked total energy differed by only floating-point roundoff, about -1.82e-12 J.

This is specifically a composition inconsistency, not proof that the total scalar mass or energy vanishes. Conservation tests must check species/element inventories and chemical consistency as well as scalar mass and energy.

Required direction: amount-based reaction extent, products and unreacted fractions, one consistent chemical-energy reference. Generic wood/oil can remain declared pseudo-materials, but their yields and inventory accounting must be defined.

### 3. A cold FIRE identity can ignite elemental reactions

Evidence: F07 and F08. Source: `src/physics/element-reactions.ts:177-191`; `src/simulation/physics.ts:66-71`; `src/physics/reactions.ts:237-248`.

The elemental path considers a neighboring FIRE cell sufficient to make `ignited` true, without checking that neighbor's temperature or available ignition energy. Elemental reactions run before generic FIRE quenching.

A stone-enclosed setup with hydrogen, oxygen, and a neighboring FIRE parcel, all initially at 22 C, reacted and released approximately 9.9849 J. This occurred both in the isolated elemental step and through the real full physics pipeline.

Required direction: derive ignition from admissible thermal/reactant conditions and a funded ignition source, not a render/material label or stale burning flag. Test cooled and quenched former flames before subsequent reaction passes.

### 4. Generic flames are hot-air proxies, not reacting fuel vapor

Evidence: F05 and F10, plus source inspection. Source: `src/physics/reactions.ts:172-203,237-268`; `src/materials/definitions.ts:950-971,995-1015,1327-1345`.

`emitFlame()` changes neighboring EMPTY air to FIRE and transfers heat from the fuel. The new FIRE parcel retains the air mass and has zero chemical inventory. Its own `releaseChemicalEnergy()` call therefore produces no additional combustion heat in this pathway. This avoids free energy, but it does not represent transport and burning of volatile fuel.

The resulting parcel changes state based on temperature, water contact, oxygen availability and a timer. Non-brush flames can become SMOKE without a transported soot/carbon inventory. In a sealed cavity, expired SMOKE becomes EMPTY/Air while retaining mass, heat and the existing oxygen inventory. The test did not create fresh oxygen during this sealed conversion, but it did lose the smoke identity.

Required direction: keep FIRE as a brush and a derived visual phenomenon; represent fuel vapor, oxidizer, products and soot with actual amounts. Cosmetic fading must not erase physical composition. Use explicitly declared yields and, where enabled, deposition or oxidation instead of a material-expiry timer.

### 5. Water quenching is a material-contact switch

Evidence: F06. Source: `src/physics/reactions.ts:222-225,237-243`; see separate thermal transport in `src/physics/thermal.ts` and its invocation in `src/simulation/physics.ts:58`.

One neighboring WATER cell disables generic fuel combustion and converts FIRE irrespective of water amount or actual cooling. The isolated reaction test used 1e-12 kg water next to 500 C wood. Burning was disabled without changing either cell's thermal energy or vaporizing water in that reaction call.

There is a separate thermal solver; this finding does not claim the entire game lacks water cooling or boiling. The defect in the model contract is that the extinguishing decision itself bypasses those quantities. It is not evidence that the extreme tiny-water setup stays liquid in a complete integrated simulation.

Required direction: bounded water heat uptake, evaporation and mixing/oxygen effects; extinction evaluated from the resulting local state. A separately labelled arcade contact-quench rule is possible, but should not be mistaken for physical suppression.

### 6. Oxidizer availability has incompatible material-specific rules

Evidence: F09 and source inspection. Source: `src/physics/element-reactions.ts:123-170,172-211`; `src/physics/reactions.ts:107-130`.

Hot hydrogen reacts with explicit OXYGEN, or an effectively unlimited open-air oxygen source that is at least ledgered in this path. It cannot use finite oxygen stored in ordinary air enclosed by stone. Carbon and sulfur only use the explicit OXYGEN-pair route in this module. Wood/oil/plant can consume neighboring enclosed-air oxygen.

The hydrogen sealed-air limitation is already disclosed in its material guide. It is a documented scope limitation rather than an undisclosed universal-combustion implementation. However, it cannot become the shared species model for the expanded catalogue.

Required direction: all eligible reaction channels read the same transported reactant inventories, with one arbitration process so neighboring reactions cannot spend the same oxygen.

## Safeguards worth keeping

F04 confirmed that the simple generic reaction conserves its tracked chemical-plus-thermal energy in the tested case. Source: `reactions.ts:160-169`.

F11 confirmed that the ignition brush adds ledgered external heat, preserves the existing air mass, contains no chemical fuel, and returns to air after its lifetime without producing smoke. Source: `tools/brush.ts:106-147`; `reactions.ts:251-268`.

F12 confirmed that the explicit hot H2/O2 reaction conserves tracked total mass, momentum and total energy for one unequal-reactant fixture. Source: `element-reactions.ts:58-120`. This is a useful regression control, not validation of every thermochemical reference or every reaction.

Flame-contact and emission code make paired energy transfers instead of writing arbitrary heat into both participants. Source: `reactions.ts:133-203`. Preserve this accounting discipline while replacing the reaction and species representations.

## How this should enter the existing roadmap

The current GPU plan, M6 at `docs/plans/fluid-gpu-redesign/plan.md:198-210`, already calls for transported reactants, limiting reaction extent, fuel, oxygen, products, extinguishing and source ledgers. The chemistry plan also calls for soot, declared mixtures, and formation references. That is the correct direction, but the fire-specific contracts and acceptance fixtures need more detail.

### P1 / M0: capture regression fixtures now

Capture the two integration bugs above as expected-to-fail tests for the corrected implementation. Preserve the good brush and explicit-reactant conservation cases as controls. Do not require a complete TypeScript repair before building the Rust reference.

### P1 / M1: reserve the necessary state contracts

Declare fuel/oxidizer/product amounts, a minimal soot or smoke-proxy composition, thermal-energy ownership, reaction-source outputs, boundary fluxes and derived visual inputs. Reuse the existing active-species strategy; do not allocate one dense field for each of 118 elements merely to support a small combustion demo.

### P1 / M4: establish transport and a valid thermal domain

Validate conservative transport of active species, finite ventilation and source/energy bookkeeping. Establish appropriate thermodynamic support before promoting hot fire/water scenes. The current provisional water table ends at 623.15 K, about 350 C (`GPU plan:88`), below the existing 450 C emitted flame and other legacy fire temperatures. Extend support or reject the scene explicitly; do not clamp temperatures or declare those scenes supported by the initial water subset.

### P1 / M6: implement a minimal coherent combustion slice

Implement a small declared fuel-vapor/oxygen reaction with products, optional calibrated soot yield, funded ignition and extinction. For wood/oil, introduce a bounded material-specific release model: heating produces vapor through a declared pyrolysis or evaporation approximation, and that vapor supplies gas-phase combustion. Surface/char oxidation can be a separate supported reaction. Follow the existing Rust CPU reference to WGSL parity process.

A useful invariant is that the reacted amount is bounded by reaction kinetics/mixing over the accepted time interval, available fuel and available oxidizer. Charge chemical energy exactly once using the chosen formation-energy or explicit-store convention. Account for any radiation or external heat losses explicitly.

Replace per-tick rates and timer-controlled physical changes with physical-time rates suitable for the planned adaptive substeps. The current fixed 1/60-second step makes its constants coherent as gameplay tuning; the migration must not reinterpret the same per-tick release as occurring on every new substep.

### P4: extend chemistry, not defer foundational correctness

Add additional fuel models, reaction channels, char, soot oxidation/deposition, fuel-specific flame appearance and richer radiation where supported. Keep advanced features optional. Boundary semantics, oxygen/product accounting and funded ignition are required before initial GPU fire scenes are promoted, not only after the 40-element expansion.

## Proposed fire-specific acceptance gates

| Fixture | Required corrected behavior |
| --- | --- |
| Closed-boundary zero-oxygen chamber | No oxygen appears; no oxidation occurs without an explicit source. |
| Explicit sealed finite-air chamber | Supported fuels consume a finite shared O2 inventory; extinction does not require every molecule to be consumed. |
| Reopened vent | New oxygen and energy enter through measured boundary fluxes, not flood-fill replenishment. |
| Cold former FIRE beside H2/O2 | A cold visual/material label alone cannot ignite a reaction. |
| Limited reactants and two competing flames | No negative inventories, duplicate oxygen spending or lost residual reagent identity. |
| Progressive fuel loss | Consumed fuel and oxidizer become defined products; scalar and element/species inventories reconcile. |
| Ignition tool in clean air | Only ledgered external heat; no unbudgeted fuel, carbon or smoke. |
| Small versus large water dose | Outcomes follow the supported heat/mixing model, not identical adjacency logic. |
| Sealed smoke cooled below visibility | Rendering may fade; physical tracked smoke/soot composition remains unless a supported process removes it. |
| Equal physical time at dt and dt/2 | Reaction amount and released energy converge within a declared tolerance. |
| CPU versus GPU and alternative scan order | Invariants and aggregate outcomes agree within tolerances; no uncontrolled race-dependent oxygen consumption. |
| High-temperature property boundary | Out-of-domain states are explicitly unsupported or covered by validated properties. |

## Scientific reference context

The recommendations are a reduced game model, not a claim to implement the full Fire Dynamics Simulator. NIST's FDS technical documentation is useful primary-source context because it treats combustion through species production rates and heat release, and distinguishes condensed-phase pyrolysis/evaporation from gas-phase combustion. Its extinction model also accounts for local composition and thermal state rather than assuming any nonzero oxygen guarantees sustained burning.

Primary references reviewed:

- NIST FDS project: `https://pages.nist.gov/fds/`
- FDS combustion chapter: `https://github.com/firemodels/fds/blob/master/Manuals/FDS_Technical_Reference_Guide/Combustion_Chapter.tex`
- FDS solid/pyrolysis chapter: `https://github.com/firemodels/fds/blob/master/Manuals/FDS_Technical_Reference_Guide/Solid_Chapter.tex`

These are contextual sources, not evidence that the uploaded game implements or matches FDS.
