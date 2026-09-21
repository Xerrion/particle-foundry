import { describe, expect, test } from "bun:test";
import {
	COOLER,
	EMPTY,
	ERASER,
	HEATER,
	ICE,
	MOLTEN_METAL,
	SAND,
	STEAM,
	STONE,
	WATER,
} from "../../src/materials";
import { energyAtTemperature, initialTemperature } from "../../src/physics/thermal";
import { CELL_VOLUME_M3 } from "../../src/simulation/physical-scale";
import { createWorld, type World } from "../../src/simulation/world";

const ambientEnergy = energyAtTemperature(EMPTY, 22, undefined, 1.204 * CELL_VOLUME_M3);

function fields(world: World) {
	return [world.grid, world.energy, world.lifetime, world.variation, world.moved];
}

function snapshot(world: World) {
	return {
		fields: fields(world).map((field) => Array.from(field)),
		count: world.getParticleCount(),
	};
}

describe("world initialization and ownership", () => {
	test("initializes every field to ambient air and uses row-major coordinates", () => {
		const world = createWorld(3, 2);
		expect([world.width, world.height, world.size]).toEqual([3, 2, 6]);
		expect(world.grid).toEqual(new Uint8Array(6));
		expect(world.energy).toEqual(new Float64Array(6).fill(ambientEnergy));
		expect(world.lifetime).toEqual(new Uint16Array(6));
		expect(world.variation).toEqual(new Uint8Array(6));
		expect(world.moved).toEqual(new Uint8Array(6));
		expect(world.getParticleCount()).toBe(0);
		expect(world.getCell(2, 1)).toEqual({
			material: EMPTY,
			temperature: 22,
			energy: ambientEnergy,
		});
		world.setCell(5, WATER);
		expect(world.getCell(2, 1).material).toBe(WATER);
		expect(world.getCell(1, 1).material).toBe(EMPTY);
		expect(world.getParticleCount()).toBe(1);
	});

	test.each([EMPTY, WATER, ICE, STEAM, MOLTEN_METAL])(
		"initializes material %i with its spawn energy and fresh lifetime",
		(material) => {
			const world = createWorld(3, 2);
			world.setCell(4, SAND);
			world.energy[4] = 501.25;
			world.lifetime[4] = 37;
			world.setCell(4, material);
			const temperature = initialTemperature(material);
			expect(world.getCell(1, 1).material).toBe(material);
			expect(world.energy[4]).toBe(
				energyAtTemperature(material, temperature, undefined, world.massKg[4]),
			);
			expect(world.temperatureAt(4)).toBeCloseTo(temperature, 10);
			expect(world.lifetime[4]).toBe(0);
			expect(world.variation[4]).toBeGreaterThanOrEqual(0);
			expect(world.variation[4]).toBeLessThan(4);
			expect(world.getParticleCount()).toBe(material === EMPTY ? 0 : 1);
			for (const index of [0, 1, 2, 3, 5]) {
				expect(fields(world).map((field) => field[index])).toEqual([EMPTY, ambientEnergy, 0, 0, 0]);
			}
		},
	);

	test("rejects invalid dimensions before allocating a world", () => {
		for (const value of [0, -1, 1.5, NaN, Infinity, 32768]) {
			expect(() => createWorld(value, 2)).toThrow(RangeError);
			expect(() => createWorld(2, value)).toThrow(RangeError);
		}
	});

	test("invalid cell initialization leaves all arrays and counts unchanged", () => {
		const world = createWorld(3, 2);
		world.setCell(0, STONE);
		world.setCell(4, WATER);
		world.energy[4] = 517.25;
		world.lifetime[4] = 23;
		world.variation[4] = 2;
		world.moved[4] = 1;
		const before = snapshot(world);
		for (const index of [-1, world.size, 1.5, NaN, Infinity]) {
			expect(() => world.setCell(index, WATER)).toThrow(RangeError);
			expect(snapshot(world)).toEqual(before);
		}
		for (const material of [-1, 200, 1.5, NaN, Infinity, HEATER, COOLER, ERASER]) {
			expect(() => world.setCell(4, material)).toThrow(RangeError);
			expect(snapshot(world)).toEqual(before);
		}
		world.setCell(4, EMPTY);
		expect(world.getParticleCount()).toBe(1);
		expect(world.getCell(1, 1).energy).toBe(ambientEnergy);
	});

	test("readings are detached and out-of-bounds probes cannot wrap to another row", () => {
		const world = createWorld(3, 2);
		world.setCell(3, WATER);
		const before = snapshot(world);
		const reading = world.getCell(0, 1);
		Object.assign(reading, { material: STONE, temperature: -20, energy: -42 });
		for (const [x, y] of [
			[3, 0],
			[-1, 1],
			[0, 2],
			[0, -1],
			[0.5, 1],
			[0, NaN],
		]) {
			expect(() => world.getCell(x, y)).toThrow(RangeError);
		}
		expect(snapshot(world)).toEqual(before);
		expect(world.getCell(0, 1).material).toBe(WATER);
	});

	test("instances own disjoint fields and mutations never affect another world", () => {
		const first = createWorld(3, 2);
		const second = createWorld(2, 3);
		second.setCell(0, STONE);
		second.setCell(5, WATER);
		const before = snapshot(second);
		for (const firstField of fields(first)) {
			for (const secondField of fields(second)) {
				expect(firstField.buffer).not.toBe(secondField.buffer);
			}
		}
		for (const mutate of [
			() => first.setCell(1, WATER),
			() => first.swap(1, 4),
			() => first.changeMaterial(4, EMPTY),
			() => first.clear(),
		]) {
			mutate();
			expect(snapshot(second)).toEqual(before);
		}
	});
});

describe("atomic cell state operations", () => {
	test.each([
		[WATER, EMPTY],
		[SAND, WATER],
	])("swaps all particle properties for %i and %i without changing counts", (a, b) => {
		const world = createWorld(4, 1);
		world.setCell(0, STONE);
		world.setCell(1, a);
		world.setCell(2, b);
		world.energy[1] = 412.125;
		world.energy[2] = -37.625;
		world.lifetime.set([9, 11, 23, 45]);
		world.variation.set([0, 1, 3, 2]);
		world.moved.set([1, 0, 0, 0]);
		const before = snapshot(world);
		const energy = world.energy.reduce((sum, value) => sum + value, 0);
		world.swap(1, 2);
		const particleFields = fields(world).slice(0, 4);
		for (let i = 0; i < particleFields.length; i += 1) {
			const original = before.fields[i];
			expect(Array.from(particleFields[i])).toEqual([
				original[0],
				original[2],
				original[1],
				original[3],
			]);
		}
		expect(Array.from(world.moved)).toEqual([1, 1, 1, 0]);
		expect(world.getParticleCount()).toBe(before.count);
		expect(world.energy.reduce((sum, value) => sum + value, 0)).toBe(energy);
		world.swap(1, 2);
		expect(particleFields.map((field) => Array.from(field))).toEqual(before.fields.slice(0, 4));
		expect(Array.from(world.moved)).toEqual([1, 1, 1, 0]);
		expect(world.getParticleCount()).toBe(before.count);
	});

	test("material and phase changes retain enthalpy and update counts immediately", () => {
		const world = createWorld(3, 1);
		world.setCell(1, WATER);
		world.energy[1] = energyAtTemperature(ICE, -20, undefined, world.massKg[1]);
		world.applyPhase(1);
		expect(world.getCell(1, 0)).toEqual({
			material: ICE,
			temperature: -20,
			energy: energyAtTemperature(ICE, -20, undefined, world.massKg[1]),
		});
		expect(world.getParticleCount()).toBe(1);
		world.changeMaterial(1, STONE);
		expect(world.energy[1]).toBe(energyAtTemperature(ICE, -20, undefined, world.massKg[1]));
		expect(world.getParticleCount()).toBe(1);
		world.changeMaterial(1, EMPTY);
		expect(world.energy[1]).toBe(energyAtTemperature(ICE, -20, undefined, world.massKg[1]));
		expect(world.getParticleCount()).toBe(0);
	});

	test("clear resets every field including air energy without replacing owned arrays", () => {
		const world = createWorld(3, 2);
		for (let i = 0; i < world.size; i += 1) {
			world.setCell(i, i === 0 ? EMPTY : WATER);
			world.energy[i] = 400 + i;
			world.lifetime[i] = 23 + i;
			world.variation[i] = 3;
			world.moved[i] = 1;
		}
		const owned = fields(world);
		world.clear();
		for (let i = 0; i < owned.length; i += 1) {
			expect(fields(world)[i]).toBe(owned[i]);
		}
		expect(snapshot(world)).toEqual(snapshot(createWorld(3, 2)));
		expect(world.getParticleCount()).toBe(0);
		world.setCell(5, WATER);
		expect(world.getParticleCount()).toBe(1);
		world.clear();
		expect(snapshot(world)).toEqual(snapshot(createWorld(3, 2)));
	});
});
