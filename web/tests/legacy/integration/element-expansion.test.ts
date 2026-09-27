import { describe, expect, test } from "bun:test";
import { createElementReactions } from "../../../src/legacy/physics/element-reactions";
import { energyAtTemperature } from "../../../src/legacy/physics/thermal";
import { compareConservation, measureWorld } from "../../../src/legacy/simulation/diagnostics";
import { createPhysics } from "../../../src/legacy/simulation/physics";
import { createWorld } from "../../../src/legacy/simulation/world";
import { createBrush } from "../../../src/legacy/tools/brush";
import {
	CARBON,
	CARBON_DIOXIDE,
	FIRE,
	HYDROGEN,
	LIQUID_NITROGEN,
	LIQUID_SULFUR,
	NITROGEN,
	OXYGEN,
	SOLID_NITROGEN,
	STEAM,
	STONE,
	SULFUR,
	SULFUR_DIOXIDE,
	SULFUR_VAPOR,
} from "../../../src/materials";

describe("individual carbon, nitrogen and sulfur models", () => {
	test.each([
		[CARBON, CARBON_DIOXIDE, 700],
		[SULFUR, SULFUR_DIOXIDE, 300],
		[LIQUID_SULFUR, SULFUR_DIOXIDE, 300],
		[SULFUR_VAPOR, SULFUR_DIOXIDE, 550],
	])(
		"%i reacts with pure oxygen to form %i without losing mass or energy",
		(fuel, product, temperature) => {
			const w = createWorld(2, 1);
			w.setCell(0, fuel);
			w.setCell(1, OXYGEN);
			// Enough oxygen to exhaust the fuel; the residual remains an O2 parcel.
			const molarMass = fuel === CARBON ? 0.012011 : 0.03206;
			const ratio = 0.031998 / molarMass;
			w.massKg[1] = w.massKg[0] * ratio * 1.5;
			w.oxygenKg[1] = w.massKg[1];
			w.energy[0] = energyAtTemperature(fuel, temperature, undefined, w.massKg[0]);
			w.energy[1] = energyAtTemperature(OXYGEN, temperature, undefined, w.massKg[1]);
			const before = measureWorld(w);
			expect(createElementReactions(w).step()).toBe(1);
			expect(w.grid[0]).toBe(product);
			expect(w.grid[1]).toBe(OXYGEN);
			expect(w.oxygenKg[1]).toBeCloseTo(w.massKg[1], 14);
			const conservation = compareConservation(before, measureWorld(w));
			expect(conservation.massWithinTolerance).toBe(true);
			expect(conservation.energyWithinTolerance).toBe(true);
		},
	);

	test("nitrogen has reversible solid, liquid and gas phases and never oxidizes", () => {
		const w = createWorld(2, 1);
		w.setCell(0, NITROGEN);
		w.setCell(1, OXYGEN);
		const mass = w.massKg[0];
		for (const [phase, temperature] of [
			[LIQUID_NITROGEN, -200],
			[SOLID_NITROGEN, -220],
			[NITROGEN, 22],
		] as const) {
			w.energy[0] = energyAtTemperature(phase, temperature, undefined, mass);
			w.applyPhase(0);
			expect(w.grid[0]).toBe(phase);
			expect(w.massKg[0]).toBe(mass);
		}
		w.energy[0] = energyAtTemperature(NITROGEN, 900, undefined, mass);
		expect(createElementReactions(w).step()).toBe(0);
	});

	test("sulfur melts and vaporizes through its own latent intervals", () => {
		const w = createWorld(1, 1);
		w.setCell(0, SULFUR);
		const mass = w.massKg[0];
		for (const [phase, temperature] of [
			[LIQUID_SULFUR, 200],
			[SULFUR_VAPOR, 500],
			[SULFUR, 22],
		] as const) {
			w.energy[0] = energyAtTemperature(phase, temperature, undefined, mass);
			w.applyPhase(0);
			expect(w.grid[0]).toBe(phase);
			expect(w.massKg[0]).toBe(mass);
		}
	});

	test.each([
		[CARBON, CARBON_DIOXIDE],
		[SULFUR, SULFUR_DIOXIDE],
	] as const)(
		"the Fire brush ignites %i against oxygen in a full physics tick",
		(fuel, product) => {
			const w = createWorld(4, 3);
			for (let i = 0; i < w.size; i++) w.setCell(i, STONE);
			w.setCell(5, fuel);
			w.setCell(6, OXYGEN);
			const brush = createBrush(w, () => {});
			brush.setSize(1);
			brush.setMaterial(FIRE);
			brush.paintCircle(1, 1);
			createPhysics(w).step();
			expect(w.countMaterial(product)).toBe(1);
		},
	);
});

describe("hydrogen ignition", () => {
	test("Fire on oxygen heats its hydrogen contact and starts water production", () => {
		const w = createWorld(2, 1);
		w.setCell(0, HYDROGEN);
		w.setCell(1, OXYGEN);
		const brush = createBrush(w, () => {});
		brush.setSize(1);
		brush.setMaterial(FIRE);
		brush.paintCircle(1, 0);
		expect(w.grid[1]).toBe(OXYGEN);
		expect(w.temperatureAt(1)).toBeGreaterThan(500);
		expect(createElementReactions(w).step()).toBe(1);
	});

	test("the full physics tick ignites a sealed H₂/O₂ pair from the oxygen side", () => {
		const w = createWorld(4, 3);
		for (let i = 0; i < w.size; i++) w.setCell(i, STONE);
		w.setCell(5, HYDROGEN);
		w.setCell(6, OXYGEN);
		const brush = createBrush(w, () => {});
		brush.setSize(1);
		brush.setMaterial(FIRE);
		brush.paintCircle(2, 1);
		createPhysics(w).step();
		expect(w.countMaterial(HYDROGEN)).toBe(0);
		expect(w.countMaterial(STEAM)).toBeGreaterThan(0);
	});

	test("ignited hydrogen burns in connected air with its oxygen source accounted for", () => {
		const w = createWorld(3, 3);
		w.setCell(4, HYDROGEN);
		const before = measureWorld(w);
		w.energy[4] = energyAtTemperature(HYDROGEN, 600, undefined, w.massKg[4]);
		// Heating is an external input independent of the oxygen-source accounting.
		w.ledger.externalEnergyAdded +=
			w.energy[4] - energyAtTemperature(HYDROGEN, 22, undefined, w.massKg[4]);
		const heated = measureWorld(w);
		expect(createElementReactions(w).step()).toBe(1);
		expect(w.grid[4]).not.toBe(HYDROGEN);
		expect(w.ledger.massAddedKg).toBeGreaterThan(heated.ledger.massAddedKg);
		expect(compareConservation(heated, measureWorld(w), true).massWithinTolerance).toBe(true);
		expect(compareConservation(heated, measureWorld(w), true).energyWithinTolerance).toBe(true);
		expect(compareConservation(before, measureWorld(w), true).energyWithinTolerance).toBe(true);
	});

	test("painting Fire onto hydrogen in open air ignites it", () => {
		const w = createWorld(5, 5);
		w.setCell(12, HYDROGEN);
		w.random.next = () => 1; // Keep the sparse flame fringe off adjacent air.
		const brush = createBrush(w, () => {});
		brush.setSize(1);
		brush.setMaterial(FIRE);
		brush.paintCircle(2, 2);
		expect(w.grid[12]).toBe(HYDROGEN);
		expect(w.temperatureAt(12)).toBeGreaterThan(500);
		expect(createElementReactions(w).step()).toBe(1);
		expect(w.grid[12]).not.toBe(HYDROGEN);
	});

	test("hydrogen still ignites when the Fire brush fills every adjacent air cell with flame", () => {
		const w = createWorld(5, 5);
		w.setCell(12, HYDROGEN);
		w.random.next = () => 0;
		const brush = createBrush(w, () => {});
		brush.setSize(1);
		brush.setMaterial(FIRE);
		brush.paintCircle(2, 2);
		for (const index of [7, 11, 13, 17]) expect(w.grid[index]).toBe(FIRE);
		createPhysics(w).step();
		expect(w.countMaterial(HYDROGEN)).toBe(0);
	});

	test("the full physics tick burns a freshly ignited hydrogen parcel", () => {
		const w = createWorld(5, 5);
		w.setCell(12, HYDROGEN);
		w.random.next = () => 1;
		const brush = createBrush(w, () => {});
		brush.setSize(1);
		brush.setMaterial(FIRE);
		brush.paintCircle(2, 2);
		createPhysics(w).step();
		expect(w.countMaterial(HYDROGEN)).toBe(0);
	});

	test("sealed hydrogen cannot pull oxygen through a wall, and oxygen alone never burns", () => {
		const w = createWorld(3, 3);
		for (let i = 0; i < w.size; i++) w.setCell(i, STONE);
		w.setCell(4, HYDROGEN);
		w.energy[4] = energyAtTemperature(HYDROGEN, 800, undefined, w.massKg[4]);
		expect(createElementReactions(w).step()).toBe(0);
		w.setCell(4, OXYGEN);
		w.energy[4] = energyAtTemperature(OXYGEN, 800, undefined, w.massKg[4]);
		expect(createElementReactions(w).step()).toBe(0);
	});
});
