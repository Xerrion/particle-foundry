import { EMPTY, materialsById, STEAM } from "../materials";
import { canDisplaceFluid, isGas, isLiquid, liquidMotion } from "../materials/queries";
import {
	CELL_WIDTH_METERS,
	FIXED_TIME_STEP_SECONDS,
	GRAVITY_M_PER_S2,
} from "../simulation/physical-scale";
import type { World } from "../simulation/world";

const floatingSolids = new Uint8Array(256);
for (const material of Object.values(materialsById)) {
	floatingSolids[material.id] = Number(material.state === "solid" && material.falls);
}

/** Creates discrete conservative movement rules on a trusted world. No thermal sources or reactions. */
export function createMotion(world: World) {
	const { width, height, grid, moved } = world;
	const liquidQueue = new Int32Array(world.size);
	const liquidSeen = new Uint32Array(world.size);
	let liquidSearch = 0;

	function moveInto(index: number, target: number): void {
		const material = grid[index];
		const drop = (Math.floor(target / width) - Math.floor(index / width)) * CELL_WIDTH_METERS;
		const netFallingWork = Math.max(
			0,
			(world.massKg[index] - world.massKg[target]) * GRAVITY_M_PER_S2 * drop,
		);
		world.transport(index, target);
		if (drop > 0 && (isLiquid(material) || materialsById[material].falls)) {
			world.velocityY[target] = Math.sqrt(
				world.velocityY[target] ** 2 + (2 * netFallingWork) / world.massKg[target],
			);
			world.energy[target] -= netFallingWork;
		}
		// A parcel moves only once, but the air it leaves behind must accept the
		// next parcel in a column or plume. Otherwise every other row becomes a hole.
		if (grid[index] === EMPTY) moved[index] = 0;
	}

	function effectiveGasDensity(index: number): number {
		const kelvin = Math.max(1, world.temperatureAt(index) + 273.15);
		return materialsById[grid[index]].densityKgPerM3 * (293.15 / kelvin);
	}

	function canRiseInto(index: number, target: number, upward = false): boolean {
		if (!isGas(grid[target])) return false;
		// A denser gas can be pushed down by successive buoyant parcels, just
		// as oil can be pushed up by falling water. Its own active movement
		// remains marked complete. Do not permit processed lateral swaps.
		if (moved[target] && (!upward || grid[target] === EMPTY)) return false;
		if (grid[index] === grid[target]) return false;
		return effectiveGasDensity(index) + 1e-9 < effectiveGasDensity(target);
	}

	function canSink(material: number, target: number): boolean {
		return (
			canDisplaceFluid(material, target) ||
			(floatingSolids[target] === 1 &&
				isLiquid(material) &&
				materialsById[material].displacementDensity > materialsById[target].displacementDensity)
		);
	}

	function findLiquidOutlet(target: number, granular: number, throughGrains: boolean): number {
		liquidSearch += 1;
		if (liquidSearch === 0xffffffff) {
			liquidSeen.fill(0);
			liquidSearch = 1;
		}
		const liquid = grid[target];
		let head = 0;
		let tail = 1;
		liquidQueue[0] = target;
		liquidSeen[target] = liquidSearch;
		function visit(neighbor: number): number {
			if (grid[neighbor] === liquid || (throughGrains && grid[neighbor] === granular)) {
				if (liquidSeen[neighbor] === liquidSearch) return -1;
				liquidSeen[neighbor] = liquidSearch;
				liquidQueue[tail++] = neighbor;
				return -1;
			}
			return isGas(grid[neighbor]) && !moved[neighbor] ? neighbor : -1;
		}
		while (head < tail) {
			const cell = liquidQueue[head++];
			const x = cell % width;
			if (cell >= width) {
				const outlet = visit(cell - width);
				if (outlet >= 0) return outlet;
			}
			if (x > 0) {
				const outlet = visit(cell - 1);
				if (outlet >= 0) return outlet;
			}
			if (x + 1 < width) {
				const outlet = visit(cell + 1);
				if (outlet >= 0) return outlet;
			}
			if (cell + width < world.size && grid[cell + width] === liquid) visit(cell + width);
		}
		return -1;
	}

	function settleGranularIntoLiquid(index: number, target: number): boolean {
		// Displace water to a reachable surface rather than swapping it into
		// the sand source. Search the connected pool first, then its porous sand.
		let outlet = findLiquidOutlet(target, grid[index], false);
		if (outlet < 0) outlet = findLiquidOutlet(target, grid[index], true);
		if (outlet >= 0) {
			world.transport(target, outlet);
			moveInto(index, target);
			return true;
		}
		// An isolated top grain can exchange with water in a one-cell tube.
		// A buried grain waits until the pool has somewhere to expand.
		if (index < width || isGas(grid[index - width])) {
			moveInto(index, target);
			return true;
		}
		return false;
	}

	function tryDownwardMove(index: number, target: number, material: number): boolean {
		if (!canSink(material, grid[target])) return false;
		if (isLiquid(grid[target]) && materialsById[material].state === "granular") {
			return !moved[target] && settleGranularIntoLiquid(index, target);
		}
		if (moved[target] && !(isLiquid(material) && isLiquid(grid[target]))) return false;
		moveInto(index, target);
		return true;
	}

	function downward(index: number, direction: number, material: number): boolean {
		const x = index % width;
		const y = Math.floor(index / width);
		if (y + 1 >= height) return false;
		const down = index + width;
		if (tryDownwardMove(index, down, material)) return true;
		for (const dx of [direction, -direction]) {
			if (x + dx < 0 || x + dx >= width) continue;
			const side = index + dx;
			const target = down + dx;
			// A diagonal move needs an open route; sealed corners are not pores.
			if (!canSink(material, grid[side]) && !canSink(material, grid[down])) continue;
			if (tryDownwardMove(index, target, material)) return true;
		}
		return false;
	}

	function spread(index: number, direction: number, distance: number): void {
		const x = index % width;
		const y = Math.floor(index / width);
		const material = grid[index];
		const hasColumnAbove = y > 0 && grid[index - width] === material;
		for (const dx of [direction, -direction]) {
			let target = index;
			for (let step = 1; step <= distance; step += 1) {
				const nx = x + dx * step;
				if (nx < 0 || nx >= width) break;
				const next = index + dx * step;
				// Denser liquid also spreads through a lighter immiscible layer.
				// Restricting this to gas made water pile up like sand inside oil.
				if (!canSink(material, grid[next]) || moved[next]) break;
				const hasOutlet = y + 1 < height && canSink(material, grid[next + width]);
				// A lateral step needs somewhere lower to drain, or room to lower
				// the column above it. Equal-height swaps do no settling work and
				// just alternate liquid/air or liquid/oil cells forever (visible as
				// a blinking checkerboard at lava's three-tick motion cadence).
				if (hasOutlet || (hasColumnAbove && grid[next - width] !== material)) target = next;
				if (hasOutlet) break;
			}
			if (target === index) continue;
			for (let from = index; from !== target; from += dx) world.transport(from, from + dx);
			return;
		}
	}

	function rise(index: number, direction: number, material: number): void {
		const x = index % width;
		const y = Math.floor(index / width);
		if (
			materialsById[material].atomicNumber &&
			y + 1 < height &&
			isGas(grid[index + width]) &&
			!moved[index + width] &&
			effectiveGasDensity(index) > effectiveGasDensity(index + width) + 1e-9
		) {
			moveInto(index, index + width);
			return;
		}
		const windX = world.velocityX[index];
		if (Math.abs(windX) > 0.15 && world.random.next() < Math.min(0.9, Math.abs(windX) * 0.35)) {
			const windDirection = Math.sign(windX);
			if (x + windDirection >= 0 && x + windDirection < width) {
				const side = index + windDirection;
				if (canRiseInto(index, side)) {
					moveInto(index, side);
					return;
				}
			}
		}
		const temperature = world.temperatureAt(index);
		const profile = materialsById[material].gasMotion;
		const riseLimit = profile && temperature > profile.hotTemperature ? profile.hotRise : 1;
		function diagonalFrom(from: number): number {
			const column = from % width;
			if (from < width) return from;
			for (const dx of [direction, -direction]) {
				if (column + dx < 0 || column + dx >= width) continue;
				const side = from + dx;
				if (!canRiseInto(from, side, true) || !canRiseInto(from, side - width, true)) continue;
				moveInto(from, side);
				moveInto(side, side - width);
				return side - width;
			}
			return from;
		}
		let current = index;
		for (let distance = 0; distance < riseLimit; distance += 1) {
			if (current < width) break;
			// Small seeded drift breaks straight, phase-locked chimney stripes.
			// It still gains one row and must pass through an open side cell.
			if (distance === 0 && world.random.next() < (profile?.risingDriftChance ?? 0)) {
				const next = diagonalFrom(current);
				if (next !== current) {
					current = next;
					continue;
				}
			}
			const above = current - width;
			if (!canRiseInto(current, above, true)) break;
			moveInto(current, above);
			current = above;
		}
		if (current !== index) {
			const distanceMeters = (y - Math.floor(current / width)) * CELL_WIDTH_METERS;
			const previousVelocity = world.velocityY[current];
			world.velocityY[current] = Math.min(
				world.velocityY[current],
				-distanceMeters / FIXED_TIME_STEP_SECONDS,
			);
			world.energy[current] -=
				0.5 * world.massKg[current] * (world.velocityY[current] ** 2 - previousVelocity ** 2);
			return;
		}
		// Check both upward routes before diffusing sideways. Release air
		// at both vacated cells so the following parcel can fill the gap.
		current = diagonalFrom(index);
		if (current !== index) {
			const previousVelocity = world.velocityY[current];
			world.velocityY[current] = Math.min(
				world.velocityY[current],
				-CELL_WIDTH_METERS / FIXED_TIME_STEP_SECONDS,
			);
			world.energy[current] -=
				0.5 * world.massKg[current] * (world.velocityY[current] ** 2 - previousVelocity ** 2);
			return;
		}
		// Always taking a lateral step locks smoke into an alternating lattice.
		// Seeded, intermittent diffusion breaks that grid rhythm without making
		// replay depend on frame rate or on the renderer's random stream.
		if (world.random.next() >= (profile?.lateralChance ?? 0)) return;
		for (const dx of [direction, -direction]) {
			if (x + dx < 0 || x + dx >= width) continue;
			const side = index + dx;
			if (!canRiseInto(index, side)) continue;
			moveInto(index, side);
			return;
		}
	}

	function update(index: number, tick: number): void {
		if (moved[index] || grid[index] === EMPTY) return;
		const material = grid[index];
		const pressureDirection =
			isLiquid(material) && Math.abs(world.velocityX[index]) > 1e-6
				? Math.sign(world.velocityX[index])
				: 0;
		const direction =
			pressureDirection ||
			(isGas(material)
				? world.random.next() < 0.5
					? -1
					: 1
				: (tick + (index % width) + Math.floor(index / width)) % 2 === 0
					? -1
					: 1);
		if (isLiquid(material)) {
			const profile = liquidMotion[material];
			if (!profile || tick % profile.interval !== 0) return;
			if (!downward(index, direction, material)) spread(index, direction, profile.spread);
			return;
		}
		if (materialsById[material].falls) {
			downward(index, direction, material);
			return;
		}
		if (material === STEAM) {
			if (tick % 4 === 0) world.variation[index] = (world.variation[index] + 1) % 4;
			rise(index, direction, material);
		} else if (materialsById[material].state === "gas") rise(index, direction, material);
	}

	return { update };
}
