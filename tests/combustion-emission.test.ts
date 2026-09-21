import { describe, expect, test } from "bun:test";
import { combustionProfile, EMPTY, FIRE, OIL, PLANT, WOOD } from "../src/materials";
import { createReactions } from "../src/reactions";
import { energyAtTemperature } from "../src/thermal";
import { createWorld } from "../src/world";
import { burningOilPool, longestElevatedRun } from "./fixtures/burning-oil-pool";

describe("asynchronous combustion emission", () => {
	test("a deferred emission keeps its heat and still consumes only the released chemical energy", () => {
		const world = createWorld(1, 3);
		world.random.next = () => 0.99;
		world.setCell(2, OIL);
		world.energy[2] = energyAtTemperature(OIL, 800);
		const thermal = world.energy[2];
		const chemical = world.chemicalEnergyKj[2];
		const airEnergy = world.energy[1];
		const reactions = createReactions(world);
		reactions.beginStep();
		reactions.update(2);
		expect(world.grid[1]).toBe(EMPTY);
		expect(world.energy[1]).toBe(airEnergy);
		expect(world.burning[2]).toBe(1);
		expect(world.energy[2]).toBeCloseTo(thermal + combustionProfile.heatPerTick, 9);
		expect(world.chemicalEnergyKj[2]).toBeCloseTo(
			chemical - combustionProfile.heatPerTick / 1000,
			12,
		);
		// A later accepted release spends stored heat; deferral is not a
		// deletion, a hidden cooling term, or a fresh external heat source.
		world.random.next = () => 0;
		world.moved.fill(0);
		reactions.beginStep();
		reactions.update(2);
		expect(world.grid[1]).toBe(FIRE);
		expect(world.energy[1] + world.energy[2] + world.chemicalEnergyKj[2] * 1000).toBeCloseTo(
			airEnergy + thermal + chemical * 1000,
			8,
		);
	});

	test.each([OIL, WOOD, PLANT])(
		"equally hot fuel %i emits individual flames, not an entire sheet in one tick",
		(material) => {
			const world = createWorld(48, 3, { seed: 42 });
			for (let x = 0; x < 48; x++) {
				world.setCell(x + 96, material);
				world.energy[x + 96] = energyAtTemperature(material, 800);
			}
			const totalEnergy = () =>
				world.energy.reduce((a, b) => a + b, 0) +
				world.chemicalEnergyKj.reduce((a, b) => a + b, 0) * 1000;
			const before = totalEnergy();
			const mass = world.massKg.reduce((a, b) => a + b, 0);
			const fuel = world.chemicalEnergyKj.slice();
			const reactions = createReactions(world);
			reactions.beginStep();
			for (let i = 0; i < world.size; i++) reactions.update(i);
			const flames = world.grid.filter((id) => id === FIRE).length;
			expect(flames).toBeGreaterThan(0);
			expect(flames).toBeLessThan(48);
			for (let x = 0; x < 48; x++) {
				expect(world.burning[x + 96]).toBe(1);
				expect(world.chemicalEnergyKj[x + 96]).toBeLessThan(fuel[x + 96]);
			}
			expect(totalEnergy()).toBeCloseTo(before, 7);
			expect(world.massKg.reduce((a, b) => a + b, 0)).toBeCloseTo(mass, 12);
		},
	);

	for (const temperature of [400, 800, 1900]) {
		test.each([1, 42, 123])(
			`a wide oil fire at ${temperature} C does not launch detached horizontal sheets (seed %i)`,
			(seed) => {
				const scene = burningOilPool(seed, temperature);
				const { world, physics, surfaceY, firstX, lastX } = scene;
				let emittingFrames = 0;
				for (let tick = 0; tick < 240; tick++) {
					physics.step();
					const run = longestElevatedRun(world, FIRE, surfaceY);
					if (run > 0) emittingFrames++;
					// Ignore the actual burning oil surface. Above it, reject the
					// large detached bars visible in the reported screenshots.
					expect(run).toBeLessThanOrEqual((lastX - firstX + 1) / 2);
				}
				expect(emittingFrames).toBeGreaterThan(200);
			},
		);
	}

	test("emission replay is independent of cosmetic random draws", () => {
		const first = burningOilPool(42);
		const second = burningOilPool(42);
		for (let tick = 0; tick < 120; tick++) {
			first.physics.step();
			for (let i = 0; i < 10; i++) second.world.visualRandom.next();
			second.physics.step();
		}
		expect(second.world.grid).toEqual(first.world.grid);
		expect(second.world.energy).toEqual(first.world.energy);
		expect(second.world.lifetime).toEqual(first.world.lifetime);
		expect(second.world.chemicalEnergyKj).toEqual(first.world.chemicalEnergyKj);
	});
});
