# Particle Foundry: phases and milestones

This is the plain-language guide to what the project is building and in what
order. A **phase** is a major outcome, such as a new engine or a larger set of
materials. A **milestone** is a smaller result within that phase. Finishing a
milestone requires its tests and evidence; a name in this guide is not a claim
that the feature already works.

**Where things stand (27 September 2026):** The browser sandbox still runs on
the existing TypeScript engine. The Rust/WebAssembly foundation and browser
contract exist, but the new fluid solver and GPU simulation do not. P1 is under
way; P2-P9 are planned. The baseline (M0) is validated. M1's foundation work is
validated through E03, while its separate circuit work (E04) remains planned.
The next scoped fluid task is E05 in M2. For live status, see the
[phase tracker](data/development-phases.json) and
[engine work tracker](data/engine-migration-work.json).

## The route at a glance

1. **Build the engine - P1-P2.** Replace the old physics for *supported* scenes
   with a tested Rust/WebAssembly and GPU engine. Then add pressure waves and
   breakable materials as a separate physical model.
2. **Build useful matter - P3-P5.** Establish trustworthy material and reaction
   data, then grow from 40 tested core element forms to 83. Compounds and
   mixtures are separate from element identities.
3. **Add nuclear behavior and prepare release - P6-P9.** Add decay, radiation,
   selected fusion and fission, then the final 118-identity catalogue. Validate
   the combinations people can actually use before calling the product ready.

The letters show which plan a milestone belongs to: **M** for the new engine,
**W** for waves and breakage, **C** for chemistry and materials, **N** for nuclear
work, and **Q** for release quality. The E-numbers are smaller engineering tasks
inside P1; you can ignore them to understand the overall route.

## P1 - Build and prove the new engine

The goal is a browser engine that can run selected scenes on the GPU while
keeping the current sandbox available for scenes the new engine cannot yet
handle. The first GPU demo is a checkpoint, not the finished replacement.
[Detailed engine plan](plans/fluid-gpu-redesign/plan.md).

**Foundation and first demo**

- **M0 - Capture the starting point.** Record the existing behavior and tests so
  later changes can be judged against real evidence. **Validated.**
- **M1 - Establish the Rust/browser connection.** Define how scenes, materials,
  commands and results move through the new engine. The foundation is in place;
  the independent circuit reference is still planned. **Partly complete.**
- **M2 - Prove fluid physics on the CPU.** Build a small, precise Rust model of
  pressure, liquid movement and transport; show that it conserves what it should.
  **Next fluid milestone.**
- **M3 - Show the first GPU scene.** Run the same basic fluid model in the browser
  and draw it directly from GPU state. It is an experimental scene with limited
  materials, not a replacement for every existing scene. **Planned.**

**Turn the demo into supported scenes**

- **M4 - Add heat and phase changes.** Make heated water, steam, air and finite
  venting obey tested mass, energy and volume rules. **Planned.**
- **M5 - Add sand and moving solids.** Make grains, obstacles and liquids interact
  without losing material when many things move at once. **Planned.**
- **M6 - Restore useful interactions.** Add supported reactions, corrected fire,
  circuits, tools and controls to the new engine. **Planned.**
- **M7 - Decide what can replace the old engine.** Test recovery, browser
  behavior and performance on real devices. Promote only scenes that pass; keep
  the old engine for the rest. **Planned.**

## P2 - Add pressure waves and breakage

This phase adds a separate model for travelling pressure waves, explosions that
spend finite fuel, and solids that can fracture. The P1 fluid model does not
automatically provide these effects.
[Detailed waves and breakage plan](plans/pressure-waves-breakage/plan.md).

- **W0 - Define the starting tests and supported scenes.**
- **W1 - Prove gas waves and wall reflections in a Rust CPU model.**
- **W2 - Match that model on the GPU and in the browser.**
- **W3 - Add funded reactions, damage and moving fragments.**
- **W4 - Test complete scenes, controls, saves and performance before release.**

## P3 - Give matter a reliable data foundation

The reference catalogue can name all 118 elements, but a name alone does not
mean the sandbox can simulate that material. P3 makes the data, composition and
reaction rules explicit.
[Detailed materials plan](plans/materials-and-chemistry/plan.md).

- **C0 - Build a checked catalogue of elements, forms and properties.** Record
  where values came from and which conditions they apply to.
- **C1 - Preserve amounts through movement and change.** Mixing, melting,
  splitting and saving should not silently lose composition.
- **C2 - Add a bounded reaction engine.** Reactions must use available
  ingredients, produce declared products and account for energy.

## P4 - Deliver 40 core element forms and essential compounds

Each of the 40 core element identities should have at least one tested material
form in a stated range. This does not promise every possible phase or reaction.
[Detailed materials plan](plans/materials-and-chemistry/plan.md).

- **C3 - Move the eight existing element models to the new engine.**
- **C4 - Add the remaining core forms and useful compounds**, including selected
  water, air, mineral and fuel cases.
- **C5 - Add selected reactions and solutions**, such as bounded oxidation and
  acid/base behavior.
- **C6 - Check the complete 40-element set** in real scenes, with visible limits
  and source information.

## P5 - Expand to 83 element forms

Add 43 more element identities with tested base forms. Use shared material
systems where they fit rather than inventing a unique physics engine for every
element. [Detailed materials plan](plans/materials-and-chemistry/plan.md).

- **C7 - Add the exact 43-element extended group.**
- **C8 - Add selected richer behaviors**, such as alloys, coatings and
  semiconductor examples, where tests and data support them.
- **C9 - Validate the 83-element set**, including mixtures, performance and
  clearly labelled limits.

## P6 - Add decay and radiation

Add nuclear behavior to selected isotopes and 20 more element identities. The
target of 103 identities means 83 material-first identities plus 20 that may
initially exist mainly as isotopes; it does not mean 103 realistic bulk
substances. [Detailed nuclear plan](plans/nuclear-simulation/plan.md).

- **N0 - Import and check isotope and decay data.**
- **N1 - Prove decay chains and daughter products in a CPU reference.**
- **N2 - Account for radiation as it travels, deposits energy or escapes.**
- **N3 - Add the 20 nuclear-first identities and usable isotope controls.**

## P7 - Add selected fusion and fission

Model specific nuclear reactions with known inputs, products and energy
accounting. This is a limited simulation, not a general reactor or star model.
[Detailed nuclear plan](plans/nuclear-simulation/plan.md).

- **N4 - Validate a small fusion reaction network.**
- **N5 - Add selected isotope-specific fission and neutron interactions.**
- **N6 - Connect approved nuclear reactions to supported scenes** and test
  recovery, energy accounting and model limits.

## P8 - Cover all 118 identities

Add the final 15 superheavy identities. Where science has no measured bulk
property, the sandbox should say so. Optional creative behavior must be clearly
labelled and saved with the scene.
[Detailed nuclear plan](plans/nuclear-simulation/plan.md).

- **N7 - Add the last 15 identities with honest data/support labels.**
- **N8 - Add optional creative material behavior** without presenting it as a
  measured property.

## P9 - Check the whole product

Test which combinations of engine, materials, reactions and settings are truly
supported. Make the result understandable and recoverable for players.
[Detailed release checks](validation/acceptance.md).

- **Q0 - Check the supported-feature matrix.** Reject combinations that the
  simulation cannot safely or honestly run.
- **Q1 - Finish the user experience and save/recovery paths.** Make materials,
  explanations, controls and tutorial scenes usable.
- **Q2 - Assemble release evidence.** Run numerical, browser and device tests;
  align documentation and claims with the results.

**What the counts mean:** 40, 83, 103 and 118 count element *identities* at
successive targets. They do not count fully realistic substances, all their
isotopes, or every possible chemical reaction. Each phase ends only when its
own [acceptance checks](validation/acceptance.md) pass for the scenes it claims
to support.
