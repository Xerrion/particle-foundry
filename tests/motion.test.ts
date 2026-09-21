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
} from "../src/materials";
import { createMotion } from "../src/motion";
import { createPhysics } from "../src/physics";
import { createWorld } from "../src/world";

describe("conservative local transport", () => {
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
		const energy = world.energy.reduce((a, b) => a + b, 0);
		createMotion(world).update(7, 0);
		expect(world.grid[11] === WATER || world.grid[13] === WATER).toBe(true);
		expect(world.grid[7]).toBe(EMPTY);
		expect(world.energy.reduce((a, b) => a + b, 0)).toBe(energy);
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
		const energy = world.energy.reduce((sum, value) => sum + value, 0);
		createMotion(world).update(2, 0);
		expect(world.grid[0]).toBe(STEAM);
		expect(world.massKg[0]).toBe(mass);
		expect(world.energy.reduce((sum, value) => sum + value, 0)).toBe(energy);
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
