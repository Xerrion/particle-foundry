import { describe, expect, test } from "bun:test";
import { createExplosions } from "../../../src/legacy/physics/explosions";
import { createGasDynamics } from "../../../src/legacy/physics/gas-dynamics";
import { createReactions } from "../../../src/legacy/physics/reactions";
import { energyAtTemperature } from "../../../src/legacy/physics/thermal";
import { AMBIENT_PRESSURE_PA } from "../../../src/legacy/simulation/physical-scale";
import { createPhysics } from "../../../src/legacy/simulation/physics";
import { createSandbox } from "../../../src/legacy/simulation/sandbox";
import { createWorld, type World } from "../../../src/legacy/simulation/world";
import { BLAST, EMPTY, FIRE, GLASS, GUNPOWDER, SMOKE, STONE } from "../../../src/materials";

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

function glassVessel(size: number): World {
	const world = createWorld(size, size);
	for (let y = 0; y < size; y += 1) {
		for (let x = 0; x < size; x += 1) {
			if (x === 0 || x === size - 1 || y === 0 || y === size - 1)
				world.setCell(x + y * size, GLASS);
		}
	}
	return world;
}

describe("energy-funded local explosions", () => {
	test("gunpowder releases finite chemical energy into heat and outward motion", () => {
		const world = createWorld(13, 13);
		const center = 6 + 6 * world.width;
		world.setCell(center, GUNPOWDER);
		const beforeMass = world.massKg.reduce((sum, mass) => sum + mass, 0);
		const beforeEnergy = trackedEnergyJ(world);
		const sourceEnergy = world.ledger.externalEnergyAdded;
		const releasedJ = world.chemicalEnergyKj[center] * 1000;
		expect(createExplosions(world).detonate(center)).toBe(true);
		expect(world.grid[center]).toBe(FIRE);
		expect(world.chemicalEnergyKj[center]).toBe(0);
		expect(world.grid.filter((material) => material === FIRE).length).toBeGreaterThan(1);
		expect(world.velocityX[center - 1]).toBeLessThan(0);
		expect(world.velocityX[center + 1]).toBeGreaterThan(0);
		expect(world.massKg.reduce((sum, mass) => sum + mass, 0)).toBeCloseTo(beforeMass, 12);
		expect(trackedEnergyJ(world)).toBeCloseTo(beforeEnergy, 7);
		expect(world.ledger.externalEnergyAdded).toBe(sourceEnergy);
		createGasDynamics(world).derivePressure();
		expect(world.pressurePa[center]).toBeGreaterThan(AMBIENT_PRESSURE_PA);
		expect(releasedJ).toBeGreaterThan(0);
	});

	test("an anchored wall blocks blast heat, flame and motion", () => {
		const world = createWorld(11, 7);
		for (let y = 0; y < world.height; y += 1) world.setCell(5 + y * world.width, STONE);
		const right = Array.from({ length: world.height }, (_, y) => 6 + y * world.width);
		const before = right.map((index) => [world.grid[index], world.energy[index]]);
		const sourceEnergy = world.ledger.externalEnergyAdded;
		expect(createExplosions(world).blast(3, 3, 5)).toBe(true);
		for (const [offset, index] of right.entries()) {
			expect([world.grid[index], world.energy[index]]).toEqual(before[offset]);
			expect(world.velocityX[index]).toBe(0);
			expect(world.velocityY[index]).toBe(0);
		}
		expect(world.grid[5 + 3 * world.width]).toBe(STONE);
		expect(world.grid[4 + 3 * world.width]).toBe(FIRE);
		expect(world.ledger.externalEnergyAdded - sourceEnergy).toBe(5 * 800);
	});

	test("glass contains shared blast pressure through fire and smoke expiry", () => {
		const world = glassVessel(9);
		const center = 4 + 4 * 9;
		const cornerAir = 1 + 1 * 9;
		world.setCell(center, GUNPOWDER);
		expect(createExplosions(world).detonate(center)).toBe(true);
		const gas = createGasDynamics(world);
		gas.derivePressure();
		const blastPressure = world.pressurePa[center];
		expect(blastPressure).toBeGreaterThan(AMBIENT_PRESSURE_PA);
		expect(world.pressurePa[cornerAir]).toBeCloseTo(blastPressure, 8);
		expect(world.pressurePa[0]).toBe(0);

		const gasVolume = world.volumeM3[center];
		world.changeMaterial(center, SMOKE);
		expect(world.volumeM3[center]).toBeGreaterThan(gasVolume);
		gas.derivePressure();
		expect(world.pressurePa[cornerAir]).toBeGreaterThan(blastPressure * 0.9);

		world.lifetime[center] = 1;
		const reactions = createReactions(world);
		reactions.beginStep();
		const mass = world.massKg[center];
		reactions.update(center);
		expect(world.grid[center]).toBe(EMPTY);
		expect(world.massKg[center]).toBe(mass);
		gas.derivePressure();
		expect(world.pressurePa[cornerAir]).toBeGreaterThan(blastPressure * 0.9);
	});

	test("a sealed glass blast retains overpressure after gas lifetimes expire", () => {
		const world = glassVessel(9);
		const center = 4 + 4 * 9;
		world.setCell(center, GUNPOWDER);
		const physics = createPhysics(world);
		expect(physics.blast(4, 4, 2)).toBe(true);
		for (let tick = 0; tick < 450; tick += 1) physics.step();
		expect(world.pressurePa[1 + 1 * 9]).toBeGreaterThan(AMBIENT_PRESSURE_PA);
		expect(world.pressurePa[4 + 4 * 9]).toBeCloseTo(world.pressurePa[1 + 1 * 9], 8);
		expect(world.grid[0]).toBe(GLASS);
	});

	test("blast motion decays while remaining hot air retains thermodynamic pressure", () => {
		const world = createWorld(80, 50, { seed: 7 });
		for (let y = 35; y < 38; y += 1) {
			for (let x = 25; x < 55; x += 1) world.setCell(x + y * world.width, GUNPOWDER);
		}
		world.setCell(24 + 36 * world.width, FIRE);
		const physics = createPhysics(world);
		for (let tick = 0; tick < 480; tick += 1) physics.step();
		let maximumSpeed = 0;
		for (let index = 0; index < world.size; index += 1) {
			maximumSpeed = Math.max(
				maximumSpeed,
				Math.hypot(world.velocityX[index], world.velocityY[index]),
			);
		}
		expect(maximumSpeed).toBeLessThan(2);
		expect(world.grid.every((material) => material === EMPTY)).toBe(true);
		for (let i = 0; i < world.size; i++) {
			expect(world.pressurePa[i]).toBeCloseTo(
				(AMBIENT_PRESSURE_PA * (world.temperatureAt(i) + 273.15)) / (22 + 273.15),
				4,
			);
		}
	});

	test("opening a glass vessel vents its air without coupling through intact glass", () => {
		const world = glassVessel(7);
		const gas = createGasDynamics(world);
		gas.derivePressure();
		expect(world.pressurePa[3 + 3 * 7]).toBeCloseTo(AMBIENT_PRESSURE_PA, 8);
		world.setCell(3 + 3 * 7, FIRE);
		gas.derivePressure();
		expect(world.pressurePa[1 + 1 * 7]).toBeGreaterThan(AMBIENT_PRESSURE_PA);
		world.setCell(3, EMPTY);
		gas.derivePressure();
		expect(world.pressurePa[1 + 1 * 7]).toBe(AMBIENT_PRESSURE_PA);
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
		world.energy[0] = energyAtTemperature(GUNPOWDER, 249, undefined, world.massKg[0]);
		expect(explosions.step()).toBe(0);
		expect(world.grid[0]).toBe(GUNPOWDER);
		world.energy[0] = energyAtTemperature(GUNPOWDER, 250, undefined, world.massKg[0]);
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
