import { EMPTY, materialsById, STEAM } from "./materials";
import { CELL_WIDTH_METERS, FIXED_TIME_STEP_SECONDS } from "./physical-scale";
import type { World } from "./world";

// Derived lookup tables, compiled from the catalogue once. No material constants
// are duplicated here and the transport hot path avoids repeated object lookups.
const gasMaterials = new Uint8Array(256);
const liquidMaterials = new Uint8Array(256);
const floatingSolids = new Uint8Array(256);
const density = new Float64Array(256);
for (const material of Object.values(materialsById)) {
	gasMaterials[material.id] = Number(material.state === "ambient" || material.state === "gas");
	liquidMaterials[material.id] = Number(material.state === "liquid");
	floatingSolids[material.id] = Number(material.state === "solid" && material.falls);
	density[material.id] = material.displacementDensity;
}

/** Motion policy in fixed 60 Hz ticks, not SI viscosity. Larger intervals resist flow more. */
export const liquidMotion: Readonly<
	Record<number, { readonly interval: number; readonly spread: number } | undefined>
> = Object.fromEntries(
	Object.values(materialsById).map((material) => [material.id, material.flow]),
);

/** Identifies matter represented by the liquid transport model. */
export function isLiquid(material: number): boolean {
	return liquidMaterials[material] === 1;
}

/** Identifies cells that can be displaced by falling matter, including empty air. */
export function isGas(material: number): boolean {
	return gasMaterials[material] === 1;
}

/** A denser parcel can settle into gas or lighter liquid, without merging materials. */
export function canDisplaceFluid(material: number, target: number): boolean {
	return isGas(target) || (isLiquid(target) && density[material] > density[target]);
}

/** Creates discrete conservative movement rules on a trusted world. No thermal sources or reactions. */
export function createMotion(world: World) {
	const { width, height, grid, moved } = world;

	function moveInto(index: number, target: number): void {
		world.swap(index, target);
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
			(floatingSolids[target] === 1 && isLiquid(material) && density[material] > density[target])
		);
	}

	function downward(index: number, direction: number, material: number): boolean {
		const x = index % width;
		const y = Math.floor(index / width);
		if (y + 1 >= height) return false;
		const down = index + width;
		// Lighter liquid may already have been displaced this tick. Let the next
		// denser parcel push it upward too; otherwise sideways flow locks the
		// whole interface and water remains suspended inside oil indefinitely.
		if (canSink(material, grid[down]) && (!moved[down] || isLiquid(grid[down]))) {
			moveInto(index, down);
			return true;
		}
		for (const dx of [direction, -direction]) {
			if (x + dx < 0 || x + dx >= width) continue;
			const side = index + dx;
			const target = down + dx;
			// A diagonal move needs an open route; sealed corners are not pores.
			if (!canSink(material, grid[side]) && !canSink(material, grid[down])) continue;
			if (canSink(material, grid[target]) && (!moved[target] || isLiquid(grid[target]))) {
				moveInto(index, target);
				return true;
			}
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
			for (let from = index; from !== target; from += dx) world.swap(from, from + dx);
			return;
		}
	}

	function rise(index: number, direction: number, material: number): void {
		const x = index % width;
		const y = Math.floor(index / width);
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
			world.velocityY[current] = Math.min(
				world.velocityY[current],
				-distanceMeters / FIXED_TIME_STEP_SECONDS,
			);
			return;
		}
		// Check both upward routes before diffusing sideways. Release air
		// at both vacated cells so the following parcel can fill the gap.
		current = diagonalFrom(index);
		if (current !== index) {
			world.velocityY[current] = Math.min(
				world.velocityY[current],
				-CELL_WIDTH_METERS / FIXED_TIME_STEP_SECONDS,
			);
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
		const direction = isGas(material)
			? world.random.next() < 0.5
				? -1
				: 1
			: (tick + (index % width) + Math.floor(index / width)) % 2 === 0
				? -1
				: 1;
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
