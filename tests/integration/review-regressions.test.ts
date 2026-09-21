import { describe, expect, test } from "bun:test";
import {
	COPPER,
	EMPTY,
	HELIUM,
	HYDROGEN,
	IRON,
	LIQUID_COPPER,
	LIQUID_IRON,
	LIQUID_OXYGEN,
	METAL,
	MOLTEN_METAL,
	materialsById,
	OXYGEN,
	PLANT,
	SOLID_OXYGEN,
	STEAM,
	STONE,
	WATER,
	WOOD,
} from "../../src/materials";
import { createElementReactions } from "../../src/physics/element-reactions";
import { createFluidSolver } from "../../src/physics/fluid-solver";
import { createGasDynamics } from "../../src/physics/gas-dynamics";
import { createReactions } from "../../src/physics/reactions";
import { createSolidMechanics } from "../../src/physics/solid-mechanics";
import { createThermalSolver, energyAtTemperature } from "../../src/physics/thermal";
import { compareConservation, measureWorld } from "../../src/simulation/diagnostics";
import { AMBIENT_PRESSURE_PA } from "../../src/simulation/physical-scale";
import { createWorld } from "../../src/simulation/world";

describe("attached physics audit regressions", () => {
	test("symmetric pressure gives opposing edge motion and a stationary center", () => {
		const w = createWorld(3, 1);
		w.energy[1] = energyAtTemperature(EMPTY, 600, undefined, w.massKg[1]);
		createGasDynamics(w).step();
		expect(w.velocityX[0]).toBeLessThan(0);
		expect(w.velocityX[2]).toBeGreaterThan(0);
		expect(w.velocityX[0]).toBeCloseTo(-w.velocityX[2], 12);
		expect(w.velocityX[1]).toBeCloseTo(0, 12);
	});
	test("pressure exchange conserves unequal-mass momentum and total energy", () => {
		const w = createWorld(2, 1);
		w.setCell(0, HELIUM);
		w.energy[0] = energyAtTemperature(HELIUM, 800, undefined, w.massKg[0]);
		const before = measureWorld(w);
		createGasDynamics(w).step();
		expect(w.massKg[0] * w.velocityX[0] + w.massKg[1] * w.velocityX[1]).toBeCloseTo(0, 15);
		expect(compareConservation(before, measureWorld(w)).energyWithinTolerance).toBe(true);
	});
	test("an upward launched solid moves upward, traverses cells and cannot tunnel through a roof", () => {
		const w = createWorld(1, 12);
		w.setCell(2, STONE);
		w.setCell(8, METAL);
		w.setDynamic(8, true);
		w.velocityY[8] = -3;
		const solver = createSolidMechanics(w);
		solver.step();
		const position = w.grid.indexOf(METAL);
		expect(position).toBeLessThan(8);
		expect(w.velocityY[position]).toBeLessThan(0);
		solver.step();
		expect(w.grid.indexOf(METAL)).toBe(3);
		expect(w.grid[2]).toBe(STONE);
	});
	test("resting contact adds no heat over 600 isolated steps", () => {
		const w = createWorld(1, 2);
		w.setCell(0, STONE);
		w.setCell(1, STONE);
		w.setDynamic(0, true);
		const before = w.energy.slice();
		const solver = createSolidMechanics(w);
		for (let i = 0; i < 600; i++) solver.step();
		expect(w.energy).toEqual(before);
	});
	test("liquid drag records lost kinetic energy as heat", () => {
		const w = createWorld(1, 1);
		w.setCell(0, WATER);
		w.velocityX[0] = 10;
		const before = measureWorld(w);
		createFluidSolver(w).step(0);
		const after = measureWorld(w);
		expect(after.thermalEnergy).toBeGreaterThan(before.thermalEnergy);
		expect(compareConservation(before, after).energyWithinTolerance).toBe(true);
	});
	test("temperature and diffusion respond to actual parcel mass across phases", () => {
		const w = createWorld(2, 1);
		w.setCell(0, WATER);
		w.changeMaterial(0, STEAM);
		w.setCell(1, STEAM);
		for (const i of [0, 1]) w.energy[i] = energyAtTemperature(STEAM, 150, undefined, w.massKg[i]);
		expect(w.energy[0] / w.energy[1]).toBeCloseTo(w.massKg[0] / w.massKg[1], 8);
		expect(w.temperatureAt(0)).toBeCloseTo(150, 10);
		expect(w.temperatureAt(1)).toBeCloseTo(150, 10);
		const before = w.energy.slice();
		createThermalSolver(2, 1).diffuse(w.grid, w.energy, undefined, w.massKg);
		expect(w.energy).toEqual(before);
		for (const i of [0, 1]) w.energy[i] += 1000 * w.massKg[i];
		expect(w.temperatureAt(0)).toBeCloseTo(w.temperatureAt(1), 10);
	});
	test("steam temperature uses the pressure that initialized its enthalpy", () => {
		const w = createWorld(1, 1);
		w.setCell(0, STEAM);
		w.pressurePa[0] = AMBIENT_PRESSURE_PA * 2;
		w.energy[0] = energyAtTemperature(STEAM, 150, w.pressurePa[0], w.massKg[0]);
		expect(w.temperatureAt(0)).toBeCloseTo(150, 10);
	});
	test("liquid inherits subambient cavity pressure", () => {
		const w = createWorld(3, 4);
		for (let i = 0; i < w.size; i++) w.setCell(i, STONE);
		w.setCell(4, EMPTY);
		w.setCell(7, WATER);
		w.energy[4] = energyAtTemperature(EMPTY, -50, undefined, w.massKg[4]);
		createGasDynamics(w).derivePressure();
		expect(w.pressurePa[7]).toBeLessThan(AMBIENT_PRESSURE_PA);
		expect(w.pressurePa[7] - w.pressurePa[4]).toBeCloseTo(997 * 9.80665 * 0.01, 8);
	});
	test("a free molten parcel stays movable after freezing", () => {
		const w = createWorld(1, 4);
		w.setCell(0, MOLTEN_METAL);
		w.energy[0] = energyAtTemperature(METAL, 20, undefined, w.massKg[0]);
		w.applyPhase(0);
		expect(w.grid[0]).toBe(METAL);
		expect(w.dynamic[0]).toBe(1);
		const solver = createSolidMechanics(w);
		for (let i = 0; i < 5; i++) solver.step();
		expect(w.grid.indexOf(METAL)).toBeGreaterThan(0);
	});
	test("growth initializes biomass and fuel as an accounted external source", () => {
		const w = createWorld(3, 5);
		w.setCell(10, PLANT);
		w.setCell(11, WATER);
		w.random.next = () => 0;
		const before = measureWorld(w);
		createReactions(w).update(10);
		expect(w.grid[7]).toBe(PLANT);
		expect(w.massKg[7]).toBeCloseTo(materialsById[PLANT].densityKgPerM3 * 1e-6, 12);
		expect(w.chemicalEnergyKj[7]).toBeGreaterThan(0);
		expect(compareConservation(before, measureWorld(w), true).energyWithinTolerance).toBe(true);
	});
	test("painting is source-aware and gas totals include steam and elemental gases", () => {
		const w = createWorld(3, 1);
		const before = measureWorld(w);
		w.setCell(0, WOOD);
		w.setCell(1, STEAM);
		w.setCell(2, HELIUM);
		const after = measureWorld(w);
		expect(after.gasMassKg).toBeCloseTo(w.massKg[1] + w.massKg[2], 15);
		expect(compareConservation(before, after, true).massWithinTolerance).toBe(true);
		expect(compareConservation(before, after, true).energyWithinTolerance).toBe(true);
	});
});

describe("individually modeled elements", () => {
	test("oxygen cools through both phases without changing mass or oxygen reserve", () => {
		const w = createWorld(1, 1);
		w.setCell(0, OXYGEN);
		const mass = w.massKg[0];
		const oxygen = w.oxygenKg[0];
		for (const [phase, temperature] of [
			[LIQUID_OXYGEN, -190],
			[SOLID_OXYGEN, -230],
			[OXYGEN, 22],
		]) {
			w.energy[0] = energyAtTemperature(phase, temperature, undefined, mass);
			const energy = w.energy[0];
			w.applyPhase(0);
			expect(w.grid[0]).toBe(phase);
			expect(w.energy[0]).toBe(energy);
			expect(w.massKg[0]).toBe(mass);
			expect(w.oxygenKg[0]).toBe(oxygen);
			expect(w.temperatureAt(0)).toBeCloseTo(temperature, 9);
		}
	});
	test("oxygen supports finite fuel combustion and helium does not", () => {
		for (const gas of [OXYGEN, HELIUM]) {
			const w = createWorld(3, 3);
			for (let i = 0; i < w.size; i++) w.setCell(i, STONE);
			w.setCell(4, WOOD);
			w.setCell(1, gas);
			w.energy[4] = energyAtTemperature(WOOD, 600, undefined, w.massKg[4]);
			const fuel = w.chemicalEnergyKj[4];
			const before = measureWorld(w);
			const reactions = createReactions(w);
			reactions.beginStep();
			reactions.update(4);
			expect(w.burning[4]).toBe(gas === OXYGEN ? 1 : 0);
			expect(w.oxygenKg[1]).toBeCloseTo(0, 15);
			if (gas === OXYGEN) expect(w.chemicalEnergyKj[4]).toBeLessThan(fuel);
			else expect(w.chemicalEnergyKj[4]).toBe(fuel);
			expect(compareConservation(before, measureWorld(w)).energyWithinTolerance).toBe(true);
			const remaining = w.chemicalEnergyKj[4];
			reactions.beginStep();
			reactions.update(4);
			expect(w.chemicalEnergyKj[4]).toBe(remaining);
		}
	});
	test("a wall or diagonal-only contact prevents hydrogen oxidation", () => {
		for (const oxygenIndex of [2, 4]) {
			const w = createWorld(3, 2);
			for (let i = 0; i < w.size; i++) w.setCell(i, STONE);
			w.setCell(0, HYDROGEN);
			w.setCell(1, STONE);
			w.setCell(oxygenIndex, OXYGEN);
			w.energy[0] = energyAtTemperature(HYDROGEN, 800, undefined, w.massKg[0]);
			const before = measureWorld(w);
			expect(createElementReactions(w).step()).toBe(0);
			expect(compareConservation(before, measureWorld(w)).energyWithinTolerance).toBe(true);
		}
	});

	test.each([
		[IRON, LIQUID_IRON, 1538],
		[COPPER, LIQUID_COPPER, 1084.62],
	])("metal %i melts and refreezes with retained mass and enthalpy", (solid, liquid, melt) => {
		const w = createWorld(1, 1);
		w.setCell(0, solid);
		const mass = w.massKg[0];
		w.energy[0] = energyAtTemperature(liquid, melt + 10, undefined, mass);
		const heat = w.energy[0];
		w.applyPhase(0);
		expect(w.grid[0]).toBe(liquid);
		expect(w.energy[0]).toBe(heat);
		expect(w.massKg[0]).toBe(mass);
		w.energy[0] = energyAtTemperature(solid, melt - 10, undefined, mass);
		w.applyPhase(0);
		expect(w.grid[0]).toBe(solid);
		expect(w.massKg[0]).toBe(mass);
		expect(w.dynamic[0]).toBe(1);
	});
	test.each([0.5, 1, 2])(
		"hydrogen/oxygen reaction respects limiting mass ratio %s and conserves energy",
		(ratio) => {
			const w = createWorld(2, 1);
			w.setCell(0, HYDROGEN);
			w.setCell(1, OXYGEN);
			const h = w.massKg[0];
			const oxygenRatio = 0.031998 / (2 * 0.002016);
			w.massKg[1] = h * oxygenRatio * ratio;
			w.oxygenKg[1] = w.massKg[1];
			for (let i = 0; i < 2; i++)
				w.energy[i] = energyAtTemperature(w.grid[i], 600, undefined, w.massKg[i]);
			w.velocityX[0] = 2;
			w.velocityX[1] = -1;
			const momentum = w.massKg[0] * w.velocityX[0] + w.massKg[1] * w.velocityX[1];
			const before = measureWorld(w);
			expect(createElementReactions(w).step()).toBe(1);
			expect(w.massKg[0] * w.velocityX[0] + w.massKg[1] * w.velocityX[1]).toBeCloseTo(momentum, 15);
			const after = measureWorld(w);
			expect(compareConservation(before, after).massWithinTolerance).toBe(true);
			expect(compareConservation(before, after).energyWithinTolerance).toBe(true);
			const product = w.grid[0] === STEAM ? 0 : 1;
			expect(w.grid[product]).toBe(STEAM);
			if (ratio < 1) {
				expect(w.grid[0]).toBe(HYDROGEN);
				expect(w.massKg[0]).toBeCloseTo(h * (1 - ratio), 15);
			}
			if (ratio > 1) {
				expect(w.grid[1]).toBe(OXYGEN);
				expect(w.massKg[1]).toBeCloseTo(h * oxygenRatio * (ratio - 1), 15);
				expect(w.oxygenKg[1]).toBeCloseTo(w.massKg[1], 15);
			}
		},
	);
	test("cold reactants and inert helium do not spontaneously react", () => {
		const w = createWorld(2, 1);
		w.setCell(0, HYDROGEN);
		w.setCell(1, OXYGEN);
		expect(createElementReactions(w).step()).toBe(0);
		w.setCell(1, HELIUM);
		w.energy[0] = energyAtTemperature(HYDROGEN, 900, undefined, w.massKg[0]);
		expect(createElementReactions(w).step()).toBe(0);
		expect(w.grid[0]).toBe(HYDROGEN);
	});
});
