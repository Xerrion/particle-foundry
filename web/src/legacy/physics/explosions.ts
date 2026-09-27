import { EMPTY, FIRE, materialsById } from "../../materials";
import { isGas, isLiquid } from "../../materials/queries";
import { CELL_VOLUME_M3 } from "../simulation/physical-scale";
import type { World } from "../simulation/world";
import { energyAtTemperature } from "./thermal";

/** A local, energy-funded blast. Cardinal reachability keeps solid walls opaque. */
export function createExplosions(world: World) {
	const explosiveIds = Object.values(materialsById)
		.filter((material) => material.explosive)
		.map((material) => material.id);
	const visited = new Uint32Array(world.size);
	const queue = new Int32Array(world.size);
	const pending = new Int32Array(world.size);
	let epoch = 0;

	function mobile(index: number): boolean {
		const material = world.grid[index];
		return (
			isGas(material) ||
			isLiquid(material) ||
			materialsById[material].falls ||
			world.dynamic[index] === 1
		);
	}

	function release(index: number, radius: number, speed: number, releasedJ: number): void {
		epoch = (epoch + 1) >>> 0;
		if (epoch === 0) {
			visited.fill(0);
			epoch = 1;
		}
		const originX = index % world.width;
		const originY = Math.floor(index / world.width);
		let head = 0;
		let tail = 1;
		queue[0] = index;
		visited[index] = epoch;
		function visit(target: number): void {
			if (visited[target] === epoch || !mobile(target)) return;
			const dx = (target % world.width) - originX;
			const dy = Math.floor(target / world.width) - originY;
			if (dx * dx + dy * dy > radius * radius) return;
			visited[target] = epoch;
			queue[tail++] = target;
		}
		while (head < tail) {
			const current = queue[head++];
			const x = current % world.width;
			if (x > 0) visit(current - 1);
			if (x + 1 < world.width) visit(current + 1);
			if (current >= world.width) visit(current - world.width);
			if (current + world.width < world.size) visit(current + world.width);
		}

		if (world.grid[index] !== FIRE && isGas(world.grid[index])) {
			world.changeMaterial(index, FIRE);
			world.volumeM3[index] = Math.min(world.volumeM3[index], CELL_VOLUME_M3);
		}
		let budgetJ = releasedJ;
		const fireReserveJ = Math.max(
			0,
			energyAtTemperature(FIRE, 450, undefined, world.massKg[index]) - world.energy[index],
		);
		for (let cursor = 1; cursor < tail; cursor += 1) {
			const target = queue[cursor];
			const dx = (target % world.width) - originX;
			const dy = Math.floor(target / world.width) - originY;
			const distance = Math.hypot(dx, dy);
			if (world.grid[target] === EMPTY && distance <= Math.min(2, radius)) {
				const flameEnergy = Math.max(
					0,
					energyAtTemperature(FIRE, 450, undefined, world.massKg[target]) - world.energy[target],
				);
				if (budgetJ - flameEnergy >= fireReserveJ) {
					world.changeMaterial(target, FIRE);
					world.volumeM3[target] = Math.min(world.volumeM3[target], CELL_VOLUME_M3);
					world.energy[index] -= flameEnergy;
					world.energy[target] += flameEnergy;
					budgetJ -= flameEnergy;
				}
			}
			const addedSpeed = speed * (1 - distance / (radius + 1));
			const vx = world.velocityX[target];
			const vy = world.velocityY[target];
			const nextX = vx + (dx / distance) * addedSpeed;
			const nextY = vy + (dy / distance) * addedSpeed;
			const kineticJ =
				0.5 * world.massKg[target] * (nextX * nextX + nextY * nextY - vx * vx - vy * vy);
			if (kineticJ > budgetJ - fireReserveJ) continue;
			world.velocityX[target] = nextX;
			world.velocityY[target] = nextY;
			world.energy[index] -= kineticJ;
			budgetJ -= kineticJ;
		}
		world.applyPhase(index);
	}

	function detonate(index: number): boolean {
		const profile = materialsById[world.grid[index]].explosive;
		if (!profile || world.chemicalEnergyKj[index] <= 0) return false;
		const releasedJ = world.chemicalEnergyKj[index] * 1000;
		world.chemicalEnergyKj[index] = 0;
		world.energy[index] += releasedJ;
		world.changeMaterial(index, FIRE);
		world.volumeM3[index] = Math.min(world.volumeM3[index], CELL_VOLUME_M3);
		release(index, profile.radiusCells, profile.impulseSpeedMPerS, releasedJ);
		return true;
	}

	function blast(x: number, y: number, radius: number): boolean {
		if (
			!Number.isSafeInteger(x) ||
			!Number.isSafeInteger(y) ||
			!Number.isInteger(radius) ||
			radius < 1 ||
			radius > 12
		) {
			throw new RangeError("Blast requires integer coordinates and radius in 1..12");
		}
		if (!world.inBounds(x, y)) return false;
		const index = x + y * world.width;
		if (!mobile(index)) return false;
		if (materialsById[world.grid[index]].explosive) return detonate(index);
		const releasedJ = radius * 800;
		world.addExternalEnergy(index, releasedJ);
		release(index, radius, 3, releasedJ);
		return true;
	}

	function step(): number {
		if (!explosiveIds.some((id) => world.countMaterial(id) > 0)) return 0;
		let count = 0;
		for (let index = 0; index < world.size; index += 1) {
			const profile = materialsById[world.grid[index]].explosive;
			if (!profile || world.chemicalEnergyKj[index] <= 0) continue;
			const x = index % world.width;
			const touchingFire =
				(x > 0 && world.grid[index - 1] === FIRE) ||
				(x + 1 < world.width && world.grid[index + 1] === FIRE) ||
				(index >= world.width && world.grid[index - world.width] === FIRE) ||
				(index + world.width < world.size && world.grid[index + world.width] === FIRE);
			if (world.temperatureAt(index) >= profile.triggerTemperatureC || touchingFire) {
				pending[count++] = index;
			}
		}
		for (let cursor = 0; cursor < count; cursor += 1) detonate(pending[cursor]);
		return count;
	}

	return { blast, detonate, step };
}
