import { EMPTY } from "./material-ids";
import { materialsById } from "./material-physics";
import { isGas, isLiquid } from "./motion";
import { AMBIENT_PRESSURE_PA, FIXED_TIME_STEP_SECONDS, IDEAL_GAS_CONSTANT } from "./physical-scale";
import type { World } from "./world";

/** Ideal-gas pressure and conservative pressure impulses on neighboring parcels. */
export function createGasDynamics(world: World) {
	const pressureDelta = new Float64Array(world.size);

	function derivePressure(): void {
		for (let index = 0; index < world.size; index += 1) {
			const material = world.grid[index];
			if (material === EMPTY) {
				world.pressurePa[index] = AMBIENT_PRESSURE_PA;
				continue;
			}
			if (!isGas(material)) {
				world.pressurePa[index] = 0;
				continue;
			}
			const molarMass = materialsById[material].molarMassKgPerMol;
			const kelvin = Math.max(1, world.temperatureAt(index) + 273.15);
			const moles = molarMass > 0 ? world.massKg[index] / molarMass : 0;
			world.pressurePa[index] =
				(moles * IDEAL_GAS_CONSTANT * kelvin) / Math.max(Number.MIN_VALUE, world.volumeM3[index]);
		}
		for (let index = 0; index < world.size; index += 1) {
			if (!isLiquid(world.grid[index])) continue;
			const x = index % world.width;
			let boundaryPressure = AMBIENT_PRESSURE_PA;
			for (const neighbor of [
				x > 0 ? index - 1 : -1,
				x + 1 < world.width ? index + 1 : -1,
				index >= world.width ? index - world.width : -1,
				index + world.width < world.size ? index + world.width : -1,
			]) {
				if (neighbor >= 0 && isGas(world.grid[neighbor])) {
					boundaryPressure = Math.max(boundaryPressure, world.pressurePa[neighbor]);
				}
			}
			world.pressurePa[index] = boundaryPressure;
		}
	}

	function exchange(a: number, b: number): void {
		const aGas = isGas(world.grid[a]);
		const bGas = isGas(world.grid[b]);
		if (!aGas && !bGas) return;
		const difference = world.pressurePa[a] - world.pressurePa[b];
		const impulse =
			Math.max(-50, Math.min(50, difference / AMBIENT_PRESSURE_PA)) * FIXED_TIME_STEP_SECONDS;
		pressureDelta[a] -= impulse;
		pressureDelta[b] += impulse;
	}

	function step(): void {
		derivePressure();
		pressureDelta.fill(0);
		for (let y = 0; y < world.height; y += 1) {
			for (let x = 0; x < world.width; x += 1) {
				const index = x + y * world.width;
				if (x + 1 < world.width) exchange(index, index + 1);
				if (y + 1 < world.height) exchange(index, index + world.width);
			}
		}
		for (let index = 0; index < world.size; index += 1) {
			const coupling = isGas(world.grid[index])
				? 1
				: isLiquid(world.grid[index]) || world.dynamic[index]
					? 0.25
					: 0;
			if (coupling === 0) continue;
			world.velocityY[index] += pressureDelta[index] * coupling;
		}
	}

	derivePressure();
	return { step, derivePressure };
}
