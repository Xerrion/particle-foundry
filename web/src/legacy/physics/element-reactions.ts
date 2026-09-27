import {
	CARBON,
	CARBON_DIOXIDE,
	EMPTY,
	FIRE,
	HYDROGEN,
	LIQUID_SULFUR,
	materialsById,
	OXYGEN,
	STEAM,
	SULFUR,
	SULFUR_DIOXIDE,
	SULFUR_VAPOR,
} from "../../materials";
import { CELL_WIDTH_METERS, GRAVITY_M_PER_S2 } from "../simulation/physical-scale";
import type { World } from "../simulation/world";
import { energyAtTemperature } from "./thermal";

type Fuel =
	| typeof HYDROGEN
	| typeof CARBON
	| typeof SULFUR
	| typeof LIQUID_SULFUR
	| typeof SULFUR_VAPOR;
const fuels: readonly Fuel[] = [HYDROGEN, CARBON, SULFUR, LIQUID_SULFUR, SULFUR_VAPOR];

/** Stoichiometric contact oxidation. Air connected to an edge is an explicit
 * open oxygen source for hydrogen; pure O2 pairs conserve their own mass. */
export function createElementReactions(world: World) {
	const reacted = new Uint8Array(world.size);
	const ventilated = new Uint8Array(world.size);
	const queue = new Int32Array(world.size);
	const oxygenMolarMass = materialsById[OXYGEN].molarMassKgPerMol;
	function neighbors(index: number): number[] {
		const x = index % world.width;
		const result: number[] = [];
		if (x > 0) result.push(index - 1);
		if (x + 1 < world.width) result.push(index + 1);
		if (index >= world.width) result.push(index - world.width);
		if (index + world.width < world.size) result.push(index + world.width);
		return result;
	}
	function elevation(index: number): number {
		return (world.height - Math.floor(index / world.width) - 0.5) * CELL_WIDTH_METERS;
	}
	function productFor(fuel: Fuel): number {
		return fuel === HYDROGEN ? STEAM : fuel === CARBON ? CARBON_DIOXIDE : SULFUR_DIOXIDE;
	}
	function oxygenPerFuel(fuel: Fuel): number {
		const molarMass =
			fuel === HYDROGEN
				? 2 * materialsById[HYDROGEN].molarMassKgPerMol
				: fuel === CARBON
					? materialsById[CARBON].molarMassKgPerMol
					: 0.03206;
		return oxygenMolarMass / molarMass;
	}
	function reactPair(f: number, o: number, fuel: Fuel): boolean {
		const mf = world.massKg[f];
		const mo = world.massKg[o];
		const fuelEnergy = materialsById[fuel].chemicalEnergyKjPerKg;
		const ratio = oxygenPerFuel(fuel);
		const usedFuel = Math.min(
			mf,
			world.chemicalEnergyKj[f] / fuelEnergy,
			world.oxygenKg[o] / ratio,
		);
		if (usedFuel <= 1e-15) return false;
		const usedOxygen = usedFuel * ratio;
		const remainingFuel = mf - usedFuel;
		const remainingOxygen = mo - usedOxygen;
		// Two cells cannot hold product and both distinct residual reactants.
		if (remainingFuel > mf * 1e-10 && remainingOxygen > mo * 1e-10) return false;
		const totalMass = mf + mo;
		const vx = (mf * world.velocityX[f] + mo * world.velocityX[o]) / totalMass;
		const vy = (mf * world.velocityY[f] + mo * world.velocityY[o]) / totalMass;
		const kineticBefore =
			0.5 *
			(mf * (world.velocityX[f] ** 2 + world.velocityY[f] ** 2) +
				mo * (world.velocityX[o] ** 2 + world.velocityY[o] ** 2));
		const potentialBefore = GRAVITY_M_PER_S2 * (mf * elevation(f) + mo * elevation(o));
		const heatBefore = world.energy[f] + world.energy[o];
		const chemicalBefore = world.chemicalEnergyKj[f] + world.chemicalEnergyKj[o];
		const oxygenBefore = world.oxygenKg[o];
		const remainingChemical = Math.max(0, chemicalBefore - usedFuel * fuelEnergy);
		let residual = -1;
		let residualMaterial = OXYGEN;
		let residualMass = 0;
		if (remainingFuel > mf * 1e-10) {
			residual = f;
			residualMaterial = fuel;
			residualMass = remainingFuel;
		} else if (remainingOxygen > mo * 1e-10) {
			residual = o;
			residualMass = remainingOxygen;
		}
		for (const index of [f, o]) {
			const material = index === residual ? residualMaterial : productFor(fuel);
			world.massKg[index] =
				residual < 0 ? totalMass / 2 : index === residual ? residualMass : totalMass - residualMass;
			world.changeMaterial(index, material);
			world.velocityX[index] = vx;
			world.velocityY[index] = vy;
			world.oxygenKg[index] = material === OXYGEN ? Math.max(0, oxygenBefore - usedOxygen) : 0;
			world.chemicalEnergyKj[index] = material === fuel ? remainingChemical : 0;
			reacted[index] = 1;
		}
		const potentialAfter =
			GRAVITY_M_PER_S2 * (world.massKg[f] * elevation(f) + world.massKg[o] * elevation(o));
		const heatAfter =
			heatBefore +
			usedFuel * fuelEnergy * 1000 +
			kineticBefore -
			0.5 * totalMass * (vx * vx + vy * vy) +
			potentialBefore -
			potentialAfter;
		world.energy[f] = (heatAfter * world.massKg[f]) / totalMass;
		world.energy[o] = heatAfter - world.energy[f];
		world.applyPhase(f);
		world.applyPhase(o);
		return true;
	}
	function markVentilatedAir(): void {
		ventilated.fill(0);
		let head = 0;
		let tail = 0;
		function enqueue(index: number): void {
			if ((world.grid[index] !== EMPTY && world.grid[index] !== FIRE) || ventilated[index]) return;
			ventilated[index] = 1;
			queue[tail++] = index;
		}
		for (let x = 0; x < world.width; x++) {
			enqueue(x);
			enqueue(world.size - world.width + x);
		}
		for (let y = 1; y + 1 < world.height; y++) {
			enqueue(y * world.width);
			enqueue(y * world.width + world.width - 1);
		}
		while (head < tail) for (const n of neighbors(queue[head++])) enqueue(n);
	}
	function reactWithAir(h: number): boolean {
		const fuelMass = world.massKg[h];
		const fuelEnergy = materialsById[HYDROGEN].chemicalEnergyKjPerKg;
		if (world.chemicalEnergyKj[h] < fuelMass * fuelEnergy * (1 - 1e-10)) return false;
		const oxygenMass = fuelMass * oxygenPerFuel(HYDROGEN);
		const productMass = fuelMass + oxygenMass;
		const initialKinetic = 0.5 * fuelMass * (world.velocityX[h] ** 2 + world.velocityY[h] ** 2);
		const velocityX = (world.velocityX[h] * fuelMass) / productMass;
		const velocityY = (world.velocityY[h] * fuelMass) / productMass;
		const ambientHeat = energyAtTemperature(OXYGEN, 22, undefined, oxygenMass);
		const incomingEnergy = ambientHeat + oxygenMass * GRAVITY_M_PER_S2 * elevation(h);
		world.ledger.massAddedKg += oxygenMass;
		world.ledger.externalEnergyAdded += incomingEnergy;
		const heat =
			world.energy[h] +
			ambientHeat +
			world.chemicalEnergyKj[h] * 1000 +
			initialKinetic -
			0.5 * productMass * (velocityX * velocityX + velocityY * velocityY);
		world.massKg[h] = productMass;
		world.changeMaterial(h, STEAM);
		world.energy[h] = heat;
		world.chemicalEnergyKj[h] = 0;
		world.oxygenKg[h] = 0;
		world.velocityX[h] = velocityX;
		world.velocityY[h] = velocityY;
		world.applyPhase(h);
		reacted[h] = 1;
		return true;
	}
	function step(): number {
		if (fuels.every((fuel) => world.countMaterial(fuel) === 0)) return 0;
		reacted.fill(0);
		let airMarked = false;
		let count = 0;
		for (let f = 0; f < world.size; f++) {
			const fuel = world.grid[f];
			if (!fuels.includes(fuel as Fuel) || reacted[f]) continue;
			const touching = neighbors(f);
			const ignition = materialsById[fuel].ignitionTemperatureC;
			const ignited =
				world.burning[f] !== 0 ||
				world.temperatureAt(f) >= ignition ||
				touching.some((n) => world.grid[n] === FIRE || world.burning[n]);
			if (world.countMaterial(OXYGEN) > 0) {
				let done = false;
				for (const o of touching) {
					if (world.grid[o] !== OXYGEN || reacted[o]) continue;
					if (!ignited && world.temperatureAt(o) < ignition) continue;
					if (reactPair(f, o, fuel as Fuel)) {
						count++;
						done = true;
						break;
					}
				}
				if (done) continue;
			}
			if (fuel !== HYDROGEN) {
				world.burning[f] = 0;
				continue;
			}
			if (ignited && touching.some((n) => world.grid[n] === EMPTY || world.grid[n] === FIRE)) {
				if (!airMarked) {
					markVentilatedAir();
					airMarked = true;
				}
				if (touching.some((n) => ventilated[n]) && reactWithAir(f)) count++;
			}
			if (world.grid[f] === HYDROGEN) world.burning[f] = 0;
		}
		return count;
	}
	return { step };
}
