import { describe, expect, test } from "bun:test";
import { compareConservation, measureWorld } from "../src/diagnostics";
import { createFluidSolver } from "../src/fluid-solver";
import { createGasDynamics } from "../src/gas-dynamics";
import { EMPTY, LAVA, STEAM, STONE, WATER } from "../src/materials";
import { AMBIENT_PRESSURE_PA, CELL_VOLUME_M3 } from "../src/physical-scale";
import { createSeededRandom } from "../src/random";
import { createSolidMechanics } from "../src/solid-mechanics";
import { createWorld } from "../src/world";

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

	test("only explicitly dynamic solids fall and dissipate impact energy", () => {
		const world = createWorld(1, 3);
		world.setCell(0, STONE);
		world.setCell(2, STONE);
		const solver = createSolidMechanics(world);
		solver.step();
		expect(world.grid[0]).toBe(STONE);
		world.setDynamic(0, true);
		solver.step();
		expect(world.grid[1]).toBe(STONE);
		expect(world.dynamic[1]).toBe(1);
		solver.step();
		expect(world.grid[1]).toBe(STONE);
		expect(world.velocityY[1]).toBe(0);
	});
});
