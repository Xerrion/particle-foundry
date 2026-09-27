# Combustion in the current TypeScript backend

**Scope:** Fire brush, generic fuel burning, explicit elemental oxidation and flame/smoke lifecycle in `web/src/legacy/`.
**Status:** implemented game behavior with known defects; corrected Rust/GPU combustion remains planned.
**Source review:** 27 September 2026, against the current working tree. This is a code inspection with linked regression coverage, not a rerun of the archived fire audit or a claim of validated combustion physics.

Use this article to locate and understand current combustion behavior. The
[physical model](model.md) owns shared units and conservation conventions.
[Fire integration](plans/fire-combustion/plan.md) and [FIRE-A gates](validation/fire-combustion.md#corrected-gates)
own the future correction requirements. [The knowledge map](README.md) routes other topics.

## Source ownership

| Responsibility | Source and symbols |
| --- | --- |
| Parcel state and reaction parameters | [world.ts](../web/src/legacy/simulation/world.ts): `World`, `changeMaterial`, `swap`, `addExternalEnergy`; [definitions.ts](../web/src/materials/definitions.ts): `combustionProfile`, material ignition thresholds and `ignitionBrush` |
| User-funded ignition | [brush.ts](../web/src/legacy/tools/brush.ts): `createBrush`, Fire branch of `paintCell` |
| Generic fuels, flame emission, oxygen shortcut and smoke expiry | [reactions.ts](../web/src/legacy/physics/reactions.ts): `createReactions`, `beginStep`, `update`, `releaseChemicalEnergy`, `emitFlame` |
| Hydrogen, carbon and sulfur oxidation | [element-reactions.ts](../web/src/legacy/physics/element-reactions.ts): `createElementReactions`, `reactPair`, `reactWithAir` |
| Coupling to heat and movement | [physics.ts](../web/src/legacy/simulation/physics.ts): `createPhysics.step`; [thermal.ts](../web/src/legacy/physics/thermal.ts): `energyAtTemperature`; [motion.ts](../web/src/legacy/physics/motion.ts): `createMotion` |

Nested function names above are search targets within their modules, not public
APIs. Browser code reaches the backend through [engine-client](../web/src/engine-client/index.ts);
these source links do not authorize importing legacy internals from the UI.

## Implemented behavior

### Fire brush supplies ignition heat

Painting Fire on wood, oil or plant heats the existing parcel toward its ignition
threshold plus the catalogue margin. It preserves material identity, mass and
chemical fuel. The added heat goes through `addExternalEnergy`, so it is an
external source rather than energy released by combustion.

On ambient air (`EMPTY`), the brush changes its identity to `FIRE`, adds only the
heat needed to reach the ignition-brush temperature, and sets `ignitionFlame` with
a short lifetime. It adds no chemical fuel. Painting an existing flame does not
restart its lifetime or add another heat dose. Brush-created gas returns to air
when quenched or expired, preserving its mass/heat without manufacturing smoke.

The center of the brush lights reliably; the surrounding fringe is sampled.
This bounds held-brush effects. Parameters belong to `definitions.ts`, not a
second documentation table. Painting oxygen beside hydrogen heats the oxidizer
and marks the hydrogen for ignition; oxygen is not treated as fuel.

### Generic fuels retain their identity while burning

`createReactions.update` handles wood, oil and plant. It skips hydrogen, carbon
and sulfur forms because their oxidation belongs to `createElementReactions`.
Generic burning requires the ignition temperature, available oxygen and stored
chemical energy; an adjacent `WATER` cell disables it.

`releaseChemicalEnergy` limits each release by the per-tick policy, remaining
chemical energy and room below the product-temperature ceiling. Chemical state
is stored in kJ and thermal state in J: the debit is multiplied by 1000 when
added to `world.energy`. The ceiling can pause heat release while retaining fuel
and the burning flag. Exhausted generic fuel changes to `SMOKE`.

Contact heating transfers equal and opposite energy from hot flame or burning
fuel to its neighbor. A burning solid keeps an ignition reserve. `emitFlame`
heats an eligible neighboring air parcel using energy transferred from the fuel;
it does not transfer chemical fuel into the new flame. Seeded emission sampling
avoids synchronized horizontal sheets. A skipped emission leaves heat in the fuel.

### Elemental oxidation follows a separate path

`reactPair` consumes adjacent fuel and pure oxygen in bounded stoichiometric
amounts. Hydrogen forms the water family, carbon forms CO2, and sulfur forms SO2.
The calculation retains excess reactant, combines momentum and accounts for
chemical, kinetic and gravitational energy changes. It refuses a pair when the
two cells cannot represent products and both distinct residual reactants.

Hydrogen additionally uses `reactWithAir` when neighboring air/flame cells connect
to a geometric edge. This path imports oxygen with an external mass/energy entry.
It does not consume finite oxygen stored in enclosed ordinary air. Carbon and
sulfur require explicit oxygen parcels. These are separate legacy mechanisms,
not a universal mixture reaction solver.

### Tick ordering separates reactions from transport

In `createPhysics.step`, thermal diffusion and phase handling precede explicit
element reactions. Generic `beginStep` and `update` run after element reactions,
explosions and airflow, and before parcel movement. This makes contact heating
possible before flame and fuel separate.

Newly emitted flames and quenched or expired gas cells are marked `moved` to avoid
a second reaction in the same pass. The flag is reset before transport so fresh flames and
quenched smoke can move immediately. Material gases move before ambient air.
Preserve the separate reaction and movement checks when changing this order;
isolated reaction tests cannot establish full-tick behavior.

### Flame and smoke identity have physical consequences

`FIRE`, `SMOKE`, `burning` and `ignitionFlame` are legacy state, not merely renderer
labels. `changeMaterial` preserves mass, heat and chemical/oxygen inventories,
but resets burning/ignition flags and assigns a new lifetime. `setCell` instead
initializes a parcel and records external replacement; the methods are not
interchangeable. `swap` carries parcel metadata with the moving matter.

Ordinary flames can become smoke. Expired smoke connected to an edge is removed
through source/sink accounting; in an enclosed region it can become `EMPTY`
without losing its mass/heat. This removes its smoke identity, which is a
composition limitation rather than validated physical dispersion.

## Known defects and evidence limits

The mechanisms below are still visible in the reviewed working-tree source.
Historical check IDs link to the [preserved finding-to-work mapping](validation/fire-combustion.md#finding-to-work-traceability).
Historical measurements and execution claims belong to that audit's snapshot;
they were not reproduced by this source inspection.

| Limitation | Current source mechanism | Historical evidence |
| --- | --- | --- |
| Closed-boundary oxygen shortcut | Generic `beginStep` replenishes edge-connected air and `consumeOxygen` grants fresh oxygen without a debit. Neither it nor elemental `markVentilatedAir` consults `boundariesEnabled`; a drawn stone enclosure has different semantics. | F01-F03 characterize generic burning; the flag does not prove a sealed combustion budget. |
| Incomplete generic products | Generic release spends chemical energy and, for enclosed gas, a separate oxygen counter without progressively consuming fuel/O2 material mass into declared products. Elemental `reactPair` is a separate mechanism. | F04/F05; F12 is an explicit-reaction control. Scalar energy conservation alone does not establish correct composition. |
| Cold flame label ignites reactants | Elemental `step` accepts an adjacent `FIRE` identity or burning flag without requiring that neighbor to be hot. It runs before generic flame quenching. | F07/F08 include the cold-H2/O2 full-tick case. |
| Water contact ignores dose | Generic `update` disables burning for any orthogonally adjacent `WATER`, without a cooling or evaporation debit in that reaction call. | F06 isolates reactions. Thermal diffusion runs separately; this is not evidence of full-tick cooling performance. |
| Fuel-dependent oxygen and disappearing smoke identity | Enclosed-air oxygen cannot feed hydrogen's `reactWithAir`; generic smoke can expire to `EMPTY` while retaining mass/heat. | F09/F10. F11 separately covers funded brush ignition. |

## Verification

These current regression suites protect specific behavior, including useful
controls. Passing them does not establish that the defects above are corrected.

| Claim or regression | Existing tests |
| --- | --- |
| Finite chemical heat, contact transfer, ceiling and funded asynchronous emission | [reactions.test.ts](../web/tests/legacy/physics/reactions.test.ts), [combustion-emission.test.ts](../web/tests/legacy/physics/combustion-emission.test.ts) |
| Brush accounting, no lifetime refresh, finite fuel and held-brush behavior | [fire-brush.test.ts](../web/tests/legacy/tools/fire-brush.test.ts), [interaction-regressions.test.ts](../web/tests/legacy/integration/interaction-regressions.test.ts) |
| Explicit oxidation, residual reactants, source accounting and hydrogen ignition | [element-expansion.test.ts](../web/tests/legacy/integration/element-expansion.test.ts), [review-regressions.test.ts](../web/tests/legacy/integration/review-regressions.test.ts) |
| Same-tick plume transport, wall geometry and seeded gas movement | [gas-motion.test.ts](../web/tests/legacy/physics/gas-motion.test.ts) |

After the root setup, run the focused suites from the repository root:

```sh
mise exec -- bun test \
  ./web/tests/legacy/physics/reactions.test.ts \
  ./web/tests/legacy/physics/combustion-emission.test.ts \
  ./web/tests/legacy/tools/fire-brush.test.ts \
  ./web/tests/legacy/physics/gas-motion.test.ts \
  ./web/tests/legacy/integration/element-expansion.test.ts \
  ./web/tests/legacy/integration/review-regressions.test.ts \
  ./web/tests/legacy/integration/interaction-regressions.test.ts
```

Use `mise run test` for the full test suite and `mise run docs:check` for knowledge
links and preserved evidence. The focused command runs headless TypeScript tests;
it does not run the historical fire harness, browser rendering, Rust or GPU gates.
Record fresh results separately using the [evidence template](templates/phase-evidence.md)
when claiming acceptance of a work item.

## Planned corrections

[Fire integration](plans/fire-combustion/plan.md) owns finite shared reactants,
admissible ignition, explicit products, amount-based water suppression, persistent
smoke composition and derived visuals. [FIRE-A acceptance](validation/fire-combustion.md#corrected-gates)
owns the corrected assertions; [the fire manifest](data/fire-combustion-work.json)
owns work/fixture status. Consult those sources rather than copying their tables.

The first GPU demonstrator remains nonreactive. A passing legacy regression or
historical characterization check does not pass a corrected FIRE-A gate, and
reading this article does not start the migration or authorize fixing these defects.
