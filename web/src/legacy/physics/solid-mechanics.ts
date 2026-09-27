import { materialsById } from "../../materials/physical-properties";
import { isGas, isLiquid } from "../../materials/queries";
import {
	CELL_WIDTH_METERS,
	FIXED_TIME_STEP_SECONDS,
	GRAVITY_M_PER_S2,
} from "../simulation/physical-scale";
import type { World } from "../simulation/world";

/** Signed velocity integration, subcell displacement and collision-aware traversal. */
export function createSolidMechanics(world: World) {
	const processed = new Uint8Array(world.size);
	function canEnter(index: number, target: number): boolean {
		return (
			isGas(world.grid[target]) ||
			(isLiquid(world.grid[target]) &&
				materialsById[world.grid[index]].densityKgPerM3 >
					materialsById[world.grid[target]].densityKgPerM3)
		);
	}
	function stop(index: number, horizontal: boolean): void {
		const velocity = horizontal ? world.velocityX : world.velocityY;
		const displacement = horizontal ? world.displacementX : world.displacementY;
		world.energy[index] += 0.5 * world.massKg[index] * velocity[index] ** 2;
		velocity[index] = 0;
		displacement[index] = 0;
	}
	function traverse(start: number, horizontal: boolean): number {
		const displacement = horizontal ? world.displacementX : world.displacementY;
		let index = start;
		while (Math.abs(displacement[index]) >= 1) {
			const direction = Math.sign(displacement[index]);
			const nx = (index % world.width) + (horizontal ? direction : 0);
			const ny = Math.floor(index / world.width) + (horizontal ? 0 : direction);
			const target = nx + ny * world.width;
			if (!world.inBounds(nx, ny) || !canEnter(index, target)) {
				stop(index, horizontal);
				break;
			}
			displacement[index] -= direction;
			world.transport(index, target);
			processed[target] = 1;
			index = target;
		}
		return index;
	}
	function step(): void {
		processed.fill(0);
		for (let start = world.size - 1; start >= 0; start--) {
			const definition = materialsById[world.grid[start]];
			if (
				processed[start] ||
				!world.dynamic[start] ||
				definition.state !== "solid" ||
				definition.falls
			)
				continue;
			processed[start] = 1;
			const below = start + world.width;
			const supported = below >= world.size || !canEnter(start, below);
			if (supported && world.velocityY[start] >= 0) {
				// Persistent contact balances gravity. Dissipate only incoming motion.
				stop(start, false);
			} else {
				const displacedDensity =
					below < world.size && isLiquid(world.grid[below])
						? materialsById[world.grid[below]].densityKgPerM3
						: 0;
				const acceleration = GRAVITY_M_PER_S2 * (1 - displacedDensity / definition.densityKgPerM3);
				const oldVelocity = world.velocityY[start];
				world.velocityY[start] += acceleration * FIXED_TIME_STEP_SECONDS;
				// Until a cell boundary is crossed, the cell-centered energy budget
				// carries subcell gravitational work as a reversible thermal debit.
				world.energy[start] -=
					0.5 * world.massKg[start] * (world.velocityY[start] ** 2 - oldVelocity ** 2);
				world.displacementY[start] +=
					((oldVelocity + world.velocityY[start]) * 0.5 * FIXED_TIME_STEP_SECONDS) /
					CELL_WIDTH_METERS;
			}
			world.displacementX[start] +=
				(world.velocityX[start] * FIXED_TIME_STEP_SECONDS) / CELL_WIDTH_METERS;
			const index = traverse(start, true);
			traverse(index, false);
		}
	}
	return { step };
}
