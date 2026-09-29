import { describe, expect, test } from "bun:test";
import { createBoiling } from "../../../src/legacy/physics/boiling";
import { createHydrostatics } from "../../../src/legacy/physics/hydrostatics";
import { energyAtTemperature } from "../../../src/legacy/physics/thermal";
import { measureWorld } from "../../../src/legacy/simulation/diagnostics";
import { AMBIENT_PRESSURE_PA } from "../../../src/legacy/simulation/physical-scale";
import { createPhysics } from "../../../src/legacy/simulation/physics";
import { createWorld } from "../../../src/legacy/simulation/world";
import { EMPTY, METAL, OIL, STEAM, STONE, WATER } from "../../../src/materials";
import { boilingPot } from "../fixtures/boiling-pot";

describe("visible, energy-funded boiling", () => {
	test("partial latent heat forms a complete bubble without creating mass or energy", () => {
		const w = createWorld(3, 3);
		for (const i of [1, 3, 4, 5, 7]) {
			w.setCell(i, WATER);
			w.energy[i] = 1500;
		}
		w.random.next = () => 0;
		const before = w.energy.reduce((a, b) => a + b, 0);
		const mass = w.massKg.slice();
		const ledger = { ...w.ledger };
		expect(createBoiling(w).step(0)).toBe(1);
		const bubble = w.grid.indexOf(STEAM);
		expect(bubble).toBeGreaterThanOrEqual(0);
		expect(w.energy[bubble]).toBe(energyAtTemperature(STEAM, 100, undefined, w.massKg[bubble]));
		for (let i = 0; i < w.size; i++)
			if (w.grid[i] === WATER)
				expect(w.energy[i]).toBeGreaterThanOrEqual(
					energyAtTemperature(WATER, 100, undefined, w.massKg[i]),
				);
		expect(w.energy.reduce((a, b) => a + b, 0)).toBeCloseTo(before, 9);
		expect(w.massKg).toEqual(mass);
		expect(w.ledger).toEqual(ledger);
	});

	test.each([425.96, 752, 800])("insufficient heat (%i) cannot create bubbles", (energy) => {
		const w = createWorld(5, 5);
		for (let i = 0; i < w.size; i++) {
			w.setCell(i, WATER);
			w.energy[i] = energy;
		}
		const before = w.energy.slice();
		w.random.next = () => 0;
		expect(createBoiling(w).step(0)).toBe(0);
		expect(w.energy).toEqual(before);
		expect(w.grid.includes(STEAM)).toBe(false);
	});

	test.each([STONE, OIL, EMPTY])(
		"heat cannot be borrowed across material %i or a diagonal corner",
		(barrier) => {
			const w = createWorld(3, 3);
			for (let i = 0; i < w.size; i++) {
				w.setCell(i, WATER);
				w.energy[i] = 2999;
			}
			for (const i of [1, 3, 5, 7]) w.setCell(i, barrier);
			w.random.next = () => 0;
			expect(createBoiling(w).step(0)).toBe(0);
			expect(w.grid[4]).toBe(WATER);
		},
	);

	test("elevated pressure raises the heat budget rather than producing free vapor", () => {
		const w = createWorld(3, 3);
		for (const i of [1, 3, 4, 5, 7]) {
			w.setCell(i, WATER);
			w.energy[i] = 1500;
		}
		w.random.next = () => 0;
		w.pressurePa.fill(AMBIENT_PRESSURE_PA * 10);
		const solver = createBoiling(w);
		expect(solver.step(0)).toBe(0);
		w.pressurePa.fill(AMBIENT_PRESSURE_PA);
		expect(solver.step(1)).toBe(1);
	});

	test.each([1, 2])(
		"leveling does not teleport a submerged %i-cell bubble to the surface",
		(size) => {
			const w = createWorld(7, 14);
			for (let y = 0; y < 14; y++)
				for (let x = 0; x < 7; x++) {
					const material = x === 0 || x === 6 || y === 13 ? STONE : y >= 3 ? WATER : EMPTY;
					w.setCell(x + y * 7, material);
					w.energy[x + y * 7] = energyAtTemperature(material, 100, undefined, w.massKg[x + y * 7]);
				}
			for (let n = 0; n < size; n++) {
				const i = 3 + (10 - n) * 7;
				w.setCell(i, STEAM);
				w.energy[i] = energyAtTemperature(STEAM, 100, undefined, w.massKg[i]);
			}
			const before = w.grid.slice();
			expect(createHydrostatics(w).step(0)).toBe(0);
			expect(w.grid).toEqual(before);
			createPhysics(w).step();
			const firstY = Math.floor(w.grid.indexOf(STEAM) / 7);
			expect(firstY).toBeGreaterThanOrEqual(10 - size);
			expect(firstY).toBeLessThan(11 - size);
		},
	);

	// Full 420-tick physics runs slow down during parallel Rust builds on shared CI runners.
	test.each([1, 42, 123])(
		"heated pot produces rising bubbles and escaping steam, not sheets (seed %i)",
		(seed) => {
			const { world: w, step } = boilingPot(seed);
			const mass = w.massKg.reduce((a, b) => a + b, 0);
			const energy = measureWorld(w).totalTrackedEnergyKj;
			const externalBefore = w.ledger.externalEnergyAdded;
			let submergedTicks = 0;
			let escapedTicks = 0;
			let maxRun = 0;
			for (let t = 0; t < 420; t++) {
				step();
				if (w.grid.some((id, i) => id === STEAM && i >= 25 * 48)) submergedTicks++;
				if (w.grid.some((id, i) => id === STEAM && i < 25 * 48)) escapedTicks++;
				for (let y = 25; y < 44; y++) {
					let run = 0;
					for (let x = 10; x < 38; x++) {
						run = w.grid[x + y * 48] === STEAM ? run + 1 : 0;
						maxRun = Math.max(maxRun, run);
					}
				}
			}
			expect(submergedTicks).toBeGreaterThan(200);
			expect(escapedTicks).toBeGreaterThan(200);
			expect(maxRun).toBeLessThan(8); // Previously whole 28-cell rows changed phase together.
			expect(w.grid.includes(WATER)).toBe(true);
			expect(w.massKg.reduce((a, b) => a + b, 0)).toBeCloseTo(mass, 9);
			// Pressure work stores some input heat as motion. Cell swaps also
			// approximate gravitational work, so allow sub-joule discretization drift.
			expect(
				Math.abs(
					(measureWorld(w).totalTrackedEnergyKj - energy) * 1000 -
						(w.ledger.externalEnergyAdded - externalBefore),
				),
			).toBeLessThan(1);
			for (let x = 9; x <= 38; x++) expect(w.grid[x + 44 * 48]).toBe(METAL);
		},
		60_000,
	);

	test("a below-boiling hot plate does not create steam", () => {
		const { world, step } = boilingPot(42, 90);
		for (let tick = 0; tick < 360; tick++) step();
		expect(world.grid.includes(STEAM)).toBe(false);
	});

	test("boiling replay is independent of render randomness", () => {
		const a = boilingPot();
		const b = boilingPot();
		for (let t = 0; t < 180; t++) {
			a.step();
			for (let n = 0; n < 17; n++) b.world.visualRandom.next();
			b.step();
		}
		expect(a.world.grid).toEqual(b.world.grid);
		expect(a.world.energy).toEqual(b.world.energy);
	});
});
