import { describe, expect, test } from "bun:test";
import { createVisualWaves, type VisualWaves } from "../../../src/legacy/rendering/visual-waves";
import { createWorld, type World } from "../../../src/legacy/simulation/world";
import {
	EMPTY,
	FIRE,
	GLASS,
	ICE,
	LAVA,
	METAL,
	MOLTEN_METAL,
	OIL,
	PLANT,
	SAND,
	SMOKE,
	STEAM,
	STONE,
	WATER,
	WOOD,
} from "../../../src/materials";

function pool(material = WATER, surface = 8, width = 13, height = 20): World {
	const world = createWorld(width, height);
	for (let y = surface; y < height; y += 1) {
		for (let x = 0; x < width; x += 1) world.setCell(x + y * width, material);
	}
	return world;
}

function appearance(world: World, waves: VisualWaves): number[] {
	const result: number[] = [];
	for (let i = 0; i < world.size; i += 1) {
		const x = i % world.width;
		const y = Math.floor(i / world.width);
		const material = waves.materialAt(x, y, world.grid[i]);
		result.push(material, Number(waves.isSurface(x, y, material)));
	}
	return result;
}

describe("rendering-only surface waves", () => {
	test.each([WATER, OIL, LAVA])("detects supported surfaces for material %i", (material) => {
		const world = pool(material);
		const waves = createVisualWaves(world);
		expect(waves.isEnabled()).toBe(true);
		expect(waves.isSurface(6, 8, material)).toBe(false);
		waves.update();
		expect(waves.isSurface(6, 8, material)).toBe(true);
		expect(waves.isSurface(6, 9, material)).toBe(false);
		expect(waves.materialAt(6, 7, EMPTY)).toBe(EMPTY);
	});

	test("ignores isolated drops and supports narrow worlds and horizontal pools", () => {
		const isolated = createWorld(1, 1);
		isolated.setCell(0, WATER);
		const drop = createVisualWaves(isolated);
		drop.disturb(0, WATER, -1, 0);
		drop.update();
		expect(drop.materialAt(0, 0, WATER)).toBe(WATER);
		expect(drop.isSurface(0, 0, WATER)).toBe(false);
		for (const world of [pool(WATER, 1, 1, 3), pool(WATER, 0, 2, 1)]) {
			const waves = createVisualWaves(world);
			waves.update();
			const y = world.height === 1 ? 0 : 1;
			expect(waves.isSurface(0, y, WATER)).toBe(true);
		}
	});

	test.each([WATER, OIL, LAVA])(
		"changes lighting but never occupancy for material %i",
		(material) => {
			const world = pool(material);
			const waves = createVisualWaves(world);
			waves.update();
			waves.disturb(6, material, -1, 0);
			expect(waves.materialAt(6, 7, EMPTY)).toBe(EMPTY);
			waves.update();
			expect(waves.materialAt(6, 7, EMPTY)).toBe(EMPTY);
			expect(waves.materialAt(6, 7, SMOKE)).toBe(SMOKE);
			expect(waves.materialAt(6, 7, STEAM)).toBe(STEAM);
			expect(waves.shimmerAt(6, 8, material)).toBeLessThan(0);
			waves.reset();
			waves.update();
			waves.disturb(6, material, 1, 0);
			waves.update();
			expect(waves.materialAt(6, 8, material)).toBe(material);
			expect(waves.materialAt(6, 9, material)).toBe(material);
			expect(waves.isSurface(6, 8, material)).toBe(true);
			expect(waves.shimmerAt(6, 8, material)).toBeGreaterThan(0);
		},
	);

	test("preserves the default impulse falloff and clips impulses at world edges", () => {
		const waves = createVisualWaves(pool());
		waves.update();
		waves.disturb(6, WATER, -1);
		waves.update();
		for (const x of [4, 5, 6, 7, 8]) expect(waves.shimmerAt(x, 8, WATER)).toBeLessThan(0);
		for (const x of [0, 3, 9, 12]) expect(waves.isSurface(x, 8, WATER)).toBe(true);
		waves.reset();
		waves.update();
		waves.disturb(-1, WATER, -2, 1);
		waves.disturb(13, WATER, -2, 1);
		waves.update();
		expect(waves.shimmerAt(0, 8, WATER)).toBeLessThan(0);
		expect(waves.shimmerAt(12, 8, WATER)).toBeLessThan(0);
		expect(waves.isSurface(1, 8, WATER)).toBe(true);
	});

	test("propagates a damped ripple to connected neighbors and eventually settles", () => {
		const waves = createVisualWaves(pool());
		waves.update();
		waves.disturb(6, WATER, -1, 0);
		waves.update();
		expect(waves.shimmerAt(6, 8, WATER)).toBeLessThan(0);
		expect(waves.isSurface(5, 8, WATER)).toBe(true);
		waves.update();
		expect(waves.shimmerAt(6, 8, WATER)).toBeLessThan(0);
		waves.update();
		expect(waves.shimmerAt(5, 8, WATER)).toBeLessThan(0);
		expect(waves.shimmerAt(7, 8, WATER)).toBeLessThan(0);
		for (let step = 0; step < 1500; step += 1) waves.update();
		for (let x = 0; x < 13; x += 1) expect(waves.shimmerAt(x, 8, WATER)).toBeCloseTo(0, 4);
	});

	test("does not couple across a gap or a surface step larger than three cells", () => {
		for (const rightSurface of [-1, 12]) {
			const world = pool();
			for (let y = 0; y < world.height; y += 1) {
				world.setCell(7 + y * world.width, y >= rightSurface && rightSurface >= 0 ? WATER : EMPTY);
			}
			const waves = createVisualWaves(world);
			waves.update();
			waves.disturb(6, WATER, -1, 0);
			for (let step = 0; step < 10; step += 1) waves.update();
			expect(waves.isSurface(8, 8, WATER)).toBe(true);
		}
	});

	test("turns detected surface shifts into the original visual impulse", () => {
		const world = pool();
		const waves = createVisualWaves(world);
		waves.update();
		for (let y = 5; y < 8; y += 1) {
			for (let x = 0; x < world.width; x += 1) world.setCell(x + y * world.width, WATER);
		}
		waves.update();
		expect(waves.isSurface(6, 5, WATER)).toBe(true);
		expect(waves.shimmerAt(6, 5, WATER)).toBeLessThan(0);
		world.clear();
		waves.update();
		expect(waves.materialAt(6, 4, EMPTY)).toBe(EMPTY);
		expect(waves.isSurface(6, 4, WATER)).toBe(false);
	});

	test.each([
		[WATER, 4],
		[OIL, 3],
		[LAVA, 2],
	])("bounds the lighting amplitude for material %i", (material, _limit) => {
		const waves = createVisualWaves(pool(material));
		for (const direction of [-1, 1]) {
			waves.reset();
			waves.update();
			waves.disturb(6, material, direction * 100, 0);
			waves.update();
			expect(waves.isSurface(6, 8, material)).toBe(true);
			expect(waves.shimmerAt(6, 8, material)).toBeCloseTo(direction, 6);
		}
	});

	test("never replaces newly painted solids, fire, or molten metal with stale waves", () => {
		const world = pool();
		const waves = createVisualWaves(world);
		for (const strength of [-2, 2]) {
			waves.reset();
			waves.update();
			waves.disturb(6, WATER, strength, 0);
			waves.update();
			const y = strength < 0 ? 7 : 8;
			for (const material of [SAND, STONE, WOOD, METAL, PLANT, ICE, GLASS, FIRE, MOLTEN_METAL]) {
				world.setCell(6 + y * world.width, material);
				expect(waves.materialAt(6, y, world.grid[6 + y * world.width])).toBe(material);
				expect(waves.isSurface(6, y, material)).toBe(false);
			}
			world.setCell(6 + y * world.width, strength < 0 ? EMPTY : WATER);
		}
	});

	test("reset erases all surfaces and motion and keeps the enabled setting", () => {
		const world = pool();
		const waves = createVisualWaves(world);
		waves.update();
		waves.disturb(6, WATER, -3);
		waves.update();
		waves.reset();
		expect(waves.isEnabled()).toBe(true);
		const fresh = createVisualWaves(world);
		expect(appearance(world, waves)).toEqual(appearance(world, fresh));
		for (let step = 0; step < 20; step += 1) {
			waves.update();
			fresh.update();
			expect(appearance(world, waves)).toEqual(appearance(world, fresh));
		}
	});

	test("disabled updates and impulses stay erased; enabling detects current surfaces", () => {
		const world = pool();
		const waves = createVisualWaves(world);
		waves.update();
		waves.disturb(6, WATER, -3);
		waves.update();
		waves.setEnabled(false);
		waves.reset();
		expect(waves.isEnabled()).toBe(false);
		waves.disturb(6, WATER, -3);
		waves.update();
		expect(waves.materialAt(6, 7, EMPTY)).toBe(EMPTY);
		expect(waves.isSurface(6, 8, WATER)).toBe(false);
		world.clear();
		world.setCell(6 + 10 * world.width, WATER);
		world.setCell(6 + 11 * world.width, WATER);
		waves.setEnabled(true);
		expect(waves.isEnabled()).toBe(true);
		expect(waves.isSurface(6, 10, WATER)).toBe(true);
	});

	test("instances do not share motion, settings or world dimensions", () => {
		const world = pool();
		const first = createVisualWaves(world);
		const second = createVisualWaves(world);
		const narrow = createVisualWaves(pool(OIL, 1, 1, 3));
		first.update();
		second.update();
		narrow.update();
		first.disturb(6, WATER, -1, 0);
		first.update();
		expect(first.shimmerAt(6, 8, WATER)).not.toBe(0);
		expect(second.shimmerAt(6, 8, WATER)).toBe(0);
		expect(second.isSurface(6, 8, WATER)).toBe(true);
		first.setEnabled(false);
		expect(second.isEnabled()).toBe(true);
		expect(narrow.isSurface(0, 1, OIL)).toBe(true);
	});

	test.each([WATER, OIL, LAVA])(
		"all visual operations preserve every world field for material %i",
		(material) => {
			const world = pool(material);
			const fields = [world.grid, world.energy, world.lifetime, world.variation, world.moved];
			const before = fields.map((field) => Array.from(field));
			const waves = createVisualWaves(world);
			const operations = [
				() => waves.update(),
				() => waves.disturb(6, material, -2),
				() => waves.update(),
				() => appearance(world, waves),
				() => waves.setEnabled(false),
				() => waves.update(),
				() => waves.setEnabled(true),
				() => waves.reset(),
			];
			for (const operation of operations) {
				operation();
				expect(fields.map((field) => Array.from(field))).toEqual(before);
			}
		},
	);

	test("ignores unsupported materials and rejects invalid impulses without poisoning motion", () => {
		const waves = createVisualWaves(pool());
		waves.update();
		for (const material of [EMPTY, STONE, MOLTEN_METAL, -1, 255, NaN]) {
			waves.disturb(6, material, -10);
		}
		for (const strength of [NaN, Infinity, -Infinity, Number.MAX_VALUE]) {
			expect(() => waves.disturb(6, WATER, strength, 0)).toThrow(RangeError);
		}
		for (const value of [0.5, NaN, Infinity]) {
			expect(() => waves.disturb(value, WATER, 1)).toThrow(RangeError);
			expect(() => waves.disturb(6, WATER, 1, value)).toThrow(RangeError);
		}
		expect(() => waves.disturb(6, WATER, 1, -1)).toThrow(RangeError);
		waves.update();
		expect(waves.isSurface(6, 8, WATER)).toBe(true);
	});
});
