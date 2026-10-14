# Combustion in the current TypeScript backend

**Scope:** Fire brush, generic fuel burning, explicit elemental oxidation and flame/smoke lifecycle in `web/src/legacy/`.
**Status:** implemented game behavior with known defects; corrected Rust/GPU combustion remains planned.
**Source review:** 27 September 2026, with the Wood/Oil tempo update on 14 October 2026. Linked regressions and fresh tempo evidence cover their stated scope; they do not rerun the archived fire audit or validate complete combustion physics.

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
chemical energy and oxygen eligibility. Chemical state is stored in kJ and
thermal state in J: the debit is multiplied by 1000 when added to `world.energy`.
Normally air-fed Wood/Oil use the existing legacy fresh-air reservoir to export
newly released heat that cannot fit below the product-temperature ceiling.
`World.addExternalEnergy` removes those joules from the parcel and records the
same amount in `externalEnergyRemoved`. The existing finite release rate and
chemical reserves are unchanged. This declared gameplay approximation lets fuel
consumption continue after the parcel reaches the ceiling; it is not resolved
gas exhaust or radiative heat transport. Enclosed Wood/Oil and other generic
fuels still pause at the ceiling while retaining fuel and the burning flag.
Exhausted generic fuel changes to `SMOKE`.

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
| Closed-boundary oxygen shortcut | Generic `beginStep` replenishes edge-connected air and `consumeOxygen` grants fresh oxygen without a debit. Neither it nor elemental `markVentilatedAir` consults `boundariesEnabled`; a drawn stone enclosure has different semantics. The Wood/Oil product-heat outflow uses the same legacy reservoir eligibility, including that flag limitation. Its energy removal is ledgered. | F01-F03 characterize generic burning; the flag does not prove a sealed combustion budget. |
| Incomplete generic products | Generic release spends chemical energy and, for enclosed gas, a separate oxygen counter without progressively consuming fuel/O2 material mass into declared products. Elemental `reactPair` is a separate mechanism. | F04/F05; F12 is an explicit-reaction control. Scalar energy conservation alone does not establish correct composition. |
| Cold flame label ignites reactants | Elemental `step` accepts an adjacent `FIRE` identity or burning flag without requiring that neighbor to be hot. It runs before generic flame quenching. | F07/F08 include the cold-H2/O2 full-tick case. |
| Water contact ignores dose | Generic `update` disables burning for any orthogonally adjacent `WATER`, without a cooling or evaporation debit in that reaction call. | F06 isolates reactions. Thermal diffusion runs separately; this is not evidence of full-tick cooling performance. |
| Fuel-dependent oxygen and disappearing smoke identity | Enclosed-air oxygen cannot feed hydrogen's `reactWithAir`; generic smoke can expire to `EMPTY` while retaining mass/heat. | F09/F10. F11 separately covers funded brush ignition. |

## Verification

These current regression suites protect specific behavior, including useful
controls. Passing them does not establish that the defects above are corrected.

| Claim or regression | Existing tests |
| --- | --- |
| Full-physics Wood/Oil spread and finite fuel depletion, bounded heat outflow and extinction controls | [combustion-tempo.test.ts](../web/tests/legacy/physics/combustion-tempo.test.ts), [playable-fuel-fire.ts](../web/tests/legacy/fixtures/playable-fuel-fire.ts) |
| Finite chemical heat, contact transfer, ceiling and funded asynchronous emission | [reactions.test.ts](../web/tests/legacy/physics/reactions.test.ts), [combustion-emission.test.ts](../web/tests/legacy/physics/combustion-emission.test.ts) |
| Brush accounting, no lifetime refresh, finite fuel and held-brush behavior | [fire-brush.test.ts](../web/tests/legacy/tools/fire-brush.test.ts), [interaction-regressions.test.ts](../web/tests/legacy/integration/interaction-regressions.test.ts) |
| Explicit oxidation, residual reactants, source accounting and hydrogen ignition | [element-expansion.test.ts](../web/tests/legacy/integration/element-expansion.test.ts), [review-regressions.test.ts](../web/tests/legacy/integration/review-regressions.test.ts) |
| Same-tick plume transport, wall geometry and seeded gas movement | [gas-motion.test.ts](../web/tests/legacy/physics/gas-motion.test.ts) |

After the root setup, run the focused suites from the repository root:

```sh
mise exec -- bun test \
  ./web/tests/legacy/physics/reactions.test.ts \
  ./web/tests/legacy/physics/combustion-tempo.test.ts \
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

### Legacy Wood/Oil tempo checkpoint on 14 October 2026

[XER-99](https://linear.app/xerrion/issue/XER-99/restore-playable-wood-and-oil-burning-in-the-current-sandbox)
records this bounded correction. Runtime commit `c62516775eafa5896eff979b89460a1846d3d7a5`
contains the correction against baseline `f47e2eb7f8a2e5128b5bba09b420c31560a4c092`.
Local raw results, source hashes, the patch and
logs are in ignored `artifacts/wood-oil-burn-tempo-2026-10-14/`.

Full-physics characterization found that the product-energy ceiling leaves
almost no thermal room for subsequent chemical release. Wood approaches
705.88 C and Oil 600 C because the ceiling uses Smoke's specific heat. Raising
the per-tick rate alone cannot remove this stall. The retained correction uses
the explicit, ledgered atmospheric heat outflow described above; the existing
100 J per-tick release policy, chemical reserves and fixed 1/60 s tick remain.

The [frozen fixture](../web/tests/legacy/fixtures/playable-fuel-fire.ts) uses a
32 x 32 world, an eight-cell-wide Wood block or open Oil basin, initial 22 C
fuel and funded ignition. Before tuning, the fully ignited eight-cell strips
required complete exhaustion within 600 ticks (10 model seconds). The 8 x 3
fixtures required spread beyond the four brush-heated cells and at least 95%
chemical-fuel consumption within 1800 ticks (30 model seconds). Those latter
checks cover seeds 1, 42 and 123. Seed 42 measurements are:

| Scene | Before: fuel consumed after 60 model s | After: model s to 95% consumption |
| --- | --- | --- |
| Wood block, one Fire click | 13.94% | 5.12 |
| Oil basin, one Fire click | 5.31% | 17.22 |
| Wood strip, all cells preheated | 24.52% | 1.78 |
| Oil strip, all cells preheated | 7.73% | 5.67 |

Held Fire for three model seconds followed by release reaches 95% at 5.45 s
for Wood and 17.28 s for Oil. With the default boundary flag enabled, the same
one-click fixtures reach 95% at 5.25 s and 17.37 s. This flag is not evidence of
a physically sealed oxygen budget. All eight 60-model-second runs pass mass
and energy accounting with external sources and sinks included. Each headless
run takes approximately 2 to 3 wall seconds; that is execution cost, not a
measurement of the browser's 1x clock or full-grid performance.

Both strip regressions fail before the correction. After it, the eight focused
suites pass 137 tests and 31,265 assertions. Controls preserve enclosed fuel
at the ceiling and prevent heat outflow with no oxygen, adjacent water or
cooled fuel. Independent static review found no actionable defect.

A manual localhost browser smoke check also showed a small drawn Wood sample
burn away and an air-exposed Oil stroke lose its visible fuel and chemical
energy after funded Fire input. Oil was observed depleted by 17.7 wall seconds.
This interactive check used the default 480 x 270 grid; it did not measure
accepted model time or freeze the numerical fixture. An earlier Oil setup
overwrote its Stone container and was discarded. Screenshots are local
diagnostic artifacts, not a browser performance qualification. User play review
and the corrected Rust/GPU FIRE-A gates remain pending.

Complete pinned CI passes: 522 TypeScript tests with zero failures, Rust
workspace tests, formatting and Clippy, lint and type checks, production and
reporting builds, WASM/catalogue freshness, documentation and browser smoke.
The browser report records `gpu.status: pass` for the existing nonreactive
preview. Ignored native hardware fixtures and sustained GPU qualification were
not rerun for this legacy-only correction. Exact local command:

```sh
env PATH=/home/xerrion/.cargo/bin:$PATH \
  CHROME_BIN=/home/xerrion/Projects/particle-foundry/artifacts/xer-64/rtx-baseline-after-reboot-2026-10-14/chromium-rtx \
  VK_DRIVER_FILES=/usr/share/vulkan/icd.d/nvidia_icd.json \
  mise run ci
```

The passing log is `artifacts/wood-oil-burn-tempo-2026-10-14/ci-verified.log`.
Final documentation-only evidence text is checked again with `mise run docs:check`.
Delivery is [PR #32](https://github.com/Xerrion/particle-foundry/pull/32).
The PR and XER-99 comments record merge and hosted-check status. The local
checks above do not establish public deployment or user play acceptance.

## Planned corrections

[Fire integration](plans/fire-combustion/plan.md) owns finite shared reactants,
admissible ignition, explicit products, amount-based water suppression, persistent
smoke composition and derived visuals. [FIRE-A acceptance](validation/fire-combustion.md#corrected-gates)
owns the corrected assertions; [the fire manifest](data/fire-combustion-work.json)
owns work/fixture status. Consult those sources rather than copying their tables.

The first GPU demonstrator remains nonreactive. A passing legacy regression or
historical characterization check does not pass a corrected FIRE-A gate, and
reading this article does not start the migration or authorize fixing these defects.
