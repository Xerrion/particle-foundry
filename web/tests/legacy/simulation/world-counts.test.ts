import { expect, test } from "bun:test";
import { createWorld } from "../../../src/legacy/simulation/world";
import { EMPTY, STONE, WATER } from "../../../src/materials";

test("material counts follow replacements, swaps, removal and clear", () => {
	const world = createWorld(3, 1);
	expect(world.countMaterial(EMPTY)).toBe(3);
	world.setCell(0, WATER);
	world.setCell(1, STONE);
	expect(world.countMaterial(WATER)).toBe(1);
	expect(world.countMaterial(STONE)).toBe(1);
	expect(world.getParticleCount()).toBe(2);
	world.swap(0, 2);
	expect(world.countMaterial(WATER)).toBe(1);
	world.changeMaterial(2, STONE);
	expect(world.countMaterial(WATER)).toBe(0);
	expect(world.countMaterial(STONE)).toBe(2);
	world.removeMatter(1);
	expect(world.getParticleCount()).toBe(1);
	world.clear();
	expect(world.countMaterial(EMPTY)).toBe(3);
	expect(world.getParticleCount()).toBe(0);
	expect(() => world.countMaterial(255)).toThrow(RangeError);
});
