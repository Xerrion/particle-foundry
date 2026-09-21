import { describe, expect, test } from "bun:test";
import { EMPTY, LAVA, STEAM, STONE, WATER } from "../../src/materials";
import { materialsById } from "../../src/materials/physical-properties";
import { createFluidSolver } from "../../src/physics/fluid-solver";
import { createGasDynamics } from "../../src/physics/gas-dynamics";
import { createSolidMechanics } from "../../src/physics/solid-mechanics";
import { energyAtTemperature } from "../../src/physics/thermal";
import { compareConservation, measureWorld } from "../../src/simulation/diagnostics";
import {
	AMBIENT_PRESSURE_PA,
	CELL_VOLUME_M3,
	FIXED_TIME_STEP_SECONDS,
	IDEAL_GAS_CONSTANT,
} from "../../src/simulation/physical-scale";
import { createPhysics } from "../../src/simulation/physics";
import { createSeededRandom } from "../../src/simulation/random";
import { createWorld } from "../../src/simulation/world";

describe("physical state ownership and diagnostics", () => {
	test("seeded streams replay exactly and reset independently", () => {
		const first = createSeededRandom(42);
		const second = createSeededRandom(42);
		const values = Array.from({ length: 16 }, () => first.next());
		expect(Array.from({ length: 16 }, () => second.next())).toEqual(values);
		first.reset();
		expect(Array.from({ length: 16 }, () => first.next())).toEqual(values);
	});

	test("mass, volume, momentum and chemistry move atomically", () => {
		const world = createWorld(2, 1, { seed: 7 });
		world.setCell(0, WATER);
		world.velocityX[0] = 3;
		world.velocityY[0] = -2;
		world.chemicalEnergyKj[0] = 9;
		const parcel = {
			mass: world.massKg[0],
			volume: world.volumeM3[0],
			vx: world.velocityX[0],
			vy: world.velocityY[0],
			chemical: world.chemicalEnergyKj[0],
		};
		world.swap(0, 1);
		expect({
			mass: world.massKg[1],
			volume: world.volumeM3[1],
			vx: world.velocityX[1],
			vy: world.velocityY[1],
			chemical: world.chemicalEnergyKj[1],
		}).toEqual(parcel);
	});

	test("a closed stationary scene reports zero conservation drift", () => {
		const world = createWorld(3, 2);
		world.setCell(4, STONE);
		const before = measureWorld(world);
		const after = measureWorld(world);
		expect(compareConservation(before, after)).toEqual({
			massDriftKg: 0,
			energyDriftKj: 0,
			massWithinTolerance: true,
			energyWithinTolerance: true,
		});
	});

	test("diagnostics detect actual mass and energy changes", () => {
		const world = createWorld(2, 1);
		const before = measureWorld(world);
		world.setCell(0, WATER);
		world.addExternalEnergy(1, 100);
		const after = measureWorld(world);
		const report = compareConservation(before, after);
		expect(report.massDriftKg).toBeCloseTo(world.ledger.massAddedKg, 12);
		expect(report.energyDriftKj).toBeGreaterThan(0);
		expect(report.massWithinTolerance).toBe(false);
		expect(report.energyWithinTolerance).toBe(false);
		expect(compareConservation(before, after, true).energyWithinTolerance).toBe(true);
	});
});

describe("coupled solvers", () => {
	test("derived fluid columns use authoritative parcel volume", () => {
		const world = createWorld(2, 3);
		world.setCell(0, WATER);
		world.setCell(2, WATER);
		world.setCell(5, LAVA);
		const solver = createFluidSolver(world);
		expect(solver.columns.volumeM3[0]).toBeCloseTo(2 * CELL_VOLUME_M3, 15);
		expect(solver.columns.volumeM3[1]).toBeCloseTo(CELL_VOLUME_M3, 15);
		expect(solver.columns.surfaceHeightM[0]).toBeCloseTo(0.03, 12);
	});

	test("higher viscosity damps parcel momentum more strongly", () => {
		const water = createWorld(1, 1);
		water.setCell(0, WATER);
		water.velocityX[0] = 10;
		createFluidSolver(water).step(0);
		const lava = createWorld(1, 1);
		lava.setCell(0, LAVA);
		lava.velocityX[0] = 10;
		createFluidSolver(lava).step(0);
		expect(lava.velocityX[0]).toBeLessThan(water.velocityX[0]);
	});

	test("ideal-gas state gives ambient air a defined pressure", () => {
		const world = createWorld(2, 1);
		const gas = createGasDynamics(world);
		gas.step();
		expect(world.grid).toEqual(new Uint8Array([EMPTY, EMPTY]));
		expect(world.pressurePa[0]).toBe(AMBIENT_PRESSURE_PA);
		expect(world.pressurePa[1]).toBe(AMBIENT_PRESSURE_PA);
	});

	test("heating a fixed amount of enclosed gas raises ideal-gas pressure", () => {
		const world = createWorld(1, 1);
		world.setCell(0, STEAM);
		const gas = createGasDynamics(world);
		gas.derivePressure();
		const coldPressure = world.pressurePa[0];
		world.energy[0] += 200;
		gas.derivePressure();
		expect(world.pressurePa[0]).toBeGreaterThan(coldPressure);
	});

	test("water in a sealed hot vessel can keep heating above 100 C", () => {
		const world = createWorld(5, 5);
		for (let y = 0; y < 5; y += 1) {
			for (let x = 0; x < 5; x += 1) {
				const index = x + y * 5;
				if (x === 0 || x === 4 || y === 0 || y === 4) world.setCell(index, STONE);
				else world.energy[index] = energyAtTemperature(EMPTY, 500, undefined, world.massKg[index]);
			}
		}
		const water = 2 + 3 * 5;
		world.setCell(water, WATER);
		const physics = createPhysics(world);
		expect(world.pressurePa[water]).toBeGreaterThan(AMBIENT_PRESSURE_PA * 2);
		world.energy[water] = energyAtTemperature(
			WATER,
			110,
			world.pressurePa[water],
			world.massKg[water],
		);
		expect(world.temperatureAt(water)).toBeCloseTo(110, 8);
		physics.step();
		expect(world.grid[water]).toBe(WATER);
		expect(world.temperatureAt(water)).toBeGreaterThan(100);
	});

	test("enclosed steam follows the ideal gas equation for mass, volume and temperature", () => {
		const world = createWorld(1, 1);
		world.setCell(0, STEAM);
		const gas = createGasDynamics(world);
		const expectedPressure = () =>
			((world.massKg[0] / materialsById[STEAM].molarMassKgPerMol) *
				IDEAL_GAS_CONSTANT *
				(world.temperatureAt(0) + 273.15)) /
			world.volumeM3[0];
		expect(world.pressurePa[0]).toBeCloseTo(expectedPressure(), 5);
		const initialPressure = world.pressurePa[0];
		world.massKg[0] *= 2;
		world.energy[0] *= 2;
		gas.derivePressure();
		expect(world.pressurePa[0]).toBeCloseTo(expectedPressure(), 5);
		expect(world.pressurePa[0]).toBeGreaterThan(initialPressure);
		world.volumeM3[0] *= 2;
		gas.derivePressure();
		expect(world.pressurePa[0]).toBeCloseTo(expectedPressure(), 5);
	});

	test("pressure work exchanges stored energy for motion without creating energy", () => {
		const world = createWorld(40, 1);
		for (let index = 0; index < world.size; index += 2) {
			world.setCell(index, STEAM);
			world.volumeM3[index] = 1e-8;
		}
		const before = measureWorld(world);
		createGasDynamics(world).step();
		const after = measureWorld(world);
		const report = compareConservation(before, after);
		expect(after.kineticEnergyKj).toBeGreaterThan(before.kineticEnergyKj);
		expect(after.thermalEnergy).toBeLessThan(before.thermalEnergy);
		expect(Math.abs(report.energyDriftKj)).toBeLessThan(1e-10);
	});

	test("an extreme pressure gradient has a bounded impulse per tick", () => {
		const world = createWorld(2, 1);
		world.setCell(0, STEAM);
		world.volumeM3[0] = 1e-9;
		createGasDynamics(world).step();
		const speedLimit = 3 * FIXED_TIME_STEP_SECONDS;
		expect(world.velocityX[0]).toBeGreaterThanOrEqual(-speedLimit);
		expect(world.velocityX[1]).toBeLessThanOrEqual(speedLimit);
		expect(world.velocityX[1]).toBeGreaterThan(0);
	});

	test("liquid pressure increases with depth and a horizontal gradient drives velocity", () => {
		const world = createWorld(2, 2);
		for (const index of [0, 2, 3]) world.setCell(index, WATER);
		const gas = createGasDynamics(world);
		expect(world.pressurePa[2]).toBeGreaterThan(world.pressurePa[0]);
		expect(world.pressurePa[2]).toBeGreaterThan(world.pressurePa[3]);
		gas.step();
		expect(world.velocityX[2]).toBeLessThan(0);
		expect(world.velocityX[3]).toBeGreaterThan(0);
	});

	test("a solid wall blocks pressure impulse between gas cells", () => {
		const world = createWorld(3, 1);
		world.setCell(0, STEAM);
		world.setCell(1, STONE);
		const gas = createGasDynamics(world);
		gas.step();
		expect(world.velocityX[0]).toBe(0);
		expect(world.velocityX[2]).toBe(0);
	});

	test("only explicitly dynamic solids fall and dissipate impact energy", () => {
		const world = createWorld(1, 3);
		world.setCell(0, STONE);
		world.setCell(2, STONE);
		const solver = createSolidMechanics(world);
		solver.step();
		expect(world.grid[0]).toBe(STONE);
		world.setDynamic(0, true);
		for (let tick = 0; tick < 3; tick++) solver.step();
		expect(world.grid[1]).toBe(STONE);
		expect(world.dynamic[1]).toBe(1);
		solver.step();
		expect(world.grid[1]).toBe(STONE);
		expect(world.velocityY[1]).toBe(0);
	});

	test("a dynamic solid converts its impact kinetic energy to heat", () => {
		const world = createWorld(1, 2);
		world.setCell(0, STONE);
		world.setCell(1, STONE);
		world.setDynamic(0, true);
		world.velocityY[0] = 3;
		const initialHeat = world.energy[0];
		const impactSpeed = 3; // Existing contact must not add gravity before impact.
		const expectedHeat = 0.5 * world.massKg[0] * impactSpeed ** 2;
		createSolidMechanics(world).step();
		expect(world.grid[0]).toBe(STONE);
		expect(world.velocityY[0]).toBe(0);
		expect(world.energy[0] - initialHeat).toBeCloseTo(expectedHeat, 10);
	});
});
