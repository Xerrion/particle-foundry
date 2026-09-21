import type { MaterialDefinition, PhaseFamily } from "./material-definitions";

interface CatalogueTool {
	readonly id: number;
	readonly key: string;
	readonly shortcut?: string;
}

/** Validate authoring contracts once, never in a per-cell hot path. No mutation or defaults. */
export function validateMaterialCatalogue(
	definitions: Readonly<Record<number, MaterialDefinition>>,
	tools: readonly CatalogueTool[],
): void {
	function require(condition: boolean, message: string): asserts condition {
		if (!condition) throw new Error(`Invalid material catalogue: ${message}`);
	}
	function positive(value: number): boolean {
		return Number.isFinite(value) && value > 0;
	}
	function nonnegative(value: number): boolean {
		return Number.isFinite(value) && value >= 0;
	}
	function probability(value: number): boolean {
		return nonnegative(value) && value <= 1;
	}
	function range(values: readonly [number, number], label: string): void {
		require(values.every((v) => Number.isInteger(v) && v > 0 && v <= 65535) &&
			values[0] < values[1], `${label}: invalid lifetime range`);
	}
	const ids = new Set<number>();
	const keys = new Set<string>();
	const shortcuts = new Set<string>();
	function identity(entry: CatalogueTool): void {
		require(Number.isInteger(entry.id) &&
			entry.id >= 0 &&
			entry.id <= 255 &&
			!ids.has(entry.id), `duplicate or invalid ID ${entry.id}`);
		ids.add(entry.id);
		require(entry.key.length > 0 && !keys.has(entry.key), `duplicate or empty key ${entry.key}`);
		keys.add(entry.key);
		if (entry.shortcut !== undefined) {
			const shortcut = entry.shortcut.toLowerCase();
			require(shortcut.length === 1 &&
				!shortcuts.has(shortcut), `duplicate or invalid shortcut ${entry.shortcut}`);
			shortcuts.add(shortcut);
		}
	}
	const families = new Set<PhaseFamily>();
	for (const [key, material] of Object.entries(definitions)) {
		const label = material.key;
		require(String(material.id) === key, `${label}: registry key does not match ID`);
		identity(material);
		require(["ambient", "granular", "solid", "liquid", "gas"].includes(
			material.state,
		), `${label}: invalid state`);
		for (const field of ["cellHeatCapacity", "displacementDensity", "densityKgPerM3"] as const)
			require(positive(material[field]), `${label}: ${field} must be positive and finite`);
		for (const field of [
			"heatTransferCoefficient",
			"viscosityPas",
			"molarMassKgPerMol",
			"chemicalEnergyKjPerKg",
		] as const)
			require(nonnegative(material[field]), `${label}: ${field} must be nonnegative and finite`);
		require(Number.isFinite(material.spawnTemperatureC), `${label}: invalid spawn temperature`);
		require(probability(material.oxygenMassFraction), `${label}: invalid oxygen fraction`);
		require(material.ignitionTemperatureC === Infinity ||
			Number.isFinite(material.ignitionTemperatureC), `${label}: invalid ignition temperature`);
		if (Number.isFinite(material.ignitionTemperatureC))
			require(positive(
				material.chemicalEnergyKjPerKg,
			), `${label}: combustible material needs chemical energy`);
		if (material.state === "gas" || material.state === "ambient")
			require(positive(material.molarMassKgPerMol), `${label}: gas needs positive molar mass`);
		if (["gas", "ambient", "liquid"].includes(material.state))
			require(positive(material.viscosityPas), `${label}: fluid needs positive viscosity`);
		if (material.state === "gas") require(!!material.gasMotion, `${label}: gas needs gasMotion`);
		if (material.state === "liquid") require(!!material.flow, `${label}: liquid needs flow`);
		if (material.flow) {
			require(material.state === "liquid", `${label}: flow requires liquid state`);
			require(Number.isInteger(material.flow.interval) &&
				material.flow.interval > 0 &&
				Number.isInteger(material.flow.spread) &&
				material.flow.spread > 0, `${label}: invalid flow interval or spread`);
		}
		if (material.gasMotion) {
			const gas = material.gasMotion;
			require(material.state === "gas" &&
				Number.isFinite(gas.hotTemperature) &&
				Number.isInteger(gas.hotRise) &&
				gas.hotRise > 0 &&
				probability(gas.risingDriftChance) &&
				probability(gas.lateralChance), `${label}: invalid gasMotion`);
		}
		if (material.lifetimeTicks) range(material.lifetimeTicks, label);
		if (material.ignitionBrush) {
			range(material.ignitionBrush.lifetimeTicks, `${label} brush`);
			require(material.state === "gas" &&
				Number.isFinite(material.ignitionBrush.temperatureC) &&
				probability(material.ignitionBrush.fringeChance), `${label}: invalid ignition brush`);
		}
		if (material.explosive) {
			const profile = material.explosive;
			require(material.chemicalEnergyKjPerKg > 0 &&
				Number.isFinite(profile.triggerTemperatureC) &&
				Number.isInteger(profile.radiusCells) &&
				profile.radiusCells > 0 &&
				profile.radiusCells <= 12 &&
				positive(profile.impulseSpeedMPerS), `${label}: invalid explosive profile`);
		}
		if (material.electrical) {
			const electrical = material.electrical;
			require(material.state === "solid" &&
				positive(
					electrical.resistanceOhms,
				), `${label}: electrical conductors need solid state and positive resistance`);
			if (electrical.terminal === "source")
				require(positive(electrical.voltageV ?? NaN) &&
					material.chemicalEnergyKjPerKg >
						0, `${label}: electrical sources need voltage and stored energy`);
			else
				require(electrical.voltageV === undefined &&
					(electrical.terminal === undefined ||
						electrical.terminal === "ground"), `${label}: invalid electrical terminal`);
		}
		if (material.neutralization) {
			const profile = material.neutralization;
			require(material.state === "liquid" &&
				positive(
					profile.equivalentsMolPerL,
				), `${label}: neutralization needs liquid state and positive equivalents`);
			if (profile.role === "acid")
				require(positive(profile.heatJPerMol ?? NaN) &&
					material.chemicalEnergyKjPerKg >
						0, `${label}: acid needs reaction heat and stored chemical energy`);
			else
				require(profile.role === "base" &&
					profile.heatJPerMol === undefined, `${label}: invalid neutralization base`);
		}
		if (material.transforms) {
			require(Object.hasOwn(
				definitions,
				material.transforms.target,
			), `${label}: unknown transform target`);
			require(Number.isFinite(
				material.transforms.temperature,
			), `${label}: invalid transform temperature`);
		}
		if (material.phaseFamily) {
			require(material.phaseFamily.phases.includes(
				material.id,
			), `${label}: missing from its phase family`);
			families.add(material.phaseFamily);
		}
	}
	for (const tool of tools) identity(tool);
	for (const family of families) {
		require(family.phases.length >= 2 &&
			family.transitions.length === family.phases.length - 1, "phase/transition count mismatch");
		require(new Set(family.phases).size === family.phases.length, "duplicate phase ID");
		for (const id of family.phases) {
			require(Object.hasOwn(definitions, id), `unknown phase ID ${id}`);
			require(definitions[id]?.phaseFamily ===
				family, `phase ${id} does not point back to its family`);
		}
		for (const [i, transition] of family.transitions.entries()) {
			require([
				transition.temperature,
				transition.lowerCellEnthalpy,
				transition.upperCellEnthalpy,
			].every(Number.isFinite) &&
				transition.upperCellEnthalpy > transition.lowerCellEnthalpy, "invalid latent interval");
			if (transition.vaporPressure)
				require(positive(transition.vaporPressure.gasConstant) &&
					positive(transition.vaporPressure.latentJPerKg), "invalid vapor-pressure constants");
			const previous = family.transitions[i - 1];
			if (previous) {
				require(transition.temperature > previous.temperature &&
					transition.lowerCellEnthalpy > previous.upperCellEnthalpy, "unordered phase transitions");
				const phaseId = family.phases[i];
				const member = phaseId === undefined ? undefined : definitions[phaseId];
				require(member !== undefined, "missing intermediate phase");
				const expected =
					previous.upperCellEnthalpy +
					member.cellHeatCapacity * (transition.temperature - previous.temperature);
				require(Math.abs(transition.lowerCellEnthalpy - expected) <=
					1e-9 * Math.max(1, Math.abs(expected)), "discontinuous phase enthalpy endpoints");
			}
		}
		for (const [i, transition] of family.transitions.entries()) {
			if (transition.nucleationChancePerTick !== undefined) {
				const liquid = family.phases[i];
				const vapor = family.phases[i + 1];
				require(probability(transition.nucleationChancePerTick) &&
					liquid !== undefined &&
					vapor !== undefined &&
					definitions[liquid]?.state === "liquid" &&
					definitions[vapor]?.state === "gas", "invalid boiling nucleation profile");
			}
		}
	}
}
