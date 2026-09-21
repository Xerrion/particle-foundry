#!/usr/bin/env node
'use strict';
/** Targeted regression/invariant tests for src(1).zip. No simulation code is modified.
 * Usage: node audit.cjs <compiled-directory> [results.json]
 * FAIL means the stated invariant is violated; some invariants require a model redesign.
 */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(process.argv[2] || '.audit-build');
const load = name => require(path.join(root, name + '.js'));
const { createWorld } = load('world'), { createPhysics } = load('physics');
const { createGasDynamics } = load('gas-dynamics'), { createSolidMechanics } = load('solid-mechanics');
const { createMotion } = load('motion'), { createFluidSolver } = load('fluid-solver');
const { createReactions } = load('reactions'), { createElectricity } = load('electricity');
const { createElementReactions } = load('element-reactions'), { createNeutralization } = load('neutralization');
const { createAirflow } = load('airflow');
const { createThermalSolver, energyAtTemperature, temperatureFromEnergy, heatCapacityForMass } = load('thermal');
const { measureWorld, compareConservation } = load('diagnostics');
const M = load('materials'), S = load('physical-scale');
const results = [];
const sum = a => a.reduce((s, v) => s + v, 0);
const total = w => measureWorld(w).totalTrackedEnergyKj * 1000;
const kinetic = w => measureWorld(w).kineticEnergyKj * 1000;
const px = w => w.massKg.reduce((s, m, i) => s + m * w.velocityX[i], 0);
const py = w => w.massKg.reduce((s, m, i) => s + m * w.velocityY[i], 0);
const near = (a, b, atol = 1e-9, rtol = 1e-10) => Math.abs(a - b) <= atol + rtol * Math.max(Math.abs(a), Math.abs(b));
const temp = (w, i, t, p = S.AMBIENT_PRESSURE_PA) => w.energy[i] = energyAtTemperature(w.grid[i], t, p, w.massKg[i]);
function box(width, height) { const w = createWorld(width, height); for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++)
        if (x === 0 || y === 0 || x === width - 1 || y === height - 1)
            w.setCell(x + y * width, M.STONE); return w; }
function check(id, title, kind, locations, fn) {
    const r = { id, title, kind, locations, passed: false, metrics: {} };
    try {
        fn(r.metrics);
        r.passed = true;
    }
    catch (e) {
        r.error = e.message;
    }
    results.push(r);
    console.log(`${r.passed ? 'PASS' : 'FAIL'} ${id} ${title}\n  ${JSON.stringify(r.metrics)}`);
    if (r.error)
        console.log('  ' + r.error);
}
// New pressure implementation, tested independently of transport and thermal diffusion.
check('N01', 'Uniform pressurized gas in a rigid sealed cavity remains at rest', 'implementation', ['src/gas-dynamics.ts:144-195'], m => {
    const w = box(5, 3);
    for (const i of [6, 7, 8])
        temp(w, i, 60);
    const g = createGasDynamics(w);
    m.pressurePa = [6, 7, 8].map(i => w.pressurePa[i]);
    g.step();
    m.velocityX = [6, 7, 8].map(i => w.velocityX[i]);
    m.velocityY = [6, 7, 8].map(i => w.velocityY[i]);
    assert(m.velocityX.every(v => Math.abs(v) < 1e-12) && m.velocityY.every(v => Math.abs(v) < 1e-12), 'Uniform chamber pressure creates motion next to stationary walls.');
});
check('N02', 'A monotone pressure gradient accelerates an interior gas cell', 'numerical', ['src/gas-dynamics.ts:155-166'], m => {
    const w = createWorld(3, 1);
    [22.1, 22.2, 22.3].forEach((t, i) => temp(w, i, t));
    const g = createGasDynamics(w);
    m.pressurePa = Array.from(w.pressurePa);
    m.facePressureClipPa = w.massKg[1] * 3 / (S.CELL_WIDTH_METERS ** 2);
    g.step();
    m.velocityX = Array.from(w.velocityX);
    assert(w.velocityX[1] < -1e-12, 'Clipping both positive face tractions to the same impulse erases a nonzero interior gradient.');
});
check('N03', 'Cellular liquid falling gives a consistent nonzero downward velocity', 'model-integration', ['src/motion.ts:47-51,135-159,284-288'], m => {
    const w = createWorld(1, 4);
    w.setCell(0, M.WATER);
    createMotion(w).update(0, 0);
    const end = Array.from(w.grid).indexOf(M.WATER);
    m.row = end;
    m.velocityY = w.velocityY[end];
    m.distanceMeters = end * S.CELL_WIDTH_METERS;
    assert(end > 0 && w.velocityY[end] > 0, 'Water changes position while its velocity stays zero.');
});
check('N04', 'A vertical cellular swap accounts for gravitational work', 'model-integration', ['src/motion.ts:47-51', 'src/world.ts:264-298'], m => {
    const w = createWorld(1, 4);
    w.setCell(0, M.WATER);
    const before = total(w);
    createMotion(w).update(0, 0);
    m.energyChangeJ = total(w) - before;
    m.expectedPotentialDropJ = (M.materialsById[M.WATER].densityKgPerM3 - M.materialsById[M.EMPTY].densityKgPerM3) * S.CELL_VOLUME_M3 * S.GRAVITY_M_PER_S2 * S.CELL_WIDTH_METERS;
    assert(near(total(w), before, 1e-10, 0), 'Lost potential energy is not kinetic energy, heat, or a declared sink.');
});
check('N05', 'Horizontal liquid velocity transports an unblocked supported parcel', 'model-integration', ['src/motion.ts:162-188,268-288'], m => {
    const w = box(8, 3);
    w.setCell(9, M.WATER);
    w.velocityX[9] = 1;
    const motion = createMotion(w);
    for (let t = 0; t < 10; t++) {
        w.moved.fill(0);
        for (let i = 0; i < w.size; i++)
            motion.update(i, t);
    }
    m.startIndex = 9;
    m.endIndex = Array.from(w.grid).indexOf(M.WATER);
    m.velocityX = w.velocityX[m.endIndex];
    m.ticks = 10;
    assert(m.endIndex !== m.startIndex, 'A liquid with horizontal velocity remains fixed on a level support, despite an open path.');
});
check('N06', 'Nonzero ambient-air velocity transports its temperature marker', 'model-integration', ['src/motion.ts:59-66,268-297'], m => {
    const w = createWorld(8, 4);
    const start = 17;
    temp(w, start, 60);
    w.velocityX.fill(.6);
    const before = Array.from(w.energy);
    const motion = createMotion(w);
    for (let i = 0; i < w.size; i++)
        motion.update(i, 0);
    m.changedEnergyCells = Array.from(w.energy).filter((v, i) => v !== before[i]).length;
    m.markerTemperatureC = w.temperatureAt(start);
    assert(m.changedEnergyCells > 0, 'Ambient air is unconditionally excluded from the only transport pass.');
});
check('N07', 'Hydrostatic transfer conserves the tracked mechanical/thermal budget', 'model-integration', ['src/hydrostatics.ts:50-77', 'src/fluid-solver.ts:25-42'], m => {
    const w = box(5, 6);
    for (const i of [11, 16, 21, 22])
        w.setCell(i, M.WATER);
    const f = createFluidSolver(w);
    const b = measureWorld(w);
    m.transfers = f.step(0);
    const a = measureWorld(w);
    m.energyChangeJ = (a.totalTrackedEnergyKj - b.totalTrackedEnergyKj) * 1000;
    m.kineticChangeJ = (a.kineticEnergyKj - b.kineticEnergyKj) * 1000;
    m.potentialChangeJ = (a.potentialEnergyKj - b.potentialEnergyKj) * 1000;
    m.thermalChangeJ = (a.thermalEnergy - b.thermalEnergy) * 1000;
    assert(m.transfers > 0);
    assert(near(a.totalTrackedEnergyKj, b.totalTrackedEnergyKj, 1e-13, 0), 'The transfer only funds damped target kinetic energy and omits displaced-fluid work/lost damping.');
});
check('N08', 'EOS/phase iteration converges rather than flipping material identity', 'model-consistency', ['src/gas-dynamics.ts:64-140', 'src/world.ts:255-261'], m => {
    const w = box(3, 3);
    w.setCell(4, M.WATER);
    temp(w, 4, 150);
    const g = createGasDynamics(w);
    m.states = [];
    for (let n = 0; n < 8; n++) {
        g.derivePressure();
        const p = w.pressurePa[4];
        w.applyPhase(4);
        m.states.push({ material: M.materialsById[w.grid[4]].key, pressureUsedPa: p, energyJ: w.energy[4], volumeInCells: w.volumeM3[4] / S.CELL_VOLUME_M3 });
    }
    assert(m.states.slice(-4).every(s => s.material === m.states.at(-1).material), 'A fixed-mass, fixed-energy cell alternates between water and steam as pressure is refreshed.');
});
check('N09', 'Venting does not silently switch the same parcel to a different EOS volume', 'model-consistency', ['src/gas-dynamics.ts:80-83,116-118'], m => {
    const w = box(3, 3);
    w.setCell(4, M.WATER);
    temp(w, 4, 150);
    w.applyPhase(4);
    const g = createGasDynamics(w);
    m.sealedPressurePa = w.pressurePa[4];
    const mass = w.massKg[4], e = w.energy[4];
    w.setCell(1, M.EMPTY);
    g.derivePressure();
    m.openPressurePa = w.pressurePa[4];
    m.unchangedParcelMassAndEnergy = (mass === w.massKg[4] && e === w.energy[4]);
    m.volumeInCells = w.volumeM3[4] / S.CELL_VOLUME_M3;
    const geometricEOS = w.massKg[4] / M.materialsById[M.STEAM].molarMassKgPerMol * S.IDEAL_GAS_CONSTANT * (w.temperatureAt(4) + 273.15) / S.CELL_VOLUME_M3;
    m.geometricEOSPa = geometricEOS;
    assert(near(w.pressurePa[4], geometricEOS, 1e-4, 1e-8), 'The open path uses reference parcel volume, not the geometric volume used by the sealed path.');
});
check('N10', 'Free fall and impact close the solid gravitational energy budget', 'integration/accounting', ['src/solid-mechanics.ts:20-56', 'src/diagnostics.ts:45-47'], m => {
    const w = createWorld(1, 22);
    w.setCell(1, M.STONE);
    w.setDynamic(1, true);
    w.setCell(21, M.METAL);
    const s = createSolidMechanics(w), before = measureWorld(w);
    for (let t = 0; t < 120; t++)
        s.step();
    const after = measureWorld(w);
    m.finalRow = Array.from(w.grid).indexOf(M.STONE);
    m.energyChangeJ = (after.totalTrackedEnergyKj - before.totalTrackedEnergyKj) * 1000;
    m.potentialChangeJ = (after.potentialEnergyKj - before.potentialEnergyKj) * 1000;
    m.thermalChangeJ = (after.thermalEnergy - before.thermalEnergy) * 1000;
    m.finalSpeed = w.velocityY[m.finalRow];
    m.finalDisplacement = w.displacementY[m.finalRow];
    assert(near(after.totalTrackedEnergyKj, before.totalTrackedEnergyKj, 1e-12, 0), 'Grid-quantized positions/impact integration do not close the gravitational work budget.');
});
check('N11', 'Long DC resistor chain satisfies its linear voltage solution', 'numerical', ['src/electricity.ts:29-30,55-77'], m => {
    const n = 102, w = createWorld(n, 1);
    for (let i = 0; i < n; i++)
        w.setCell(i, i === 0 ? M.BATTERY : i === n - 1 ? M.GROUND : M.WIRE);
    const e = createElectricity(w);
    m.deliveredJ = e.step();
    m.midpointVoltageV = e.voltageAt(51);
    m.expectedMidpointVoltageV = 12 * (1 - 51 / (n - 1));
    m.expectedEnergyJ = 144 / (n - 1) * S.FIXED_TIME_STEP_SECONDS;
    m.sourceCurrentA = 12 - e.voltageAt(1);
    m.groundCurrentA = e.voltageAt(n - 2);
    m.maxInteriorKirchhoffResidualA = 0;
    for (let i = 1; i < n - 1; i++)
        m.maxInteriorKirchhoffResidualA = Math.max(m.maxInteriorKirchhoffResidualA, Math.abs(2 * e.voltageAt(i) - e.voltageAt(i - 1) - e.voltageAt(i + 1)));
    const first = e.voltageAt(51);
    e.step();
    m.secondStepMidpointVoltageV = e.voltageAt(51);
    m.restartsSameUnconvergedSolution = first === m.secondStepMidpointVoltageV;
    assert(near(m.midpointVoltageV, m.expectedMidpointVoltageV, 1e-4, 0), '128 cold-start iterations leave a large DC error; every tick restarts the same incomplete solve.');
});
check('N12', 'Joule heat is distributed in proportion to endpoint half-resistances', 'discretization', ['src/electricity.ts:90-95,113-117'], m => {
    const w = createWorld(3, 1);
    [M.BATTERY, M.LAMP, M.GROUND].forEach((mat, i) => w.setCell(i, mat));
    const old = Array.from(w.energy);
    createElectricity(w).step();
    m.actualHeatJ = Array.from(w.energy, (v, i) => v - old[i]);
    const current = 12 / 5;
    m.expectedHeatJ = [.5, 4, .5].map(r => current ** 2 * r * S.FIXED_TIME_STEP_SECONDS);
    assert(m.actualHeatJ.every((v, i) => near(v, m.expectedHeatJ[i], 1e-9, 0)), 'Equal endpoint heating contradicts unequal endpoint resistances in the same series circuit.');
});
check('N13', 'A battery with zero terminal current is not drained', 'implementation', ['src/electricity.ts:99-111'], m => {
    const w = createWorld(3, 1);
    [M.BATTERY, M.BATTERY, M.GROUND].forEach((mat, i) => w.setCell(i, mat));
    const b = Array.from(w.chemicalEnergyKj);
    const e = createElectricity(w);
    e.step();
    m.voltageV = [0, 1, 2].map(i => e.voltageAt(i));
    m.firstBatteryCurrentA = e.voltageAt(0) - e.voltageAt(1);
    m.batteryDebitsJ = [0, 1].map(i => (b[i] - w.chemicalEnergyKj[i]) * 1000);
    assert(near(m.batteryDebitsJ[0], 0, 1e-9, 0), 'Total heat is debited by stored-capacity share, not each terminal\'s electrical work.');
});
check('N14', 'Unequal acid/base equivalents do not become two fully neutral cells', 'model-domain', ['src/neutralization.ts:37-51'], m => {
    const w = createWorld(2, 1);
    w.setCell(0, M.HYDROCHLORIC_ACID);
    w.setCell(1, M.SODIUM_HYDROXIDE);
    w.massKg[0] *= 2;
    w.volumeM3[0] *= 2;
    w.chemicalEnergyKj[0] *= 2;
    w.energy[0] *= 2;
    m.initialAcidMol = .002;
    m.initialBaseMol = .001;
    m.releasedJ = createNeutralization(w).step();
    m.finalMaterials = Array.from(w.grid, id => M.materialsById[id].key);
    m.remainingAcidChemicalJ = w.chemicalEnergyKj[0] * 1000;
    assert(w.grid[0] !== M.NEUTRAL_SOLUTION, 'A valid variable-mass parcel with excess acid loses its acid identity; equal-volume UI presets are unaffected.');
});
check('N15', 'Tangential gas motion transfers shear between adjacent rows', 'model-limitation', ['src/airflow.ts:18-34,39-49'], m => {
    const w = createWorld(1, 2);
    w.velocityX[0] = 1;
    createAirflow(w).step(0);
    m.velocityX = Array.from(w.velocityX);
    m.velocityY = Array.from(w.velocityY);
    assert(w.velocityX[1] > 0, 'The adjacency-normal-only exchange transmits no tangential momentum between rows.');
});
// Narrow controls for the new fixes and for features not implicated above.
check('C01', 'Symmetric gas setup retains reflection symmetry', 'control', ['src/gas-dynamics.ts:155-166'], m => {
    const w = createWorld(3, 1);
    [22, 600, 22].forEach((t, i) => { w.setCell(i, M.FIRE); temp(w, i, t); });
    createGasDynamics(w).step();
    m.velocityX = Array.from(w.velocityX);
    assert(near(w.velocityX[0], -w.velocityX[2]) && near(w.velocityX[1], 0));
});
check('C02', 'Unequal-mass pressure pair preserves total momentum', 'control', ['src/gas-dynamics.ts:159-166'], m => {
    const w = createWorld(2, 1);
    w.setCell(0, M.FIRE);
    w.setCell(1, M.SMOKE);
    temp(w, 0, 600);
    temp(w, 1, 22);
    const p = px(w);
    createGasDynamics(w).step();
    m.momentumChange = px(w) - p;
    assert(near(px(w), p, 1e-18, 0));
});
check('C03', 'Converted and painted steam have equal specific enthalpy at equal temperature', 'control', ['src/thermal.ts:19-55'], m => {
    const w = createWorld(2, 1);
    w.setCell(0, M.WATER);
    temp(w, 0, 150);
    w.applyPhase(0);
    w.setCell(1, M.STEAM);
    m.massRatio = w.massKg[0] / w.massKg[1];
    m.energyRatio = w.energy[0] / w.energy[1];
    m.temperaturesC = [w.temperatureAt(0), w.temperatureAt(1)];
    assert(near(m.massRatio, m.energyRatio));
});
check('C04', 'Steam initialization and reading use the same pressure', 'control', ['src/world.ts:168-170'], m => {
    const w = createWorld(1, 1);
    w.setCell(0, M.STEAM);
    w.pressurePa[0] = 2 * S.AMBIENT_PRESSURE_PA;
    temp(w, 0, 150, w.pressurePa[0]);
    m.temperatureC = w.temperatureAt(0);
    assert(near(m.temperatureC, 150));
});
check('C05', 'A resting supported dynamic solid does not self-heat', 'control', ['src/solid-mechanics.ts:43-46'], m => {
    const w = createWorld(1, 1);
    w.setCell(0, M.STONE);
    w.setDynamic(0, true);
    const s = createSolidMechanics(w), before = w.energy[0];
    for (let t = 0; t < 600; t++)
        s.step();
    m.heatAddedJ = w.energy[0] - before;
    assert(near(w.energy[0], before, 1e-12, 0));
});
check('C06', 'Upward solid velocity produces upward transport', 'control', ['src/solid-mechanics.ts:20-56'], m => {
    const w = createWorld(1, 40);
    w.setCell(20, M.STONE);
    w.setDynamic(20, true);
    w.velocityY[20] = -3;
    createSolidMechanics(w).step();
    m.endRow = Array.from(w.grid).indexOf(M.STONE);
    m.velocityY = w.velocityY[m.endRow];
    assert(m.endRow < 20 && m.velocityY < 0);
});
check('C07', 'Liquid drag returns lost kinetic energy to heat', 'control', ['src/fluid-solver.ts:65-74'], m => {
    const w = createWorld(1, 1);
    w.setCell(0, M.WATER);
    w.velocityX[0] = 10;
    const before = total(w);
    createFluidSolver(w).step(0);
    m.energyChangeJ = total(w) - before;
    assert(near(total(w), before, 1e-10, 0));
});
check('C08', 'Freezing molten metal remains movable', 'control', ['src/world.ts:173-175'], m => {
    const w = createWorld(1, 4);
    w.setCell(1, M.MOLTEN_METAL);
    temp(w, 1, 22);
    w.applyPhase(1);
    m.material = M.materialsById[w.grid[1]].key;
    m.dynamic = w.dynamic[1];
    assert(w.grid[1] === M.METAL && w.dynamic[1] === 1);
});
check('C09', 'Painting is balanced by the external-source ledger', 'control', ['src/world.ts:190-225', 'src/diagnostics.ts:66-84'], m => {
    const w = createWorld(2, 2), b = measureWorld(w);
    w.setCell(0, M.WOOD);
    const r = compareConservation(b, measureWorld(w), true);
    Object.assign(m, r);
    assert(r.massWithinTolerance && r.energyWithinTolerance);
});
check('C10', 'Gas totals include steam', 'control', ['src/diagnostics.ts:37-41'], m => {
    const w = createWorld(1, 1);
    w.setCell(0, M.STEAM);
    m.actual = measureWorld(w).gasMassKg;
    m.expected = w.massKg[0];
    assert(near(m.actual, m.expected, 1e-18, 0));
});
check('C11', 'Subambient gas pressure reaches the liquid boundary', 'control', ['src/gas-dynamics.ts:125-140'], m => {
    const w = box(3, 4);
    temp(w, 4, -50);
    w.setCell(7, M.WATER);
    createGasDynamics(w);
    m.gasPressurePa = w.pressurePa[4];
    m.liquidPressurePa = w.pressurePa[7];
    m.expectedPa = m.gasPressurePa + M.materialsById[M.WATER].densityKgPerM3 * S.GRAVITY_M_PER_S2 * S.CELL_WIDTH_METERS;
    assert(near(m.liquidPressurePa, m.expectedPa, 1e-7, 0));
});
check('C12', 'Gameplay growth creates fuel and records external sources', 'control', ['src/reactions.ts:81-100'], m => {
    const w = createWorld(5, 5);
    w.setCell(17, M.PLANT);
    w.setCell(18, M.WATER);
    w.random.next = () => 0;
    const b = measureWorld(w), r = createReactions(w);
    r.beginStep();
    r.update(17);
    m.newMaterial = M.materialsById[w.grid[12]].key;
    m.chemicalKj = w.chemicalEnergyKj[12];
    Object.assign(m, compareConservation(b, measureWorld(w), true));
    assert(w.grid[12] === M.PLANT && m.chemicalKj > 0 && m.massWithinTolerance && m.energyWithinTolerance);
});
check('C13', 'Insulated diffusion conserves energy and does not overshoot', 'control', ['src/thermal.ts:309-321,350-389'], m => {
    const w = createWorld(2, 2);
    [22, 100, 60, 200].forEach((t, i) => { w.setCell(i, M.IRON); temp(w, i, t); });
    const b = sum(w.energy), s = createThermalSolver(2, 2);
    for (let t = 0; t < 200; t++)
        s.diffuse(w.grid, w.energy, w.pressurePa, w.massKg);
    m.energyChangeJ = sum(w.energy) - b;
    m.temperaturesC = Array.from(w.grid, (_, i) => w.temperatureAt(i));
    assert(near(sum(w.energy), b, 1e-8, 0) && m.temperaturesC.every(t => t >= 22 - 1e-8 && t <= 200 + 1e-8));
});
for (const [id, ratio] of [['C14', 1], ['C15', .2]])
    check(id, 'Hydrogen/oxygen reaction conserves mass, momentum and tracked energy; oxygen scaling ' + ratio, 'control', ['src/element-reactions.ts:26-68'], m => {
        const w = createWorld(1, 2);
        w.setCell(0, M.HYDROGEN);
        w.setCell(1, M.OXYGEN);
        w.massKg[1] *= ratio;
        w.oxygenKg[1] *= ratio;
        w.volumeM3[1] *= ratio;
        temp(w, 0, 500);
        temp(w, 1, 22);
        w.velocityX[0] = 2;
        w.velocityY[1] = -1;
        const b = measureWorld(w), bx = px(w), by = py(w);
        m.reactions = createElementReactions(w).step();
        m.finalMaterials = Array.from(w.grid, id => M.materialsById[id].key);
        m.momentumXChange = px(w) - bx;
        m.momentumYChange = py(w) - by;
        Object.assign(m, compareConservation(b, measureWorld(w)));
        assert(m.reactions === 1 && m.massWithinTolerance && m.energyWithinTolerance && near(px(w), bx, 1e-17, 0) && near(py(w), by, 1e-17, 0));
    });
check('C16', 'Equal-volume acid/base pair conserves tracked energy', 'control', ['src/neutralization.ts:37-54'], m => {
    const w = createWorld(2, 1);
    w.setCell(0, M.HYDROCHLORIC_ACID);
    w.setCell(1, M.SODIUM_HYDROXIDE);
    const b = measureWorld(w);
    m.releasedJ = createNeutralization(w).step();
    Object.assign(m, compareConservation(b, measureWorld(w)));
    assert(near(m.releasedJ, 57.2) && m.massWithinTolerance && m.energyWithinTolerance);
});
check('C17', 'Short uniform DC circuit reaches its analytic voltages', 'control', ['src/electricity.ts:55-77'], m => {
    const w = createWorld(5, 1);
    [M.BATTERY, M.WIRE, M.WIRE, M.WIRE, M.GROUND].forEach((mat, i) => w.setCell(i, mat));
    const e = createElectricity(w);
    m.deliveredJ = e.step();
    m.voltageV = [0, 1, 2, 3, 4].map(i => e.voltageAt(i));
    assert(m.voltageV.every((v, i) => near(v, 12 - 3 * i, 1e-6, 0)));
});
check('C18', 'Electrical heat equals total battery depletion', 'control', ['src/electricity.ts:98-119'], m => {
    const w = createWorld(3, 1);
    [M.BATTERY, M.LAMP, M.GROUND].forEach((mat, i) => w.setCell(i, mat));
    const b = measureWorld(w);
    m.deliveredJ = createElectricity(w).step();
    Object.assign(m, compareConservation(b, measureWorld(w)));
    assert(m.massWithinTolerance && m.energyWithinTolerance);
});
check('C19', 'Uniform ambient full simulation remains stationary', 'control', ['src/physics.ts:29-70'], m => {
    const w = box(8, 6), b = measureWorld(w), p = createPhysics(w);
    for (let t = 0; t < 60; t++)
        p.step();
    Object.assign(m, compareConservation(b, measureWorld(w)));
    m.maxSpeed = measureWorld(w).maximumSpeedMPerS;
    assert(m.energyWithinTolerance && m.massWithinTolerance && m.maxSpeed < 1e-8);
});
check('C20', 'Thermal round trips work at spawn temperature for every catalogue material', 'control', ['src/thermal.ts:46-156'], m => {
    m.checked = 0;
    m.maxErrorC = 0;
    for (const mat of M.materialsById) {
        const mass = mat.densityKgPerM3 * S.CELL_VOLUME_M3;
        const e = energyAtTemperature(mat.id, mat.spawnTemperatureC, S.AMBIENT_PRESSURE_PA, mass);
        const t = temperatureFromEnergy(mat.id, e, S.AMBIENT_PRESSURE_PA, mass);
        m.maxErrorC = Math.max(m.maxErrorC, Math.abs(t - mat.spawnTemperatureC));
        m.checked++;
    }
    assert(m.maxErrorC < 1e-8);
});
const report = { archive: 'src(1).zip', sha256: '89bf7a00ab52f86c360ec32d0c83570078e4c6f5c68f552310de34b95275646a', node: process.version, scope: 'Unmodified TypeScript compiled to CommonJS; focused subsystem/invariant tests, not a browser or GPU benchmark. Some failures describe a deliberately simplified model\'s contract rather than a local coding error.', results };
const out = path.resolve(process.argv[3] || 'audit-results.json');
fs.writeFileSync(out, JSON.stringify(report, null, 2) + '\n');
const failed = results.filter(r => !r.passed).length;
console.log(`\n${results.length} checks: ${results.length - failed} pass, ${failed} fail.`);
process.exitCode = failed ? 1 : 0;
