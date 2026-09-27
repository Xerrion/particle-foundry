import { describe, expect, test } from "bun:test";
import { createAirflow } from "../../../src/legacy/physics/airflow";
import { createMotion } from "../../../src/legacy/physics/motion";
import { createWorld, type World } from "../../../src/legacy/simulation/world";
import { SMOKE, STONE, WATER } from "../../../src/materials";

function energyWithMotion(world: World): number {
	let total = 0;
	for (let i = 0; i < world.size; i += 1) {
		total += world.energy[i];
		total += 0.5 * world.massKg[i] * (world.velocityX[i] ** 2 + world.velocityY[i] ** 2);
	}
	return total;
}

describe("airflow", () => {
	test("a gust reaches adjacent air and retains its total energy", () => {
		const world = createWorld(5, 3);
		world.velocityX[1 + world.width] = 4;
		const before = energyWithMotion(world);
		createAirflow(world).step(1);
		expect(world.velocityX[2 + world.width]).toBeGreaterThan(0);
		expect(world.velocityX[1 + world.width]).toBeLessThan(4);
		expect(energyWithMotion(world)).toBeCloseTo(before, 10);
	});

	test("wind travels across several air cells over successive ticks", () => {
		const world = createWorld(9, 1);
		world.velocityX[0] = 3;
		const airflow = createAirflow(world);
		for (let tick = 0; tick < 4; tick += 1) airflow.step(tick);
		expect(world.velocityX[4]).toBeGreaterThan(0);
		expect(world.velocityX[4]).toBeLessThan(3);
		expect(world.velocityX[8]).toBe(0);
	});

	test("dense blast gas cannot multiply neighboring air speed", () => {
		const world = createWorld(2, 1);
		world.setCell(0, SMOKE);
		world.massKg[0] = 0.01;
		world.velocityX[0] = 20;
		const before = energyWithMotion(world);
		createAirflow(world).step(0);
		expect(world.velocityX[1]).toBeGreaterThan(0);
		expect(world.velocityX[1]).toBeLessThanOrEqual(20);
		expect(energyWithMotion(world)).toBeCloseTo(before, 8);
	});

	test("a free gust dissipates after its source is gone", () => {
		const world = createWorld(1, 1);
		world.velocityX[0] = 3;
		const before = energyWithMotion(world);
		const airflow = createAirflow(world);
		for (let tick = 0; tick < 300; tick += 1) airflow.step(tick);
		expect(world.velocityX[0]).toBeLessThan(0.01);
		expect(energyWithMotion(world)).toBeCloseTo(before, 10);
	});

	test("wind transfers momentum to nearby liquid without adding energy", () => {
		const world = createWorld(2, 1);
		world.setCell(1, WATER);
		world.velocityX[0] = 3;
		const before = energyWithMotion(world);
		const initialMomentum = world.massKg[0] * world.velocityX[0];
		createAirflow(world).step(0);
		expect(world.velocityX[1]).toBeGreaterThan(0);
		const finalMomentum =
			world.massKg[0] * world.velocityX[0] + world.massKg[1] * world.velocityX[1];
		expect(finalMomentum).toBeGreaterThan(0);
		expect(finalMomentum).toBeLessThan(initialMomentum);
		expect(energyWithMotion(world)).toBeCloseTo(before, 10);
	});

	test("anchored walls block wind from reaching air on the other side", () => {
		const world = createWorld(5, 5);
		for (let y = 0; y < world.height; y += 1) world.setCell(2 + y * world.width, STONE);
		world.velocityX[1 + 2 * world.width] = 4;
		const airflow = createAirflow(world);
		for (let tick = 0; tick < 10; tick += 1) airflow.step(tick);
		for (let y = 0; y < world.height; y += 1) {
			for (const x of [3, 4]) expect(world.velocityX[x + y * world.width]).toBe(0);
		}
	});

	test("moving air carries smoke sideways before its normal rise", () => {
		const world = createWorld(5, 3);
		const smoke = 2 + world.width;
		world.setCell(smoke, SMOKE);
		world.velocityX[smoke - 1] = 4;
		createAirflow(world).step(1);
		expect(world.velocityX[smoke]).toBeGreaterThan(0.15);
		world.random.next = () => 0;
		createMotion(world).update(smoke, 0);
		expect(world.grid[smoke + 1]).toBe(SMOKE);
	});
});
