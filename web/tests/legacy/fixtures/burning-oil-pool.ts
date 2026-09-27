import { energyAtTemperature } from "../../../src/legacy/physics/thermal";
import { createPhysics } from "../../../src/legacy/simulation/physics";
import { createWorld, type World } from "../../../src/legacy/simulation/world";
import { OIL, STONE } from "../../../src/materials";

/** Wide, evenly ignited oil surface reproducing the reported detached flame sheets. */
export function burningOilPool(seed = 42, ignitionTemperature = 800) {
	const world = createWorld(64, 80, { seed });
	const firstX = 10;
	const lastX = 53;
	const surfaceY = 70;
	for (let y = 65; y < 80; y++) {
		world.setCell(firstX - 1 + y * 64, STONE);
		world.setCell(lastX + 1 + y * 64, STONE);
	}
	for (let x = firstX; x <= lastX; x++) {
		world.setCell(x + 79 * 64, STONE);
		for (let y = surfaceY; y < 79; y++) world.setCell(x + y * 64, OIL);
		world.energy[x + surfaceY * 64] = energyAtTemperature(
			OIL,
			ignitionTemperature,
			undefined,
			world.massKg[x + surfaceY * 64],
		);
	}
	return { world, physics: createPhysics(world), firstX, lastX, surfaceY };
}

/** Longest solid row of one material, excluding the burning surface itself. */
export function longestElevatedRun(world: World, material: number, surfaceY: number): number {
	let longest = 0;
	for (let y = 0; y < surfaceY - 2; y++) {
		let run = 0;
		for (let x = 0; x < world.width; x++) {
			run = world.grid[x + y * world.width] === material ? run + 1 : 0;
			longest = Math.max(longest, run);
		}
	}
	return longest;
}
