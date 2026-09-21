import { EMPTY } from "./material-ids";
import { materialsById } from "./material-physics";
import { isGas, isLiquid } from "./motion";
import { FIXED_TIME_STEP_SECONDS, GRAVITY_M_PER_S2 } from "./physical-scale";
import type { World } from "./world";

function isSupportedDynamicSolid(material: number): boolean {
	return materialsById[material].state === "solid" && !materialsById[material].falls;
}

/** Mechanics for explicitly movable solid parcels. Painted walls stay anchored. */
export function createSolidMechanics(world: World) {
	function dissipate(index: number): void {
		const mass = world.massKg[index];
		const speedSquared = world.velocityX[index] ** 2 + world.velocityY[index] ** 2;
		world.energy[index] += 0.5 * mass * speedSquared;
		world.velocityX[index] = 0;
		world.velocityY[index] = 0;
	}

	function step(): void {
		for (let y = world.height - 1; y >= 0; y -= 1) {
			for (let x = 0; x < world.width; x += 1) {
				const index = x + y * world.width;
				const material = world.grid[index];
				if (!world.dynamic[index] || !isSupportedDynamicSolid(material)) continue;
				world.velocityY[index] += GRAVITY_M_PER_S2 * FIXED_TIME_STEP_SECONDS;
				if (y + 1 >= world.height) {
					dissipate(index);
					continue;
				}
				const target = index + world.width;
				const targetMaterial = world.grid[target];
				const displacedDensity = materialsById[targetMaterial].densityKgPerM3;
				const density = materialsById[material].densityKgPerM3;
				const canDisplace =
					isGas(targetMaterial) || (isLiquid(targetMaterial) && density > displacedDensity);
				if (canDisplace && !world.moved[target]) {
					if (isLiquid(targetMaterial)) {
						const buoyancyRatio = Math.min(1, displacedDensity / density);
						world.velocityY[index] *= 1 - buoyancyRatio;
						world.velocityY[target] -= world.velocityY[index] * 0.35;
					}
					world.swap(index, target);
				} else if (targetMaterial !== EMPTY) dissipate(index);
			}
		}
	}

	return { step };
}
