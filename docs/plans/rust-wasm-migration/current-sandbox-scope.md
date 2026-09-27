# P1 current-sandbox acceptance scope

**Authority:** this document fixes the existing feature coverage required to complete
P1 and switch the current sandbox to Rust/WASM + wgpu. The [work-package plan](plan.md)
assigns E00-E14; the [fluid plan](../fluid-gpu-redesign/plan.md) owns M0-M7 numerical
gates. This checklist adds required product coverage; it does not replace those
gates. P2-P9 and new palette/reaction content are deferred until separately selected.

**Baseline:** source inspection of commit `69889ceb9714524858f4edaf344db103e636adff`
on 2026-09-27. The source and test links below describe the TypeScript legacy
implementation. No physical Rust/GPU acceptance is established by this inventory.
E00-E03 evidence covers its stated baseline, bootstrap and contract scope only.

## Completion rule

M3 remains a small, nonreactive water/air/fixed-wall scene. Thermal markers at M3
do not establish thermodynamics, and a fixed stone wall does not establish the
stone/lava phase family. Later P1 work adds the required groups below. Every
`CUR-*` row must have new evidence before M7 can close or the full switch can be
claimed. Experimental subset delivery and temporary legacy fallback are allowed;
they do not satisfy missing current-feature rows.

Preserve user operations, material identities, observable interactions and existing
named regression scenarios. Do not require bit-identical cellular trajectories,
old internal storage or synchronous probes. The new conservative solver and corrected
fire/circuit accounting intentionally change those implementation details. Record
each changed expectation with its defect, replacement contract and independent test.

Coverage means the observed/reachable behavior and named fixtures identified here,
including existing API-only phase fixtures. It does not mean every arbitrary
combination of all catalogue IDs, unbounded grids or extrapolated property domains.
Before implementing each row, freeze its fixture inputs, valid domains and tolerances
under the existing numerical gate. If a current control or required fixture exceeds
the proposed model/property domain, resolve the gap or obtain a user-approved scoped
correction; do not delete the row, silently clamp it or relabel it as deferred.

## Fixed catalogue and tools

The authored source is [definitions.ts](../../../web/src/materials/definitions.ts):
`materialDefinitions`, `phaseFamilies`, `selectableMaterials`, `toolDefinitions`
and `pickerIds`. The following inventory preserves legacy identities, not a second
authored property table. Generate engine projections from that source.

| Current selectable entries | Legacy IDs and keys |
| --- | --- |
| Basic palette | `1 sand`, `2 water`, `3 wood`, `4 oil`, `5 stone`, `6 metal`, `7 plant`, `8 fire`, `9 lava` |
| Additional matter | `12 ice`, `13 glass`, `14 moltenMetal`, `15 gunpowder` |
| Circuit presets | `16 wire`, `17 battery`, `18 ground`, `19 lamp` |
| Aqueous presets | `20 hydrochloricAcid`, `21 sulfuricAcid`, `22 sodiumHydroxide` |
| Eight elemental models | `24 hydrogen`, `25 oxygen`, `26 helium`, `27 iron`, `30 copper`, `35 carbon`, `36 nitrogen`, `39 sulfur` |

There are **28 selectable legacy entries**, including Fire. Fire's ID remains an
input/compatibility identity; the corrected engine treats ignition as a funded
command and flame appearance as derived visualization, not a conserved FIRE species.

| Current nonselectable entries | Legacy IDs and keys |
| --- | --- |
| Ambient, water and generic products | `0 air`, `10 smoke`, `11 steam`, `23 neutralSolution` |
| Iron and copper phases | `28 liquidIron`, `29 ironVapor`, `31 liquidCopper`, `32 copperVapor` |
| Oxygen and nitrogen phases | `33 liquidOxygen`, `34 solidOxygen`, `37 liquidNitrogen`, `38 solidNitrogen` |
| Sulfur phases and oxidation products | `40 liquidSulfur`, `41 sulfurVapor`, `42 carbonDioxide`, `43 sulfurDioxide` |

These 16 entries complete the **44 legacy catalogue IDs, 0-43**. Their presence
requires identity mapping and the existing derived/API behavior below; it does not
make them new palette buttons. Physical smoke/products must remain accounted for
independently of visual lifetime.

| Tool | Legacy ID/key | Existing input |
| --- | --- | --- |
| Blast | `252 blast` | `B`; funded local impulse/heat source |
| Cool | `253 cool` | `C`; external heat removal, brush floor −200 °C |
| Heat | `254 heat` | `H`; external heat input, brush ceiling 3000 °C |
| Eraser | `255 eraser` | `0`; replace with ambient air and account for removal |

Keys `1`-`9` select the corresponding basic-palette entries. The brush radius is
an integer from 1 to 12, default 4. Tool limits restrict applied input; they are
not permission to clamp passive thermal evolution. Evidence:
[createBrush](../../../web/src/legacy/tools/brush.ts),
[material catalogue tests](../../../web/tests/materials/material-catalogue.test.ts),
[brush tests](../../../web/tests/legacy/tools/brush.test.ts),
[Fire brush tests](../../../web/tests/legacy/tools/fire-brush.test.ts).

## Eight-element compatibility boundary

These are existing forms/interactions from `materialDefinitions`, `phaseFamilies`,
[createElementReactions](../../../web/src/legacy/physics/element-reactions.ts) and
[createElectricity](../../../web/src/legacy/physics/electricity.ts). Their legacy
constants are approximations, not independent acceptance for the new solver.

| Identity | Required existing forms | Required existing interaction/limit |
| --- | --- | --- |
| H | Hydrogen gas, ID 24 | Heated/funded ignition with O₂ produces water; open-air burning remains available through finite oxygen transport. No cryogenic hydrogen or dissociation model. |
| He | Helium gas, ID 26 | Inert transport, thermal exchange and pressure. No combustion or oxygen supply. |
| C | Graphite, ID 35 | Ignited contact oxidation with O₂ produces CO₂ (42), retaining excess reactants. No diamond, CO or sublimation expansion. |
| N | Nitrogen gas/liquid/solid, IDs 36/37/38 | Inert phase family; transitions near −195.795/−210 °C. No fixation chemistry. |
| O | Oxygen gas/liquid/solid, IDs 25/33/34 | Existing oxidizer behavior and phase family near −182.95/−218.79 °C; oxygen alone does not burn. |
| S | Sulfur solid/liquid/vapor, IDs 39/40/41 | Phase family near 115.21/444.61 °C; ignited oxidation produces SO₂ (43). No sulfuric-acid synthesis. |
| Fe | Iron solid/liquid/vapor, IDs 27/28/29 | Distinct thermal/phase data; solid-state conduction; painted solids anchored, free molten parcels remain movable after freezing. No rust or alloys. |
| Cu | Copper solid/liquid/vapor, IDs 30/31/32 | Distinct thermal/phase and solid-state electrical properties. No oxidation, corrosion or alloys. |

Solid nitrogen and solid oxygen are below the current Cool tool floor. Preserve
their existing model/API fixtures and conversion paths; this does not request new
cryogenic controls. Nonselectable elemental phases are reachable through phase
evolution or model fixtures, not independent palette selection. The other 110
periodic-table entries remain disabled reference records.

The existing `Sandbox.step()` API remains a controlled stepping/test interface.
There is no current single-step button or key to preserve; adding that UI is
deferred. Browser integration tests can exercise stepping through a harness.

## Required acceptance rows

All rows are **required; Rust/GPU completion evidence pending**. Existing tests are
characterization and regression inputs. Where a legacy assertion encodes a known
defect, retain its historical result and add the corrected assertion rather than
claiming pixel or numerical parity proves correctness. E/M owners below identify
where coverage must be delivered; all rows are audited together at E14/M7.

| Stable ID | Required behavior and source owner | Work owner and existing test families | Evidence still required |
| --- | --- | --- | --- |
| CUR-01 | Preserve the inventory above, identity validation, generated catalogue provenance, material search/category filtering, selection guides and eight enabled element tiles. [bindMaterialPicker](../../../web/src/app/material-picker.ts), [materialGuide](../../../web/src/materials/guide.ts). | E03, E09-E12 / M1, M4-M6. [Catalogue](../../../web/tests/materials/material-catalogue.test.ts), [projection](../../../web/tests/materials/engine-projection.test.ts), [element reference](../../../web/tests/materials/element-reference.test.ts), [browser controls](../../../web/tests/app/browser-controls.test.ts). | Full required projection/mapping, round trips for derived IDs, valid/invalid input cases and actual-browser selection on the owning engine. The existing air/water/stone candidate projection is only the first subset. |
| CUR-02 | Circle/continuous-line painting, held mouse/touch tools, paused edits, no refill of existing material fuel/energy by repaint, coordinate/radius validation and source-aware erase. [createBrush](../../../web/src/legacy/tools/brush.ts), [bindControls](../../../web/src/app/controls.ts). | E03, E11-E12 / M1, M6. [Brush](../../../web/tests/legacy/tools/brush.test.ts), [Fire brush](../../../web/tests/legacy/tools/fire-brush.test.ts), [interaction regressions](../../../web/tests/legacy/integration/interaction-regressions.test.ts), [session contract](../../../web/tests/app/engine-session.test.ts). | Every selectable entry/tool through the real queued owner; bounded backlog; exactly-once input/source commits; zero-tick paused edits; reset during held input. |
| CUR-03 | Pause/resume, 0.5×/1×/2×/4× speed, fixed model time, clear/reset, brush size, World boundaries and Surface shimmer toggles, and keyboard shortcuts. Preserve pointer cancellation, focus/resize behavior and accessible tab/filter controls. [bindControls](../../../web/src/app/controls.ts), [createSimulationClock](../../../web/src/app/simulation-clock.ts). | E08, E12 / M3, M6. [Browser controls](../../../web/tests/app/browser-controls.test.ts), [clock](../../../web/tests/app/simulation-clock.test.ts). | Actual-browser running/paused/hidden-tab and boundary/shimmer toggle tests; speed measured by accepted time; responsive controls under GPU load; no delayed input applied after reset. |
| CUR-04 | Material rendering, temperature/pressure/velocity maps and legends, air/gas direction arrows, cursor, surface shimmer, glow, zoom/pan/fullscreen and correct pointer mapping. [createRenderer](../../../web/src/legacy/rendering/renderer.ts), [bindViewControls](../../../web/src/app/view-controls.ts), [createViewport](../../../web/src/app/viewport.ts). | E08, E12 / M3, M6. [Renderer](../../../web/tests/legacy/rendering/renderer.test.ts), [visual waves](../../../web/tests/legacy/rendering/visual-waves.test.ts), [viewport](../../../web/tests/app/viewport.test.ts), [pointer mapping](../../../web/tests/app/pointer-mapping.test.ts). | Same-device direct rendering at 480×270; maps usable while paused; display scaling/fullscreen checks; no fake liquid at interfaces; render/quality toggles leave physical inventories unchanged. |
| CUR-05 | Material/temperature/pressure/velocity probes; particle count, matter mass, tracked energy and maximum pressure readouts. [Sandbox](../../../web/src/legacy/simulation/sandbox.ts), [measureWorld](../../../web/src/legacy/simulation/diagnostics.ts). | E08, E12 / M3, M6. [Physical model](../../../web/tests/legacy/integration/physical-model.test.ts), [world counts](../../../web/tests/legacy/simulation/world-counts.test.ts), [browser controls](../../../web/tests/app/browser-controls.test.ts). | Async tick/epoch-labelled observations, defined count semantics for fractional occupancy, no stale sample displayed as current, bounded readback and correct diagnostics after paint/erase/reset. |
| CUR-06 | Ambient-air motion, gas confinement/advection, water pooling and falling strokes, liquid separation, viscosity response, wall exclusion and closed/open boundaries with finite venting. [createPhysics](../../../web/src/legacy/simulation/physics.ts), [createMotion](../../../web/src/legacy/physics/motion.ts), [createGasDynamics](../../../web/src/legacy/physics/gas-dynamics.ts). | E05-E10 / M2-M5. [Motion](../../../web/tests/legacy/physics/motion.test.ts), [airflow](../../../web/tests/legacy/physics/airflow.test.ts), [gas motion](../../../web/tests/legacy/physics/gas-motion.test.ts), [hydrostatics](../../../web/tests/legacy/physics/hydrostatics.test.ts), [liquid regressions](../../../web/tests/legacy/integration/interaction-regressions.test.ts). | M2/M3 independent operators, transport and refinement results, then additional current liquids/gases; mass/momentum/energy and boundary-flux balances at equal accepted times. Retain the ambient-air movement regression. |
| CUR-07 | Heat exchange, Heat/Cool, water freeze/melt/boil/condense and rising steam bubbles; pressure-dependent water boiling; metal/molten-metal and stone/lava transitions; irreversible sand-to-glass. [createThermalSolver](../../../web/src/legacy/physics/thermal.ts), [createBoiling](../../../web/src/legacy/physics/boiling.ts). | E09 / M4. [Thermal](../../../web/tests/legacy/physics/thermal.test.ts), [boiling](../../../web/tests/legacy/physics/boiling.test.ts), [sandbox thermal integration](../../../web/tests/legacy/integration/sandbox.test.ts). | Mass-scaled phase/energy closure, supported hot/cold property domains, partial-phase and sealed/vented fixtures; equivalent accessible operations under the new internal-energy contract. |
| CUR-08 | Sand/gunpowder settling, anchored painted walls/structures, explicitly dynamic solids, displacement/buoyancy, collisions and movable solidification. [createMotion](../../../web/src/legacy/physics/motion.ts), [createSolidMechanics](../../../web/src/legacy/physics/solid-mechanics.ts), `Sandbox.setCellDynamic`. | E10 / M5. [Motion](../../../web/tests/legacy/physics/motion.test.ts), [physical model](../../../web/tests/legacy/integration/physical-model.test.ts), [review regressions](../../../web/tests/legacy/integration/review-regressions.test.ts). | M5 contention/no-outlet fixtures, conservative swept volume/work and impact heating, no wall tunnelling, and browser sand-over-water measurements. No fracture or new rigid-body feature set. |
| CUR-09 | All eight existing elemental forms and their transport, thermal and phase behavior listed above; inert He/N₂ and distinct Fe/Cu properties. [materialDefinitions](../../../web/src/materials/definitions.ts). | E09-E11 / M4-M6. [Element expansion](../../../web/tests/legacy/integration/element-expansion.test.ts), [review regressions](../../../web/tests/legacy/integration/review-regressions.test.ts), [thermal](../../../web/tests/legacy/physics/thermal.test.ts). | Corrected Rust references and GPU parity for each existing phase family, including API-only cold states, generated properties and explicit supported domains. This existing coverage cannot wait for P4/C3. |
| CUR-10 | H₂/O₂→water, carbon/O₂→CO₂ and sulfur/O₂→SO₂, funded ignition from either relevant H₂/O₂ interface side, open-air hydrogen burning and retained excess reactants. [createElementReactions](../../../web/src/legacy/physics/element-reactions.ts). | E09, E11 / M4, M6. [Element expansion](../../../web/tests/legacy/integration/element-expansion.test.ts), [review regressions](../../../web/tests/legacy/integration/review-regressions.test.ts), [Fire brush](../../../web/tests/legacy/tools/fire-brush.test.ts). | Amount-based products/residuals, finite oxygen flux, source/energy closure and correct ignition in the full pipeline; wall/diagonal exclusions for existing contact fixtures. No new reaction catalogue. |
| CUR-11 | Fire ignition, finite wood/oil/plant burning, heat transfer, derived flame/smoke appearance, water thermal suppression and existing plant growth with funded mass/chemical energy. [createReactions](../../../web/src/legacy/physics/reactions.ts), [createBrush](../../../web/src/legacy/tools/brush.ts). | E09, E11-E14 / M4, M6-M7; FIRE-W02-W07. [Reactions](../../../web/tests/legacy/physics/reactions.test.ts), [emission](../../../web/tests/legacy/physics/combustion-emission.test.ts), [interaction](../../../web/tests/legacy/integration/interaction-regressions.test.ts), [growth accounting](../../../web/tests/legacy/integration/review-regressions.test.ts). | Corrected [FIRE-A/FIRE-M7 gates](../../validation/fire-combustion.md), oxygen/products/soot persistence, thermal suppression and render-on/off equivalence. Growth remains a declared external source, not new plant chemistry. |
| CUR-12 | Existing aqueous presets: hydrochloric acid 1 M, sulfuric acid 0.5 M, sodium hydroxide 1 M, and neutralized solution. Adjacent acid/base reaction releases funded heat; walls/diagonal-only contact block the existing fixture. [createNeutralization](../../../web/src/legacy/physics/neutralization.ts). | E11 / M6. [Neutralization](../../../web/tests/legacy/physics/neutralization.test.ts), [catalogue](../../../web/tests/materials/material-catalogue.test.ts). | Amount-based limiting-reagent/residual tests, finite source/heat accounting and coupled transport. Correct the whole-cell conversion approximation where it loses excess reagent; no new pH UI, corrosion or synthesis feature set. |
| CUR-13 | Blast and gunpowder as calibrated local heat/impulse sources: finite gunpowder energy, externally funded Blast, ignition/chain behavior and anchored-wall shielding. [createExplosions](../../../web/src/legacy/physics/explosions.ts). | E11-E12 / M6. [Explosions](../../../web/tests/legacy/physics/explosions.test.ts), [brush](../../../web/tests/legacy/tools/brush.test.ts). | Funded source transactions, exactly-once detonation, wall/closed-vessel/open-vent fixtures and documented low-Mach source approximation/domain. Resolve current-tool domain conflicts explicitly; resolved pressure waves, shocks and fracture remain deferred P2. |
| CUR-14 | Finite Battery→Wire/Metal/Fe/Cu→Lamp→Ground circuits, open/disconnected/diagonal exclusions, Joule heat, depletion and conductor changes on melting/editing. [createElectricity](../../../web/src/legacy/physics/electricity.ts). | E04, E11 / M1, M6. [Electricity](../../../web/tests/legacy/physics/electricity.test.ts), [element circuits](../../../web/tests/legacy/integration/review-regressions.test.ts); corrected N11-N13 and C17/C18 gates in the [fluid plan](../fluid-gpu-redesign/plan.md). | Independent residual/terminal-work/branch-heat references, exhaustion re-solve, graph versioning, bounded GPU/CPU transfers, sparse overflow and dense cost. E04 is required before full completion although it does not block M3. |
| CUR-15 | Current starter scene, deterministic seed/reset/clear, ambient 22 °C clear state and retained floor when top/sides open. [seedStarterScene](../../../web/src/legacy/scenes/starter-scene.ts), [createSandbox](../../../web/src/legacy/simulation/sandbox.ts), [main.ts](../../../web/src/main.ts). | E12-E14 / M6-M7. [Sandbox reset](../../../web/tests/legacy/integration/sandbox.test.ts), [world](../../../web/tests/legacy/simulation/world.test.ts), [physics](../../../web/tests/legacy/integration/physics.test.ts); [boiling-pot](../../../web/tests/legacy/fixtures/boiling-pot.ts) and [burning-oil-pool](../../../web/tests/legacy/fixtures/burning-oil-pool.ts) fixtures. | One complete starter scene at 480×270 plus coupled existing feature scenes. Replay/seed tests use declared same-configuration tolerances; no cross-backend chaotic pixel identity requirement. |
| CUR-16 | One live state owner; preserve current controls across startup, reset and renderer replacement; migration-required snapshots/device-loss recovery. [createEngineSession](../../../web/src/engine-client/session.ts), [selectCanvasContext](../../../web/src/engine-client/canvas.ts). | E03, E12-E14 / M1, M6-M7; MIG-02-MIG-12. [Session](../../../web/tests/app/engine-session.test.ts), [canvas owner](../../../web/tests/app/canvas-owner.test.ts), [browser bootstrap](../../../web/tests/browser/engine-smoke.ts). | Real-device lifecycle/failure injection, atomic save/load and version/capacity rejection, rollback notices, stale callback cancellation and compatible fallback. Saves/loads are new migration reliability contracts; the current `Sandbox` has no existing save UI or save format to preserve. |
| CUR-17 | Responsive, sustained complete sandbox with bounded allocations/transfers and completed-work telemetry. [main.ts](../../../web/src/main.ts) is the current host baseline; [backend migration](../../validation/backend-migration.md) owns measurement protocol. | E14 / M7; MIG-08, MIG-13, MIG-14 and GPU-M7. Existing regression families above and [benchmarks](../../../web/benchmarks). | Agreed browser/device matrix, full current coverage, idle/pool/sand/thermal/fire/circuit/painting workloads, p50/p95 input/frame timing, accepted simulation throughput, memory and readback. Historical timings and a native-only pass do not establish promotion. |

## Corrections, gaps and evidence records

The compatibility target includes present features, not known defects. In particular,
replace connected-air oxygen refill, cold FIRE-label ignition, missing combustion
products, contact-only water extinction and lifetime-based physical smoke removal
under [corrected fire acceptance](../../validation/fire-combustion.md). Replace the
circuit convergence/work/heat defects under N11-N13/C17-C18. Replace cellular fluid
transport and inconsistent volume/energy closure under M2-M5. Preserve the useful
operation in each case and record how its observable outcome changes.

The current [browser control test](../../../web/tests/app/browser-controls.test.ts)
uses Happy DOM and a mocked `Sandbox`. E03 session tests use mock owners. The current
browser smoke proves bootstrap/device construction, not a running physical scene.
Legacy physics tests do not execute Rust/WGSL. These are explicit evidence gaps for
every corresponding `CUR-*` row, not reasons to mark product parity complete.

At E14, record one evidence entry per row using the
[phase evidence template](../../templates/phase-evidence.md): fixture/test identifiers,
Git revision and working-tree changes, domains/tolerances, native reference results,
browser/adapter results, corrected expectations, and any unrun cases. Keep failures
visible. A required row with unavailable hardware evidence or an unresolved current
feature/domain gap blocks the full-switch claim.

Only internal species/products needed to conserve the existing reactions may be
added during this migration. Such an addition must name the existing `CUR-*` row,
declare properties/domain and carry independent amount/energy evidence. It does not
authorize new selectable compounds, elemental cohorts, additional chemistry,
pressure-wave/fracture features or nuclear work. Existing 118-entry reference
search remains presentation-only.
