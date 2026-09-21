import { describe, expect, test } from "bun:test";
import { createBrush } from "../src/brush";
import { createHydrostatics } from "../src/hydrostatics";
import {
	EMPTY,
	FIRE,
	ICE,
	isMatterId,
	LAVA,
	materialDefinitions,
	materialsById,
	OIL,
	SMOKE,
	STEAM,
	STONE,
	WATER,
	WOOD,
} from "../src/materials";
import { createPhysics } from "../src/physics";
import { createReactions } from "../src/reactions";
import { energyAtTemperature, phaseFromEnergy } from "../src/thermal";
import { createVisualWaves } from "../src/visual-waves";
import { createWorld } from "../src/world";

describe("reported liquid artifacts", () => {
	test("lava falls through oil, levels out, then stops shuffling without losing either liquid", () => {
		const world = createWorld(16, 12);
		for (let y = 0; y < 12; y++) {
			for (let x = 0; x < 16; x++) {
				const material =
					x === 0 || x === 15 || y === 11
						? STONE
						: y >= 7
							? OIL
							: y >= 2 && y <= 4 && x >= 4 && x <= 11
								? LAVA
								: EMPTY;
				const index = x + y * 16;
				world.setCell(index, material);
				// Isothermal, depleted fuel isolates immiscible transport from
				// cooling and combustion; phase and reaction tests cover those.
				world.energy[index] = energyAtTemperature(material, 1200);
				world.chemicalEnergyKj[index] = 0;
			}
		}
		const mass = world.massKg.reduce((a, b) => a + b, 0);
		const energy = world.energy.reduce((a, b) => a + b, 0);
		const physics = createPhysics(world);
		for (let tick = 0; tick < 240; tick++) physics.step();
		expect(world.grid.filter((material) => material === LAVA)).toHaveLength(24);
		expect(world.grid.filter((material) => material === OIL)).toHaveLength(56);
		for (let x = 1; x < 15; x++) {
			expect(world.grid[x + 10 * 16]).toBe(LAVA);
			for (let y = 6; y < 9; y++) expect(world.grid[x + y * 16]).toBe(OIL);
		}
		const settled = world.grid.slice();
		const variation = world.variation.slice();
		for (let tick = 0; tick < 120; tick++) {
			physics.step();
			expect(world.grid).toEqual(settled);
			expect(world.variation).toEqual(variation);
		}
		expect(world.massKg.reduce((a, b) => a + b, 0)).toBeCloseTo(mass, 10);
		expect(Math.abs(world.energy.reduce((a, b) => a + b, 0) - energy)).toBeLessThan(1e-5);
	});

	test("a falling water stroke stays compact with no checkerboard gaps or pinned top", () => {
		const world = createWorld(21, 40);
		for (let y = 2; y < 9; y++) for (let x = 7; x < 14; x++) world.setCell(x + y * 21, WATER);
		const before = world.grid.slice();
		const physics = createPhysics(world);
		for (let tick = 1; tick <= 15; tick++) {
			physics.step();
			for (let y = 0; y < 40; y++)
				for (let x = 0; x < 21; x++) {
					expect(world.grid[x + y * 21]).toBe(y >= tick ? before[x + (y - tick) * 21] : EMPTY);
				}
		}
	});

	test("hydrostatic relaxation never stretches an unsupported falling blob", () => {
		const world = createWorld(9, 20);
		for (let y = 3; y < 8; y++) for (let x = 2; x < 7; x++) world.setCell(x + y * 9, WATER);
		const before = world.grid.slice();
		expect(createHydrostatics(world).step(0)).toBe(0);
		expect(world.grid).toEqual(before);
	});

	test("water poured above oil displaces it and settles into compact separate layers", () => {
		const world = createWorld(20, 30);
		for (let y = 24; y < 30; y++) for (let x = 0; x < 20; x++) world.setCell(x + y * 20, OIL);
		for (let y = 2; y < 8; y++) for (let x = 5; x < 15; x++) world.setCell(x + y * 20, WATER);
		const mass = world.massKg.reduce((a, b) => a + b, 0);
		const energy = world.energy.reduce((a, b) => a + b, 0);
		const physics = createPhysics(world);
		for (let tick = 0; tick < 300; tick++) physics.step();
		expect(world.grid.filter((m) => m === OIL)).toHaveLength(120);
		expect(world.grid.filter((m) => m === WATER)).toHaveLength(60);
		for (let y = 21; y < 30; y++)
			for (let x = 0; x < 20; x++) {
				expect(world.grid[x + y * 20]).toBe(y < 27 ? OIL : WATER);
			}
		expect(world.massKg.reduce((a, b) => a + b, 0)).toBeCloseTo(mass, 10);
		expect(Math.abs(world.energy.reduce((a, b) => a + b, 0) - energy)).toBeLessThan(1e-5);
	});

	test("ripples never hide water beneath oil or paint fake droplets above either", () => {
		const world = createWorld(16, 20);
		for (let y = 8; y < 20; y++)
			for (let x = 0; x < 16; x++) {
				world.setCell(x + y * 16, y < 10 ? OIL : WATER);
			}
		const waves = createVisualWaves(world);
		waves.update();
		waves.disturb(8, WATER, 100);
		waves.disturb(8, OIL, -100);
		for (let tick = 0; tick < 40; tick++) {
			waves.update();
			for (let i = 0; i < world.size; i++) {
				expect(waves.materialAt(i % 16, Math.floor(i / 16), world.grid[i])).toBe(world.grid[i]);
			}
			expect(waves.isSurface(8, 10, WATER)).toBe(false);
		}
	});

	test("a near-boiling parcel does not repeatedly flip phase as its derived pressure changes", () => {
		const world = createWorld(1, 1);
		world.setCell(0, STEAM);
		world.energy[0] = 1880; // Halfway through condensation: still contains latent heat.
		const physics = createPhysics(world);
		for (let tick = 0; tick < 300; tick++) {
			physics.step();
			expect(world.grid[0]).toBe(STEAM);
			expect(world.energy[0]).toBe(1880);
		}
		world.energy[0] = 740;
		physics.step();
		expect(world.grid[0]).toBe(WATER);
		for (let tick = 0; tick < 100; tick++) physics.step();
		expect(world.grid[0]).toBe(WATER);
	});

	test("small pressure fluctuations do not discard incomplete vaporization/condensation", () => {
		for (const pressure of [100_000, 101_325, 102_000, 104_000]) {
			expect(phaseFromEnergy(WATER, 1880, pressure)).toBe(WATER);
			expect(phaseFromEnergy(STEAM, 1880, pressure)).toBe(STEAM);
		}
	});
});

describe("ignition and held painting", () => {
	test("painting fire heats wood without replacing or replenishing its fuel", () => {
		const world = createWorld(9, 9);
		world.setCell(40, WOOD);
		world.chemicalEnergyKj[40] = 3;
		const mass = world.massKg[40];
		const brush = createBrush(world, () => {});
		brush.setSize(1);
		brush.setMaterial(FIRE);
		brush.paintCircle(4, 4);
		expect(world.grid[40]).toBe(WOOD);
		expect(world.massKg[40]).toBe(mass);
		expect(world.chemicalEnergyKj[40]).toBe(3);
		expect(world.temperatureAt(40)).toBeGreaterThan(materialsById[WOOD].ignitionTemperatureC);
		expect(world.ledger.externalEnergyAdded).toBeGreaterThan(0);
		createPhysics(world).step();
		expect(world.grid[40]).toBe(WOOD);
		expect(world.burning[40]).toBe(1);
		expect(world.chemicalEnergyKj[40]).toBeLessThan(3);
	});

	test("direct flame contact ignites an adjacent interior log before the flame rises", () => {
		const world = createWorld(20, 20);
		const fuel = 10 + 10 * 20;
		world.setCell(fuel, WOOD);
		world.setCell(fuel - 1, FIRE);
		createPhysics(world).step();
		expect(world.grid[fuel]).toBe(WOOD);
		expect(world.burning[fuel]).toBe(1);
	});

	test("a burning log stays anchored, emits flames, spreads ignition and spends finite fuel", () => {
		const world = createWorld(21, 30);
		const source = 10 + 22 * 21;
		for (let x = 8; x <= 12; x++) world.setCell(x + 22 * 21, WOOD);
		world.energy[source] = energyAtTemperature(WOOD, 600);
		const physics = createPhysics(world);
		let emitted = false;
		let spread = false;
		for (let tick = 0; tick < 100; tick++) {
			physics.step();
			emitted ||= world.grid.some((material, index) => material === FIRE && index < source - 21);
			spread ||= world.burning[source - 1] === 1 || world.burning[source + 1] === 1;
		}
		expect(emitted).toBe(true);
		expect(spread).toBe(true);
		expect(world.chemicalEnergyKj[source]).toBeLessThan(11.2);
		expect(world.grid[source]).toBe(WOOD);
	});

	test("water quenches fuel; sealed oxygen-depleted fuel does not burn", () => {
		for (const neighbor of [WATER, STONE]) {
			const world = createWorld(3, 3);
			for (let i = 0; i < world.size; i++) world.setCell(i, neighbor);
			world.setCell(4, WOOD);
			world.energy[4] = energyAtTemperature(WOOD, 600);
			const chemical = world.chemicalEnergyKj[4];
			const reactions = createReactions(world);
			reactions.beginStep();
			reactions.update(4);
			expect(world.burning[4]).toBe(0);
			expect(world.chemicalEnergyKj[4]).toBe(chemical);
		}
	});

	test("burning and flame emission conserve thermal plus chemical energy", () => {
		const world = createWorld(9, 9);
		// Force a release to include the flame transfer in the energy audit.
		world.random.next = () => 0;
		world.setCell(40, WOOD);
		world.energy[40] = energyAtTemperature(WOOD, 800);
		const total = () =>
			world.energy.reduce((a, b) => a + b, 0) +
			world.chemicalEnergyKj.reduce((a, b) => a + b, 0) * 1000;
		const before = total();
		const reactions = createReactions(world);
		reactions.beginStep();
		reactions.update(40);
		expect(world.grid.includes(FIRE)).toBe(true);
		expect(total()).toBeCloseTo(before, 8);
	});

	test("repainting existing matter preserves its energy, age, mass and fuel", () => {
		for (const material of [WATER, OIL, FIRE, WOOD]) {
			const world = createWorld(1, 1);
			world.setCell(0, material);
			world.energy[0] = 700;
			world.lifetime[0] = 7;
			world.chemicalEnergyKj[0] = 2;
			const brush = createBrush(world, () => {});
			brush.setMaterial(material);
			brush.paintCircle(0, 0);
			expect(world.energy[0]).toBe(700);
			expect(world.lifetime[0]).toBe(7);
			expect(world.chemicalEnergyKj[0]).toBe(2);
		}
	});

	test("burn state moves with oil and is cleared by replacement and reset", () => {
		const world = createWorld(2, 1);
		world.setCell(0, OIL);
		world.burning[0] = 1;
		world.swap(0, 1);
		expect(Array.from(world.burning)).toEqual([0, 1]);
		world.changeMaterial(1, SMOKE);
		expect(world.burning[1]).toBe(0);
		world.burning[0] = 1;
		world.clear();
		expect(Array.from(world.burning)).toEqual([0, 0]);
	});
});

test("every phase link and material behaviour resolves through the central catalogue", () => {
	expect(materialDefinitions[ICE].phaseFamily).toBe(materialDefinitions[WATER].phaseFamily);
	expect(materialDefinitions[WATER].phaseFamily).toBe(materialDefinitions[STEAM].phaseFamily);
	for (const definition of Object.values(materialDefinitions)) {
		expect(definition.id).toBeGreaterThanOrEqual(0);
		expect(definition.spawnTemperatureC).toBeFinite();
		for (const phase of definition.phaseFamily?.phases ?? []) expect(isMatterId(phase)).toBe(true);
		if (definition.state === "liquid") expect(definition.flow).toBeDefined();
		if (Number.isFinite(definition.ignitionTemperatureC))
			expect(definition.chemicalEnergyKjPerKg).toBeGreaterThan(0);
	}
});
