import { describe, expect, test } from "bun:test";
import { energyAtTemperature } from "../../../src/legacy/physics/thermal";
import { createVisualWaves } from "../../../src/legacy/rendering/visual-waves";
import { measureWorld } from "../../../src/legacy/simulation/diagnostics";
import { createPhysics } from "../../../src/legacy/simulation/physics";
import { createWorld, type World } from "../../../src/legacy/simulation/world";
import { EMPTY, ICE, METAL, MOLTEN_METAL, STEAM, STONE, WATER } from "../../../src/materials";

const narrowVessel = [
	"#.###.#",
	"#W###.#",
	"#W###.#",
	"#W###.#",
	"#W###.#",
	"#W###W#",
	"#WWWWW#",
	"#######",
];

function waterVessel(rows: readonly string[]): World {
	if (rows.length === 0 || rows.some((row) => row.length !== rows[0].length)) {
		throw new Error("Vessel fixtures must be nonempty rectangles");
	}
	const world = createWorld(rows[0].length, rows.length);
	for (let y = 0; y < world.height; y += 1) {
		for (let x = 0; x < world.width; x += 1) {
			const symbol = rows[y][x];
			if (symbol !== "#" && symbol !== "." && symbol !== "W") {
				throw new Error(`Unknown vessel cell: ${symbol}`);
			}
			world.setCell(x + y * world.width, symbol === "#" ? STONE : symbol === "W" ? WATER : EMPTY);
		}
	}
	return world;
}

function totalEnergy(world: World): number {
	return measureWorld(world).totalTrackedEnergyKj * 1000;
}

function materialCounts(world: World): number[] {
	const counts = new Array<number>(MOLTEN_METAL + 1).fill(0);
	for (const material of world.grid) counts[material] += 1;
	return counts;
}

function waterCount(world: World, firstX: number, lastX: number): number {
	let count = 0;
	for (let y = 0; y < world.height; y += 1) {
		for (let x = firstX; x <= lastX; x += 1) {
			if (world.grid[x + y * world.width] === WATER) count += 1;
		}
	}
	return count;
}

function surfaceRows(world: World, columns: readonly number[]): number[] {
	return columns.map((x) => {
		for (let y = 0; y < world.height; y += 1) {
			if (world.grid[x + y * world.width] === WATER) return y;
		}
		throw new Error(`Vessel column ${x} has lost all water`);
	});
}

function advanceVessel(world: World, steps: number): void {
	const physics = createPhysics(world);
	const energy = totalEnergy(world);
	const counts = materialCounts(world);
	const particles = world.getParticleCount();
	const walls = Array.from(world.grid.keys()).filter((index) => world.grid[index] === STONE);
	for (let step = 0; step < steps; step += 1) {
		physics.step();
		expect(materialCounts(world)).toEqual(counts);
		expect(world.getParticleCount()).toBe(particles);
		expect(Math.abs(totalEnergy(world) - energy)).toBeLessThan(1e-3);
		for (const index of walls) expect(world.grid[index]).toBe(STONE);
	}
}

describe("full physics pipeline in communicating vessels", () => {
	for (const { name, rows, columns, expectedWater } of [
		{
			name: "one-cell arms",
			rows: narrowVessel,
			columns: [1, 5],
			expectedWater: 11,
		},
		{
			name: "unequal-width arms",
			rows: [
				"#..###...#",
				"#WW###...#",
				"#WW###...#",
				"#WW###...#",
				"#WW###...#",
				"#WW###WWW#",
				"#WWWWWWWW#",
				"##########",
			],
			columns: [1, 2, 6, 7, 8],
			expectedWater: 21,
		},
	]) {
		test(`${name} reach equal head within one pixel without losing heat or crossing walls`, () => {
			const world = waterVessel(rows);
			for (const x of columns) expect(world.grid[x]).toBe(EMPTY);
			for (let i = 0; i < world.size; i += 1) {
				if (world.grid[i] === WATER) {
					world.energy[i] = energyAtTemperature(
						WATER,
						20 + (i % world.width) * 3,
						undefined,
						world.massKg[i],
					);
				}
			}
			expect(waterCount(world, 0, world.width - 1)).toBe(expectedWater);
			const before = surfaceRows(world, columns);
			expect(Math.max(...before) - Math.min(...before)).toBe(4);
			advanceVessel(world, 240);
			const heads = surfaceRows(world, columns);
			expect(
				Math.max(...heads) - Math.min(...heads),
				`Surface rows after 240 ticks: ${heads.join(", ")}`,
			).toBeLessThanOrEqual(1);
			for (const y of heads) expect(y).toBeGreaterThanOrEqual(3);
			for (const y of heads) expect(y).toBeLessThanOrEqual(4);
		});
	}

	test("sealing the connecting channel preserves unequal heads", () => {
		const world = waterVessel(narrowVessel);
		world.setCell(3 + 6 * world.width, STONE);
		const before = world.grid.slice();
		expect(surfaceRows(world, [1, 5])).toEqual([1, 5]);
		advanceVessel(world, 240);
		expect(world.grid).toEqual(before);
		expect(surfaceRows(world, [1, 5])).toEqual([1, 5]);
	});

	test("separate U-tubes each settle without exchanging their different water volumes", () => {
		const world = waterVessel([
			"#.###.###.###.#",
			"#W###.###.###W#",
			"#W###.###.###W#",
			"#W###.###.###W#",
			"#W###.###W###W#",
			"#W###W###W###W#",
			"#WWWWW###WWWWW#",
			"###############",
		]);
		expect(waterCount(world, 1, 5)).toBe(11);
		expect(waterCount(world, 9, 13)).toBe(12);
		advanceVessel(world, 240);
		expect(waterCount(world, 1, 5)).toBe(11);
		expect(waterCount(world, 9, 13)).toBe(12);
		expect(surfaceRows(world, [1, 5])).toEqual([3, 3]);
		expect(surfaceRows(world, [9, 13]).sort()).toEqual([2, 3]);
	});

	test("pools touching opposite grid edges do not connect across row boundaries", () => {
		const world = waterVessel([
			".#####.",
			".#####W",
			".#####W",
			".#####W",
			".#####W",
			"W#####W",
			"W#####W",
			"#######",
		]);
		const before = world.grid.slice();
		expect(waterCount(world, 0, 0)).toBe(2);
		expect(waterCount(world, 6, 6)).toBe(6);
		advanceVessel(world, 240);
		expect(world.grid).toEqual(before);
		expect(surfaceRows(world, [0, 6])).toEqual([5, 1]);
	});
});

describe("passive phase and transport ordering", () => {
	for (const [name, material, energy, surroundings, phase, destinationY] of [
		["freezing water", WATER, 1, -20, ICE, 3],
		["melting ice", ICE, 333, 22, WATER, 3],
		["boiling water", WATER, 3007, 150, STEAM, 1],
		["condensing steam", STEAM, 600, 22, WATER, 3],
		["solidifying metal", MOLTEN_METAL, 600, 22, METAL, 3],
	] as const) {
		test(`${name} uses the new phase for movement and preserves stored energy`, () => {
			const world = waterVessel(["###", "#.#", "#W#", "#.#", "###"]);
			const center = 1 + 2 * world.width;
			world.setCell(center, material);
			for (let i = 0; i < world.size; i += 1) {
				world.energy[i] = energyAtTemperature(
					world.grid[i],
					surroundings,
					undefined,
					world.massKg[i],
				);
			}
			// A subcell mass avoids placing a full liquid gram into a sealed three-cell gas cavity.
			if (name === "boiling water") {
				world.massKg[center] = 0.6e-6;
				world.volumeM3[center] = world.massKg[center] / 997;
			}
			world.energy[center] = (energy * world.massKg[center]) / 0.001;
			const initialEnergy = totalEnergy(world);
			const count = world.getParticleCount();
			const physics = createPhysics(world);
			for (let step = 0; step < 120; step += 1) {
				physics.step();
				// Trapped hot air briefly raises the boiling threshold before the
				// water has enough heat to complete its phase change.
				if (step < 4) continue;
				expect(world.grid[1 + destinationY * world.width]).toBe(phase);
				expect(world.grid.filter((value) => value === phase)).toHaveLength(1);
				expect(world.getParticleCount()).toBe(count);
				expect(Math.abs(totalEnergy(world) - initialEnergy)).toBeLessThan(1e-2);
			}
		});
	}
});

test("active visual waves and disabled waves produce identical physical trajectories headlessly", () => {
	const first = waterVessel(narrowVessel);
	const second = waterVessel(narrowVessel);
	const withWaves = createPhysics(first);
	const withoutWaves = createPhysics(second);
	const enabled = createVisualWaves(first);
	const disabled = createVisualWaves(second);
	enabled.update();
	disabled.setEnabled(false);
	enabled.disturb(1, WATER, 2, 0);
	disabled.disturb(1, WATER, 2, 0);
	enabled.update();
	disabled.update();
	expect(enabled.materialAt(1, 1, WATER)).toBe(WATER);
	expect(enabled.shimmerAt(1, 1, WATER)).not.toBe(0);
	expect(disabled.materialAt(1, 1, WATER)).toBe(WATER);
	for (let step = 0; step < 240; step += 1) {
		withWaves.step();
		withoutWaves.step();
		enabled.disturb(1, WATER, step % 2 === 0 ? 1 : -1, 0);
		enabled.update();
		disabled.update();
		for (let i = 0; i < first.size; i += 1) {
			const x = i % first.width;
			const y = Math.floor(i / first.width);
			enabled.isSurface(x, y, enabled.materialAt(x, y, first.grid[i]));
			disabled.isSurface(x, y, disabled.materialAt(x, y, second.grid[i]));
		}
		expect(first.grid).toEqual(second.grid);
		expect(first.energy).toEqual(second.energy);
		expect(first.lifetime).toEqual(second.lifetime);
		expect(first.moved).toEqual(second.moved);
		expect(first.getParticleCount()).toBe(second.getParticleCount());
	}
});
