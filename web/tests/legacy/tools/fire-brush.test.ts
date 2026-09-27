import { describe, expect, test } from "bun:test";
import { createReactions } from "../../../src/legacy/physics/reactions";
import { energyAtTemperature } from "../../../src/legacy/physics/thermal";
import { createPhysics } from "../../../src/legacy/simulation/physics";
import { createWorld } from "../../../src/legacy/simulation/world";
import { createBrush } from "../../../src/legacy/tools/brush";
import {
	EMPTY,
	FIRE,
	materialsById,
	OIL,
	PLANT,
	SMOKE,
	STEAM,
	STONE,
	WATER,
	WOOD,
} from "../../../src/materials";

describe("controlled ignition brush", () => {
	test("a click reliably lights the center with a sparse, short-lived fringe", () => {
		const world = createWorld(13, 13, { seed: 42 });
		const brush = createBrush(world, () => {});
		brush.setMaterial(FIRE);
		const profile = materialsById[FIRE].ignitionBrush;
		if (!profile) throw new Error("Missing ignition brush profile");
		const initialMass = world.massKg.slice();
		brush.paintCircle(6, 6);
		expect(world.grid[6 + 6 * 13]).toBe(FIRE);
		const flames = world.grid.reduce((count, id) => count + Number(id === FIRE), 0);
		expect(flames).toBeGreaterThan(1);
		expect(flames).toBeLessThan(25); // Fewer than half the 49 cells in a radius-4 disk.
		for (let i = 0; i < world.size; i++) {
			if (world.grid[i] !== FIRE) continue;
			expect(world.temperatureAt(i)).toBeCloseTo(profile.temperatureC, 10);
			expect(world.lifetime[i]).toBeGreaterThanOrEqual(profile.lifetimeTicks[0]);
			expect(world.lifetime[i]).toBeLessThan(profile.lifetimeTicks[1]);
			expect(world.ignitionFlame[i]).toBe(1);
		}
		expect(world.massKg).toEqual(initialMass);
		expect(world.ledger.massAddedKg).toBe(0);
		expect(world.ledger.massRemovedKg).toBe(0);
		expect(world.ledger.externalEnergyAdded).toBeCloseTo(
			(flames * (profile.temperatureC - 22) * world.massKg[0]) / 0.001,
			9,
		);
	});

	test("repainting an ignition flame does not restart its lifetime or add heat", () => {
		const world = createWorld(1, 1);
		const brush = createBrush(world, () => {});
		brush.setMaterial(FIRE);
		brush.paintCircle(0, 0);
		createReactions(world).update(0);
		const lifetime = world.lifetime[0];
		const energy = world.energy[0];
		const added = world.ledger.externalEnergyAdded;
		brush.paintCircle(0, 0);
		expect(world.ignitionFlame[0]).toBe(1);
		expect(world.lifetime[0]).toBe(lifetime);
		expect(world.energy[0]).toBe(energy);
		expect(world.ledger.externalEnergyAdded).toBe(added);
	});

	test("ignition identity follows the parcel and clears on every replacement path", () => {
		const world = createWorld(2, 1);
		const brush = createBrush(world, () => {});
		brush.setMaterial(FIRE);
		expect(world.ignitionFlame).toEqual(new Uint8Array(2));
		for (const replace of [
			() => world.changeMaterial(1, SMOKE),
			() => world.setCell(1, FIRE),
			() => world.removeMatter(1),
			() => world.clear(),
		]) {
			world.clear();
			world.setCell(1, STONE);
			brush.paintCircle(0, 0);
			const energy = world.energy[0];
			const lifetime = world.lifetime[0];
			world.swap(0, 1);
			expect(Array.from(world.ignitionFlame)).toEqual([0, 1]);
			expect(world.grid[1]).toBe(FIRE);
			expect(world.energy[1]).toBe(energy);
			expect(world.lifetime[1]).toBe(lifetime);
			replace();
			expect(world.ignitionFlame).toEqual(new Uint8Array(2));
		}
	});

	test("ordinary fire can still become combustion smoke", () => {
		const world = createWorld(1, 1);
		world.setCell(0, FIRE);
		world.lifetime[0] = 1;
		world.random.next = () => 0;
		createReactions(world).update(0);
		expect(world.grid[0]).toBe(SMOKE);
		expect(world.ignitionFlame[0]).toBe(0);
	});

	test.each(["expired", "cold", "water", "sealed"])(
		"ignition returns to air without losing mass or heat (reason: %s)",
		(reason) => {
			const world = createWorld(3, 3);
			for (let i = 0; i < world.size; i++) world.setCell(i, STONE);
			world.setCell(4, EMPTY);
			if (reason !== "sealed") world.setCell(1, EMPTY);
			if (reason === "water") world.setCell(5, WATER);
			const brush = createBrush(world, () => {});
			brush.setSize(1);
			brush.setMaterial(FIRE);
			brush.paintCircle(1, 1);
			if (reason === "expired") world.lifetime[4] = 1;
			if (reason === "cold")
				world.energy[4] = energyAtTemperature(FIRE, 100, undefined, world.massKg[4]);
			const mass = world.massKg[4];
			const energy = world.energy[4];
			const ledger = { ...world.ledger };
			const reactions = createReactions(world);
			reactions.beginStep();
			reactions.update(4);
			expect(world.grid[4]).toBe(EMPTY);
			expect(world.ignitionFlame[4]).toBe(0);
			expect(world.massKg[4]).toBe(mass);
			expect(world.energy[4]).toBe(energy);
			expect(world.ledger).toEqual(ledger);
		},
	);

	test.each([1, 4, 12])(
		"a held radius-%i brush stays local, produces no soot without fuel and fades after release",
		(radius) => {
			const world = createWorld(64, 96, { seed: 42 });
			const brush = createBrush(world, () => {});
			brush.setMaterial(FIRE);
			brush.setSize(radius);
			const physics = createPhysics(world);
			const sourceY = 78;
			let peakRise = 0;
			let activeTicks = 0;
			const mass = world.massKg.reduce((a, b) => a + b, 0);
			const heat = world.energy.reduce((a, b) => a + b, 0);
			for (let tick = 0; tick < 180; tick++) {
				brush.paintCircle(32, sourceY);
				physics.step();
				const first = world.grid.indexOf(FIRE);
				if (first >= 0) {
					activeTicks++;
					peakRise = Math.max(peakRise, sourceY - Math.floor(first / world.width));
				}
				expect(world.grid.includes(SMOKE)).toBe(false);
			}
			expect(activeTicks).toBeGreaterThan(170);
			expect(peakRise).toBeLessThanOrEqual(radius + 18);
			// No paint calls after release. Unfuelled ignition gas must not
			// keep climbing or leave a screen-filling smoke cloud behind.
			for (let tick = 0; tick < 20; tick++) physics.step();
			expect(world.grid.every((id) => id === EMPTY)).toBe(true);
			expect(world.massKg.reduce((a, b) => a + b, 0)).toBeCloseTo(mass, 10);
			expect(
				Math.abs(
					world.energy.reduce((a, b) => a + b, 0) -
						(heat + world.ledger.externalEnergyAdded - world.ledger.externalEnergyRemoved),
				),
			).toBeLessThan(2e-3);
		},
	);

	test.each([WOOD, OIL, PLANT])(
		"the gentle brush still ignites fuel %i without replacing it",
		(fuel) => {
			const world = createWorld(9, 9);
			world.setCell(40, fuel);
			const mass = world.massKg[40];
			const chemical = world.chemicalEnergyKj[40];
			const brush = createBrush(world, () => {});
			brush.setSize(1);
			brush.setMaterial(FIRE);
			brush.paintCircle(4, 4);
			expect(world.grid[40]).toBe(fuel);
			expect(world.massKg[40]).toBe(mass);
			expect(world.chemicalEnergyKj[40]).toBe(chemical);
			// Constrain oil motion, not combustion.
			for (const index of [48, 49, 50]) world.setCell(index, STONE);
			createPhysics(world).step();
			expect(world.burning.some((value) => value === 1)).toBe(true);
			expect(world.chemicalEnergyKj.reduce((a, b) => a + b, 0)).toBeLessThan(chemical);
		},
	);

	test.each([WOOD, OIL, PLANT])(
		"fuel %i sustains ordinary fire and smoke after a single click",
		(fuel) => {
			const world = createWorld(32, 40, { seed: 42 });
			for (let y = 33; y < 39; y++) {
				for (let x = 11; x < 21; x++) world.setCell(x + y * 32, fuel);
			}
			for (let y = 30; y < 40; y++) {
				world.setCell(10 + y * 32, STONE);
				world.setCell(21 + y * 32, STONE);
			}
			for (let x = 10; x <= 21; x++) world.setCell(x + 39 * 32, STONE);
			const brush = createBrush(world, () => {});
			brush.setMaterial(FIRE);
			brush.paintCircle(16, 33);
			const suppliedHeat = world.ledger.externalEnergyAdded;
			const chemical = world.chemicalEnergyKj.reduce((a, b) => a + b, 0);
			const physics = createPhysics(world);
			for (let tick = 0; tick < 180; tick++) physics.step();
			expect(world.burning.includes(1)).toBe(true);
			expect(world.grid.includes(FIRE)).toBe(true);
			expect(world.grid.includes(SMOKE)).toBe(true);
			expect(world.ignitionFlame.includes(1)).toBe(false);
			expect(world.ledger.externalEnergyAdded).toBe(suppliedHeat);
			expect(world.chemicalEnergyKj.reduce((a, b) => a + b, 0)).toBeLessThan(chemical);
		},
	);

	test.each([SMOKE, STEAM, WATER, STONE])(
		"the fire brush does not overwrite nonfuel material %i",
		(material) => {
			const world = createWorld(1, 1);
			world.setCell(0, material);
			const before = [world.grid[0], world.energy[0], world.massKg[0], world.lifetime[0]];
			const brush = createBrush(world, () => {});
			brush.setMaterial(FIRE);
			brush.paintCircle(0, 0);
			expect([world.grid[0], world.energy[0], world.massKg[0], world.lifetime[0]]).toEqual(before);
		},
	);

	test("ignition adds recorded heat but never cools already-hot air", () => {
		const world = createWorld(1, 1);
		world.energy[0] = energyAtTemperature(EMPTY, 2200, undefined, world.massKg[0]);
		const mass = world.massKg[0];
		const brush = createBrush(world, () => {});
		brush.setMaterial(FIRE);
		brush.paintCircle(0, 0);
		expect(world.grid[0]).toBe(FIRE);
		expect(world.energy[0]).toBe(energyAtTemperature(EMPTY, 2200, undefined, mass));
		expect(world.massKg[0]).toBe(mass);
		expect(world.ledger.externalEnergyAdded).toBe(0);
	});
});
