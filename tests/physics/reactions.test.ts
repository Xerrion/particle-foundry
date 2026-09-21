import { describe, expect, test } from "bun:test";
import { EMPTY, FIRE, SMOKE, STONE, WOOD } from "../../src/materials";
import { createReactions } from "../../src/physics/reactions";
import { energyAtTemperature } from "../../src/physics/thermal";
import { measureWorld } from "../../src/simulation/diagnostics";
import { createWorld } from "../../src/simulation/world";

describe("bounded combustion", () => {
	test("new flame is not aged or reacted twice before the separate transport pass", () => {
		const world = createWorld(3, 3);
		// Force an emission so the duplicate-reaction guard is exercised.
		world.random.next = () => 0;
		world.setCell(1, STONE);
		world.setCell(3, STONE);
		world.setCell(4, WOOD);
		world.energy[4] = energyAtTemperature(WOOD, 1000, undefined, world.massKg[4]);
		const reactions = createReactions(world);
		reactions.beginStep();
		reactions.update(4);
		expect(world.grid[5]).toBe(FIRE);
		const lifetime = world.lifetime[5];
		const energy = world.energy[5];
		reactions.update(5);
		expect(world.lifetime[5]).toBe(lifetime);
		expect(world.energy[5]).toBe(energy);
	});

	test("converts stored chemical energy to heat with a replayable seed", () => {
		const first = createWorld(1, 1, { seed: 17 });
		const second = createWorld(1, 1, { seed: 17 });
		for (const world of [first, second]) {
			world.setCell(0, WOOD);
			world.energy[0] = energyAtTemperature(WOOD, 400, undefined, world.massKg[0]);
			createReactions(world).update(0);
		}
		expect(first.grid).toEqual(second.grid);
		expect(first.lifetime).toEqual(second.lifetime);
		expect(first.energy).toEqual(second.energy);
		expect(first.chemicalEnergyKj).toEqual(second.chemicalEnergyKj);
		expect(first.grid[0]).toBe(WOOD);
		expect(first.burning[0]).toBe(1);
		expect(first.chemicalEnergyKj[0]).toBeLessThan(11.2);
	});

	test("a sealed fire with no fuel or oxygen becomes smoke", () => {
		const world = createWorld(3, 3);
		for (let index = 0; index < world.size; index += 1) world.setCell(index, WOOD);
		world.changeMaterial(4, FIRE);
		world.chemicalEnergyKj[4] = 0;
		createReactions(world).update(4);
		expect(world.grid[4]).toBe(SMOKE);
	});

	test("a painted flame persists as cooling hot gas without inventing chemical heat", () => {
		const world = createWorld(1, 1, { seed: 31 });
		world.setCell(0, FIRE);
		const energy = world.energy[0];
		const lifetime = world.lifetime[0];
		createReactions(world).update(0);
		expect(world.grid[0]).toBe(FIRE);
		expect(world.energy[0]).toBe(energy);
		expect(world.lifetime[0]).toBe(lifetime - 1);
	});

	test("flame contact preheats fuel with equal and opposite energy transfer", () => {
		const world = createWorld(2, 1);
		world.setCell(0, FIRE);
		world.setCell(1, WOOD);
		world.energy[0] = energyAtTemperature(FIRE, 400, undefined, world.massKg[0]);
		const total = world.energy[0] + world.energy[1];
		const woodEnergy = world.energy[1];
		createReactions(world).update(1);
		expect(world.grid[1]).toBe(WOOD);
		expect(world.energy[1]).toBeGreaterThan(woodEnergy);
		expect(world.energy[0] + world.energy[1]).toBe(total);
	});

	test("smoke expiry is recorded as an open-boundary mass loss", () => {
		const world = createWorld(1, 1);
		world.setCell(0, SMOKE);
		world.lifetime[0] = 1;
		const energy = world.energy[0];
		const before = measureWorld(world);
		const ledgerBefore = { ...world.ledger };
		createReactions(world).update(0);
		const after = measureWorld(world);
		expect(world.grid[0]).toBe(EMPTY);
		expect(world.energy[0]).toBe(energy);
		expect(world.ledger.massRemovedKg).toBeGreaterThan(0);
		expect(
			world.ledger.massAddedKg -
				ledgerBefore.massAddedKg -
				(world.ledger.massRemovedKg - ledgerBefore.massRemovedKg),
		).toBeCloseTo((after.totalMassKg ?? 0) - (before.totalMassKg ?? 0), 12);
	});

	test("dense smoke in an open room vents instead of becoming heavy ambient air", () => {
		const world = createWorld(7, 5);
		const index = 3 + 2 * world.width;
		world.setCell(index, SMOKE);
		world.massKg[index] = 0.0017;
		world.lifetime[index] = 1;
		const removedBefore = world.ledger.massRemovedKg;
		const reactions = createReactions(world);
		reactions.beginStep();
		reactions.update(index);
		expect(world.grid[index]).toBe(EMPTY);
		expect(world.massKg[index]).toBeCloseTo(world.massKg[0], 12);
		expect(world.ledger.massRemovedKg - removedBefore).toBeCloseTo(0.0017, 12);
	});
});
