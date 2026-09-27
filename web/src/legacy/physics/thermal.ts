import {
	EMPTY,
	materialsById,
	type PhaseTransition,
	phaseFamilies,
	physicalProperties,
} from "../../materials";
import {
	AMBIENT_PRESSURE_PA,
	AMBIENT_TEMPERATURE_C,
	CELL_WIDTH_METERS,
	FIXED_TIME_STEP_SECONDS,
	REPRESENTED_DEPTH_METERS,
} from "../simulation/physical-scale";

const materialPhysics = physicalProperties;

/** Catalogue curves describe one gram; runtime energy is parcel enthalpy in joules. */
const referenceMasses = materialsById.map((material) => material.thermalReferenceMassKg ?? 0.001);

export function thermalMassScale(material: number, massKg: number): number {
	if (!Number.isFinite(massKg) || massKg <= 0)
		throw new RangeError("Thermal mass must be positive and finite");
	return massKg / referenceMasses[material];
}

export function heatCapacityForMass(material: number, massKg: number): number {
	return materialsById[material].cellHeatCapacity * thermalMassScale(material, massKg);
}

function finiteValue(value: number, name: string): number {
	if (!Number.isFinite(value)) throw new RangeError(`${name} must be finite; received ${value}`);
	return value;
}

/** Piecewise sensible/latent heat evaluated from the shared material catalogue. */
export function energyAtTemperature(
	material: number,
	temperature: number,
	pressurePa = AMBIENT_PRESSURE_PA,
	massKg?: number,
): number {
	return (
		referenceEnergyAtTemperature(material, temperature, pressurePa) *
		(massKg === undefined ? 1 : thermalMassScale(material, massKg))
	);
}

function referenceEnergyAtTemperature(
	material: number,
	temperature: number,
	pressurePa: number,
): number {
	const definition = materialPhysics(material);
	finiteValue(temperature, "Temperature");
	finiteValue(pressurePa, "Pressure");
	if (pressurePa <= 0) throw new RangeError("Pressure must be positive");
	const family = definition.phaseFamily;
	if (!family) return finiteValue(temperature * definition.cellHeatCapacity, "Derived energy");
	for (let i = 0; i < family.transitions.length; i += 1) {
		const transition = family.transitions[i];
		const transitionTemperature = transitionTemperatureAtPressure(transition, pressurePa);
		const offset =
			(transitionTemperature - transition.temperature) *
			materialsById[family.phases[i]].cellHeatCapacity;
		if (temperature < transitionTemperature) {
			return finiteValue(
				transition.lowerCellEnthalpy +
					offset +
					(temperature - transitionTemperature) * materialsById[family.phases[i]].cellHeatCapacity,
				"Derived energy",
			);
		}
		if (temperature === transitionTemperature) {
			return family.phases.indexOf(material) > i
				? transition.upperCellEnthalpy + offset
				: transition.lowerCellEnthalpy + offset;
		}
	}
	const last = family.transitions[family.transitions.length - 1];
	const lastTemperature = transitionTemperatureAtPressure(last, pressurePa);
	const lastOffset =
		(lastTemperature - last.temperature) *
		materialsById[family.phases[family.phases.length - 2]].cellHeatCapacity;
	return finiteValue(
		last.upperCellEnthalpy +
			lastOffset +
			(temperature - lastTemperature) *
				materialsById[family.phases[family.phases.length - 1]].cellHeatCapacity,
		"Derived energy",
	);
}

function temperatureForEnergy(
	material: number,
	energy: number,
	pressurePa = AMBIENT_PRESSURE_PA,
): number {
	const definition = materialsById[material];
	const family = definition.phaseFamily;
	if (!family) return energy / definition.cellHeatCapacity;
	for (let i = 0; i < family.transitions.length; i += 1) {
		const transition = family.transitions[i];
		const transitionTemperature = transitionTemperatureAtPressure(transition, pressurePa);
		const offset =
			(transitionTemperature - transition.temperature) *
			materialsById[family.phases[i]].cellHeatCapacity;
		if (energy < transition.lowerCellEnthalpy + offset) {
			return (
				transitionTemperature +
				(energy - transition.lowerCellEnthalpy - offset) /
					materialsById[family.phases[i]].cellHeatCapacity
			);
		}
		if (energy <= transition.upperCellEnthalpy + offset) return transitionTemperature;
	}
	const last = family.transitions[family.transitions.length - 1];
	const lastTemperature = transitionTemperatureAtPressure(last, pressurePa);
	const lastOffset =
		(lastTemperature - last.temperature) *
		materialsById[family.phases[family.phases.length - 2]].cellHeatCapacity;
	return (
		lastTemperature +
		(energy - last.upperCellEnthalpy - lastOffset) /
			materialsById[family.phases[family.phases.length - 1]].cellHeatCapacity
	);
}

/** Temperature derives from enthalpy; a latent plateau never discards partial heat. */
export function temperatureFromEnergy(
	material: number,
	energy: number,
	pressurePa = AMBIENT_PRESSURE_PA,
	massKg?: number,
): number {
	materialPhysics(material);
	finiteValue(energy, "Energy");
	finiteValue(pressurePa, "Pressure");
	if (pressurePa <= 0) throw new RangeError("Pressure must be positive");
	return finiteValue(
		temperatureForEnergy(
			material,
			energy / (massKg === undefined ? 1 : thermalMassScale(material, massKg)),
			pressurePa,
		),
		"Derived temperature",
	);
}

// Compile this derived lookup once, not an array allocation for every cell/tick.
const minimumCapacities: Readonly<Record<number, number>> = Object.fromEntries(
	Object.values(materialsById).map((definition) => [
		definition.id,
		definition.phaseFamily
			? Math.min(
					...definition.phaseFamily.phases.map((phase) => materialsById[phase].cellHeatCapacity),
				)
			: definition.cellHeatCapacity,
	]),
);

function minimumHeatCapacity(material: number): number {
	return minimumCapacities[material];
}

export function initialTemperature(material: number): number {
	return materialPhysics(material).spawnTemperatureC;
}

function transitionTemperatureAtPressure(transition: PhaseTransition, pressurePa: number): number {
	const pressure = transition.vaporPressure;
	if (!pressure || pressurePa === AMBIENT_PRESSURE_PA) return transition.temperature;
	const inverseKelvin =
		1 / (transition.temperature + 273.15) -
		(pressure.gasConstant / pressure.latentJPerKg) * Math.log(pressurePa / AMBIENT_PRESSURE_PA);
	if (inverseKelvin <= 0) throw new RangeError("Pressure is outside the boiling approximation");
	return finiteValue(1 / inverseKelvin - 273.15, "Boiling temperature");
}

/** Water convenience API; its constants are owned by the phase-family catalogue. */
export function boilingTemperatureAtPressure(pressurePa: number): number {
	finiteValue(pressurePa, "Pressure");
	if (pressurePa <= 0) throw new RangeError("Pressure must be positive");
	return transitionTemperatureAtPressure(phaseFamilies.water.transitions[1], pressurePa);
}

/** Common pressure correction for phase resolution and local bubble nucleation. */
export function phaseEnthalpyOffset(
	transition: PhaseTransition,
	lowerPhase: number,
	pressurePa: number,
): number {
	return (
		(transitionTemperatureAtPressure(transition, pressurePa) - transition.temperature) *
		materialsById[lowerPhase].cellHeatCapacity
	);
}

/**
 * Resolve only completed latent transitions. Pressure shifts both enthalpy
 * endpoints together; comparing a 100 C plateau directly with the boiling point
 * made steam condense and immediately re-boil at virtually unchanged energy.
 */
export function phaseFromEnergy(
	material: number,
	energy: number,
	pressurePa = AMBIENT_PRESSURE_PA,
	massKg?: number,
): number {
	const definition = materialPhysics(material);
	finiteValue(energy, "Energy");
	finiteValue(pressurePa, "Pressure");
	if (pressurePa <= 0) throw new RangeError("Pressure must be positive");
	if (massKg !== undefined) energy /= thermalMassScale(material, massKg);
	const family = definition.phaseFamily;
	if (family) {
		for (let i = 0; i < family.transitions.length; i += 1) {
			const transition = family.transitions[i];
			const offset = phaseEnthalpyOffset(transition, family.phases[i], pressurePa);
			if (energy <= transition.lowerCellEnthalpy + offset) return family.phases[i];
			if (energy < transition.upperCellEnthalpy + offset) {
				return family.phases.indexOf(material) > i ? family.phases[i + 1] : family.phases[i];
			}
		}
		return family.phases[family.phases.length - 1];
	}
	const transform = definition.transforms;
	if (transform && temperatureForEnergy(material, energy, pressurePa) >= transform.temperature)
		return transform.target;
	return material;
}

/**
 * Creates an insulated, row-major four-neighbor solver for positive safe-integer
 * dimensions whose product is at most 2^32 - 1. Scratch storage is instance-local
 * and allocated once. No sources, sinks, movement or phase-ID changes are applied.
 *
 * diffuse requires non-overlapping Uint8Array matter and Float64Array enthalpy
 * fields of exactly width * height cells, including air. An optional pressure
 * field supplies pressure for every phase; zero uses ambient. Optional mass in kg
 * scales the reference curves to parcel enthalpy in joules. It snapshots temperatures,
 * visits each edge once and applies equal/opposite energy deltas synchronously.
 * It changes only energy; phaseFromEnergy can then update IDs without resetting it.
 * Minimum family capacities limit each pair's flux to one quarter of its sensible
 * equalization budget, preventing four simultaneous neighbors from overshooting.
 * Conservation is subject only to Float64 rounding; no energy is clamped or dropped.
 *
 * Wrong array types throw TypeError. Invalid dimensions, lengths, overlapping
 * buffers, unknown IDs, nonfinite energy or numeric overflow throw RangeError
 * before either caller-owned field changes. Native allocation failures propagate.
 */
export interface ThermalSolverOptions {
	readonly timeStepSeconds?: number;
	readonly cellWidthMeters?: number;
	readonly representedDepthMeters?: number;
}

export function createThermalSolver(
	width: number,
	height: number,
	options: ThermalSolverOptions = {},
) {
	if (
		!Number.isSafeInteger(width) ||
		!Number.isSafeInteger(height) ||
		width <= 0 ||
		height <= 0 ||
		!Number.isSafeInteger(width * height) ||
		width * height > 0xffffffff
	) {
		throw new RangeError(
			"Thermal dimensions must be positive safe integers with at most 2^32 - 1 cells",
		);
	}
	const cellCount = width * height;
	const timeStepSeconds = options.timeStepSeconds ?? FIXED_TIME_STEP_SECONDS;
	const cellWidthMeters = options.cellWidthMeters ?? CELL_WIDTH_METERS;
	const representedDepthMeters = options.representedDepthMeters ?? REPRESENTED_DEPTH_METERS;
	if (
		![timeStepSeconds, cellWidthMeters, representedDepthMeters].every(
			(value) => Number.isFinite(value) && value > 0,
		)
	) {
		throw new RangeError("Thermal scale values must be finite and positive");
	}
	const conductanceScale =
		(timeStepSeconds / FIXED_TIME_STEP_SECONDS) *
		(representedDepthMeters / REPRESENTED_DEPTH_METERS);
	const capacityScale =
		(cellWidthMeters / CELL_WIDTH_METERS) ** 2 *
		(representedDepthMeters / REPRESENTED_DEPTH_METERS);
	const temperatures = new Float64Array(cellCount);
	const energyChanges = new Float64Array(cellCount);
	const capacities = new Float64Array(cellCount);
	const conductivities = new Float64Array(cellCount);
	const ambientEnergy = energyAtTemperature(EMPTY, AMBIENT_TEMPERATURE_C);
	const ambientCapacity = minimumHeatCapacity(EMPTY) * capacityScale;
	const ambientConductivity = materialsById[EMPTY].heatTransferCoefficient;

	function transfer(a: number, b: number): void {
		if (temperatures[a] === temperatures[b]) return;
		const pairCapacity = 1 / (1 / capacities[a] + 1 / capacities[b]);
		const conductance = Math.min(
			conductivities[a] * conductanceScale,
			conductivities[b] * conductanceScale,
			pairCapacity / 4,
		);
		// Scaling first avoids overflow when two finite temperatures have opposite signs.
		const flux = temperatures[b] * conductance - temperatures[a] * conductance;
		energyChanges[a] += flux;
		energyChanges[b] -= flux;
	}

	function diffuse(
		materials: Uint8Array,
		energy: Float64Array,
		pressurePa?: Float64Array,
		massKg?: Float64Array,
	): void {
		if (massKg && (!(massKg instanceof Float64Array) || massKg.length !== cellCount))
			throw new RangeError("Thermal mass must contain one Float64 value per cell");
		if (!(materials instanceof Uint8Array) || !(energy instanceof Float64Array)) {
			throw new TypeError("Thermal fields must be Uint8Array materials and Float64Array energy");
		}
		if (pressurePa !== undefined && !(pressurePa instanceof Float64Array)) {
			throw new TypeError("Thermal pressure must be a Float64Array");
		}
		if (materials.length !== cellCount || energy.length !== cellCount) {
			throw new RangeError(`Thermal fields must each contain ${cellCount} cells`);
		}
		if (pressurePa && pressurePa.length !== cellCount) {
			throw new RangeError(`Thermal pressure must contain ${cellCount} cells`);
		}
		if (
			materials.buffer === energy.buffer &&
			materials.byteOffset < energy.byteOffset + energy.byteLength &&
			energy.byteOffset < materials.byteOffset + materials.byteLength
		) {
			throw new RangeError("Thermal material and energy fields must not overlap");
		}
		for (let i = 0; i < cellCount; i += 1) {
			const material = materials[i];
			const physics = materialsById[material];
			if (!physics) throw new RangeError(`Unknown material: ${material}`);
			const cellEnergy = finiteValue(energy[i], "Cell energy");
			const pressure = pressurePa ? finiteValue(pressurePa[i], "Cell pressure") : 0;
			if (pressure < 0) throw new RangeError("Cell pressure must be nonnegative");
			if (
				material === EMPTY &&
				cellEnergy === ambientEnergy &&
				(!massKg || massKg[i] === referenceMasses[EMPTY])
			) {
				temperatures[i] = AMBIENT_TEMPERATURE_C;
				capacities[i] = ambientCapacity;
				conductivities[i] = ambientConductivity;
				continue;
			}
			const effectivePressure = pressure > 0 ? pressure : AMBIENT_PRESSURE_PA;
			const massScale = massKg ? thermalMassScale(material, massKg[i]) : 1;
			temperatures[i] = finiteValue(
				physics.phaseFamily
					? temperatureForEnergy(material, cellEnergy / massScale, effectivePressure)
					: cellEnergy / (physics.cellHeatCapacity * massScale),
				"Derived temperature",
			);
			capacities[i] = minimumHeatCapacity(material) * capacityScale * massScale;
			conductivities[i] = physics.heatTransferCoefficient;
		}
		energyChanges.fill(0);
		for (let y = 0; y < height; y += 1) {
			const rowEnd = (y + 1) * width;
			for (let i = y * width; i < rowEnd; i += 1) {
				if (i + 1 < rowEnd) transfer(i, i + 1);
				if (y + 1 < height) transfer(i, i + width);
			}
		}
		for (let i = 0; i < cellCount; i += 1) {
			energyChanges[i] = finiteValue(energy[i] + energyChanges[i], "Diffused energy");
		}
		energy.set(energyChanges);
	}

	return { diffuse };
}
