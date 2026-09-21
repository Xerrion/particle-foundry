/**
 * Authoritative material catalogue. IDs, physical/phase/reaction data, motion,
 * colours and picker metadata live here. Solvers implement behaviours, not tables
 * of material-specific constants. Shared phase families are declared once below.
 */
import { validateMaterialCatalogue } from "./validate-material-catalogue";

/** Catalogue data is plain objects/arrays. Freeze shared nested profiles as well as entries. */
function freezeData<T>(value: T): T {
	if (value !== null && typeof value === "object" && !Object.isFrozen(value)) {
		for (const child of Object.values(value)) freezeData(child);
		Object.freeze(value);
	}
	return value;
}
export const EMPTY = 0;
export const SAND = 1;
export const WATER = 2;
export const WOOD = 3;
export const OIL = 4;
export const STONE = 5;
export const METAL = 6;
export const PLANT = 7;
export const FIRE = 8;
export const LAVA = 9;
export const SMOKE = 10;
export const STEAM = 11;
export const ICE = 12;
export const GLASS = 13;
export const MOLTEN_METAL = 14;
export const GUNPOWDER = 15;
export const WIRE = 16;
export const BATTERY = 17;
export const GROUND = 18;
export const LAMP = 19;
export const BLAST = 252;
export const COOLER = 253;
export const HEATER = 254;
export const ERASER = 255;

export type Color = readonly [number, number, number];
export type MaterialState = "ambient" | "granular" | "solid" | "liquid" | "gas";
export interface PhaseTransition {
	readonly temperature: number;
	/** Calibrated per-cell enthalpy at each side of the latent interval, not kJ/kg.
	 * Reference: the family's coldest phase at 0 C has zero enthalpy.
	 * Intermediate phases add latent offsets and cellHeatCapacity * delta C. */
	readonly lowerCellEnthalpy: number;
	readonly upperCellEnthalpy: number;
	readonly vaporPressure?: { readonly gasConstant: number; readonly latentJPerKg: number };
	/** Seeded local phase separation per 1/60 s tick; requires enough stored latent heat. */
	readonly nucleationChancePerTick?: number;
}
export interface PhaseFamily {
	readonly phases: readonly number[];
	readonly transitions: readonly PhaseTransition[];
}
export interface WaveProfile {
	readonly tension: number;
	readonly restoring: number;
	readonly damping: number;
	readonly limit: number;
}
export interface GlowProfile {
	readonly startsC: number;
	readonly fullC: number;
	readonly brightAboveC: number;
	readonly warmColor: Color;
	readonly brightColor: Color;
}
export interface MaterialDefinition {
	readonly id: number;
	readonly key: string;
	readonly name: string;
	readonly palette: readonly Color[];
	readonly selectable: boolean;
	readonly shortcut?: string;
	/** Tuned enthalpy transfer per degree difference per 1/60 s tick; not W/(m K).
	 * The thermal solver additionally limits this coefficient for numerical stability. */
	readonly heatTransferCoefficient: number;
	/** Calibrated cell-enthalpy units per degree C, NOT mass-specific heat capacity.
	 * Diagnostics divides cell enthalpy by 1000 for its kJ-like energy ledger. */
	readonly cellHeatCapacity: number;
	/** Dimensionless ordering for cellular swaps, independent of SI mass/buoyancy density. */
	readonly displacementDensity: number;
	readonly densityKgPerM3: number;
	readonly viscosityPas: number;
	readonly molarMassKgPerMol: number;
	readonly state: MaterialState;
	readonly chemicalEnergyKjPerKg: number;
	readonly oxygenMassFraction: number;
	readonly spawnTemperatureC: number;
	readonly ignitionTemperatureC: number;
	readonly phaseFamily?: PhaseFamily;
	readonly transforms?: { readonly temperature: number; readonly target: number };
	/** Movement interval in fixed 1/60 s ticks; spread in cells. */
	readonly flow?: { readonly interval: number; readonly spread: number };
	readonly wave?: WaveProfile;
	readonly gasMotion?: {
		readonly hotTemperature: number;
		readonly hotRise: number;
		/** Chance of one diagonal drift during an otherwise upward movement. */
		readonly risingDriftChance: number;
		/** Chance of a lateral diffusion step when no upward route is available. */
		readonly lateralChance: number;
	};
	readonly lifetimeTicks?: readonly [number, number];
	/** Brush-only ignition gas; independent of fuel-fed flames and generic spawn data. */
	readonly ignitionBrush?: {
		readonly temperatureC: number;
		readonly lifetimeTicks: readonly [number, number];
		readonly fringeChance: number;
	};
	/** Calibrated game-model detonation, funded by this parcel's chemical energy. */
	readonly explosive?: {
		readonly triggerTemperatureC: number;
		readonly radiusCells: number;
		readonly impulseSpeedMPerS: number;
	};
	/** Resistance and optional fixed-potential circuit terminal for the game circuit solver. */
	readonly electrical?: {
		readonly resistanceOhms: number;
		readonly terminal?: "source" | "ground";
		readonly voltageV?: number;
	};
	readonly falls: boolean;
	readonly heatGlow?: GlowProfile;
}

/** Common enthalpy endpoints; each member points to the same family object. */
export const phaseFamilies = freezeData({
	water: {
		phases: [ICE, WATER, STEAM],
		transitions: [
			{ temperature: 0, lowerCellEnthalpy: 0, upperCellEnthalpy: 334 },
			{
				temperature: 100,
				lowerCellEnthalpy: 752,
				upperCellEnthalpy: 3008,
				vaporPressure: { gasConstant: 461.5, latentJPerKg: 2_256_000 },
				nucleationChancePerTick: 0.18,
			},
		],
	},
	metal: {
		phases: [METAL, MOLTEN_METAL],
		transitions: [{ temperature: 1538, lowerCellEnthalpy: 692.1, upperCellEnthalpy: 939.1 }],
	},
	rock: {
		phases: [STONE, LAVA],
		transitions: [{ temperature: 1200, lowerCellEnthalpy: 1008, upperCellEnthalpy: 1408 }],
	},
} as const satisfies Record<string, PhaseFamily>);

const metalGlow: GlowProfile = {
	startsC: 500,
	fullC: 1700,
	brightAboveC: 1300,
	warmColor: [245, 93, 55],
	brightColor: [255, 226, 125],
};

function define(
	data: Pick<
		MaterialDefinition,
		| "id"
		| "key"
		| "name"
		| "palette"
		| "heatTransferCoefficient"
		| "cellHeatCapacity"
		| "displacementDensity"
		| "densityKgPerM3"
		| "state"
	> &
		Partial<MaterialDefinition>,
): MaterialDefinition {
	// Identical object shapes keep the per-cell hot path monomorphic.
	return freezeData({
		id: data.id,
		key: data.key,
		name: data.name,
		palette: data.palette,
		selectable: data.selectable ?? true,
		shortcut: data.shortcut,
		heatTransferCoefficient: data.heatTransferCoefficient,
		cellHeatCapacity: data.cellHeatCapacity,
		displacementDensity: data.displacementDensity,
		densityKgPerM3: data.densityKgPerM3,
		// Zero means not modelled for nonfluids; validation rejects missing fluid inputs.
		viscosityPas: data.viscosityPas ?? 0,
		molarMassKgPerMol: data.molarMassKgPerMol ?? 0,
		// Zero and Infinity deliberately describe noncombustible matter.
		chemicalEnergyKjPerKg: data.chemicalEnergyKjPerKg ?? 0,
		oxygenMassFraction: data.oxygenMassFraction ?? 0,
		state: data.state,
		spawnTemperatureC: data.spawnTemperatureC ?? 22,
		ignitionTemperatureC: data.ignitionTemperatureC ?? Infinity,
		falls: data.falls ?? data.state === "granular",
		heatGlow: data.heatGlow,
		phaseFamily: data.phaseFamily,
		transforms: data.transforms,
		flow: data.flow,
		wave: data.wave,
		gasMotion: data.gasMotion,
		lifetimeTicks: data.lifetimeTicks,
		ignitionBrush: data.ignitionBrush,
		explosive: data.explosive,
		electrical: data.electrical,
	});
}

export const materialDefinitions = Object.freeze({
	[EMPTY]: define({
		id: EMPTY,
		key: "air",
		oxygenMassFraction: 0.232,
		name: "Air",
		palette: [],
		heatTransferCoefficient: 0.025,
		cellHeatCapacity: 1,
		displacementDensity: 0.0012,
		densityKgPerM3: 1.204,
		viscosityPas: 0.0000181,
		state: "ambient",
		molarMassKgPerMol: 0.02897,
		selectable: false,
	}),
	[SAND]: define({
		id: SAND,
		key: "sand",
		name: "Sand",
		palette: [
			[244, 207, 102],
			[226, 183, 73],
			[202, 151, 57],
			[238, 197, 88],
		],
		heatTransferCoefficient: 0.11,
		cellHeatCapacity: 0.8,
		displacementDensity: 2.6,
		densityKgPerM3: 1600,
		viscosityPas: 0,
		state: "granular",
		shortcut: "1",
		transforms: { temperature: 1700, target: GLASS },
	}),
	[WATER]: define({
		id: WATER,
		key: "water",
		name: "Water",
		palette: [
			[38, 145, 198],
			[36, 140, 192],
			[40, 150, 202],
			[35, 137, 189],
		],
		heatTransferCoefficient: 0.34,
		cellHeatCapacity: 4.18,
		displacementDensity: 1,
		densityKgPerM3: 997,
		viscosityPas: 0.00089,
		state: "liquid",
		molarMassKgPerMol: 0.018015,
		shortcut: "2",
		phaseFamily: phaseFamilies.water,
		flow: { interval: 1, spread: 4 },
		wave: { tension: 0.16, restoring: 0.018, damping: 0.987, limit: 4.2 },
	}),
	[WOOD]: define({
		id: WOOD,
		key: "wood",
		name: "Wood",
		palette: [
			[137, 82, 43],
			[113, 64, 37],
			[159, 94, 46],
			[121, 72, 39],
		],
		heatTransferCoefficient: 0.05,
		cellHeatCapacity: 1.7,
		displacementDensity: 0.55,
		densityKgPerM3: 700,
		viscosityPas: 0,
		state: "solid",
		molarMassKgPerMol: 0,
		chemicalEnergyKjPerKg: 16000,
		shortcut: "3",
		ignitionTemperatureC: 300,
	}),
	[OIL]: define({
		id: OIL,
		key: "oil",
		name: "Oil",
		palette: [
			[60, 48, 86],
			[57, 45, 81],
			[64, 51, 89],
			[59, 47, 84],
		],
		heatTransferCoefficient: 0.07,
		cellHeatCapacity: 2,
		displacementDensity: 0.78,
		densityKgPerM3: 850,
		viscosityPas: 0.065,
		state: "liquid",
		molarMassKgPerMol: 0,
		chemicalEnergyKjPerKg: 42000,
		shortcut: "4",
		ignitionTemperatureC: 260,
		flow: { interval: 2, spread: 3 },
		wave: { tension: 0.095, restoring: 0.024, damping: 0.968, limit: 2.8 },
	}),
	[STONE]: define({
		id: STONE,
		key: "stone",
		name: "Stone",
		palette: [
			[82, 99, 105],
			[70, 85, 91],
			[99, 112, 116],
			[77, 91, 96],
		],
		heatTransferCoefficient: 0.18,
		cellHeatCapacity: 0.84,
		displacementDensity: 2.7,
		densityKgPerM3: 2700,
		viscosityPas: 0,
		state: "solid",
		shortcut: "5",
		phaseFamily: phaseFamilies.rock,
	}),
	[METAL]: define({
		id: METAL,
		key: "metal",
		name: "Metal",
		palette: [
			[151, 170, 175],
			[119, 140, 147],
			[180, 194, 196],
			[133, 153, 159],
		],
		heatTransferCoefficient: 0.62,
		cellHeatCapacity: 0.45,
		displacementDensity: 7.8,
		densityKgPerM3: 7800,
		viscosityPas: 0,
		state: "solid",
		shortcut: "6",
		phaseFamily: phaseFamilies.metal,
		heatGlow: metalGlow,
		electrical: { resistanceOhms: 0.8 },
	}),
	[PLANT]: define({
		id: PLANT,
		key: "plant",
		name: "Plant",
		palette: [
			[55, 153, 73],
			[43, 125, 61],
			[75, 179, 85],
			[37, 109, 55],
		],
		heatTransferCoefficient: 0.06,
		cellHeatCapacity: 1.5,
		displacementDensity: 0.4,
		densityKgPerM3: 450,
		viscosityPas: 0,
		state: "solid",
		molarMassKgPerMol: 0,
		chemicalEnergyKjPerKg: 14000,
		shortcut: "7",
		ignitionTemperatureC: 220,
	}),
	[FIRE]: define({
		id: FIRE,
		key: "fire",
		name: "Fire",
		palette: [
			[255, 238, 91],
			[255, 167, 48],
			[255, 96, 38],
			[225, 53, 37],
		],
		heatTransferCoefficient: 0.82,
		cellHeatCapacity: 1,
		displacementDensity: 0.05,
		densityKgPerM3: 0.3,
		viscosityPas: 0.00003,
		state: "gas",
		molarMassKgPerMol: 0.029,
		shortcut: "8",
		spawnTemperatureC: 1900,
		lifetimeTicks: [55, 145],
		ignitionBrush: { temperatureC: 450, lifetimeTicks: [8, 16], fringeChance: 0.2 },
		gasMotion: { hotTemperature: 500, hotRise: 3, risingDriftChance: 0.3, lateralChance: 0.65 },
	}),
	[LAVA]: define({
		id: LAVA,
		key: "lava",
		name: "Lava",
		palette: [
			[255, 92, 44],
			[243, 55, 34],
			[255, 174, 53],
			[177, 35, 40],
		],
		heatTransferCoefficient: 0.48,
		cellHeatCapacity: 0.84,
		displacementDensity: 2.5,
		densityKgPerM3: 2600,
		viscosityPas: 100,
		state: "liquid",
		shortcut: "9",
		spawnTemperatureC: 1500,
		phaseFamily: phaseFamilies.rock,
		flow: { interval: 3, spread: 1 },
		wave: { tension: 0.055, restoring: 0.032, damping: 0.946, limit: 1.8 },
	}),
	[SMOKE]: define({
		id: SMOKE,
		key: "smoke",
		name: "Smoke",
		palette: [
			[65, 78, 82],
			[54, 65, 70],
			[79, 90, 92],
			[45, 57, 62],
		],
		heatTransferCoefficient: 0.03,
		cellHeatCapacity: 1,
		displacementDensity: 0.03,
		densityKgPerM3: 1.1,
		viscosityPas: 0.00002,
		state: "gas",
		molarMassKgPerMol: 0.03,
		selectable: false,
		spawnTemperatureC: 180,
		lifetimeTicks: [90, 240],
		gasMotion: { hotTemperature: 180, hotRise: 1, risingDriftChance: 0.2, lateralChance: 0.35 },
	}),
	[STEAM]: define({
		id: STEAM,
		key: "steam",
		name: "Steam",
		palette: [
			[156, 196, 204],
			[133, 177, 188],
			[183, 213, 217],
			[116, 159, 172],
		],
		heatTransferCoefficient: 0.12,
		cellHeatCapacity: 2,
		displacementDensity: 0.04,
		densityKgPerM3: 0.598,
		viscosityPas: 0.000013,
		state: "gas",
		molarMassKgPerMol: 0.018015,
		selectable: false,
		spawnTemperatureC: 150,
		phaseFamily: phaseFamilies.water,
		gasMotion: { hotTemperature: 100, hotRise: 2, risingDriftChance: 0.15, lateralChance: 0.25 },
	}),
	[ICE]: define({
		id: ICE,
		key: "ice",
		name: "Ice",
		palette: [
			[171, 223, 239],
			[145, 206, 229],
			[210, 242, 250],
			[126, 191, 218],
		],
		heatTransferCoefficient: 0.24,
		cellHeatCapacity: 2.1,
		displacementDensity: 0.92,
		densityKgPerM3: 917,
		viscosityPas: 0,
		state: "solid",
		molarMassKgPerMol: 0.018015,
		spawnTemperatureC: -20,
		phaseFamily: phaseFamilies.water,
		falls: true,
	}),
	[GLASS]: define({
		id: GLASS,
		key: "glass",
		name: "Glass",
		palette: [
			[132, 195, 178],
			[106, 168, 160],
			[180, 225, 207],
			[91, 148, 145],
		],
		heatTransferCoefficient: 0.09,
		cellHeatCapacity: 0.8,
		displacementDensity: 2.5,
		densityKgPerM3: 2500,
		viscosityPas: 0,
		state: "solid",
	}),
	[MOLTEN_METAL]: define({
		id: MOLTEN_METAL,
		key: "moltenMetal",
		name: "Molten Metal",
		palette: [
			[255, 204, 113],
			[255, 166, 71],
			[255, 231, 161],
			[237, 124, 47],
		],
		heatTransferCoefficient: 0.5,
		cellHeatCapacity: 0.82,
		displacementDensity: 7,
		densityKgPerM3: 7000,
		viscosityPas: 0.006,
		state: "liquid",
		spawnTemperatureC: 1700,
		phaseFamily: phaseFamilies.metal,
		flow: { interval: 2, spread: 2 },
		heatGlow: metalGlow,
	}),
	[GUNPOWDER]: define({
		id: GUNPOWDER,
		key: "gunpowder",
		name: "Gunpowder",
		palette: [
			[72, 76, 71],
			[58, 63, 59],
			[90, 88, 77],
			[49, 55, 53],
		],
		heatTransferCoefficient: 0.12,
		cellHeatCapacity: 0.8,
		displacementDensity: 1.7,
		densityKgPerM3: 1700,
		viscosityPas: 0,
		state: "granular",
		chemicalEnergyKjPerKg: 3000,
		ignitionTemperatureC: 250,
		explosive: { triggerTemperatureC: 250, radiusCells: 4, impulseSpeedMPerS: 3 },
	}),
	[WIRE]: define({
		id: WIRE,
		key: "wire",
		name: "Wire",
		palette: [
			[197, 123, 67],
			[228, 154, 91],
			[161, 95, 51],
			[205, 137, 73],
		],
		heatTransferCoefficient: 0.7,
		cellHeatCapacity: 0.9,
		displacementDensity: 2.5,
		densityKgPerM3: 8960,
		state: "solid",
		electrical: { resistanceOhms: 1 },
	}),
	[BATTERY]: define({
		id: BATTERY,
		key: "battery",
		name: "Battery",
		palette: [
			[97, 193, 204],
			[65, 158, 170],
			[118, 216, 224],
			[53, 126, 140],
		],
		heatTransferCoefficient: 0.25,
		cellHeatCapacity: 1.2,
		displacementDensity: 2.5,
		densityKgPerM3: 2000,
		state: "solid",
		chemicalEnergyKjPerKg: 1000,
		electrical: { resistanceOhms: 1, terminal: "source", voltageV: 12 },
	}),
	[GROUND]: define({
		id: GROUND,
		key: "ground",
		name: "Ground",
		palette: [
			[75, 103, 115],
			[89, 119, 132],
			[56, 79, 90],
			[107, 134, 146],
		],
		heatTransferCoefficient: 0.3,
		cellHeatCapacity: 1.2,
		displacementDensity: 2.5,
		densityKgPerM3: 2000,
		state: "solid",
		electrical: { resistanceOhms: 1, terminal: "ground" },
	}),
	[LAMP]: define({
		id: LAMP,
		key: "lamp",
		name: "Lamp",
		palette: [
			[116, 118, 112],
			[134, 136, 125],
			[91, 96, 94],
			[153, 152, 137],
		],
		heatTransferCoefficient: 0.14,
		cellHeatCapacity: 0.65,
		displacementDensity: 2.5,
		densityKgPerM3: 2500,
		state: "solid",
		electrical: { resistanceOhms: 4 },
		heatGlow: {
			startsC: 30,
			fullC: 85,
			brightAboveC: 55,
			warmColor: [255, 199, 102],
			brightColor: [255, 247, 180],
		},
	}),
} satisfies Record<number, MaterialDefinition>);

export type MaterialId = keyof typeof materialDefinitions;

export const toolDefinitions = freezeData([
	{
		id: BLAST,
		key: "blast",
		name: "Blast",
		shortcut: "B",
		palette: [
			[255, 219, 119],
			[245, 91, 48],
		],
	},
	{
		id: ERASER,
		key: "eraser",
		name: "Eraser",
		shortcut: "0",
		palette: [
			[49, 67, 74],
			[20, 36, 43],
		],
	},
	{
		id: HEATER,
		key: "heat",
		name: "Heat",
		shortcut: "H",
		palette: [
			[245, 185, 66],
			[255, 96, 70],
		],
	},
	{
		id: COOLER,
		key: "cool",
		name: "Cool",
		shortcut: "C",
		palette: [
			[92, 225, 230],
			[39, 63, 84],
		],
	},
] as const);

export type ToolId = (typeof toolDefinitions)[number]["id"];

// Small one-time check, including development initialization and headless consumers.
// Run before derived lookups or solvers can use an invalid catalogue.
validateMaterialCatalogue(materialDefinitions, toolDefinitions);

/** Shared reaction policy, in the same catalogue as individual ignition thresholds. */
export const combustionProfile = freezeData({
	/** Calibrated thermal units per burning cell per fixed 1/60 s tick. */
	heatPerTick: 100,
	oxygenKgPerKj: 0.00007,
	ignitionMarginC: 80,
	emittedFlameTemperatureC: 450,
	/** Independent per-fuel-cell emission, avoiding synchronized sheets on hot surfaces. */
	flameEmissionChancePerTick: 0.35,
	emittedLifetimeTicks: [18, 36] as const,
	quenchTemperatureC: 220,
	flameContactFraction: 0.9,
	flameHeatPerTick: 12,
	smokeProbability: 0.72,
	emberPalette: [
		[190, 55, 22],
		[199, 64, 22],
		[208, 73, 22],
		[217, 82, 22],
	] as readonly Color[],
});

export const plantGrowthProfile = Object.freeze({
	chancePerTick: 0.003,
	waterConsumptionChance: 0.25,
});

export const surfaceAppearance = freezeData({
	highlight: [18, 24, 28] as Color,
	shimmerAmplitude: 8,
});

export const MATTER_COUNT = Object.keys(materialDefinitions).length;
export const WAVE_MATERIALS: readonly number[] = Object.freeze(
	Object.values(materialDefinitions)
		.filter((m) => m.wave)
		.map((m) => m.id),
);
/** ID-indexed hot-path view of the SAME complete definitions, not a second physics table.
 * Its numeric input must already be validated. Sparse future IDs retain their indices. */
export const materialsById: readonly MaterialDefinition[] = Object.freeze(
	Object.values(materialDefinitions).reduce<MaterialDefinition[]>((entries, material) => {
		entries[material.id] = material;
		return entries;
	}, []),
);
export const selectableMaterials = Object.freeze(
	Object.values(materialDefinitions).filter((m) => m.selectable),
);
/** Picker membership only: selectable matter plus tools, not all simulated phases. */
export const pickerIds: Readonly<Record<string, MaterialId | ToolId | undefined>> = Object.freeze(
	Object.fromEntries([
		...selectableMaterials.map((material) => {
			const id = material.id;
			if (!isMatterId(id)) throw new Error(`Unknown picker material: ${id}`);
			return [material.key, id] as const;
		}),
		...toolDefinitions.map((tool) => [tool.key, tool.id] as const),
	]),
);
export const materialNames: Readonly<Record<number, string>> = Object.freeze(
	Object.fromEntries(
		[...Object.values(materialDefinitions), ...toolDefinitions].map((m) => [m.id, m.name]),
	),
);
export const palettes: Readonly<Record<number, readonly Color[]>> = Object.freeze(
	Object.fromEntries(
		Object.values(materialDefinitions)
			.filter((m) => m.palette.length)
			.map((m) => [m.id, m.palette]),
	),
);
export function isMatterId(material: number): material is MaterialId {
	return Number.isInteger(material) && Object.hasOwn(materialDefinitions, material);
}
export function physicalProperties(material: number): MaterialDefinition {
	if (!isMatterId(material)) throw new RangeError(`Unknown material: ${material}`);
	return materialDefinitions[material];
}
