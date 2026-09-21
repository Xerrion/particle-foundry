import { describe, expect, test } from "bun:test";
import { EMPTY, OIL, STONE, WATER } from "../../src/materials";
import { createHydrostatics } from "../../src/physics/hydrostatics";
import { measureWorld } from "../../src/simulation/diagnostics";
import { createWorld, type World } from "../../src/simulation/world";

function vessel(material = WATER): World {
	const world = createWorld(7, 8);
	for (let i = 0; i < world.size; i++) world.setCell(i, STONE);
	for (const x of [1, 5]) {
		for (let y = 0; y < 7; y++) world.setCell(x + y * 7, EMPTY);
	}
	for (let x = 1; x <= 5; x++) world.setCell(x + 6 * 7, material);
	for (let y = 1; y <= 5; y++) world.setCell(1 + y * 7, material);
	world.setCell(5 + 5 * 7, material);
	return world;
}

function sum(world: World): number {
	return measureWorld(world).totalTrackedEnergyKj * 1000;
}

describe("quasi-static hydrostatic relaxation", () => {
	test("equalizes a connected U-tube without crossing its solid partition", () => {
		const world = vessel();
		const solver = createHydrostatics(world);
		const initialEnergy = sum(world);
		const count = world.getParticleCount();
		const walls = Array.from(world.grid, (material, index) =>
			material === STONE ? index : -1,
		).filter((index) => index >= 0);
		for (let tick = 0; tick < 20; tick++) {
			world.moved.fill(0);
			solver.step(tick);
		}
		for (const x of [1, 5]) {
			for (let y = 0; y < 7; y++) expect(world.grid[x + y * 7]).toBe(y < 3 ? EMPTY : WATER);
		}
		for (const index of walls) expect(world.grid[index]).toBe(STONE);
		expect(world.getParticleCount()).toBe(count);
		expect(sum(world)).toBeCloseTo(initialEnergy, 9);
	});

	test("disconnected vessels do not exchange fluid through a wall", () => {
		const world = vessel();
		world.setCell(3 + 6 * 7, STONE);
		const before = world.grid.slice();
		const solver = createHydrostatics(world);
		for (let tick = 0; tick < 10; tick++) {
			world.moved.fill(0);
			solver.step(tick);
		}
		expect(world.grid).toEqual(before);
	});

	test("equilibrium is stable without random surface agitation", () => {
		const world = vessel();
		const solver = createHydrostatics(world);
		for (let tick = 0; tick < 4; tick++) {
			world.moved.fill(0);
			solver.step(tick);
		}
		const before = world.grid.slice();
		const energy = world.energy.slice();
		for (let tick = 4; tick < 100; tick++) {
			world.moved.fill(0);
			solver.step(tick);
		}
		expect(world.grid).toEqual(before);
		expect(world.energy).toEqual(energy);
	});

	test("routes displaced gas back through adjacent cells rather than teleporting heat", () => {
		const world = vessel();
		for (let i = 0; i < world.size; i++) world.energy[i] = i + 0.25;
		const before = sum(world);
		const marker = world.variation.slice();
		createHydrostatics(world).step(0);
		expect(sum(world)).toBeCloseTo(before, 9);
		expect(Array.from(world.variation).sort()).toEqual(Array.from(marker).sort());
		expect(world.energy[1 + 1 * 7]).toBeCloseTo(5 + 4 * 7 + 0.25, 3);
		expect(world.grid[1 + 1 * 7]).toBe(EMPTY);
		expect(world.grid[5 + 4 * 7]).toBe(WATER);
	});

	test("material-dependent cadence limits head relaxation", () => {
		const world = vessel(OIL);
		const before = world.grid.slice();
		const solver = createHydrostatics(world);
		solver.step(1);
		expect(world.grid).toEqual(before);
		solver.step(2);
		expect(world.grid).not.toEqual(before);
	});

	test("lighter liquid is displaced at the endpoint, never used as an interior flow path", () => {
		const world = vessel();
		world.setCell(3 + 6 * 7, OIL);
		const before = world.grid.slice();
		const energy = sum(world);
		createHydrostatics(world).step(0);
		// The left arm can push oil up to its surface, but cannot send water
		// through the oil to the disconnected right arm in the same transfer.
		expect(world.grid[3 + 6 * 7]).toBe(WATER);
		expect(world.grid[1 + 1 * 7]).toBe(OIL);
		for (let y = 0; y < 7; y++) expect(world.grid[5 + y * 7]).toBe(before[5 + y * 7]);
		expect(world.grid.filter((material) => material === OIL)).toHaveLength(1);
		expect(sum(world)).toBeCloseTo(energy, 9);
	});

	test("single cells, full worlds and edges conserve all state", () => {
		for (const [width, height] of [
			[1, 1],
			[1, 8],
			[8, 1],
			[8, 8],
		]) {
			const world = createWorld(width, height);
			for (let i = 0; i < world.size; i++) world.setCell(i, WATER);
			const before = world.grid.slice();
			const energy = sum(world);
			createHydrostatics(world).step(0);
			expect(world.grid).toEqual(before);
			expect(sum(world)).toBe(energy);
		}
	});
});
