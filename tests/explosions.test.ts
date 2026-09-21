import { describe, expect, test } from "bun:test";
import { createExplosions } from "../src/explosions";
import { createGasDynamics } from "../src/gas-dynamics";
import { BLAST, EMPTY, FIRE, GUNPOWDER, STONE } from "../src/materials";
import { AMBIENT_PRESSURE_PA } from "../src/physical-scale";
import { createSandbox } from "../src/sandbox";
import { energyAtTemperature } from "../src/thermal";
import { createWorld, type World } from "../src/world";

function trackedEnergyJ(world: World): number {
	let total = 0;
	for (let index = 0; index < world.size; index += 1) {
		total += world.energy[index];
		total += world.chemicalEnergyKj[index] * 1000;
		total +=
			0.5 * world.massKg[index] * (world.velocityX[index] ** 2 + world.velocityY[index] ** 2);
	}
	return total;
}

describe("energy-funded local explosions", () => {
	test("gunpowder releases finite chemical energy into heat and outward motion", () => {
		const world = createWorld(13, 13);
		const center = 6 + 6 * world.width;
		world.setCell(center, GUNPOWDER);
		const beforeMass = world.massKg.reduce((sum, mass) => sum + mass, 0);
		const beforeEnergy = trackedEnergyJ(world);
		const releasedJ = world.chemicalEnergyKj[center] * 1000;
		expect(createExplosions(world).detonate(center)).toBe(true);
		expect(world.grid[center]).toBe(FIRE);
		expect(world.chemicalEnergyKj[center]).toBe(0);
		expect(world.grid.filter((material) => material === FIRE).length).toBeGreaterThan(1);
		expect(world.velocityX[center - 1]).toBeLessThan(0);
		expect(world.velocityX[center + 1]).toBeGreaterThan(0);
		expect(world.massKg.reduce((sum, mass) => sum + mass, 0)).toBeCloseTo(beforeMass, 12);
		expect(trackedEnergyJ(world)).toBeCloseTo(beforeEnergy, 7);
		expect(world.ledger.externalEnergyAdded).toBe(0);
		createGasDynamics(world).derivePressure();
		expect(world.pressurePa[center]).toBeGreaterThan(AMBIENT_PRESSURE_PA);
		expect(releasedJ).toBeGreaterThan(0);
	});

	test("an anchored wall blocks blast heat, flame and motion", () => {
		const world = createWorld(11, 7);
		for (let y = 0; y < world.height; y += 1) world.setCell(5 + y * world.width, STONE);
		const right = Array.from({ length: world.height }, (_, y) => 6 + y * world.width);
		const before = right.map((index) => [world.grid[index], world.energy[index]]);
		expect(createExplosions(world).blast(3, 3, 5)).toBe(true);
		for (const [offset, index] of right.entries()) {
			expect([world.grid[index], world.energy[index]]).toEqual(before[offset]);
			expect(world.velocityX[index]).toBe(0);
			expect(world.velocityY[index]).toBe(0);
		}
		expect(world.grid[5 + 3 * world.width]).toBe(STONE);
		expect(world.grid[4 + 3 * world.width]).toBe(FIRE);
		expect(world.ledger.externalEnergyAdded).toBe(5 * 800);
	});

	test("a flame starts a one-tick chain reaction without creating fuel", () => {
		const world = createWorld(9, 3);
		world.setCell(1 + 1 * world.width, FIRE);
		world.setCell(2 + 1 * world.width, GUNPOWDER);
		world.setCell(3 + 1 * world.width, GUNPOWDER);
		const explosions = createExplosions(world);
		expect(explosions.step()).toBe(1);
		expect(world.grid[2 + 1 * world.width]).toBe(FIRE);
		expect(world.grid[3 + 1 * world.width]).toBe(GUNPOWDER);
		expect(explosions.step()).toBe(1);
		expect(world.grid[3 + 1 * world.width]).toBe(FIRE);
		expect(explosions.step()).toBe(0);
	});

	test("gunpowder ignites at its temperature threshold, not below it", () => {
		const world = createWorld(1, 1);
		world.setCell(0, GUNPOWDER);
		const explosions = createExplosions(world);
		world.energy[0] = energyAtTemperature(GUNPOWDER, 249);
		expect(explosions.step()).toBe(0);
		expect(world.grid[0]).toBe(GUNPOWDER);
		world.energy[0] = energyAtTemperature(GUNPOWDER, 250);
		expect(explosions.step()).toBe(1);
		expect(world.grid[0]).toBe(FIRE);
	});

	test("blast tool records its external source and refreshes the pressure view", () => {
		const sandbox = createSandbox(15, 15, { seed: 7 });
		sandbox.setMaterial(BLAST);
		sandbox.paintCircle(7, 7);
		expect(sandbox.getCell(7, 7).material).toBe(FIRE);
		expect(sandbox.getCellPhysics(7, 7).pressurePa).toBeGreaterThan(AMBIENT_PRESSURE_PA);
		expect(sandbox.getDiagnostics().matterMassKg).toBeGreaterThan(0);
		sandbox.clear();
		expect(sandbox.getCell(7, 7).material).toBe(EMPTY);
		expect(sandbox.getDiagnostics().maximumPressurePa).toBe(AMBIENT_PRESSURE_PA);
	});
});
