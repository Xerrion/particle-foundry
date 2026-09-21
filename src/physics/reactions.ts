import {
	CARBON,
	combustionProfile,
	EMPTY,
	FIRE,
	HYDROGEN,
	LIQUID_SULFUR,
	materialsById,
	PLANT,
	plantGrowthProfile,
	SMOKE,
	SULFUR,
	SULFUR_VAPOR,
	WATER,
} from "../materials";
import { isGas } from "../materials/queries";
import { CELL_VOLUME_M3 } from "../simulation/physical-scale";
import type { World } from "../simulation/world";
import { energyAtTemperature, heatCapacityForMass } from "./thermal";

/** Creates the explicit chemical/biological sources and sinks; it does not move particles. */
export function createReactions(world: World) {
	const { grid, energy, lifetime, moved, variation, width } = world;
	const ventilated = new Uint8Array(world.size);
	const queue = new Int32Array(world.size);

	function atBoundary(index: number): boolean {
		return (
			index < width ||
			index >= world.size - width ||
			index % width === 0 ||
			index % width === width - 1
		);
	}

	/** Only gas connected to an open edge can draw fresh air. Solid walls seal cavities. */
	function beginStep(): void {
		ventilated.fill(0);
		let head = 0;
		let tail = 0;
		function enqueue(index: number): void {
			if (ventilated[index] || !isGas(grid[index])) return;
			ventilated[index] = 1;
			queue[tail++] = index;
		}
		for (let x = 0; x < width; x += 1) {
			enqueue(x);
			enqueue(world.size - width + x);
		}
		for (let y = 1; y + 1 < world.height; y += 1) {
			enqueue(y * width);
			enqueue(y * width + width - 1);
		}
		while (head < tail) {
			const index = queue[head++];
			if (grid[index] === EMPTY) {
				world.oxygenKg[index] =
					materialsById[EMPTY].densityKgPerM3 *
					CELL_VOLUME_M3 *
					materialsById[EMPTY].oxygenMassFraction;
			}
			const x = index % width;
			if (x > 0) enqueue(index - 1);
			if (x + 1 < width) enqueue(index + 1);
			if (index >= width) enqueue(index - width);
			if (index + width < world.size) enqueue(index + width);
		}
	}

	function neighbors(index: number): number[] {
		const x = index % width;
		const result: number[] = [];
		if (x > 0) result.push(index - 1);
		if (x + 1 < width) result.push(index + 1);
		if (index >= width) result.push(index - width);
		if (index + width < world.size) result.push(index + width);
		return result;
	}

	function neighborOfType(index: number, material: number): number {
		for (const neighbor of neighbors(index)) if (grid[neighbor] === material) return neighbor;
		return -1;
	}

	function grow(index: number): void {
		const water = neighborOfType(index, WATER);
		const y = Math.floor(index / width);
		if (water < 0 || y <= 1 || world.random.next() >= plantGrowthProfile.chancePerTick) return;
		const directions = [
			[0, -1],
			[-1, 0],
			[1, 0],
			[-1, -1],
			[1, -1],
		];
		const [dx, dy] = directions[Math.floor(world.random.next() * directions.length)];
		const x = (index % width) + dx;
		if (!world.inBounds(x, y + dy)) return;
		const target = x + (y + dy) * width;
		if (grid[target] !== EMPTY) return;
		// Gameplay growth is an explicit external biomass/solar-energy source.
		world.setCell(target, PLANT);
		if (world.random.next() < plantGrowthProfile.waterConsumptionChance) world.removeMatter(water);
		moved[target] = 1;
	}

	function hasFreshAir(index: number): boolean {
		return (
			atBoundary(index) ||
			neighbors(index).some((neighbor) => ventilated[neighbor] && grid[neighbor] === EMPTY)
		);
	}

	function consumeOxygen(index: number, amountKg: number): number {
		if (hasFreshAir(index)) return amountKg;
		let remaining = amountKg;
		for (const neighbor of neighbors(index)) {
			if (!isGas(grid[neighbor])) continue;
			const used = Math.min(remaining, world.oxygenKg[neighbor]);
			world.oxygenKg[neighbor] -= used;
			remaining -= used;
		}
		return amountKg - remaining;
	}

	function hasOxygen(index: number): boolean {
		if (hasFreshAir(index)) return true;
		return neighbors(index).some(
			(neighbor) => isGas(grid[neighbor]) && world.oxygenKg[neighbor] > 0,
		);
	}

	function absorbFlameHeat(index: number): void {
		for (const source of neighbors(index)) {
			if (grid[source] !== FIRE && !world.burning[source]) continue;
			const difference = world.temperatureAt(source) - world.temperatureAt(index);
			if (difference <= 0) continue;
			const capacity = heatCapacityForMass(grid[index], world.massKg[index]);
			const sourceCapacity = heatCapacityForMass(grid[source], world.massKg[source]);
			// Contact heating is conservative and cannot run past equilibrium.
			const equilibriumTransfer =
				(difference * combustionProfile.flameContactFraction) / (1 / capacity + 1 / sourceCapacity);
			// Enhanced flame-contact transfer must not drain a burning solid's
			// entire ignition reserve into its cold neighbour in a single tick.
			const reserve =
				grid[source] === FIRE
					? 0
					: energyAtTemperature(
							grid[source],
							materialsById[grid[source]].ignitionTemperatureC + combustionProfile.ignitionMarginC,
							world.pressurePa[source] || undefined,
							world.massKg[source],
						);
			const transferred = Math.min(equilibriumTransfer, Math.max(0, energy[source] - reserve));
			energy[source] -= transferred;
			energy[index] += transferred;
		}
	}

	function releaseChemicalEnergy(index: number, thermalUnits: number): boolean {
		const requestedKj = Math.min(thermalUnits / 1000, world.chemicalEnergyKj[index]);
		// Approximate oxygen per kJ (not per J). The former factor was 1000x
		// too large, so an ordinary interior flame could never burn its fuel.
		const oxygenPerKj = combustionProfile.oxygenKgPerKj;
		const chemicalKj = consumeOxygen(index, requestedKj * oxygenPerKj) / oxygenPerKj;
		if (chemicalKj <= 1e-12) return false;
		world.chemicalEnergyKj[index] = Math.max(0, world.chemicalEnergyKj[index] - chemicalKj);
		energy[index] += chemicalKj * 1000;
		return true;
	}

	function emitFlame(index: number, ignition: number): void {
		const reserve = energyAtTemperature(
			grid[index],
			ignition + combustionProfile.ignitionMarginC,
			world.pressurePa[index] || undefined,
			world.massKg[index],
		);
		const candidates = neighbors(index).sort((a, b) => a - b);
		for (const target of candidates) {
			if (target >= index + width || grid[target] !== EMPTY || moved[target]) continue;
			const transferred = Math.max(
				0,
				energyAtTemperature(
					FIRE,
					combustionProfile.emittedFlameTemperatureC,
					world.pressurePa[target] || undefined,
					world.massKg[target],
				) - energy[target],
			);
			if (energy[index] - reserve < transferred) return;
			// Burning continues every tick, but gas is released asynchronously.
			// Identically heated fuel otherwise launches an entire horizontal
			// sheet at once. A skipped release keeps its heat in the fuel;
			// emission still transfers exactly what the new flame receives.
			if (world.random.next() >= combustionProfile.flameEmissionChancePerTick) return;
			world.changeMaterial(target, FIRE);
			const [minimum, maximum] = combustionProfile.emittedLifetimeTicks;
			world.lifetime[target] = minimum + Math.floor(world.random.next() * (maximum - minimum));
			energy[target] += transferred;
			energy[index] -= transferred;
			moved[target] = 1;
			return;
		}
	}

	function update(index: number): void {
		if (moved[index] || grid[index] === EMPTY) return;
		const material = grid[index];
		// Hydrogen has its own stoichiometric water-forming reaction, never the generic smoke rule.
		if (
			material === HYDROGEN ||
			material === CARBON ||
			material === SULFUR ||
			material === LIQUID_SULFUR ||
			material === SULFUR_VAPOR
		)
			return;
		const ignition = materialsById[material].ignitionTemperatureC;
		if (Number.isFinite(ignition)) absorbFlameHeat(index);
		const temperature = world.temperatureAt(index);
		if (Number.isFinite(ignition)) {
			if (temperature < ignition || neighborOfType(index, WATER) >= 0 || !hasOxygen(index)) {
				world.burning[index] = 0;
			} else if (world.chemicalEnergyKj[index] > 1e-9) {
				world.burning[index] = releaseChemicalEnergy(index, combustionProfile.heatPerTick) ? 1 : 0;
				if (world.burning[index])
					variation[index] = Math.floor(
						world.visualRandom.next() * combustionProfile.emberPalette.length,
					);
				if (world.burning[index]) emitFlame(index, ignition);
				if (world.chemicalEnergyKj[index] <= 1e-9) world.changeMaterial(index, SMOKE);
			}
			if (material === PLANT && !world.burning[index]) grow(index);
			return;
		}
		if (material !== FIRE && material !== SMOKE) return;
		if (
			material === FIRE &&
			(temperature < combustionProfile.quenchTemperatureC || neighborOfType(index, WATER) >= 0)
		) {
			world.changeMaterial(index, world.ignitionFlame[index] ? EMPTY : SMOKE);
			moved[index] = 1;
			return;
		}
		if (material === FIRE && !hasOxygen(index)) {
			world.changeMaterial(index, world.ignitionFlame[index] ? EMPTY : SMOKE);
			moved[index] = 1;
			return;
		}
		if (lifetime[index] > 0) lifetime[index] -= 1;
		if (lifetime[index] === 0) {
			if (material === FIRE && world.ignitionFlame[index])
				// Ignition heats existing air. Restore its identity without
				// discarding its mass/heat or inventing combustion smoke.
				world.changeMaterial(index, EMPTY);
			else if (material === FIRE && world.random.next() < combustionProfile.smokeProbability)
				world.changeMaterial(index, SMOKE);
			else if (ventilated[index] || atBoundary(index)) world.removeMatter(index);
			else world.changeMaterial(index, EMPTY);
			moved[index] = 1;
			return;
		}
		if (material === FIRE) {
			variation[index] = Math.floor(world.visualRandom.next() * 4);
			// A flame is a finite hot-gas parcel. It may persist without fuel,
			// but only stored chemical energy can create additional heat.
			releaseChemicalEnergy(index, combustionProfile.flameHeatPerTick);
		}
	}
	return { update, beginStep };
}
