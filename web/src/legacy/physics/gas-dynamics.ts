import { EMPTY } from "../../materials/ids";
import { materialsById } from "../../materials/physical-properties";
import { isGas, isLiquid } from "../../materials/queries";
import {
	AMBIENT_PRESSURE_PA,
	AMBIENT_TEMPERATURE_C,
	CELL_VOLUME_M3,
	CELL_WIDTH_METERS,
	FIXED_TIME_STEP_SECONDS,
	GRAVITY_M_PER_S2,
	IDEAL_GAS_CONSTANT,
} from "../simulation/physical-scale";
import type { World } from "../simulation/world";
import { temperatureFromEnergy } from "./thermal";

const MAX_PRESSURE_ACCELERATION_M_PER_S2 = 3;

/** Gas and liquid pressure with bounded impulses across neighboring mobile parcels. */
export function createGasDynamics(world: World) {
	const pressureDeltaX = new Float64Array(world.size);
	const pressureDeltaY = new Float64Array(world.size);
	const visited = new Uint8Array(world.size);
	const queue = new Int32Array(world.size);
	const ambientKelvin = AMBIENT_TEMPERATURE_C + 273.15;
	const airDensity = materialsById[EMPTY].densityKgPerM3;

	function pressureVolume(index: number, pressure = AMBIENT_PRESSURE_PA): number {
		const material = world.grid[index];
		const kelvin = Math.max(
			1,
			temperatureFromEnergy(material, world.energy[index], pressure, world.massKg[index]) + 273.15,
		);
		if (material === EMPTY) {
			// Calibrate the represented air mass to the specified ambient pressure.
			return (world.massKg[index] / airDensity) * AMBIENT_PRESSURE_PA * (kelvin / ambientKelvin);
		}
		return (
			(world.massKg[index] / materialsById[material].molarMassKgPerMol) *
			IDEAL_GAS_CONSTANT *
			kelvin
		);
	}

	function resolvePressure(
		indices: Int32Array | number[],
		count: number,
		volume = count * CELL_VOLUME_M3,
	): number {
		let sum = 0;
		let pressureDependent = false;
		for (let j = 0; j < count; j++) {
			sum += pressureVolume(indices[j]);
			pressureDependent ||= !!materialsById[world.grid[indices[j]]].phaseFamily?.transitions.some(
				(t) => t.vaporPressure,
			);
		}
		if (!pressureDependent) return sum / volume;
		// Solve EOS and the pressure-aware enthalpy curve together instead of
		// repeatedly changing temperature when pressure is refreshed.
		let low = 1e-6;
		let high = 0;
		for (let j = 0; j < count; j++) high += pressureVolume(indices[j], low);
		high = Math.max(1, high / volume);
		for (let iteration = 0; iteration < 40; iteration++) {
			const mid = (low + high) / 2;
			let pv = 0;
			for (let j = 0; j < count; j++) pv += pressureVolume(indices[j], mid);
			if (pv / volume > mid) low = mid;
			else high = mid;
		}
		return (low + high) / 2;
	}

	function derivePressure(): void {
		let hasGas = false;
		let hasBarrier = false;
		for (let index = 0; index < world.size; index += 1) {
			const material = world.grid[index];
			if (material === EMPTY) {
				world.pressurePa[index] = pressureVolume(index) / CELL_VOLUME_M3;
				hasGas = true;
				continue;
			}
			if (!isGas(material)) {
				world.pressurePa[index] = 0;
				hasBarrier = true;
				continue;
			}
			hasGas = true;
			// Open parcels still use free-expansion reference volume. Replacing
			// this approximation requires conservative phase-volume transport;
			// sealed chambers below use the actual available geometric volume.
			world.pressurePa[index] = resolvePressure(
				[index],
				1,
				Math.max(Number.MIN_VALUE, world.volumeM3[index]),
			);
		}
		// Connected gas in a sealed vessel shares one pressure. Empty cells hold
		// real air mass; treating each as an ambient reservoir erases blast pressure.
		// A field made entirely of gas is connected to the open boundary.
		if (hasGas && hasBarrier) visited.fill(0);
		for (let start = 0; hasGas && hasBarrier && start < world.size; start += 1) {
			if (visited[start] || !isGas(world.grid[start])) continue;
			let head = 0;
			let tail = 1;
			let vented = false;
			queue[0] = start;
			visited[start] = 1;
			function visit(neighbor: number): void {
				if (visited[neighbor] || !isGas(world.grid[neighbor])) return;
				visited[neighbor] = 1;
				queue[tail++] = neighbor;
			}
			while (head < tail) {
				const index = queue[head++];
				const x = index % world.width;
				if (
					x === 0 ||
					x === world.width - 1 ||
					index < world.width ||
					index >= world.size - world.width
				)
					vented = true;
				if (x > 0) visit(index - 1);
				if (x + 1 < world.width) visit(index + 1);
				if (index >= world.width) visit(index - world.width);
				if (index + world.width < world.size) visit(index + world.width);
			}
			if (vented) continue;
			const pressure = resolvePressure(queue, tail);
			for (let cursor = 0; cursor < tail; cursor += 1) world.pressurePa[queue[cursor]] = pressure;
		}
		// Integrate a liquid column from its free surface. A solid interrupts
		// the column; neighboring gas sets the pressure at an exposed cell.
		for (let index = 0; index < world.size; index += 1) {
			if (!isLiquid(world.grid[index])) continue;
			const x = index % world.width;
			let boundaryPressure = 0;
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
			if (boundaryPressure === 0) boundaryPressure = AMBIENT_PRESSURE_PA;
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
		// Face traction, not a second difference of cell pressure. Equal and
		// opposite impulses preserve momentum even for unequal parcel masses.
		// Ambient gauge pressure is the exterior traction at exposed boundaries.
		const faceGaugePressure = (pressureA + pressureB) * 0.5 - AMBIENT_PRESSURE_PA;
		const massA = world.massKg[a];
		const massB = world.massKg[b];
		const maximumImpulse =
			Math.min(massA, massB) * MAX_PRESSURE_ACCELERATION_M_PER_S2 * FIXED_TIME_STEP_SECONDS;
		const rawImpulse = faceGaugePressure * CELL_WIDTH_METERS ** 2 * FIXED_TIME_STEP_SECONDS;
		const impulse = Math.max(-maximumImpulse, Math.min(maximumImpulse, rawImpulse));
		const delta = horizontal ? pressureDeltaX : pressureDeltaY;
		delta[a] -= impulse / massA;
		delta[b] += impulse / massB;
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
			const coupling =
				isGas(world.grid[index]) || isLiquid(world.grid[index]) || world.dynamic[index] ? 1 : 0;
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
