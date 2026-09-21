import { describe, expect, test } from "bun:test";
import {
	BATTERY,
	EMPTY,
	FIRE,
	ICE,
	isMatterId,
	type MaterialDefinition,
	materialDefinitions,
	materialsById,
	OIL,
	type PhaseFamily,
	palettes,
	phaseFamilies,
	physicalProperties,
	pickerIds,
	STEAM,
	selectableMaterials,
	toolDefinitions,
	WATER,
	WAVE_MATERIALS,
	WIRE,
} from "../src/material-definitions";
import { validateMaterialCatalogue } from "../src/validate-material-catalogue";

function fixture() {
	const definitions: Record<number, MaterialDefinition> = structuredClone(materialDefinitions);
	return {
		definitions,
		tools: structuredClone([...toolDefinitions]),
		edit(id: number, patch: Partial<MaterialDefinition>) {
			definitions[id] = { ...definitions[id], ...patch };
		},
		waterFamily(family: PhaseFamily) {
			for (const id of [ICE, WATER, STEAM])
				definitions[id] = { ...definitions[id], phaseFamily: family };
		},
	};
}

describe("material catalogue contracts", () => {
	test("the complete catalogue validates, including falling solid ice and ambient air", () => {
		expect(() => validateMaterialCatalogue(materialDefinitions, toolDefinitions)).not.toThrow();
		expect(materialDefinitions[ICE].state).toBe("solid");
		expect(materialDefinitions[ICE].falls).toBe(true);
		expect(materialDefinitions[EMPTY].gasMotion).toBeUndefined();
	});

	test.each([
		["registry key", { id: 99 }],
		["key", { key: "oil" }],
		["shortcut", { shortcut: "H" }],
		["cellHeatCapacity", { cellHeatCapacity: 0 }],
		["displacementDensity", { displacementDensity: NaN }],
		["densityKgPerM3", { densityKgPerM3: Infinity }],
		["heatTransferCoefficient", { heatTransferCoefficient: -1 }],
		["fluid needs positive viscosity", { viscosityPas: 0 }],
		["chemicalEnergyKjPerKg", { chemicalEnergyKjPerKg: -1 }],
		["oxygen fraction", { oxygenMassFraction: 1.1 }],
		["ignition temperature", { ignitionTemperatureC: -Infinity }],
		["spawn temperature", { spawnTemperatureC: NaN }],
		["liquid needs flow", { flow: undefined }],
		["flow interval", { flow: { interval: 0.5, spread: 4 } }],
		["lifetime range", { lifetimeTicks: [10, 10] }],
		["transform target", { transforms: { target: 99, temperature: 10 } }],
	] satisfies [string, Partial<MaterialDefinition>][])("rejects invalid %s", (message, patch) => {
		const f = fixture();
		f.edit(WATER, patch);
		expect(() => validateMaterialCatalogue(f.definitions, f.tools)).toThrow(message);
	});

	test.each([
		["gas needs positive molar mass", { molarMassKgPerMol: 0 }],
		["gas needs gasMotion", { gasMotion: undefined }],
		[
			"invalid gasMotion",
			{ gasMotion: { hotTemperature: 100, hotRise: 1, risingDriftChance: 2, lateralChance: 0 } },
		],
	] satisfies [string, Partial<MaterialDefinition>][])(
		"rejects missing gas input: %s",
		(message, patch) => {
			const f = fixture();
			f.edit(STEAM, patch);
			expect(() => validateMaterialCatalogue(f.definitions, f.tools)).toThrow(message);
		},
	);

	test("combustible matter needs fuel, but zero energy is a valid noncombustible sentinel", () => {
		const f = fixture();
		f.edit(OIL, { chemicalEnergyKjPerKg: 0 });
		expect(() => validateMaterialCatalogue(f.definitions, f.tools)).toThrow("chemical energy");
		f.edit(OIL, { ignitionTemperatureC: Infinity });
		expect(() => validateMaterialCatalogue(f.definitions, f.tools)).not.toThrow();
	});

	test("electrical profiles reject invalid resistance and unfunded voltage sources", () => {
		const f = fixture();
		f.edit(WIRE, { electrical: { resistanceOhms: 0 } });
		expect(() => validateMaterialCatalogue(f.definitions, f.tools)).toThrow("positive resistance");
		f.edit(WIRE, { electrical: { resistanceOhms: 1 } });
		f.edit(BATTERY, { chemicalEnergyKjPerKg: 0 });
		expect(() => validateMaterialCatalogue(f.definitions, f.tools)).toThrow("stored energy");
	});

	test("tool IDs cannot overlap matter IDs", () => {
		expect(() =>
			validateMaterialCatalogue(materialDefinitions, [{ id: WATER, key: "new-tool" }]),
		).toThrow("ID");
	});

	test.each([
		["phase/transition count", { ...phaseFamilies.water, transitions: [] }],
		[
			"duplicate phase ID",
			{
				...phaseFamilies.water,
				phases: [ICE, WATER, STEAM, WATER],
				transitions: [...phaseFamilies.water.transitions, phaseFamilies.water.transitions[1]],
			},
		],
		[
			"unknown phase ID",
			{
				...phaseFamilies.water,
				phases: [ICE, WATER, STEAM, 99],
				transitions: [...phaseFamilies.water.transitions, phaseFamilies.water.transitions[1]],
			},
		],
		[
			"invalid latent interval",
			{
				...phaseFamilies.water,
				transitions: [
					phaseFamilies.water.transitions[0],
					{ ...phaseFamilies.water.transitions[1], upperCellEnthalpy: 750 },
				],
			},
		],
		[
			"unordered phase transitions",
			{
				...phaseFamilies.water,
				transitions: [phaseFamilies.water.transitions[1], phaseFamilies.water.transitions[0]],
			},
		],
		[
			"discontinuous phase enthalpy",
			{
				...phaseFamilies.water,
				transitions: [
					phaseFamilies.water.transitions[0],
					{ ...phaseFamilies.water.transitions[1], lowerCellEnthalpy: 753 },
				],
			},
		],
	] satisfies [string, PhaseFamily][])(
		"rejects broken phase relationship: %s",
		(message, family) => {
			const f = fixture();
			f.waterFamily(family);
			expect(() => validateMaterialCatalogue(f.definitions, f.tools)).toThrow(message);
		},
	);

	test("all phase members must point back to the same family object", () => {
		const f = fixture();
		f.edit(STEAM, { phaseFamily: structuredClone(phaseFamilies.water) });
		expect(() => validateMaterialCatalogue(f.definitions, f.tools)).toThrow("point back");
	});

	test("water's sensible interval connects the two latent plateaus", () => {
		const [melt, boil] = phaseFamilies.water.transitions;
		expect(
			melt.upperCellEnthalpy +
				materialDefinitions[WATER].cellHeatCapacity * (boil.temperature - melt.temperature),
		).toBe(boil.lowerCellEnthalpy);
	});

	test("checked lookups narrow actual IDs and picker membership stays unchanged", () => {
		for (const id of Array.from({ length: 20 }, (_, index) => index)) {
			expect(isMatterId(id)).toBe(true);
			if (isMatterId(id)) expect(physicalProperties(id)).toBe(materialDefinitions[id]);
			expect(materialsById[id]).toBe(physicalProperties(id));
		}
		for (const id of [-1, 1.5, NaN, Infinity, 20, 252, 253, 254, 255]) {
			expect(isMatterId(id)).toBe(false);
			expect(() => physicalProperties(id)).toThrow(RangeError);
		}
		expect(pickerIds).toEqual({
			sand: 1,
			water: 2,
			wood: 3,
			oil: 4,
			stone: 5,
			metal: 6,
			plant: 7,
			fire: 8,
			lava: 9,
			ice: 12,
			glass: 13,
			moltenMetal: 14,
			gunpowder: 15,
			wire: 16,
			battery: 17,
			ground: 18,
			lamp: 19,
			blast: 252,
			eraser: 255,
			heat: 254,
			cool: 253,
		});
		expect(pickerIds.steam).toBeUndefined();
	});

	test("shared nested profiles and all exported lookup collections are runtime immutable", () => {
		for (const value of [
			materialDefinitions,
			materialsById,
			selectableMaterials,
			pickerIds,
			palettes,
			toolDefinitions,
			WAVE_MATERIALS,
			phaseFamilies.water,
			phaseFamilies.water.transitions,
			phaseFamilies.water.transitions[1],
			materialDefinitions[FIRE].ignitionBrush,
		])
			expect(Object.isFrozen(value)).toBe(true);
		const transition = phaseFamilies.water.transitions[1];
		expect(Reflect.set(transition, "lowerCellEnthalpy", 999)).toBe(false);
		expect(Reflect.set(selectableMaterials, "0", undefined)).toBe(false);
		expect(materialDefinitions[WATER].phaseFamily).toBe(materialDefinitions[STEAM].phaseFamily);
		expect(transition.lowerCellEnthalpy).toBe(752);
	});
});
