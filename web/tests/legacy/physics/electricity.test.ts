import { describe, expect, test } from "bun:test";
import { createElectricity } from "../../../src/legacy/physics/electricity";
import { createPhysics } from "../../../src/legacy/simulation/physics";
import { createWorld, type World } from "../../../src/legacy/simulation/world";
import { BATTERY, GROUND, LAMP, METAL, STONE, WIRE } from "../../../src/materials";

function totalStoredEnergyJ(world: World): number {
	let total = 0;
	for (let index = 0; index < world.size; index += 1)
		total += world.energy[index] + world.chemicalEnergyKj[index] * 1000;
	return total;
}

describe("finite electrical circuits", () => {
	test("a 12 V source across two 1 ohm edges follows Ohm's law and conserves stored energy", () => {
		const world = createWorld(3, 1);
		for (const [index, material] of [BATTERY, WIRE, GROUND].entries())
			world.setCell(index, material);
		const before = totalStoredEnergyJ(world);
		const initialWireHeat = world.energy[1];
		const initialBatteryStore = world.chemicalEnergyKj[0];
		const electricity = createElectricity(world);
		// R = 2 ohms, I = 6 A, P = 72 W, time = 1/60 s.
		expect(electricity.step()).toBeCloseTo(1.2, 10);
		expect(electricity.voltageAt(1)).toBeCloseTo(6, 7);
		expect(world.energy[1] - initialWireHeat).toBeCloseTo(0.6, 10);
		expect((initialBatteryStore - world.chemicalEnergyKj[0]) * 1000).toBeCloseTo(1.2, 10);
		expect(totalStoredEnergyJ(world)).toBeCloseTo(before, 8);
	});

	test("open, insulated and diagonal paths draw no battery energy", () => {
		for (const [width, height, cells] of [
			[2, 1, [BATTERY, WIRE]],
			[3, 1, [BATTERY, STONE, GROUND]],
			[2, 2, [BATTERY, STONE, STONE, GROUND]],
		] as const) {
			const world = createWorld(width, height);
			for (const [index, material] of cells.entries()) world.setCell(index, material);
			const before = totalStoredEnergyJ(world);
			const batteryStore = world.chemicalEnergyKj[0];
			expect(createElectricity(world).step()).toBe(0);
			expect(world.chemicalEnergyKj[0]).toBe(batteryStore);
			expect(totalStoredEnergyJ(world)).toBe(before);
		}
	});

	test("a depleted battery delivers at most its remaining chemical energy", () => {
		const world = createWorld(3, 1);
		for (const [index, material] of [BATTERY, WIRE, GROUND].entries())
			world.setCell(index, material);
		world.chemicalEnergyKj[0] = 0.0001;
		const before = totalStoredEnergyJ(world);
		const electricity = createElectricity(world);
		expect(electricity.step()).toBeCloseTo(0.1, 10);
		expect(world.chemicalEnergyKj[0]).toBe(0);
		expect(electricity.step()).toBe(0);
		expect(totalStoredEnergyJ(world)).toBeCloseTo(before, 8);
	});

	test("metal conducts while a separate grounded island does not draw current", () => {
		const world = createWorld(6, 1);
		for (const [index, material] of [BATTERY, METAL, GROUND, STONE, WIRE, GROUND].entries())
			world.setCell(index, material);
		const unrelatedHeat = world.energy[4] + world.energy[5];
		const electricity = createElectricity(world);
		// R = 0.9 + 0.9 ohm; P = 12² / 1.8 = 80 W.
		expect(electricity.step()).toBeCloseTo(80 / 60, 9);
		expect(world.energy[4] + world.energy[5]).toBe(unrelatedHeat);
	});

	test("a lamp on a closed path heats while the full physics step consumes battery energy", () => {
		const world = createWorld(3, 1);
		for (const [index, material] of [BATTERY, LAMP, GROUND].entries())
			world.setCell(index, material);
		const initialLampHeat = world.energy[1];
		const initialBatteryStore = world.chemicalEnergyKj[0];
		const electricity = createElectricity(world);
		// Two 2.5-ohm edges: I = 2.4 A, total P = 28.8 W.
		expect(electricity.step()).toBeCloseTo(0.48, 10);
		expect(world.energy[1] - initialLampHeat).toBeCloseTo(0.24, 10);
		createPhysics(world).step();
		expect(world.chemicalEnergyKj[0]).toBeLessThan(initialBatteryStore - 0.00048);
	});
});
