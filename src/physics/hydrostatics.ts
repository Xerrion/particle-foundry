import { canDisplaceFluid, isGas, isLiquid, liquidMotion } from "../materials/queries";
import type { World } from "../simulation/world";

export interface HydrostaticTransfer {
	readonly source: number;
	readonly target: number;
	readonly material: number;
}

/**
 * Quasi-static cellular head relaxation, not a compressible pressure or momentum solver.
 * Per eligible connected same-liquid region, transfers at most one cell from a higher
 * surface to a lower adjacent gas or lighter-liquid cell. Connectivity uses cardinal edges only.
 * Displaced fluid travels back along that liquid path through adjacent swaps; every cell's
 * energy and material count are preserved. Insulated solid walls are never crossed.
 * Scratch buffers are allocated once. Input must be the trusted World that owns the fields.
 */
export function createHydrostatics(
	world: World,
	onTransfer: (transfer: HydrostaticTransfer) => void = () => {},
) {
	const { width, height, size, grid } = world;
	const visited = new Uint8Array(size);
	const queue = new Int32Array(size);
	const parent = new Int32Array(size);
	const pathVisited = new Uint32Array(size);
	let pathEpoch = 0;

	function canRelaxInto(material: number, index: number): boolean {
		if (!canDisplaceFluid(material, grid[index])) return false;
		const family = materialsById[material].phaseFamily;
		// A submerged vapor parcel is a bubble, not a lower free surface.
		// Routing a whole-column relaxation path through it teleported fresh
		// steam to the top in one tick. Local gravity handles its rise instead.
		if (!family || !isGas(grid[index]) || materialsById[grid[index]].phaseFamily !== family)
			return true;
		let above = index - width;
		while (above >= 0 && grid[above] === grid[index]) above -= width;
		return above < 0 || !isLiquid(grid[above]);
	}

	function visitNeighbors(index: number, visit: (neighbor: number) => void): void {
		const x = index % width;
		if (x > 0) visit(index - 1);
		if (x + 1 < width) visit(index + 1);
		if (index >= width) visit(index - width);
		if (index + width < size) visit(index + width);
	}

	function transfer(source: number, target: number, material: number): void {
		pathEpoch = (pathEpoch + 1) >>> 0;
		if (pathEpoch === 0) {
			pathVisited.fill(0);
			pathEpoch = 1;
		}
		let head = 0;
		let tail = 1;
		queue[0] = source;
		parent[source] = -1;
		pathVisited[source] = pathEpoch;
		let found = false;
		function enqueue(neighbor: number): void {
			if (found || pathVisited[neighbor] === pathEpoch) return;
			if (neighbor !== target && grid[neighbor] !== material) return;
			parent[neighbor] = queue[head];
			pathVisited[neighbor] = pathEpoch;
			queue[tail++] = neighbor;
			if (neighbor === target) found = true;
		}
		while (head < tail && !found) {
			visitNeighbors(queue[head], enqueue);
			head += 1;
		}
		if (!found) throw new Error("Connected liquid lost its hydrostatic path");
		for (let current = target; parent[current] !== -1; current = parent[current]) {
			world.transport(current, parent[current]);
		}
	}

	function step(tick: number): number {
		let transferCount = 0;
		visited.fill(0);
		for (let offset = 0; offset < size; offset += 1) {
			const start = tick % 2 === 0 ? offset : size - 1 - offset;
			const material = grid[start];
			const profile = liquidMotion[material];
			if (visited[start] || !isLiquid(material) || !profile || tick % profile.interval !== 0)
				continue;
			let head = 0;
			let tail = 1;
			let source = -1;
			let target = -1;
			let sourceY = height;
			let targetY = -1;
			let supported = false;
			queue[0] = start;
			visited[start] = 1;
			function consider(neighbor: number): void {
				if (grid[neighbor] === material && !visited[neighbor]) {
					visited[neighbor] = 1;
					queue[tail++] = neighbor;
				} else if (canRelaxInto(material, neighbor)) {
					const y = Math.floor(neighbor / width);
					if (y > targetY) {
						target = neighbor;
						targetY = y;
					}
				}
			}
			while (head < tail) {
				const index = queue[head++];
				const y = Math.floor(index / width);
				if (y === height - 1 || (grid[index + width] !== material && !isGas(grid[index + width]))) {
					supported = true;
				}
				const isFreeSurface = y === 0 || canDisplaceFluid(material, grid[index - width]);
				if (isFreeSurface && y < sourceY) {
					source = index;
					sourceY = y;
				}
				visitNeighbors(index, consider);
			}
			if (!supported || source < 0 || target < 0 || sourceY >= targetY) continue;
			transfer(source, target, material);
			onTransfer({ source, target, material });
			transferCount += 1;
			visited[target] = 1;
		}
		return transferCount;
	}

	return { step };
}

import { materialsById } from "../materials";
