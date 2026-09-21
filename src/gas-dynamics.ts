import { EMPTY } from "./material-ids";
import { materialsById } from "./material-physics";
import { isGas, isLiquid } from "./motion";
import {
	AMBIENT_PRESSURE_PA,
	CELL_WIDTH_METERS,
	FIXED_TIME_STEP_SECONDS,
	GRAVITY_M_PER_S2,
	IDEAL_GAS_CONSTANT,
} from "./physical-scale";
import type { World } from "./world";

/** Gas and liquid pressure with bounded impulses across neighboring mobile parcels. */
export function createGasDynamics(world: World) {
	const pressureDeltaX = new Float64Array(world.size);
	const pressureDeltaY = new Float64Array(world.size);

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
		// Integrate a liquid column from its free surface. A solid interrupts
		// the column; neighboring gas sets the pressure at an exposed cell.
		for (let index = 0; index < world.size; index += 1) {
			if (!isLiquid(world.grid[index])) continue;
			const x = index % world.width;
			let boundaryPressure = AMBIENT_PRESSURE_PA;
			if (x > 0 && isGas(world.grid[index - 1]))
				boundaryPressure = Math.max(boundaryPressure, world.pressurePa[index - 1]);
			if (x + 1 < world.width && isGas(world.grid[index + 1]))
				boundaryPressure = Math.max(boundaryPressure, world.pressurePa[index + 1]);
			if (index >= world.width && isGas(world.grid[index - world.width]))
				boundaryPressure = Math.max(boundaryPressure, world.pressurePa[index - world.width]);
			if (index + world.width < world.size && isGas(world.grid[index + world.width]))
				boundaryPressure = Math.max(boundaryPressure, world.pressurePa[index + world.width]);
			const above = index - world.width;
			if (above >= 0 && isLiquid(world.grid[above])) {
				boundaryPressure = Math.max(boundaryPressure, world.pressurePa[above]);
			}
			const density = materialsById[world.grid[index]].densityKgPerM3;
			world.pressurePa[index] = boundaryPressure + density * GRAVITY_M_PER_S2 * CELL_WIDTH_METERS;
		}
	}

	function exchange(a: number, b: number, horizontal: boolean): void {
		const aGas = isGas(world.grid[a]);
		const bGas = isGas(world.grid[b]);
		const aLiquid = isLiquid(world.grid[a]);
		const bLiquid = isLiquid(world.grid[b]);
		if ((!aGas && !aLiquid && !world.dynamic[a]) || (!bGas && !bLiquid && !world.dynamic[b]))
			return;
		// The vertical liquid gradient already balances gravity in a resting pool.
		if (!horizontal && aLiquid && bLiquid) return;
		const pressureA = aGas || aLiquid ? world.pressurePa[a] : AMBIENT_PRESSURE_PA;
		const pressureB = bGas || bLiquid ? world.pressurePa[b] : AMBIENT_PRESSURE_PA;
		const difference = pressureA - pressureB;
		const impulse =
			Math.max(-50, Math.min(50, difference / AMBIENT_PRESSURE_PA)) * FIXED_TIME_STEP_SECONDS;
		const delta = horizontal ? pressureDeltaX : pressureDeltaY;
		delta[a] -= impulse;
		delta[b] += impulse;
	}

	function step(): void {
		derivePressure();
		pressureDeltaX.fill(0);
		pressureDeltaY.fill(0);
		for (let y = 0; y < world.height; y += 1) {
			for (let x = 0; x < world.width; x += 1) {
				const index = x + y * world.width;
				if (x + 1 < world.width) exchange(index, index + 1, true);
				if (y + 1 < world.height) exchange(index, index + world.width, false);
			}
		}
		for (let index = 0; index < world.size; index += 1) {
			const coupling = isGas(world.grid[index])
				? 1
				: isLiquid(world.grid[index]) || world.dynamic[index]
					? 0.25
					: 0;
			if (coupling === 0) continue;
			const deltaX = pressureDeltaX[index] * coupling;
			const deltaY = pressureDeltaY[index] * coupling;
			if (deltaX === 0 && deltaY === 0) continue;
			const vx = world.velocityX[index];
			const vy = world.velocityY[index];
			const nextX = vx + deltaX;
			const nextY = vy + deltaY;
			const kineticChangeJ =
				0.5 * world.massKg[index] * (nextX * nextX + nextY * nextY - vx * vx - vy * vy);
			world.energy[index] -= kineticChangeJ;
			world.velocityX[index] = nextX;
			world.velocityY[index] = nextY;
		}
	}

	derivePressure();
	return { step, derivePressure };
}
