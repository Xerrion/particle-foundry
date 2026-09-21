import { describe, expect, test } from "bun:test";
import { EMPTY, FIRE, materialsById, SMOKE, STEAM, STONE, WOOD } from "../../src/materials";
import { createMotion } from "../../src/physics/motion";
import { energyAtTemperature } from "../../src/physics/thermal";
import { measureWorld } from "../../src/simulation/diagnostics";
import { createPhysics } from "../../src/simulation/physics";
import { createWorld } from "../../src/simulation/world";

describe("gas plume transport regressions", () => {
	test("freshly emitted flame can move in the same tick instead of pinning every second row", () => {
		const world = createWorld(1, 12);
		// Force an emission: this test checks advection, not emission frequency.
		world.random.next = () => 0;
		world.setCell(11, WOOD);
		world.energy[11] = energyAtTemperature(WOOD, 1000, undefined, world.massKg[11]);
		createPhysics(world).step();
		expect(world.grid[11]).toBe(WOOD);
		expect(world.burning[11]).toBe(1);
		expect(world.grid[10]).toBe(EMPTY);
		expect(world.grid[9]).toBe(FIRE);
	});

	test("smoke produced by quenching joins the plume immediately", () => {
		const world = createWorld(1, 5);
		world.setCell(4, FIRE);
		world.energy[4] = energyAtTemperature(FIRE, 100, undefined, world.massKg[4]);
		const energy = measureWorld(world).totalTrackedEnergyKj * 1000;
		const mass = world.massKg.reduce((a, b) => a + b, 0);
		createPhysics(world).step();
		expect(world.grid[4]).toBe(EMPTY);
		expect(world.grid[3]).toBe(SMOKE);
		expect(measureWorld(world).totalTrackedEnergyKj * 1000).toBeCloseTo(energy, 9);
		expect(world.massKg.reduce((a, b) => a + b, 0)).toBeCloseTo(mass, 12);
	});

	test.each([1, 7, 42, 123, 20260920])(
		"a smoke plume stays connected while spreading beneath a ceiling (seed %i)",
		(seed) => {
			const world = createWorld(32, 22, { seed });
			for (let x = 5; x < 27; x++) world.setCell(x + 4 * 32, STONE);
			for (let y = 10; y < 19; y++) for (let x = 11; x < 21; x++) world.setCell(x + y * 32, SMOKE);
			for (let i = 0; i < world.size; i++)
				world.energy[i] = energyAtTemperature(world.grid[i], 180, undefined, world.massKg[i]);
			const physics = createPhysics(world);
			for (let tick = 0; tick < 20; tick++) physics.step();
			let adjacentPairs = 0;
			for (let y = 0; y < 22; y++)
				for (let x = 0; x < 31; x++) {
					if (world.grid[x + y * 32] === SMOKE && world.grid[x + 1 + y * 32] === SMOKE)
						adjacentPairs++;
				}
			// A lattice of alternating gas/air breaks almost every horizontal
			// pair. Allow irregular edges, but keep most of this dense puff joined.
			expect(adjacentPairs).toBeGreaterThanOrEqual(60);
			expect(world.grid.filter((id) => id === SMOKE)).toHaveLength(90);
			for (let x = 5; x < 27; x++) expect(world.grid[x + 4 * 32]).toBe(STONE);
		},
	);

	test.each([FIRE, SMOKE, STEAM])(
		"gas %i follows air vacated by a diagonal move without leaving striped gaps",
		(material) => {
			const world = createWorld(3, 4);
			world.setCell(1, STONE);
			world.setCell(4, material);
			world.setCell(7, material);
			const energy = measureWorld(world).totalTrackedEnergyKj * 1000;
			const mass = world.massKg.reduce((a, b) => a + b, 0);
			const motion = createMotion(world);
			motion.update(4, 0);
			expect(world.grid[4]).toBe(EMPTY);
			expect(world.moved[4]).toBe(0);
			motion.update(7, 0);
			expect(world.grid[4]).toBe(material);
			expect(world.grid[7]).toBe(EMPTY);
			expect(world.grid[1]).toBe(STONE);
			expect(world.grid.filter((id) => id === material)).toHaveLength(2);
			expect(measureWorld(world).totalTrackedEnergyKj * 1000).toBeCloseTo(energy, 9);
			expect(world.massKg.reduce((a, b) => a + b, 0)).toBeCloseTo(mass, 12);
		},
	);

	test("a rising fire column can displace smoke already pushed down by its leading parcel", () => {
		const world = createWorld(1, 5);
		world.setCell(1, SMOKE);
		world.setCell(2, FIRE);
		world.setCell(3, FIRE);
		const mass = world.massKg.reduce((a, b) => a + b, 0);
		const energy = measureWorld(world).totalTrackedEnergyKj * 1000;
		const motion = createMotion(world);
		motion.update(2, 0);
		motion.update(3, 0);
		expect(Array.from(world.grid)).toEqual([FIRE, FIRE, EMPTY, SMOKE, EMPTY]);
		expect(measureWorld(world).totalTrackedEnergyKj * 1000).toBeCloseTo(energy, 9);
		expect(world.massKg.reduce((a, b) => a + b, 0)).toBeCloseTo(mass, 12);
	});

	test.each([FIRE, SMOKE, STEAM])(
		"gas %i spreads irregularly under a roof instead of marching on every tick",
		(material) => {
			const world = createWorld(201, 2, { seed: 42 });
			for (let x = 0; x < 201; x++) world.setCell(x, STONE);
			let index = 301;
			world.setCell(index, material);
			const motion = createMotion(world);
			let restingTicks = 0;
			let movingTicks = 0;
			for (let tick = 0; tick < 80; tick++) {
				world.moved.fill(0);
				motion.update(index, tick);
				const next = world.grid.indexOf(material);
				if (next === index) restingTicks++;
				else movingTicks++;
				expect(Math.abs(next - index)).toBeLessThanOrEqual(1);
				expect(next).toBeGreaterThanOrEqual(201);
				index = next;
			}
			expect(restingTicks).toBeGreaterThan(0);
			expect(movingTicks).toBeGreaterThan(0);
		},
	);

	test("smoke diffusion replays for the same seed and differs across seeds", () => {
		const trajectory = (seed: number) => {
			const world = createWorld(81, 2, { seed });
			for (let x = 0; x < 81; x++) world.setCell(x, STONE);
			world.setCell(121, SMOKE);
			const motion = createMotion(world);
			const positions: number[] = [];
			for (let tick = 0; tick < 30; tick++) {
				world.moved.fill(0);
				motion.update(world.grid.indexOf(SMOKE), tick);
				positions.push(world.grid.indexOf(SMOKE));
			}
			return positions;
		};
		expect(trajectory(7)).toEqual(trajectory(7));
		expect(trajectory(7)).not.toEqual(trajectory(123));
	});

	test.each([FIRE, SMOKE, STEAM])(
		"ascending gas %i can drift without waiting for a roof or exceeding its rise budget",
		(material) => {
			const world = createWorld(31, 100, { seed: 42 });
			let index = 15 + 95 * 31;
			world.setCell(index, material);
			const motion = createMotion(world);
			let driftSteps = 0;
			for (let tick = 0; tick < 20; tick++) {
				world.moved.fill(0);
				motion.update(index, tick);
				const next = world.grid.indexOf(material);
				const horizontal = Math.abs((next % 31) - (index % 31));
				const vertical = Math.floor(index / 31) - Math.floor(next / 31);
				if (horizontal > 0) driftSteps++;
				expect(horizontal).toBeLessThanOrEqual(1);
				expect(vertical).toBeGreaterThanOrEqual(1);
				expect(vertical).toBeLessThanOrEqual(materialsById[material].gasMotion?.hotRise ?? 1);
				// A parcel is never actively advected twice in one tick.
				motion.update(next, tick);
				expect(world.grid.indexOf(material)).toBe(next);
				index = next;
			}
			expect(driftSteps).toBeGreaterThan(0);
		},
	);

	test.each([FIRE, SMOKE, STEAM])("gas %i still respects sealed diagonal corners", (material) => {
		const world = createWorld(3, 3);
		for (const index of [1, 3, 5, 7]) world.setCell(index, STONE);
		world.setCell(4, material);
		const before = world.grid.slice();
		const motion = createMotion(world);
		for (let tick = 0; tick < 60; tick++) {
			world.moved.fill(0);
			motion.update(4, tick);
			expect(world.grid).toEqual(before);
		}
	});
});
