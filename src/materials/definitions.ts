/**
 * Authoritative material catalogue. IDs, physical/phase/reaction data, motion,
 * colours and picker metadata live here. Solvers implement behaviours, not tables
 * of material-specific constants. Shared phase families are declared once below.
 */
import { validateMaterialCatalogue } from "./validation";

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
export const HYDROCHLORIC_ACID = 20;
export const SULFURIC_ACID = 21;
export const SODIUM_HYDROXIDE = 22;
export const NEUTRAL_SOLUTION = 23;
export const HYDROGEN = 24;
export const OXYGEN = 25;
export const HELIUM = 26;
export const IRON = 27;
export const LIQUID_IRON = 28;
export const IRON_VAPOR = 29;
export const COPPER = 30;
export const LIQUID_COPPER = 31;
export const COPPER_VAPOR = 32;
export const LIQUID_OXYGEN = 33;
export const SOLID_OXYGEN = 34;
export const CARBON = 35;
export const NITROGEN = 36;
export const LIQUID_NITROGEN = 37;
export const SOLID_NITROGEN = 38;
export const SULFUR = 39;
export const LIQUID_SULFUR = 40;
export const SULFUR_VAPOR = 41;
export const CARBON_DIOXIDE = 42;
export const SULFUR_DIOXIDE = 43;
export const BLAST = 252;
export const COOLER = 253;
export const HEATER = 254;
export const ERASER = 255;

export type Color = readonly [number, number, number];
export type MaterialState = "ambient" | "granular" | "solid" | "liquid" | "gas";
export interface PhaseTransition {
	readonly temperature: number;
	/** Enthalpy in joules for the thermal reference mass (one gram by default).
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
	/** Heat capacity in J/K for thermalReferenceMassKg (one gram by default).
	 * Divide by the reference mass to obtain specific heat capacity in J/(kg K). */
	readonly cellHeatCapacity: number;
	/** Mass represented by the thermal curve; defaults to 0.001 kg, shared by a phase family. */
	readonly thermalReferenceMassKg?: number;
	readonly atomicNumber?: number;
	readonly description?: string;
	readonly interactions?: readonly string[];
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
	/** Fully dissolved aqueous preset, in acid/base equivalents; reactions are cell-pair approximations. */
	readonly neutralization?: {
		readonly role: "acid" | "base";
		readonly equivalentsMolPerL: number;
		readonly heatJPerMol?: number;
	};
	readonly falls: boolean;
	readonly heatGlow?: GlowProfile;
}

/** Common enthalpy endpoints; each member points to the same family object. */
export const phaseFamilies = freezeData({
	nitrogen: {
		phases: [SOLID_NITROGEN, LIQUID_NITROGEN, NITROGEN],
		transitions: [
			{ temperature: -210, lowerCellEnthalpy: -210 * 1.6, upperCellEnthalpy: -210 * 1.6 + 25.7 },
			{
				temperature: -195.795,
				lowerCellEnthalpy: -210 * 1.6 + 25.7 + 14.205 * 2.04,
				upperCellEnthalpy: -210 * 1.6 + 25.7 + 14.205 * 2.04 + 199,
			},
		],
	},
	sulfur: {
		phases: [SULFUR, LIQUID_SULFUR, SULFUR_VAPOR],
		transitions: [
			{
				temperature: 115.21,
				lowerCellEnthalpy: 115.21 * 0.708,
				upperCellEnthalpy: 115.21 * 0.708 + 38.1,
			},
			{
				temperature: 444.61,
				lowerCellEnthalpy: 115.21 * 0.708 + 38.1 + (444.61 - 115.21) * 1.2,
				upperCellEnthalpy: 115.21 * 0.708 + 38.1 + (444.61 - 115.21) * 1.2 + 326,
			},
		],
	},
	iron: {
		phases: [IRON, LIQUID_IRON, IRON_VAPOR],
		transitions: [
			{ temperature: 1538, lowerCellEnthalpy: 1538 * 0.449, upperCellEnthalpy: 1538 * 0.449 + 247 },
			{
				temperature: 2861,
				lowerCellEnthalpy: 1538 * 0.449 + 247 + (2861 - 1538) * 0.82,
				upperCellEnthalpy: 1538 * 0.449 + 247 + (2861 - 1538) * 0.82 + 6090,
			},
		],
	},
	copper: {
		phases: [COPPER, LIQUID_COPPER, COPPER_VAPOR],
		transitions: [
			{
				temperature: 1084.62,
				lowerCellEnthalpy: 1084.62 * 0.385,
				upperCellEnthalpy: 1084.62 * 0.385 + 205,
			},
			{
				temperature: 2562,
				lowerCellEnthalpy: 1084.62 * 0.385 + 205 + (2562 - 1084.62) * 0.517,
				upperCellEnthalpy: 1084.62 * 0.385 + 205 + (2562 - 1084.62) * 0.517 + 4730,
			},
		],
	},
	oxygen: {
		phases: [SOLID_OXYGEN, LIQUID_OXYGEN, OXYGEN],
		transitions: [
			{
				temperature: -218.79,
				lowerCellEnthalpy: -218.79 * 1.3,
				upperCellEnthalpy: -218.79 * 1.3 + 13.9,
			},
			{
				temperature: -182.95,
				lowerCellEnthalpy: -218.79 * 1.3 + 13.9 + 35.84 * 1.7,
				upperCellEnthalpy: -218.79 * 1.3 + 13.9 + 35.84 * 1.7 + 213,
			},
		],
	},
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
		thermalReferenceMassKg: data.thermalReferenceMassKg,
		atomicNumber: data.atomicNumber,
		description: data.description,
		interactions: data.interactions,
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
		neutralization: data.neutralization,
	});
}

export const materialDefinitions = Object.freeze({
	[HYDROGEN]: define({
		id: HYDROGEN,
		key: "hydrogen",
		name: "Hydrogen",
		atomicNumber: 1,
		palette: [
			[176, 206, 239],
			[150, 187, 223],
		],
		state: "gas",
		densityKgPerM3: 0.08324,
		displacementDensity: 0.00008324,
		molarMassKgPerMol: 0.002016,
		viscosityPas: 0.0000089,
		cellHeatCapacity: 14.304,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 0.18,
		chemicalEnergyKjPerKg: 241.826 / 0.002016,
		ignitionTemperatureC: 500,
		gasMotion: { hotTemperature: 500, hotRise: 2, risingDriftChance: 0.2, lateralChance: 0.3 },
		description:
			"H₂ · combustible gas. Touch oxygen or open air, then apply Fire or heat to ignite.",
		interactions: [
			"2 H₂ + O₂ → 2 H₂O: consumes measured reactant masses and releases finite heat.",
			"Excess hydrogen or oxygen remains after the reaction. Water vapor cools and condenses.",
			"Hot hydrogen also consumes oxygen from air connected to an open edge. Sealed air-mixture chemistry is not modeled.",
		],
	}),
	[HELIUM]: define({
		id: HELIUM,
		key: "helium",
		name: "Helium",
		atomicNumber: 2,
		palette: [
			[202, 168, 239],
			[177, 145, 211],
		],
		state: "gas",
		densityKgPerM3: 0.1653,
		displacementDensity: 0.0001653,
		molarMassKgPerMol: 0.0040026,
		viscosityPas: 0.0000196,
		cellHeatCapacity: 5.193,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 0.15,
		gasMotion: { hotTemperature: 500, hotRise: 2, risingDriftChance: 0.15, lateralChance: 0.25 },
		description:
			"He · inert, light gas. Rises through air, transfers heat and builds pressure when confined.",
		interactions: [
			"Does not burn or supply oxygen. A sealed helium pocket cannot sustain fuel combustion.",
			"Gas model only; superfluidity, cryogenic liquid helium and ionization are outside the model.",
		],
	}),
	[OXYGEN]: define({
		id: OXYGEN,
		key: "oxygen",
		name: "Oxygen",
		atomicNumber: 8,
		palette: [
			[112, 185, 247],
			[87, 152, 221],
		],
		state: "gas",
		densityKgPerM3: 1.3212,
		displacementDensity: 0.0013212,
		molarMassKgPerMol: 0.031998,
		viscosityPas: 0.0000203,
		cellHeatCapacity: 0.918,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 0.026,
		oxygenMassFraction: 1,
		phaseFamily: phaseFamilies.oxygen,
		gasMotion: { hotTemperature: 500, hotRise: 2, risingDriftChance: 0.1, lateralChance: 0.25 },
		description:
			"O₂ · oxidizer. Supports combustion; it does not burn on its own. Slightly denser than air.",
		interactions: [
			"Touch hot hydrogen to form water vapor. Fire works on either side of an H₂/O₂ contact.",
			"Oxygen supports burning but is not a fuel and will not ignite on its own.",
			"Supplies a finite oxygen reserve to burning wood, plants and oil in sealed spaces.",
			"Cools to liquid at −182.95 °C and solid at −218.79 °C (normal-pressure curves).",
		],
	}),
	[LIQUID_OXYGEN]: define({
		id: LIQUID_OXYGEN,
		key: "liquidOxygen",
		name: "Liquid oxygen",
		atomicNumber: 8,
		selectable: false,
		palette: [
			[99, 157, 232],
			[79, 130, 209],
		],
		state: "liquid",
		densityKgPerM3: 1141,
		displacementDensity: 1.141,
		viscosityPas: 0.000196,
		cellHeatCapacity: 1.7,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 0.15,
		oxygenMassFraction: 1,
		phaseFamily: phaseFamilies.oxygen,
		spawnTemperatureC: -190,
		flow: { interval: 1, spread: 4 },
	}),
	[SOLID_OXYGEN]: define({
		id: SOLID_OXYGEN,
		key: "solidOxygen",
		name: "Solid oxygen",
		atomicNumber: 8,
		selectable: false,
		palette: [
			[134, 183, 245],
			[117, 158, 214],
		],
		state: "solid",
		falls: true,
		densityKgPerM3: 1426,
		displacementDensity: 1.426,
		cellHeatCapacity: 1.3,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 0.2,
		oxygenMassFraction: 1,
		phaseFamily: phaseFamilies.oxygen,
		spawnTemperatureC: -230,
	}),
	[IRON]: define({
		id: IRON,
		key: "iron",
		name: "Iron",
		atomicNumber: 26,
		palette: [
			[160, 170, 180],
			[128, 138, 150],
		],
		state: "solid",
		densityKgPerM3: 7870,
		displacementDensity: 7.87,
		cellHeatCapacity: 0.449,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 0.8,
		phaseFamily: phaseFamilies.iron,
		electrical: { resistanceOhms: 0.097 },
		heatGlow: metalGlow,
		description:
			"Fe · solid conductor. Painted iron stays fixed; molten iron flows and freezes as movable matter.",
		interactions: [
			"Connect Battery → Iron → Lamp → Ground to conduct current and generate heat.",
			"Melts at 1538 °C; vaporizes at 2861 °C after supplying latent heat.",
			"Rust, acid corrosion and alloy formation are not modeled.",
		],
	}),
	[LIQUID_IRON]: define({
		id: LIQUID_IRON,
		key: "liquidIron",
		name: "Liquid iron",
		atomicNumber: 26,
		selectable: false,
		palette: [
			[248, 170, 65],
			[234, 113, 40],
		],
		state: "liquid",
		densityKgPerM3: 6980,
		displacementDensity: 6.98,
		viscosityPas: 0.006,
		cellHeatCapacity: 0.82,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 0.7,
		phaseFamily: phaseFamilies.iron,
		flow: { interval: 1, spread: 3 },
		spawnTemperatureC: 1600,
		heatGlow: metalGlow,
	}),
	[IRON_VAPOR]: define({
		id: IRON_VAPOR,
		key: "ironVapor",
		name: "Iron vapor",
		atomicNumber: 26,
		selectable: false,
		palette: [
			[215, 190, 169],
			[178, 155, 138],
		],
		state: "gas",
		densityKgPerM3: 0.218,
		displacementDensity: 0.000218,
		viscosityPas: 0.00005,
		molarMassKgPerMol: 0.055845,
		cellHeatCapacity: 0.372,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 0.03,
		phaseFamily: phaseFamilies.iron,
		spawnTemperatureC: 2900,
		gasMotion: { hotTemperature: 500, hotRise: 2, risingDriftChance: 0.15, lateralChance: 0.25 },
	}),
	[COPPER]: define({
		id: COPPER,
		key: "copper",
		name: "Copper",
		atomicNumber: 29,
		palette: [
			[205, 127, 78],
			[176, 94, 59],
		],
		state: "solid",
		densityKgPerM3: 8960,
		displacementDensity: 8.96,
		cellHeatCapacity: 0.385,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 4,
		phaseFamily: phaseFamilies.copper,
		electrical: { resistanceOhms: 0.0168 },
		heatGlow: metalGlow,
		description: "Cu · dense conductor. Transfers heat and carries current more readily than iron.",
		interactions: [
			"Connect Battery → Copper → Lamp → Ground to complete a circuit.",
			"Melts at 1084.62 °C; vaporizes at 2562 °C after supplying latent heat.",
			"Oxidation, acid reactions and alloys are not modeled. Circuit resistance is scaled for this sandbox.",
		],
	}),
	[LIQUID_COPPER]: define({
		id: LIQUID_COPPER,
		key: "liquidCopper",
		name: "Liquid copper",
		atomicNumber: 29,
		selectable: false,
		palette: [
			[246, 157, 83],
			[223, 116, 62],
		],
		state: "liquid",
		densityKgPerM3: 8020,
		displacementDensity: 8.02,
		viscosityPas: 0.004,
		cellHeatCapacity: 0.517,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 2,
		phaseFamily: phaseFamilies.copper,
		flow: { interval: 1, spread: 3 },
		spawnTemperatureC: 1150,
		heatGlow: metalGlow,
	}),
	[COPPER_VAPOR]: define({
		id: COPPER_VAPOR,
		key: "copperVapor",
		name: "Copper vapor",
		atomicNumber: 29,
		selectable: false,
		palette: [
			[211, 165, 131],
			[185, 141, 110],
		],
		state: "gas",
		densityKgPerM3: 0.274,
		displacementDensity: 0.000274,
		viscosityPas: 0.00005,
		molarMassKgPerMol: 0.063546,
		cellHeatCapacity: 0.327,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 0.03,
		phaseFamily: phaseFamilies.copper,
		spawnTemperatureC: 2700,
		gasMotion: { hotTemperature: 500, hotRise: 2, risingDriftChance: 0.15, lateralChance: 0.25 },
	}),
	[CARBON]: define({
		id: CARBON,
		key: "carbon",
		name: "Carbon (graphite)",
		atomicNumber: 6,
		palette: [
			[58, 66, 70],
			[81, 89, 91],
		],
		state: "solid",
		falls: true,
		densityKgPerM3: 2200,
		displacementDensity: 2.2,
		molarMassKgPerMol: 0.012011,
		cellHeatCapacity: 0.709,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 1.5,
		chemicalEnergyKjPerKg: 393.52 / 0.012011,
		ignitionTemperatureC: 650,
		description: "C · graphite. A hot grain consumes touching oxygen and releases carbon dioxide.",
		interactions: [
			"Fire or Heat can start C + O₂ → CO₂ at 650 °C. The gas is a separate, oxygen-free product.",
			"Reaction consumes the limiting reactant and retains leftover carbon or oxygen.",
			"Graphite is modeled; diamond, carbon monoxide and sublimation are not.",
		],
	}),
	[NITROGEN]: define({
		id: NITROGEN,
		key: "nitrogen",
		name: "Nitrogen",
		atomicNumber: 7,
		palette: [
			[149, 188, 239],
			[125, 165, 220],
		],
		state: "gas",
		densityKgPerM3: 1.145,
		displacementDensity: 0.001145,
		molarMassKgPerMol: 0.028014,
		viscosityPas: 0.0000176,
		cellHeatCapacity: 1.04,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 0.026,
		phaseFamily: phaseFamilies.nitrogen,
		gasMotion: { hotTemperature: 400, hotRise: 2, risingDriftChance: 0.1, lateralChance: 0.25 },
		description: "N₂ · mostly inert gas. Carries heat and pressure but supplies no oxygen to fire.",
		interactions: [
			"Displaces oxygen locally, so a sealed nitrogen pocket cannot sustain fuel combustion.",
			"Liquefies at −195.795 °C and freezes at −210 °C on the normal-pressure phase curve.",
			"Nitrogen fixation, ammonia and nitrogen oxides are not modeled.",
		],
	}),
	[LIQUID_NITROGEN]: define({
		id: LIQUID_NITROGEN,
		key: "liquidNitrogen",
		name: "Liquid nitrogen",
		atomicNumber: 7,
		selectable: false,
		palette: [
			[91, 149, 220],
			[71, 127, 204],
		],
		state: "liquid",
		densityKgPerM3: 808,
		displacementDensity: 0.808,
		viscosityPas: 0.00016,
		cellHeatCapacity: 2.04,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 0.14,
		phaseFamily: phaseFamilies.nitrogen,
		spawnTemperatureC: -200,
		flow: { interval: 1, spread: 4 },
	}),
	[SOLID_NITROGEN]: define({
		id: SOLID_NITROGEN,
		key: "solidNitrogen",
		name: "Solid nitrogen",
		atomicNumber: 7,
		selectable: false,
		palette: [
			[151, 187, 237],
			[131, 168, 222],
		],
		state: "solid",
		falls: true,
		densityKgPerM3: 1026,
		displacementDensity: 1.026,
		cellHeatCapacity: 1.6,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 0.17,
		phaseFamily: phaseFamilies.nitrogen,
		spawnTemperatureC: -220,
	}),
	[SULFUR]: define({
		id: SULFUR,
		key: "sulfur",
		name: "Sulfur",
		atomicNumber: 16,
		palette: [
			[239, 215, 72],
			[216, 188, 46],
		],
		state: "solid",
		falls: true,
		densityKgPerM3: 2070,
		displacementDensity: 2.07,
		cellHeatCapacity: 0.708,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 0.16,
		chemicalEnergyKjPerKg: 296.84 / 0.03206,
		ignitionTemperatureC: 250,
		phaseFamily: phaseFamilies.sulfur,
		description: "S · yellow solid. Melts, vaporizes and reacts with touching oxygen when ignited.",
		interactions: [
			"Fire or Heat can start S + O₂ → SO₂ at 250 °C. Sulfur dioxide is an oxygen-free gas.",
			"Melts at 115.21 °C and vaporizes at 444.61 °C after latent heat is supplied.",
			"Only this sulfur phase family is modeled; acid formation and changing vapor molecules are not.",
		],
	}),
	[LIQUID_SULFUR]: define({
		id: LIQUID_SULFUR,
		key: "liquidSulfur",
		name: "Liquid sulfur",
		atomicNumber: 16,
		selectable: false,
		palette: [
			[234, 177, 56],
			[220, 141, 38],
		],
		state: "liquid",
		densityKgPerM3: 1819,
		displacementDensity: 1.819,
		viscosityPas: 0.011,
		cellHeatCapacity: 1.2,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 0.18,
		chemicalEnergyKjPerKg: 296.84 / 0.03206,
		ignitionTemperatureC: 250,
		phaseFamily: phaseFamilies.sulfur,
		spawnTemperatureC: 200,
		flow: { interval: 2, spread: 3 },
	}),
	[SULFUR_VAPOR]: define({
		id: SULFUR_VAPOR,
		key: "sulfurVapor",
		name: "Sulfur vapor",
		atomicNumber: 16,
		selectable: false,
		palette: [
			[205, 140, 89],
			[183, 112, 73],
		],
		state: "gas",
		densityKgPerM3: 4.35,
		displacementDensity: 0.00435,
		molarMassKgPerMol: 0.25648,
		viscosityPas: 0.00002,
		cellHeatCapacity: 0.65,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 0.03,
		chemicalEnergyKjPerKg: 296.84 / 0.03206,
		ignitionTemperatureC: 250,
		phaseFamily: phaseFamilies.sulfur,
		spawnTemperatureC: 500,
		gasMotion: { hotTemperature: 450, hotRise: 2, risingDriftChance: 0.1, lateralChance: 0.25 },
	}),
	[CARBON_DIOXIDE]: define({
		id: CARBON_DIOXIDE,
		key: "carbonDioxide",
		name: "Carbon dioxide",
		selectable: false,
		palette: [
			[103, 143, 151],
			[79, 119, 128],
		],
		state: "gas",
		densityKgPerM3: 1.842,
		displacementDensity: 0.001842,
		molarMassKgPerMol: 0.0440095,
		viscosityPas: 0.0000148,
		cellHeatCapacity: 0.844,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 0.026,
		gasMotion: { hotTemperature: 500, hotRise: 2, risingDriftChance: 0.1, lateralChance: 0.2 },
	}),
	[SULFUR_DIOXIDE]: define({
		id: SULFUR_DIOXIDE,
		key: "sulfurDioxide",
		name: "Sulfur dioxide",
		selectable: false,
		palette: [
			[176, 140, 91],
			[151, 117, 74],
		],
		state: "gas",
		densityKgPerM3: 2.619,
		displacementDensity: 0.002619,
		molarMassKgPerMol: 0.064066,
		viscosityPas: 0.0000128,
		cellHeatCapacity: 0.622,
		thermalReferenceMassKg: 0.001,
		heatTransferCoefficient: 0.026,
		gasMotion: { hotTemperature: 500, hotRise: 2, risingDriftChance: 0.1, lateralChance: 0.2 },
	}),
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
	[HYDROCHLORIC_ACID]: define({
		id: HYDROCHLORIC_ACID,
		key: "hydrochloricAcid",
		name: "Hydrochloric acid 1 M",
		palette: [
			[130, 214, 174],
			[106, 193, 150],
			[151, 224, 184],
			[90, 178, 136],
		],
		heatTransferCoefficient: 0.34,
		cellHeatCapacity: 4.18,
		displacementDensity: 1,
		densityKgPerM3: 1000,
		viscosityPas: 0.001,
		state: "liquid",
		flow: { interval: 1, spread: 4 },
		chemicalEnergyKjPerKg: 57.2,
		neutralization: { role: "acid", equivalentsMolPerL: 1, heatJPerMol: 57_200 },
	}),
	[SULFURIC_ACID]: define({
		id: SULFURIC_ACID,
		key: "sulfuricAcid",
		name: "Sulfuric acid 0.5 M",
		palette: [
			[196, 165, 217],
			[172, 138, 197],
			[214, 185, 229],
			[150, 120, 176],
		],
		heatTransferCoefficient: 0.34,
		cellHeatCapacity: 4.18,
		displacementDensity: 1,
		densityKgPerM3: 1000,
		viscosityPas: 0.001,
		state: "liquid",
		flow: { interval: 1, spread: 4 },
		chemicalEnergyKjPerKg: 57.2,
		// 0.5 mol/L of H2SO4 contributes two acid equivalents per molecule.
		neutralization: { role: "acid", equivalentsMolPerL: 1, heatJPerMol: 57_200 },
	}),
	[SODIUM_HYDROXIDE]: define({
		id: SODIUM_HYDROXIDE,
		key: "sodiumHydroxide",
		name: "Sodium hydroxide 1 M",
		palette: [
			[102, 191, 222],
			[83, 164, 201],
			[135, 211, 234],
			[70, 146, 186],
		],
		heatTransferCoefficient: 0.34,
		cellHeatCapacity: 4.18,
		displacementDensity: 1,
		densityKgPerM3: 1000,
		viscosityPas: 0.001,
		state: "liquid",
		flow: { interval: 1, spread: 4 },
		neutralization: { role: "base", equivalentsMolPerL: 1 },
	}),
	[NEUTRAL_SOLUTION]: define({
		id: NEUTRAL_SOLUTION,
		key: "neutralSolution",
		name: "Neutralized solution",
		palette: [
			[119, 165, 176],
			[96, 145, 159],
			[141, 181, 190],
			[82, 132, 146],
		],
		heatTransferCoefficient: 0.34,
		cellHeatCapacity: 4.18,
		displacementDensity: 1,
		densityKgPerM3: 1000,
		viscosityPas: 0.001,
		state: "liquid",
		selectable: false,
		flow: { interval: 1, spread: 4 },
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
	/** Joules released per burning cell per fixed 1/60 s tick; calibrated reaction rate. */
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
