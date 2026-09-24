import { describe, expect, test } from "bun:test";
import {
	EMPTY,
	FIRE,
	ICE,
	LAVA,
	MOLTEN_METAL,
	OIL,
	SAND,
	SMOKE,
	STEAM,
	STONE,
	WATER,
} from "../../src/materials";
import { createMotion } from "../../src/physics/motion";
import { energyAtTemperature } from "../../src/physics/thermal";
import { measureWorld } from "../../src/simulation/diagnostics";
import { createPhysics } from "../../src/simulation/physics";
import { createWorld } from "../../src/simulation/world";

describe("conservative local transport", () => {
	test("open edges release rising gas and outward moving matter but retain the floor", () => {
		const world = createWorld(5, 5, { boundariesEnabled: false });
		world.setCell(world.indexAt(2, 4), SAND);
		world.setCell(world.indexAt(3, 0), SMOKE);
		world.setCell(world.indexAt(0, 2), WATER);
		world.velocityX[world.indexAt(0, 2)] = -1;
		const before = world.ledger.massRemovedKg;
		const motion = createMotion(world);
		motion.update(world.indexAt(2, 4), 0);
		motion.update(world.indexAt(3, 0), 0);
		motion.update(world.indexAt(0, 2), 0);
		expect(world.getParticleCount()).toBe(1);
		expect(world.getCell(2, 4).material).toBe(SAND);
		expect(world.ledger.massRemovedKg).toBeGreaterThan(before);
	});

	test("closed edges retain particles", () => {
		const world = createWorld(1, 1);
		world.setCell(0, SAND);
		createMotion(world).update(0, 0);
		expect(world.grid[0]).toBe(SAND);
	});

	test("open top vents heated air pressure while the floor remains solid", () => {
		const open = createWorld(5, 5, { boundariesEnabled: false });
		const closed = createWorld(5, 5);
		for (const world of [open, closed]) {
			for (let x = 0; x < 5; x += 1) world.setCell(world.indexAt(x, 4), STONE);
			world.addExternalEnergy(world.indexAt(2, 0), 100);
			createPhysics(world).step();
		}
		expect(open.pressurePa[open.indexAt(2, 0)]).toBeCloseTo(101_325, 3);
		expect(closed.pressurePa[closed.indexAt(2, 0)]).toBeGreaterThan(101_325);
		expect(open.grid[open.indexAt(2, 4)]).toBe(STONE);
	});

	test("hot ambient air rises and carries its heat out of an open top", () => {
		const world = createWorld(1, 10, { boundariesEnabled: false });
		world.energy[9] = energyAtTemperature(EMPTY, 600, undefined, world.massKg[9]);
		const physics = createPhysics(world);
		for (let tick = 0; tick < 10; tick += 1) physics.step();
		for (let index = 0; index < world.size; index += 1) expect(world.temperatureAt(index)).toBe(22);
	});

	test("rising ambient air displaces cold air by at most one cell per tick", () => {
		const world = createWorld(1, 10);
		for (let index = 0; index < world.size; index += 1)
			world.energy[index] = energyAtTemperature(EMPTY, 600, undefined, world.massKg[index]);
		world.energy[0] = energyAtTemperature(EMPTY, -100, undefined, world.massKg[0]);
		const motion = createMotion(world);
		for (let index = 0; index < world.size; index += 1) motion.update(index, 0);
		expect(world.temperatureAt(0)).toBeCloseTo(600, 3);
		expect(world.temperatureAt(1)).toBe(-100);
		for (let index = 2; index < world.size; index += 1)
			expect(world.temperatureAt(index)).toBeCloseTo(600, 3);
	});

	test("pressure from inside the world dissipates through open edges", () => {
		const open = createWorld(9, 9, { boundariesEnabled: false });
		const closed = createWorld(9, 9);
		for (const world of [open, closed]) {
			for (let x = 0; x < 9; x += 1) world.setCell(world.indexAt(x, 8), STONE);
			world.addExternalEnergy(world.indexAt(4, 2), 500);
			const physics = createPhysics(world);
			for (let step = 0; step < 20; step += 1) physics.step();
		}
		expect(open.pressurePa[open.indexAt(4, 2)]).toBeLessThan(
			closed.pressurePa[closed.indexAt(4, 2)],
		);
		expect(open.grid[open.indexAt(4, 8)]).toBe(STONE);
	});
	test.each([-1, 1])("liquid momentum prefers the %i horizontal outlet", (direction) => {
		const world = createWorld(5, 2);
		world.setCell(2, WATER);
		world.setCell(7, WATER);
		world.setCell(5, STONE);
		world.setCell(9, STONE);
		world.velocityX[7] = direction;
		createMotion(world).update(7, 0);
		expect(world.grid[7 + direction]).toBe(WATER);
		expect(world.velocityX[7 + direction]).toBe(direction);
	});

	test.each([WATER, OIL, LAVA, MOLTEN_METAL])(
		"resting material %i does not shuffle across level ground",
		(material) => {
			const world = createWorld(7, 2);
			for (let x = 0; x < 7; x++) world.setCell(x + 7, STONE);
			world.setCell(3, material);
			const before = world.grid.slice();
			const motion = createMotion(world);
			for (let tick = 0; tick < 60; tick++) {
				world.moved.fill(0);
				for (let index = 0; index < world.size; index++) motion.update(index, tick);
				expect(world.grid).toEqual(before);
			}
		},
	);

	test("lava can still drain off a ledge at its material-defined cadence", () => {
		const world = createWorld(5, 4);
		world.setCell(1, LAVA);
		world.setCell(6, LAVA);
		world.setCell(11, STONE);
		world.setCell(12, STONE);
		world.setCell(0, STONE);
		world.setCell(5, STONE);
		world.setCell(10, STONE);
		const motion = createMotion(world);
		motion.update(6, 1);
		expect(world.grid[6]).toBe(LAVA);
		// The column feeds flow across the shelf; the next step descends
		// through its outlet.
		motion.update(6, 3);
		expect(world.grid[7]).toBe(LAVA);
		world.moved.fill(0);
		motion.update(7, 6);
		expect(world.grid[13]).toBe(LAVA);
	});

	test("a level lava/oil interface does not swap back and forth every three ticks", () => {
		const world = createWorld(12, 4);
		for (let x = 0; x < 12; x++) {
			world.setCell(x, OIL);
			world.setCell(x + 12, x % 2 ? OIL : LAVA);
			world.setCell(x + 24, LAVA);
			world.setCell(x + 36, STONE);
		}
		const before = world.grid.slice();
		const motion = createMotion(world);
		for (let tick = 0; tick < 120; tick++) {
			world.moved.fill(0);
			for (let index = world.size - 1; index >= 0; index--) motion.update(index, tick);
			expect(world.grid).toEqual(before);
		}
	});

	test("water can leave a ledge and fall into an unsupported region", () => {
		const world = createWorld(5, 5);
		world.setCell(2 + 1 * 5, WATER);
		world.setCell(2 + 2 * 5, STONE);
		const energy = measureWorld(world).totalTrackedEnergyKj * 1000;
		createMotion(world).update(7, 0);
		expect(world.grid[11] === WATER || world.grid[13] === WATER).toBe(true);
		expect(world.grid[7]).toBe(EMPTY);
		expect(measureWorld(world).totalTrackedEnergyKj * 1000).toBeCloseTo(energy, 8);
	});

	test("sealed diagonal corners cannot leak sand, water or steam", () => {
		for (const material of [WATER, SAND, STEAM]) {
			const world = createWorld(3, 3);
			for (const index of [1, 3, 5, 7]) world.setCell(index, STONE);
			world.setCell(4, material);
			const before = world.grid.slice();
			createMotion(world).update(4, 0);
			expect(world.grid).toEqual(before);
		}
	});

	test("water sinks through oil and ice but not through denser lava", () => {
		for (const below of [OIL, ICE, LAVA]) {
			const world = createWorld(1, 2);
			world.setCell(0, WATER);
			world.setCell(1, below);
			createMotion(world).update(0, 0);
			expect(world.grid[1]).toBe(below === LAVA ? LAVA : WATER);
			expect(world.grid[0]).toBe(below === LAVA ? WATER : below);
		}
	});

	test("an enclosed sand column cannot pump water upward", () => {
		const world = createWorld(1, 5);
		for (let y = 0; y < 3; y++) world.setCell(y, SAND);
		world.setCell(3, WATER);
		world.setCell(4, STONE);
		const motion = createMotion(world);
		for (let y = 2; y >= 0; y--) motion.update(y, 0);
		expect(Array.from(world.grid)).toEqual([SAND, SAND, SAND, WATER, STONE]);
	});

	test("an exposed sand grain can still sink into water", () => {
		const world = createWorld(1, 3);
		world.setCell(0, SAND);
		world.setCell(1, WATER);
		world.setCell(2, STONE);
		createMotion(world).update(0, 0);
		expect(Array.from(world.grid)).toEqual([WATER, SAND, STONE]);
	});

	test("sand routes displaced pool water to the open surface without losing matter or heat", () => {
		const world = createWorld(7, 5);
		for (let x = 0; x < 7; x++) world.setCell(x + 4 * 7, STONE);
		for (let x = 1; x < 6; x++) world.setCell(x + 3 * 7, WATER);
		for (let y = 0; y < 3; y++) world.setCell(3 + y * 7, SAND);
		const before = measureWorld(world).totalTrackedEnergyKj * 1000;
		createMotion(world).update(3 + 2 * 7, 0);
		expect(world.grid[3 + 3 * 7]).toBe(SAND);
		expect(world.grid[3 + 2 * 7]).toBe(EMPTY);
		expect(world.grid.filter((material) => material === WATER)).toHaveLength(5);
		expect(world.grid.slice(2 * 7, 3 * 7).includes(WATER)).toBe(true);
		expect(measureWorld(world).totalTrackedEnergyKj * 1000).toBeCloseTo(before, 8);
	});

	test("a sealed vessel cannot vent water through an anchored wall", () => {
		const world = createWorld(3, 4);
		for (let x = 0; x < 3; x++) {
			world.setCell(x, STONE);
			world.setCell(x + 3 * 3, STONE);
		}
		for (let y = 1; y < 3; y++) {
			world.setCell(y * 3, STONE);
			world.setCell(2 + y * 3, STONE);
		}
		world.setCell(4, SAND);
		world.setCell(7, WATER);
		const before = world.grid.slice();
		createMotion(world).update(4, 0);
		expect(world.grid).toEqual(before);
	});

	test("a broad sand column does not launch water far above a pool", () => {
		const world = createWorld(80, 90);
		for (let x = 0; x < world.width; x++) world.setCell(x + 89 * world.width, STONE);
		for (let y = 65; y < 89; y++) {
			world.setCell(10 + y * world.width, STONE);
			world.setCell(30 + y * world.width, STONE);
		}
		for (let y = 75; y < 89; y++) {
			for (let x = 11; x < 30; x++) world.setCell(x + y * world.width, WATER);
		}
		for (let y = 15; y < 75; y++) {
			for (let x = 16; x < 25; x++) world.setCell(x + y * world.width, SAND);
		}
		const physics = createPhysics(world);
		// Allow the entire tall column to reach the pool and displaced water to settle.
		for (let tick = 0; tick < 240; tick++) {
			physics.step();
			let highestWater = world.height;
			for (let index = 0; index < world.size; index++) {
				if (world.grid[index] === WATER)
					highestWater = Math.min(highestWater, Math.floor(index / world.width));
			}
			expect(highestWater).toBeGreaterThanOrEqual(55);
		}
		let waterCoveredBySand = 0;
		for (let index = world.width; index < world.size; index++) {
			if (world.grid[index] === WATER && world.grid[index - world.width] === SAND)
				waterCoveredBySand++;
		}
		expect(waterCoveredBySand).toBe(0);
	});

	test("ice falls in air but remains above water", () => {
		for (const below of [EMPTY, WATER]) {
			const world = createWorld(1, 2);
			world.setCell(0, ICE);
			world.setCell(1, below);
			createMotion(world).update(0, 0);
			expect(world.grid[below === EMPTY ? 1 : 0]).toBe(ICE);
		}
	});

	test("lateral paths rotate displaced air energy without dropping intermediate cells", () => {
		const world = createWorld(5, 2);
		// The column supplies a head difference; a lone resting parcel has
		// no reason to shuffle back and forth on a level floor.
		world.setCell(0, WATER);
		world.setCell(5, WATER);
		for (let i = 5; i < 10; i++) world.energy[i] = 100 + i - 5;
		createMotion(world).update(5, 0);
		expect(Array.from(world.energy.slice(5))).toEqual([101, 102, 103, 104, 100]);
		expect(world.grid[9]).toBe(WATER);
	});

	test("gases cannot rise through roofs or move a processed target twice", () => {
		const world = createWorld(1, 4);
		world.setCell(3, STEAM);
		world.setCell(2, STONE);
		createMotion(world).update(3, 0);
		expect(world.grid[3]).toBe(STEAM);
		world.setCell(2, EMPTY);
		world.moved[2] = 1;
		createMotion(world).update(3, 0);
		expect(world.grid[3]).toBe(STEAM);
	});

	test("hotter, lighter gases rise through denser gas without losing parcel state", () => {
		const world = createWorld(1, 3);
		world.setCell(0, EMPTY);
		world.setCell(1, SMOKE);
		world.setCell(2, STEAM);
		world.energy[2] = 3108;
		const mass = world.massKg[2];
		const energy = measureWorld(world).totalTrackedEnergyKj * 1000;
		createMotion(world).update(2, 0);
		expect(world.grid[0]).toBe(STEAM);
		expect(world.massKg[0]).toBe(mass);
		expect(measureWorld(world).totalTrackedEnergyKj * 1000).toBeCloseTo(energy, 8);
	});

	test("fire has stronger buoyancy than steam and receives upward velocity", () => {
		const world = createWorld(1, 4, { seed: 9 });
		world.setCell(2, STEAM);
		world.setCell(3, FIRE);
		createMotion(world).update(3, 0);
		expect(world.grid[0]).toBe(FIRE);
		expect(world.velocityY[0]).toBeLessThan(0);
	});

	test("physical behavior is identical with and without rendering", () => {
		const a = createWorld(6, 6);
		const b = createWorld(6, 6);
		for (const world of [a, b]) {
			world.setCell(2, WATER);
			world.setCell(3, OIL);
			world.setCell(4, SAND);
		}
		const first = createPhysics(a);
		const second = createPhysics(b);
		for (let i = 0; i < 100; i++) {
			first.step();
			second.step();
		}
		expect(a.grid).toEqual(b.grid);
		expect(a.energy).toEqual(b.energy);
	});
});
