import { describe, expect, test } from "bun:test";
import {
	COOLER,
	EMPTY,
	ERASER,
	FIRE,
	GLASS,
	HEATER,
	ICE,
	LAVA,
	METAL,
	MOLTEN_METAL,
	materialNames,
	materialsById,
	OIL,
	PLANT,
	palettes,
	pickerIds,
	SAND,
	SMOKE,
	STEAM,
	STONE,
	WATER,
	WAVE_MATERIALS,
	WOOD,
} from "../src/materials";
import { AMBIENT_PRESSURE_PA } from "../src/physical-scale";
import {
	boilingTemperatureAtPressure,
	createThermalSolver,
	energyAtTemperature,
	initialTemperature,
	phaseFromEnergy,
	temperatureFromEnergy,
} from "../src/thermal";

const matterIds = [
	EMPTY,
	SAND,
	WATER,
	WOOD,
	OIL,
	STONE,
	METAL,
	PLANT,
	FIRE,
	LAVA,
	SMOKE,
	STEAM,
	ICE,
	GLASS,
	MOLTEN_METAL,
];

function energies(materials: Uint8Array, temperatures: number[]): Float64Array<ArrayBuffer> {
	return Float64Array.from(materials, (material, i) =>
		energyAtTemperature(material, temperatures[i]),
	);
}

function total(energy: Float64Array): number {
	return energy.reduce((sum, value) => sum + value, 0);
}

function expectConserved(energy: Float64Array, initialTotal: number): void {
	expect(total(energy)).toBeCloseTo(initialTotal, 7);
}

describe("material integration contract", () => {
	test("preserves IDs and wave materials, and supplies new matter and tool names", () => {
		expect(matterIds).toEqual(Array.from({ length: 15 }, (_, i) => i));
		expect([COOLER, HEATER, ERASER]).toEqual([253, 254, 255]);
		expect(WAVE_MATERIALS).toEqual([WATER, OIL, LAVA]);
		expect(pickerIds).toMatchObject({
			ice: ICE,
			glass: GLASS,
			moltenMetal: MOLTEN_METAL,
			heat: HEATER,
			cool: COOLER,
			eraser: ERASER,
		});
		expect(materialNames[EMPTY]).toBe("Air");
		expect(materialNames[SMOKE]).toBe("Smoke");
		expect(materialNames[STEAM]).toBe("Steam");
		for (const material of [...matterIds, HEATER, COOLER, ERASER]) {
			expect(materialNames[material].length).toBeGreaterThan(0);
		}
		for (const material of matterIds) {
			expect(materialsById[material].cellHeatCapacity).toBeGreaterThan(0);
			expect(materialsById[material].heatTransferCoefficient).toBeGreaterThan(0);
			expect("cooling" in materialsById[material]).toBe(false);
			if (material !== EMPTY) expect(palettes[material]).toHaveLength(4);
		}
	});

	test("uses the specified initial temperatures and round-trips spawn energy", () => {
		const defaults = [22, 22, 22, 22, 22, 22, 22, 22, 1900, 1500, 180, 150, -20, 22, 1700];
		for (const material of matterIds) {
			const temperature = initialTemperature(material);
			const energy = energyAtTemperature(material, temperature);
			expect(temperature).toBe(defaults[material]);
			expect(temperatureFromEnergy(material, energy)).toBeCloseTo(temperature, 10);
			expect(phaseFromEnergy(material, energy)).toBe(material);
		}
	});
});

describe("shared enthalpy curves", () => {
	test("water boiling point follows pressure without changing stored energy", () => {
		expect(boilingTemperatureAtPressure(AMBIENT_PRESSURE_PA)).toBeCloseTo(100, 10);
		expect(boilingTemperatureAtPressure(AMBIENT_PRESSURE_PA * 2)).toBeGreaterThan(100);
		expect(boilingTemperatureAtPressure(AMBIENT_PRESSURE_PA / 2)).toBeLessThan(100);
		const warmWater = energyAtTemperature(WATER, 110);
		expect(phaseFromEnergy(WATER, warmWater, AMBIENT_PRESSURE_PA * 2)).toBe(WATER);
		expect(phaseFromEnergy(WATER, warmWater, AMBIENT_PRESSURE_PA / 2)).toBe(STEAM);
	});

	test("water phases share both latent plateaus and all sensible heat segments", () => {
		const points = [
			[-42, -20],
			[0, 0],
			[167, 0],
			[334, 0],
			[375.8, 10],
			[752, 100],
			[1880, 100],
			[3008, 100],
			[3108, 150],
		];
		for (const material of [ICE, WATER, STEAM]) {
			for (const [energy, temperature] of points) {
				expect(temperatureFromEnergy(material, energy)).toBeCloseTo(temperature, 10);
			}
			for (const temperature of [-100, 0, 22, 99, 100, 200]) {
				expect(
					temperatureFromEnergy(material, energyAtTemperature(material, temperature)),
				).toBeCloseTo(temperature, 10);
			}
		}
	});

	test("initialization chooses phase-specific latent endpoints, not partial latent energy", () => {
		expect(energyAtTemperature(ICE, 0)).toBe(0);
		expect(energyAtTemperature(WATER, 0)).toBe(334);
		expect(energyAtTemperature(STEAM, 0)).toBe(334);
		expect(energyAtTemperature(ICE, 100)).toBe(752);
		expect(energyAtTemperature(WATER, 100)).toBe(752);
		expect(energyAtTemperature(STEAM, 100)).toBe(3008);
		expect(energyAtTemperature(METAL, 1538)).toBe(692.1);
		expect(energyAtTemperature(MOLTEN_METAL, 1538)).toBe(939.1);
		expect(energyAtTemperature(STONE, 1200)).toBe(1008);
		expect(energyAtTemperature(LAVA, 1200)).toBe(1408);
	});

	test("water transitions complete only at the correct endpoints, in both directions", () => {
		let material = ICE;
		for (const [energy, expected] of [
			[-42, ICE],
			[0, ICE],
			[333.999, ICE],
			[334, WATER],
			[752, WATER],
			[3007.999, WATER],
			[3008, STEAM],
			[3200, STEAM],
			[752.001, STEAM],
			[752, WATER],
			[0.001, WATER],
			[0, ICE],
			[-42, ICE],
		]) {
			material = phaseFromEnergy(material, energy);
			expect(material).toBe(expected);
		}
		expect(3008 - 752).toBe(2256);
		expect(phaseFromEnergy(WATER, 167)).toBe(WATER);
		expect(phaseFromEnergy(ICE, 167)).toBe(ICE);
		expect(phaseFromEnergy(WATER, 1880)).toBe(WATER);
		expect(phaseFromEnergy(STEAM, 1880)).toBe(STEAM);
	});

	test("large energy jumps cross multiple water phases directly", () => {
		for (const material of [ICE, WATER, STEAM]) {
			expect(phaseFromEnergy(material, -1000)).toBe(ICE);
			expect(phaseFromEnergy(material, 0)).toBe(ICE);
			expect(phaseFromEnergy(material, 500)).toBe(WATER);
			expect(phaseFromEnergy(material, 3008)).toBe(STEAM);
			expect(phaseFromEnergy(material, 9000)).toBe(STEAM);
		}
		expect(phaseFromEnergy(ICE, 1880)).toBe(WATER);
		expect(phaseFromEnergy(STEAM, 167)).toBe(WATER);
	});

	for (const [
		solid,
		liquid,
		meltingPoint,
		solidEnergy,
		liquidEnergy,
		solidCapacity,
		liquidCapacity,
	] of [
		[METAL, MOLTEN_METAL, 1538, 692.1, 939.1, 0.45, 0.82],
		[STONE, LAVA, 1200, 1008, 1408, 0.84, 0.84],
	]) {
		test(`${materialNames[solid]} has reversible melting with shared latent heat`, () => {
			for (const material of [solid, liquid]) {
				expect(temperatureFromEnergy(material, -10 * solidCapacity)).toBeCloseTo(-10, 10);
				expect(temperatureFromEnergy(material, solidEnergy)).toBe(meltingPoint);
				expect(temperatureFromEnergy(material, (solidEnergy + liquidEnergy) / 2)).toBe(
					meltingPoint,
				);
				expect(temperatureFromEnergy(material, liquidEnergy)).toBe(meltingPoint);
				expect(temperatureFromEnergy(material, liquidEnergy + 100 * liquidCapacity)).toBeCloseTo(
					meltingPoint + 100,
					10,
				);
				for (const temperature of [
					-20,
					22,
					meltingPoint - 1,
					meltingPoint,
					meltingPoint + 1,
					2200,
				]) {
					expect(
						temperatureFromEnergy(material, energyAtTemperature(material, temperature)),
					).toBeCloseTo(temperature, 10);
				}
				expect(phaseFromEnergy(material, -1000)).toBe(solid);
				expect(phaseFromEnergy(material, 10000)).toBe(liquid);
			}
			expect(phaseFromEnergy(solid, liquidEnergy - 0.001)).toBe(solid);
			expect(phaseFromEnergy(solid, liquidEnergy)).toBe(liquid);
			expect(phaseFromEnergy(liquid, solidEnergy + 0.001)).toBe(liquid);
			expect(phaseFromEnergy(liquid, solidEnergy)).toBe(solid);
		});
	}

	test("sand vitrifies at 1700 C with no energy or temperature jump, irreversibly", () => {
		expect(phaseFromEnergy(SAND, 1360 - 0.001)).toBe(SAND);
		expect(phaseFromEnergy(SAND, 1360)).toBe(GLASS);
		for (const temperature of [-200, 0, 22, 1699, 1700, 3000]) {
			const energy = energyAtTemperature(SAND, temperature);
			expect(energy).toBeCloseTo(0.8 * temperature, 10);
			expect(energyAtTemperature(GLASS, temperature)).toBe(energy);
			expect(temperatureFromEnergy(GLASS, energy)).toBeCloseTo(temperature, 10);
			expect(phaseFromEnergy(GLASS, energy)).toBe(GLASS);
		}
	});

	test("phase conversion preserves stored enthalpy and derived temperature", () => {
		for (const original of matterIds) {
			for (const value of [
				-500, 0, 167, 334, 692.1, 752, 939.1, 1008, 1360, 1408, 1880, 3008, 4000,
			]) {
				const materials = new Uint8Array([original]);
				const energy = new Float64Array([value]);
				const before = temperatureFromEnergy(original, value);
				materials[0] = phaseFromEnergy(original, value);
				expect(energy[0]).toBe(value);
				expect(temperatureFromEnergy(materials[0], energy[0])).toBe(before);
				expect(phaseFromEnergy(materials[0], energy[0])).toBe(materials[0]);
			}
		}
	});
});

describe("synchronous conservative diffusion", () => {
	test("exchanges equal/opposite energy between unequal capacities and reaches equilibrium", () => {
		const materials = new Uint8Array([SAND, WOOD]);
		const energy = energies(materials, [100, 0]);
		const solver = createThermalSolver(2, 1);
		solver.diffuse(materials, energy);
		expect(Array.from(energy)).toEqual([75, 5]);
		expectConserved(energy, 80);
		for (let i = 0; i < 1000; i += 1) solver.diffuse(materials, energy);
		expectConserved(energy, 80);
		expect(temperatureFromEnergy(SAND, energy[0])).toBeCloseTo(80 / 2.5, 8);
		expect(temperatureFromEnergy(WOOD, energy[1])).toBeCloseTo(80 / 2.5, 8);
	});

	test("snapshots temperatures: heat cannot cross two edges in one step", () => {
		const materials = new Uint8Array([METAL, METAL, METAL]);
		const energy = energies(materials, [100, 0, 0]);
		createThermalSolver(3, 1).diffuse(materials, energy);
		expect(energy[0]).toBeCloseTo(39.375, 12);
		expect(energy[1]).toBeCloseTo(5.625, 12);
		expect(energy[2]).toBe(0);
		expectConserved(energy, 45);
	});

	test("symmetric neighbors receive equal heat", () => {
		const materials = new Uint8Array(9).fill(METAL);
		const energy = new Float64Array(9);
		energy[4] = energyAtTemperature(METAL, 100);
		createThermalSolver(3, 3).diffuse(materials, energy);
		for (const i of [1, 3, 5, 7]) expect(energy[i]).toBe(energy[1]);
		expect(energy[1]).toBeCloseTo(5.625, 12);
		for (const i of [0, 2, 6, 8]) expect(energy[i]).toBe(0);
		expect(energy[4]).toBeCloseTo(22.5, 12);
		expectConserved(energy, 45);
	});

	test("mirroring and transposition do not change the result beyond rounding", () => {
		const width = 4;
		const height = 3;
		const materials = new Uint8Array([
			WATER,
			ICE,
			METAL,
			EMPTY,
			OIL,
			WOOD,
			FIRE,
			SAND,
			STEAM,
			LAVA,
			STONE,
			MOLTEN_METAL,
		]);
		const original = energies(
			materials,
			[22, -20, 1800, -10, 80, 0, 1900, 1700, 150, 1500, 0, 1700],
		);
		const expected = original.slice();
		createThermalSolver(width, height).diffuse(materials, expected);
		for (const mapIndex of [
			(i: number) => materials.length - 1 - i,
			(i: number) => Math.floor(i / width) * width + width - 1 - (i % width),
			(i: number) => (i % width) * height + Math.floor(i / width),
		]) {
			const mappedMaterials = new Uint8Array(materials.length);
			const mappedEnergy = new Float64Array(materials.length);
			for (let i = 0; i < materials.length; i += 1) {
				mappedMaterials[mapIndex(i)] = materials[i];
				mappedEnergy[mapIndex(i)] = original[i];
			}
			const isTransposed = mapIndex(1) === height;
			createThermalSolver(isTransposed ? height : width, isTransposed ? width : height).diffuse(
				mappedMaterials,
				mappedEnergy,
			);
			for (let i = 0; i < materials.length; i += 1) {
				expect(mappedEnergy[mapIndex(i)]).toBeCloseTo(expected[i], 10);
			}
		}
	});

	test("corner edges are insulated and consecutive rows do not wrap", () => {
		for (const [hotIndex, neighbors] of [
			[2, [1, 5]],
			[0, [1, 3]],
			[8, [5, 7]],
		] as const) {
			const materials = new Uint8Array(9);
			const energy = new Float64Array(9);
			energy[hotIndex] = 100;
			createThermalSolver(3, 3).diffuse(materials, energy);
			for (let i = 0; i < energy.length; i += 1) {
				if (i === hotIndex) expect(energy[i]).toBe(95);
				else if (neighbors.some((neighbor) => neighbor === i)) expect(energy[i]).toBe(2.5);
				else expect(energy[i]).toBe(0);
			}
			expectConserved(energy, 100);
		}
	});

	test("single rows and columns have identical edges, and a single cell is insulated", () => {
		const materials = new Uint8Array([EMPTY, METAL, WATER]);
		const row = energies(materials, [100, 20, 0]);
		const column = row.slice();
		createThermalSolver(3, 1).diffuse(materials, row);
		createThermalSolver(1, 3).diffuse(materials, column);
		expect(column).toEqual(row);
		const energy = new Float64Array([-42]);
		createThermalSolver(1, 1).diffuse(new Uint8Array([ICE]), energy);
		expect(energy[0]).toBe(-42);
	});

	test("air participates throughout the field and fire is not an implicit source", () => {
		const materials = new Uint8Array([FIRE, EMPTY, EMPTY]);
		const energy = energies(materials, [1900, 22, 22]);
		const initialTotal = total(energy);
		const solver = createThermalSolver(3, 1);
		for (let i = 0; i < 20; i += 1) solver.diffuse(materials, energy);
		expect(temperatureFromEnergy(FIRE, energy[0])).toBeLessThan(1900);
		expect(temperatureFromEnergy(EMPTY, energy[1])).toBeGreaterThan(22);
		expect(temperatureFromEnergy(EMPTY, energy[2])).toBeGreaterThan(22);
		expectConserved(energy, initialTotal);
		expect(Array.from(materials)).toEqual([FIRE, EMPTY, EMPTY]);
	});

	test("uniform mixed-material temperatures and latent plateaus are exact equilibria", () => {
		const materials = new Uint8Array(matterIds);
		const energy = energies(
			materials,
			matterIds.map(() => 22),
		);
		const before = energy.slice();
		const solver = createThermalSolver(5, 3);
		for (let i = 0; i < 20; i += 1) solver.diffuse(materials, energy);
		expect(energy).toEqual(before);
		for (const values of [
			[0, 167, 334],
			[752, 1880, 3008],
		]) {
			const latentEnergy = new Float64Array(values);
			createThermalSolver(3, 1).diffuse(new Uint8Array([ICE, WATER, STEAM]), latentEnergy);
			expect(Array.from(latentEnergy)).toEqual(values);
		}
	});

	test("repeated multi-neighbor diffusion preserves local temperature bounds and total energy", () => {
		const width = 5;
		const height = 5;
		const materials = Uint8Array.from(
			{ length: width * height },
			(_, i) => matterIds[i % matterIds.length],
		);
		const energy = energies(
			materials,
			Array.from(materials, (_, i) => (i % 2 === 0 ? -120 : 2400)),
		);
		const initialTotal = total(energy);
		const solver = createThermalSolver(width, height);
		for (let step = 0; step < 250; step += 1) {
			const before = Array.from(energy, (value, i) => temperatureFromEnergy(materials[i], value));
			solver.diffuse(materials, energy);
			for (let i = 0; i < energy.length; i += 1) {
				const neighbors = [before[i]];
				if (i % width > 0) neighbors.push(before[i - 1]);
				if (i % width < width - 1) neighbors.push(before[i + 1]);
				if (i >= width) neighbors.push(before[i - width]);
				if (i + width < energy.length) neighbors.push(before[i + width]);
				materials[i] = phaseFromEnergy(materials[i], energy[i]);
				const temperature = temperatureFromEnergy(materials[i], energy[i]);
				expect(temperature).toBeGreaterThanOrEqual(Math.min(...neighbors) - 1e-9);
				expect(temperature).toBeLessThanOrEqual(Math.max(...neighbors) + 1e-9);
			}
			expectConserved(energy, initialTotal);
		}
	});

	test("solver instances and repeated calls have no hidden field state", () => {
		const materials = new Uint8Array([METAL, WATER, EMPTY, FIRE]);
		const first = energies(materials, [1600, 22, -50, 1900]);
		const second = first.slice();
		const a = createThermalSolver(2, 2);
		const b = createThermalSolver(2, 2);
		for (let i = 0; i < 30; i += 1) {
			a.diffuse(materials, first);
			b.diffuse(materials, second);
			expect(first).toEqual(second);
		}
		const fresh = energies(materials, [1, 2, 3, 4]);
		const repeated = fresh.slice();
		a.diffuse(materials, repeated);
		createThermalSolver(2, 2).diffuse(materials, fresh);
		expect(repeated).toEqual(fresh);
	});

	test("opposite large finite temperatures do not overflow their difference", () => {
		const materials = new Uint8Array([EMPTY, EMPTY]);
		const energy = new Float64Array([1e308, -1e308]);
		createThermalSolver(2, 1).diffuse(materials, energy);
		expect(Number.isFinite(energy[0])).toBe(true);
		expect(energy[0]).toBeLessThan(1e308);
		expect(energy[1]).toBe(-energy[0]);
	});
});

describe("boundary validation", () => {
	test("explicit geometry and time step scale conductive transfer", () => {
		const materials = new Uint8Array([EMPTY, EMPTY]);
		const baseline = energies(materials, [0, 100]);
		const halfStep = baseline.slice();
		const initial = baseline[0];
		createThermalSolver(2, 1).diffuse(materials, baseline);
		createThermalSolver(2, 1, { timeStepSeconds: 1 / 120 }).diffuse(materials, halfStep);
		expect(halfStep[0] - initial).toBeCloseTo((baseline[0] - initial) / 2, 10);
	});

	test("rejects invalid physical scale options", () => {
		for (const options of [
			{ timeStepSeconds: 0 },
			{ cellWidthMeters: -1 },
			{ representedDepthMeters: Number.NaN },
		]) {
			expect(() => createThermalSolver(1, 1, options)).toThrow(RangeError);
		}
	});

	test("rejects invalid phase pressure", () => {
		for (const pressure of [0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
			expect(() => phaseFromEnergy(WATER, 752, pressure)).toThrow(RangeError);
		}
	});

	test("rejects unknown, noninteger, nonfinite and brush-tool material IDs", () => {
		for (const material of [-1, 15, 1.5, NaN, Infinity, -Infinity, HEATER, COOLER, ERASER]) {
			expect(() => initialTemperature(material)).toThrow(RangeError);
			expect(() => energyAtTemperature(material, 22)).toThrow(RangeError);
			expect(() => temperatureFromEnergy(material, 22)).toThrow(RangeError);
			expect(() => phaseFromEnergy(material, 22)).toThrow(RangeError);
		}
	});

	test("rejects nonfinite scalar values and unrepresentable derived values", () => {
		for (const value of [NaN, Infinity, -Infinity]) {
			expect(() => energyAtTemperature(WATER, value)).toThrow(RangeError);
			expect(() => temperatureFromEnergy(WATER, value)).toThrow(RangeError);
			expect(() => phaseFromEnergy(WATER, value)).toThrow(RangeError);
		}
		expect(() => energyAtTemperature(WATER, Number.MAX_VALUE)).toThrow(RangeError);
		expect(() => temperatureFromEnergy(METAL, Number.MAX_VALUE)).toThrow(RangeError);
	});

	test("rejects zero, negative, fractional, nonfinite and oversized dimensions", () => {
		for (const value of [0, -1, 1.5, NaN, Infinity, -Infinity, Number.MAX_SAFE_INTEGER, 2 ** 32]) {
			expect(() => createThermalSolver(value, 1)).toThrow(RangeError);
			expect(() => createThermalSolver(1, value)).toThrow(RangeError);
		}
		expect(() => createThermalSolver(65536, 65536)).toThrow(RangeError);
		expect(() => createThermalSolver(Number.MAX_SAFE_INTEGER, 2)).toThrow(RangeError);
	});

	test("rejects incorrect array types and lengths without partial writes", () => {
		const solver = createThermalSolver(2, 1);
		const materials = new Uint8Array([EMPTY, METAL]);
		const energy = new Float64Array([100, 0]);
		for (const wrong of [[], null, new Int8Array(2), new Uint8ClampedArray(2)]) {
			expect(() => Reflect.apply(solver.diffuse, undefined, [wrong, energy])).toThrow(TypeError);
		}
		for (const wrong of [[], null, new Float32Array(2), new Uint8Array(2)]) {
			expect(() => Reflect.apply(solver.diffuse, undefined, [materials, wrong])).toThrow(TypeError);
		}
		for (const length of [0, 1, 3]) {
			expect(() => solver.diffuse(new Uint8Array(length), energy)).toThrow(RangeError);
			expect(() => solver.diffuse(materials, new Float64Array(length))).toThrow(RangeError);
		}
		expect(Array.from(energy)).toEqual([100, 0]);
		expect(Array.from(materials)).toEqual([EMPTY, METAL]);
	});

	test("rejects invalid cells atomically and recovers on the next valid call", () => {
		const solver = createThermalSolver(2, 1);
		for (const material of [15, HEATER, COOLER, ERASER]) {
			const energy = new Float64Array([100, 0]);
			expect(() => solver.diffuse(new Uint8Array([EMPTY, material]), energy)).toThrow(RangeError);
			expect(Array.from(energy)).toEqual([100, 0]);
		}
		for (const value of [NaN, Infinity, -Infinity, Number.MAX_VALUE]) {
			const energy = new Float64Array([100, value]);
			expect(() => solver.diffuse(new Uint8Array([EMPTY, METAL]), energy)).toThrow(RangeError);
			expect(energy[0]).toBe(100);
			expect(Object.is(energy[1], value)).toBe(true);
		}
		const energy = new Float64Array([100, 0]);
		solver.diffuse(new Uint8Array([EMPTY, EMPTY]), energy);
		expect(Array.from(energy)).toEqual([97.5, 2.5]);
	});

	test("rejects overlapping fields but supports disjoint views and byte offsets", () => {
		const buffer = new ArrayBuffer(32);
		const solver = createThermalSolver(2, 1);
		const energy = new Float64Array(buffer, 8, 2);
		energy.set([100, 0]);
		expect(() => solver.diffuse(new Uint8Array(buffer, 8, 2), energy)).toThrow(RangeError);
		expect(Array.from(energy)).toEqual([100, 0]);
		const materials = new Uint8Array(buffer, 0, 2);
		solver.diffuse(materials, energy);
		expect(Array.from(energy)).toEqual([97.5, 2.5]);
		expect(Array.from(materials)).toEqual([EMPTY, EMPTY]);
	});
});
