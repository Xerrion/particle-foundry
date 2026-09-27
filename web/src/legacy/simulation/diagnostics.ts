import { EMPTY } from "../../materials/ids";
import { isGas } from "../../materials/queries";
import { CELL_WIDTH_METERS, conservationTolerance, GRAVITY_M_PER_S2 } from "./physical-scale";
import type { World, WorldLedger } from "./world";

export interface PhysicalTotals {
	/** Non-air mass, including non-air gases; overlaps gasMassKg. */
	readonly matterMassKg: number;
	readonly gasMassKg: number;
	/** Sum of parcel enthalpy, expressed in kJ. */
	readonly thermalEnergy: number;
	readonly chemicalEnergyKj: number;
	readonly kineticEnergyKj: number;
	readonly potentialEnergyKj: number;
	readonly totalTrackedEnergyKj: number;
	readonly maximumPressurePa: number;
	readonly maximumSpeedMPerS: number;
	readonly totalMassKg: number;
	readonly ledger: Readonly<WorldLedger>;
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
		if (isGas(world.grid[index])) gasMassKg += mass;
		if (world.grid[index] !== EMPTY) matterMassKg += mass;
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
		totalMassKg: world.massKg.reduce((sum, mass) => sum + mass, 0),
		ledger: Object.freeze({ ...world.ledger }),
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
	accountForSources = false,
): ConservationReport {
	const beforeMass = before.totalMassKg;
	const afterMass = after.totalMassKg;
	const netMass = (ledger: Readonly<WorldLedger>) => ledger.massAddedKg - ledger.massRemovedKg;
	const netEnergy = (ledger: Readonly<WorldLedger>) =>
		(ledger.externalEnergyAdded - ledger.externalEnergyRemoved) / 1000;
	const massDriftKg =
		afterMass -
		beforeMass -
		(accountForSources ? netMass(after.ledger) - netMass(before.ledger) : 0);
	const energyDriftKj =
		after.totalTrackedEnergyKj -
		before.totalTrackedEnergyKj -
		(accountForSources ? netEnergy(after.ledger) - netEnergy(before.ledger) : 0);
	return Object.freeze({
		massDriftKg,
		energyDriftKj,
		massWithinTolerance: Math.abs(massDriftKg) <= conservationTolerance(beforeMass),
		energyWithinTolerance:
			Math.abs(energyDriftKj) <= conservationTolerance(before.totalTrackedEnergyKj),
	});
}
