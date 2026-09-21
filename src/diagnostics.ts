import { EMPTY } from "./material-ids";
import { CELL_WIDTH_METERS, conservationTolerance, GRAVITY_M_PER_S2 } from "./physical-scale";
import type { World } from "./world";

export interface PhysicalTotals {
	readonly matterMassKg: number;
	readonly gasMassKg: number;
	readonly thermalEnergy: number;
	readonly chemicalEnergyKj: number;
	readonly kineticEnergyKj: number;
	readonly potentialEnergyKj: number;
	readonly totalTrackedEnergyKj: number;
	readonly maximumPressurePa: number;
	readonly maximumSpeedMPerS: number;
}

export interface ConservationReport {
	readonly massDriftKg: number;
	readonly energyDriftKj: number;
	readonly massWithinTolerance: boolean;
	readonly energyWithinTolerance: boolean;
}

/** Pure O(n) measurement; it never mutates or owns simulation state. */
export function measureWorld(world: World): PhysicalTotals {
	let matterMassKg = 0;
	let gasMassKg = 0;
	let thermalEnergy = 0;
	let chemicalEnergyKj = 0;
	let kineticEnergyKj = 0;
	let potentialEnergyKj = 0;
	let maximumPressurePa = 0;
	let maximumSpeedMPerS = 0;
	for (let index = 0; index < world.size; index += 1) {
		const mass = world.massKg[index];
		if (world.grid[index] === EMPTY) gasMassKg += mass;
		else matterMassKg += mass;
		thermalEnergy += world.energy[index] / 1000;
		chemicalEnergyKj += world.chemicalEnergyKj[index];
		const speedSquared = world.velocityX[index] ** 2 + world.velocityY[index] ** 2;
		kineticEnergyKj += (0.5 * mass * speedSquared) / 1000;
		const y = Math.floor(index / world.width);
		const elevation = (world.height - y - 0.5) * CELL_WIDTH_METERS;
		potentialEnergyKj += (mass * GRAVITY_M_PER_S2 * elevation) / 1000;
		maximumPressurePa = Math.max(maximumPressurePa, world.pressurePa[index]);
		maximumSpeedMPerS = Math.max(maximumSpeedMPerS, Math.sqrt(speedSquared));
	}
	return Object.freeze({
		matterMassKg,
		gasMassKg,
		thermalEnergy,
		chemicalEnergyKj,
		kineticEnergyKj,
		potentialEnergyKj,
		totalTrackedEnergyKj: thermalEnergy + chemicalEnergyKj + kineticEnergyKj + potentialEnergyKj,
		maximumPressurePa,
		maximumSpeedMPerS,
	});
}

export function compareConservation(
	before: PhysicalTotals,
	after: PhysicalTotals,
): ConservationReport {
	const beforeMass = before.matterMassKg + before.gasMassKg;
	const afterMass = after.matterMassKg + after.gasMassKg;
	const massDriftKg = afterMass - beforeMass;
	const energyDriftKj = after.totalTrackedEnergyKj - before.totalTrackedEnergyKj;
	return Object.freeze({
		massDriftKg,
		energyDriftKj,
		massWithinTolerance: Math.abs(massDriftKg) <= conservationTolerance(beforeMass),
		energyWithinTolerance:
			Math.abs(energyDriftKj) <= conservationTolerance(before.totalTrackedEnergyKj),
	});
}
