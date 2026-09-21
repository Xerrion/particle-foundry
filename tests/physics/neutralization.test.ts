import { describe, expect, test } from "bun:test";
import {
	HYDROCHLORIC_ACID,
	NEUTRAL_SOLUTION,
	SODIUM_HYDROXIDE,
	STONE,
	SULFURIC_ACID,
	WATER,
} from "../../src/materials";
import { createNeutralization } from "../../src/physics/neutralization";
import { CELL_VOLUME_M3 } from "../../src/simulation/physical-scale";
import { createPhysics } from "../../src/simulation/physics";
import { createWorld, type World } from "../../src/simulation/world";

function conservedTotal(world: World): number {
	let total = 0;
	for (let i = 0; i < world.size; i += 1)
		total += world.energy[i] + world.chemicalEnergyKj[i] * 1000;
	return total;
}

describe("aqueous neutralization presets", () => {
	test.each([HYDROCHLORIC_ACID, SULFURIC_ACID])(
		"one acid-equivalent cell of %i and one 1 M base cell yield measured-scale heat and conserve mass and energy",
		(acid) => {
			const world = createWorld(2, 1);
			world.setCell(0, acid);
			world.setCell(1, SODIUM_HYDROXIDE);
			const mass = world.massKg[0] + world.massKg[1];
			const total = conservedTotal(world);
			const initialThermal = world.energy[0] + world.energy[1];
			// 1 millilitre of a 1 equivalent/L solution reacts at 57.2 kJ per mole.
			const expectedJ = CELL_VOLUME_M3 * 1000 * 1 * 57_200;
			expect(createNeutralization(world).step()).toBeCloseTo(expectedJ, 9);
			expect(Array.from(world.grid)).toEqual([NEUTRAL_SOLUTION, NEUTRAL_SOLUTION]);
			expect(world.energy[0] + world.energy[1] - initialThermal).toBeCloseTo(expectedJ, 9);
			expect(world.temperatureAt(0)).toBeCloseTo(22 + expectedJ / (2 * 4.18), 8);
			expect(world.massKg[0] + world.massKg[1]).toBeCloseTo(mass, 12);
			expect(conservedTotal(world)).toBeCloseTo(total, 8);
			expect(createNeutralization(world).step()).toBe(0);
		},
	);

	test("water, stone barriers and diagonal contact do not neutralize", () => {
		for (const [width, height, cells] of [
			[2, 1, [HYDROCHLORIC_ACID, WATER]],
			[3, 1, [HYDROCHLORIC_ACID, STONE, SODIUM_HYDROXIDE]],
			[2, 2, [HYDROCHLORIC_ACID, STONE, STONE, SODIUM_HYDROXIDE]],
		] as const) {
			const world = createWorld(width, height);
			for (const [index, material] of cells.entries()) world.setCell(index, material);
			const before = conservedTotal(world);
			expect(createNeutralization(world).step()).toBe(0);
			expect(Array.from(world.grid)).toEqual([...cells]);
			expect(conservedTotal(world)).toBe(before);
		}
	});

	test("an acid cell pairs with only one neighbor per tick and a depleted store cannot react", () => {
		const world = createWorld(3, 1);
		for (const [index, material] of [
			SODIUM_HYDROXIDE,
			HYDROCHLORIC_ACID,
			SODIUM_HYDROXIDE,
		].entries())
			world.setCell(index, material);
		expect(createNeutralization(world).step()).toBeCloseTo(57.2, 9);
		expect(Array.from(world.grid)).toEqual([NEUTRAL_SOLUTION, NEUTRAL_SOLUTION, SODIUM_HYDROXIDE]);
		const exhausted = createWorld(2, 1);
		exhausted.setCell(0, HYDROCHLORIC_ACID);
		exhausted.setCell(1, SODIUM_HYDROXIDE);
		exhausted.chemicalEnergyKj[0] /= 2;
		expect(createNeutralization(exhausted).step()).toBe(0);
		expect(Array.from(exhausted.grid)).toEqual([HYDROCHLORIC_ACID, SODIUM_HYDROXIDE]);
	});

	test("the full physics step reacts a touching acid and base", () => {
		const world = createWorld(2, 1);
		world.setCell(0, HYDROCHLORIC_ACID);
		world.setCell(1, SODIUM_HYDROXIDE);
		createPhysics(world).step();
		expect(Array.from(world.grid)).toEqual([NEUTRAL_SOLUTION, NEUTRAL_SOLUTION]);
	});
});
