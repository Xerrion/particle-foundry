import { EMPTY } from "../materials";
import { isGas } from "../materials/queries";
import { createAirflow } from "../physics/airflow";
import { createBoiling } from "../physics/boiling";
import { createElectricity } from "../physics/electricity";
import { createElementReactions } from "../physics/element-reactions";
import { createExplosions } from "../physics/explosions";
import { createFluidSolver } from "../physics/fluid-solver";
import { createGasDynamics } from "../physics/gas-dynamics";
import { createMotion } from "../physics/motion";
import { createNeutralization } from "../physics/neutralization";
import { createReactions } from "../physics/reactions";
import { createSolidMechanics } from "../physics/solid-mechanics";
import { createThermalSolver } from "../physics/thermal";
import { AMBIENT_PRESSURE_PA, AMBIENT_TEMPERATURE_C } from "./physical-scale";
import type { World } from "./world";

/** Orders one fixed 1/60-second model tick. No browser, brush, or visual state belongs here. */
export function createPhysics(world: World) {
	const thermal = createThermalSolver(world.width, world.height);
	const boiling = createBoiling(world);
	const motion = createMotion(world);
	const reactions = createReactions(world);
	const explosions = createExplosions(world);
	const electricity = createElectricity(world);
	const neutralization = createNeutralization(world);
	const elementReactions = createElementReactions(world);
	const airflow = createAirflow(world);
	const fluids = createFluidSolver(world);
	const gas = createGasDynamics(world);
	const solids = createSolidMechanics(world);
	let tick = 0;
	function ventOpenEdges(): boolean {
		if (world.boundariesEnabled) return false;
		let changed = false;
		function vent(index: number): void {
			if (!isGas(world.grid[index])) return;
			if (
				world.grid[index] === EMPTY &&
				Math.abs(world.pressurePa[index] - AMBIENT_PRESSURE_PA) < 1e-6 &&
				Math.abs(world.temperatureAt(index) - AMBIENT_TEMPERATURE_C) < 1e-6 &&
				world.velocityX[index] === 0 &&
				world.velocityY[index] === 0
			)
				return;
			world.setCell(index, EMPTY);
			changed = true;
		}
		for (let x = 0; x < world.width; x += 1) vent(x);
		for (let y = 1; y < world.height; y += 1) {
			vent(y * world.width);
			if (world.width > 1) vent(y * world.width + world.width - 1);
		}
		return changed;
	}
	function step(): void {
		gas.derivePressure();
		thermal.diffuse(world.grid, world.energy, world.pressurePa, world.massKg);
		gas.derivePressure();
		for (let i = 0; i < world.size; i += 1) world.applyPhase(i);
		boiling.step(tick);
		world.moved.fill(0);
		gas.step();
		electricity.step();
		neutralization.step();
		elementReactions.step();
		explosions.step();
		airflow.step(tick);
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
		gas.derivePressure();
		if (ventOpenEdges()) gas.derivePressure();
		tick += 1;
	}
	return {
		step,
		blast: explosions.blast,
		refreshPressure: gas.derivePressure,
		reset: (): void => {
			tick = 0;
			fluids.reset();
			gas.derivePressure();
		},
	};
}
