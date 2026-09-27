import { isGas, isLiquid } from "../../materials/queries";
import type { World } from "../simulation/world";

const GAS_VELOCITY_RETENTION_PER_TICK = 0.98;

/** Propagates wind through gas and exchanges momentum with adjacent movable matter. */
export function createAirflow(world: World) {
	function mobileContact(a: number, b: number): boolean {
		const aGas = isGas(world.grid[a]);
		const bGas = isGas(world.grid[b]);
		return (
			(aGas && bGas) ||
			(aGas && (isLiquid(world.grid[b]) || world.dynamic[b] === 1)) ||
			(bGas && (isLiquid(world.grid[a]) || world.dynamic[a] === 1))
		);
	}

	function exchange(a: number, b: number, horizontal: boolean): void {
		if (!mobileContact(a, b)) return;
		const massA = world.massKg[a];
		const massB = world.massKg[b];
		const totalMass = massA + massB;
		if (totalMass <= 0) return;
		const velocity = horizontal ? world.velocityX : world.velocityY;
		const difference = velocity[a] - velocity[b];
		if (difference === 0) return;
		// Adjacent parcels share their normal momentum. An elastic collision
		// repeatedly doubles a light air cell's speed beside dense blast gas.
		const sharedVelocity = (massA * velocity[a] + massB * velocity[b]) / totalMass;
		const dissipatedJ = (0.5 * massA * massB * difference * difference) / totalMass;
		velocity[a] = sharedVelocity;
		velocity[b] = sharedVelocity;
		world.energy[a] += dissipatedJ / 2;
		world.energy[b] += dissipatedJ / 2;
	}

	function step(tick: number): void {
		const parity = tick % 2;
		for (let y = 0; y < world.height; y += 1) {
			for (let x = parity; x + 1 < world.width; x += 2) {
				const index = x + y * world.width;
				exchange(index, index + 1, true);
			}
		}
		for (let y = parity; y + 1 < world.height; y += 2) {
			for (let x = 0; x < world.width; x += 1) {
				const index = x + y * world.width;
				exchange(index, index + world.width, false);
			}
		}
		// Unresolved turbulence and drag dissipate a free gust instead of
		// leaving perpetual wind after its pressure source has vanished.
		const retainedEnergy = GAS_VELOCITY_RETENTION_PER_TICK ** 2;
		for (let index = 0; index < world.size; index += 1) {
			if (!isGas(world.grid[index])) continue;
			const vx = world.velocityX[index];
			const vy = world.velocityY[index];
			if (vx === 0 && vy === 0) continue;
			world.energy[index] += 0.5 * world.massKg[index] * (vx * vx + vy * vy) * (1 - retainedEnergy);
			world.velocityX[index] = vx * GAS_VELOCITY_RETENTION_PER_TICK;
			world.velocityY[index] = vy * GAS_VELOCITY_RETENTION_PER_TICK;
		}
	}

	return { step };
}
