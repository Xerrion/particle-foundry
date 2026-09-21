import { METAL, WATER } from "../../src/materials";
import { energyAtTemperature } from "../../src/physics/thermal";
import { createPhysics } from "../../src/simulation/physics";
import { createWorld } from "../../src/simulation/world";

export function boilingPot(seed = 42, plateTemperature = 600) {
	const world = createWorld(48, 48, { seed });
	for (let y = 25; y < 44; y++) for (let x = 10; x < 38; x++) world.setCell(x + y * 48, WATER);
	for (let y = 22; y <= 44; y++) for (const x of [9, 38]) world.setCell(x + y * 48, METAL);
	for (let x = 9; x <= 38; x++) world.setCell(x + 44 * 48, METAL);
	const physics = createPhysics(world);
	function step() {
		for (let x = 10; x < 38; x++) {
			const index = x + 44 * 48;
			world.addExternalEnergy(
				index,
				Math.max(
					0,
					energyAtTemperature(METAL, plateTemperature, undefined, world.massKg[index]) -
						world.energy[index],
				),
			);
		}
		physics.step();
	}
	return { world, step };
}
