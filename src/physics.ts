import { createFluidSolver } from "./fluid-solver";
import { createGasDynamics } from "./gas-dynamics";
import { createMotion, isGas } from "./motion";
import { createReactions } from "./reactions";
import { createSolidMechanics } from "./solid-mechanics";
import { createThermalSolver } from "./thermal";
import type { World } from "./world";

/** Orders one fixed 1/60-second model tick. No browser, brush, or visual state belongs here. */
export function createPhysics(world: World) {
	const thermal = createThermalSolver(world.width, world.height);
	const boiling = createBoiling(world);
	const motion = createMotion(world);
	const reactions = createReactions(world);
	const fluids = createFluidSolver(world);
	const gas = createGasDynamics(world);
	const solids = createSolidMechanics(world);
	let tick = 0;
	function step(): void {
		thermal.diffuse(world.grid, world.energy);
		gas.derivePressure();
		for (let i = 0; i < world.size; i += 1) world.applyPhase(i);
		boiling.step(tick);
		world.moved.fill(0);
		gas.step();
		reactions.beginStep();
		// Resolve contact before either the flame or the fuel can move away.
		for (let index = 0; index < world.size; index += 1) reactions.update(index);
		// Reactions mark newly created cells to prevent reacting twice in that
		// pass. Transport has its own pass: fresh flames and quenched smoke
		// must be allowed to rise immediately, not pin alternate plume rows.
		world.moved.fill(0);
		for (let y = world.height - 1; y >= 0; y -= 1) {
			for (let column = 0; column < world.width; column += 1) {
				const x = tick % 2 === 0 ? column : world.width - 1 - column;
				const index = x + y * world.width;
				if (!isGas(world.grid[index])) motion.update(index, tick);
			}
		}
		// Buoyant plumes follow vacated air from the top down, just as falling
		// liquid follows it from the bottom up.
		for (let y = 0; y < world.height; y += 1) {
			for (let column = 0; column < world.width; column += 1) {
				const x = tick % 2 === 0 ? column : world.width - 1 - column;
				const index = x + y * world.width;
				if (isGas(world.grid[index])) motion.update(index, tick);
			}
		}
		// Pressure relaxation is a separate pass over supported pools. A free-falling
		// stroke must not be pinned in midair by a hydrostatic transfer path.
		fluids.step(tick);
		solids.step();
		tick += 1;
	}
	return {
		step,
		reset: (): void => {
			tick = 0;
			fluids.reset();
			gas.derivePressure();
		},
	};
}

import { createBoiling } from "./boiling";
