# Particle Foundry: phases and milestones

**The current goal is to make today's sandbox work on the Rust/GPU engine.**
New pressure-wave physics, more elements, broader chemistry and nuclear behavior
are deferred. Finishing the GPU switch will be a point to review the result and
choose what comes next; it will not automatically start another phase.

This is the plain-language guide. A **phase** is a major outcome; a **milestone**
is a smaller result within it. A milestone is complete only when its tests and
evidence pass. The [current sandbox scope](plans/rust-wasm-migration/current-sandbox-scope.md)
holds the detailed checklist of what must work before the switch is complete.

**Where things stand (29 September 2026):** The browser sandbox still runs on
the existing TypeScript engine. The Rust/WebAssembly foundation and browser
contract exist. The small Rust CPU fluid reference has passed its local tests.
A small GPU fluid fixture now steps on native and browser hardware, and a GPU
buffer-layout check has passed on both. An opt-in browser scene now draws
directly from GPU state. A local Chrome run completed 60 full-size ticks over
one second of model time, and a native Metal run completed 120 ticks. Local
repository checks passed locally and in GitHub CI. M3 is validated; P1 is under way;
P2-P9 are deferred.
The baseline (M0), engineering work E00-E03, and the Rust fluid work E05-E06 are
validated locally. M1's separate circuit work (E04) remains planned. M2 is
validated. For live status, see the
[phase tracker](data/development-phases.json) and
[engine work tracker](data/engine-migration-work.json).

## The three checkpoints that matter now

1. **Prove a small GPU scene - M2-M3.** Get a simple liquid-and-gas scene running
   correctly in the browser, with physics and drawing on the GPU.
2. **Bring across today's sandbox - M4-M6.** Cover the current materials,
   reactions, heat, sand, circuits, scenes and controls, including the eight
   existing element models. Correct known physics errors as part of this work.
3. **Prove the default switch - M7.** Check the complete current-feature list,
   real-browser performance, checkpoints and recovery before making GPU the default.

All three checkpoints belong to P1. A fast demo alone does not finish the switch,
and moving only a few scenes across does not finish the current-feature list.
The existing engine stays available while the replacement is being proven.

## P1 - Make today's sandbox work on the new engine

Keep the browser interface and its useful behavior. Change the engine underneath
it, with one engine owning each scene. New physics must preserve material and
energy correctly; reproducing known bugs is not a requirement.
[Detailed engine plan](plans/fluid-gpu-redesign/plan.md).

### Foundation already under way

- **M0 - Capture the starting point.** Record the existing behavior and tests so
  later changes can be judged against real evidence. **Validated.**
- **M1 - Establish the Rust/browser connection.** Define how scenes, materials,
  commands and results move through the new engine. The foundation is in place;
  the independent circuit reference is still planned. **Partly complete.**

### Checkpoint 1: a working GPU proof

- **M2 - Prove fluid physics on the CPU.** Build a small, precise Rust model of
  pressure, liquid movement and transport; show that it conserves what it should.
  **Validated locally.**
- **M3 - Show the first GPU scene.** Run the same basic fluid model in the browser
  and draw it directly from GPU state. It is an experimental scene with limited
  materials, not a replacement for every existing scene. **Validated locally.**

### Checkpoint 2: today's features work on the new engine

- **M4 - Migrate heat and phase changes.** Cover today's heating, melting,
  freezing and boiling behavior, including water, steam and air. Test material,
  energy, volume and finite venting together. **Planned.**
- **M5 - Add sand and moving solids.** Make grains, obstacles and liquids interact
  without losing material when many things move at once. **Planned.**
- **M6 - Complete the current interactions.** Migrate existing reactions,
  corrected fire, circuits, tools, controls and scenes. Include the eight
  current element models: hydrogen, helium, carbon, nitrogen, oxygen, sulfur,
  iron and copper. **Planned.**

### Checkpoint 3: a verified default switch

- **M7 - Prove the complete current sandbox is ready.** Test the agreed current
  features together, browser behavior, performance on real devices, checkpoints and
  recovery. Individual scenes can be tested earlier; the switch is complete
  only when the full current-feature checklist passes. **Planned.**

After M7, stop and review the evidence. Later phases need a separate decision
to start. The completed local engineering work is **E07-E08**. Checkpoint 2
has not started; E04 circuits and M4-M6 feature coverage remain planned.

## Deferred ideas: P2-P9

Everything below is retained for future planning. None of these phases is part
of the current GPU-switch commitment, and their order does not authorize work
to start. Any small prerequisite needed to migrate an existing feature belongs
in the current scope; it does not activate a whole future phase.

The letters identify milestone groups: **M** for the engine, **W** for waves,
**C** for materials and chemistry, **N** for nuclear work, and **Q** for release
quality. The E-numbers are smaller engineering tasks inside P1. All 35 milestone
IDs remain listed here so the longer-term plans are easy to find.

### P2 - Add pressure waves and breakage - deferred

This phase adds a separate model for travelling pressure waves, explosions that
spend finite fuel, and solids that can fracture. The P1 fluid model does not
automatically provide these effects.
[Detailed waves and breakage plan](plans/pressure-waves-breakage/plan.md).

- **W0 - Define the starting tests and supported scenes.**
- **W1 - Prove gas waves and wall reflections in a Rust CPU model.**
- **W2 - Match that model on the GPU and in the browser.**
- **W3 - Add funded reactions, damage and moving fragments.**
- **W4 - Test complete scenes, controls, saves and performance before release.**

### P3 - Give matter a broader data foundation - deferred

The reference catalogue can name all 118 elements, but a name alone does not
mean the sandbox can simulate that material. P3 expands the checked data,
composition and reaction systems beyond what the current-feature migration
needs.
[Detailed materials plan](plans/materials-and-chemistry/plan.md).

- **C0 - Build a checked catalogue of elements, forms and properties.** Record
  where values came from and which conditions they apply to.
- **C1 - Extend composition tracking to the broader catalogue.** Keep the
  migration's guarantees when more substances mix, melt, split and are saved.
- **C2 - Generalize the reaction engine.** Add checked rules for a wider set of
  substances while still using available ingredients and accounting for energy.

### P4 - Deliver 40 core element forms and essential compounds - deferred

Each of the 40 core element identities should have at least one tested material
form in a stated range. This does not promise every possible phase or reaction.
[Detailed materials plan](plans/materials-and-chemistry/plan.md).

- **C3 - Recheck and extend the eight existing element models in the broader
  data system.** Their first migration to the Rust/GPU engine belongs to P1.
- **C4 - Add the remaining core forms and useful compounds**, including selected
  water, air, mineral and fuel cases.
- **C5 - Add selected reactions and solutions**, such as bounded oxidation and
  acid/base behavior.
- **C6 - Check the complete 40-element set** in real scenes, with visible limits
  and source information.

### P5 - Expand to 83 element forms - deferred

Add 43 more element identities with tested base forms. Use shared material
systems where they fit rather than inventing a unique physics engine for every
element. [Detailed materials plan](plans/materials-and-chemistry/plan.md).

- **C7 - Add the exact 43-element extended group.**
- **C8 - Add selected richer behaviors**, such as alloys, coatings and
  semiconductor examples, where tests and data support them.
- **C9 - Validate the 83-element set**, including mixtures, performance and
  clearly labelled limits.

### P6 - Add decay and radiation - deferred

Add nuclear behavior to selected isotopes and 20 more element identities. The
target of 103 identities means 83 material-first identities plus 20 that may
initially exist mainly as isotopes; it does not mean 103 realistic bulk
substances. [Detailed nuclear plan](plans/nuclear-simulation/plan.md).

- **N0 - Import and check isotope and decay data.**
- **N1 - Prove decay chains and daughter products in a CPU reference.**
- **N2 - Account for radiation as it travels, deposits energy or escapes.**
- **N3 - Add the 20 nuclear-first identities and usable isotope controls.**

### P7 - Add selected fusion and fission - deferred

Model specific nuclear reactions with known inputs, products and energy
accounting. This is a limited simulation, not a general reactor or star model.
[Detailed nuclear plan](plans/nuclear-simulation/plan.md).

- **N4 - Validate a small fusion reaction network.**
- **N5 - Add selected isotope-specific fission and neutron interactions.**
- **N6 - Connect approved nuclear reactions to supported scenes** and test
  recovery, energy accounting and model limits.

### P8 - Cover all 118 identities - deferred

Add the final 15 superheavy identities. Where science has no measured bulk
property, the sandbox should say so. Optional creative behavior must be clearly
labelled and saved with the scene.
[Detailed nuclear plan](plans/nuclear-simulation/plan.md).

- **N7 - Add the last 15 identities with honest data/support labels.**
- **N8 - Add optional creative material behavior** without presenting it as a
  measured property.

### P9 - Check the expanded product - deferred

Test the combinations of future engine modes, materials, reactions and settings.
Make that expanded product understandable and recoverable for players. The
current sandbox's usability, recovery and performance checks already belong
to P1 and cannot wait for this phase.
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
