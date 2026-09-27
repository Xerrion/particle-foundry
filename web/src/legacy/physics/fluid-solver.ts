import { materialsById } from "../../materials/physical-properties";
import { isLiquid } from "../../materials/queries";
import {
	CELL_WIDTH_METERS,
	FIXED_TIME_STEP_SECONDS,
	GRAVITY_M_PER_S2,
} from "../simulation/physical-scale";
import type { World } from "../simulation/world";
import { createHydrostatics, type HydrostaticTransfer } from "./hydrostatics";

export interface FluidColumns {
	readonly volumeM3: Float64Array;
	readonly surfaceHeightM: Float64Array;
	readonly verticalVelocityMPerS: Float64Array;
}

/**
 * Derived column measurements and head relaxation over the cellular transport grid. The World owns
 * every parcel's mass, volume, heat and momentum; these column arrays are derived
 * scratch state only. Geometry/connectivity remains cell-based, so stacked pools
 * and overhangs are supported by the hydrostatic path search.
 */
export function createFluidSolver(world: World) {
	const volumeM3 = new Float64Array(world.width);
	const surfaceHeightM = new Float64Array(world.width);
	const verticalVelocityMPerS = new Float64Array(world.width);
	const previousHeightM = new Float64Array(world.width);

	function recordTransfer(transfer: HydrostaticTransfer): void {
		const sourceX = transfer.source % world.width;
		const targetX = transfer.target % world.width;
		const dropCells =
			Math.floor(transfer.target / world.width) - Math.floor(transfer.source / world.width);
		const viscosity = materialsById[transfer.material].viscosityPas;
		const damping = 1 / (1 + Math.sqrt(Math.max(0, viscosity)));
		const mass = world.massKg[transfer.target];
		const potentialLossJ = Math.max(0, dropCells) * CELL_WIDTH_METERS * GRAVITY_M_PER_S2 * mass;
		const oldSpeedSquared =
			world.velocityX[transfer.target] ** 2 + world.velocityY[transfer.target] ** 2;
		const convertedJ = potentialLossJ * damping;
		const targetKineticJ = 0.5 * mass * oldSpeedSquared + convertedJ;
		// Transport has already recorded the actual net gravitational work,
		// including the displaced parcel. Redirect part of heat into motion.
		world.energy[transfer.target] -= convertedJ;
		const speed = mass > 0 ? Math.sqrt((2 * targetKineticJ) / mass) : 0;
		const horizontalCells = targetX - sourceX;
		const length = Math.hypot(horizontalCells, dropCells) || 1;
		world.velocityX[transfer.target] = (speed * horizontalCells) / length;
		world.velocityY[transfer.target] = (speed * Math.max(0, dropCells)) / length;
	}

	const hydrostatics = createHydrostatics(world, recordTransfer);

	function deriveColumns(): void {
		volumeM3.fill(0);
		surfaceHeightM.fill(0);
		for (let x = 0; x < world.width; x += 1) {
			let top = world.height;
			let velocityVolume = 0;
			for (let y = 0; y < world.height; y += 1) {
				const index = x + y * world.width;
				if (!isLiquid(world.grid[index])) continue;
				const volume = world.volumeM3[index];
				volumeM3[x] += volume;
				velocityVolume += world.velocityY[index] * volume;
				top = Math.min(top, y);
			}
			surfaceHeightM[x] = top === world.height ? 0 : (world.height - top) * CELL_WIDTH_METERS;
			verticalVelocityMPerS[x] = volumeM3[x] > 0 ? velocityVolume / volumeM3[x] : 0;
		}
	}

	function dampMomentum(): void {
		for (let index = 0; index < world.size; index += 1) {
			const material = world.grid[index];
			if (!isLiquid(material)) continue;
			const viscosity = materialsById[material].viscosityPas;
			const damping = Math.exp(-Math.sqrt(Math.max(0, viscosity)) * FIXED_TIME_STEP_SECONDS);
			world.energy[index] +=
				0.5 *
				world.massKg[index] *
				(world.velocityX[index] ** 2 + world.velocityY[index] ** 2) *
				(1 - damping ** 2);
			world.velocityX[index] *= damping;
			world.velocityY[index] *= damping;
		}
	}

	function step(tick: number): number {
		previousHeightM.set(surfaceHeightM);
		const transfers = hydrostatics.step(tick);
		dampMomentum();
		deriveColumns();
		for (let x = 0; x < world.width; x += 1) {
			const measuredVelocity = (previousHeightM[x] - surfaceHeightM[x]) / FIXED_TIME_STEP_SECONDS;
			if (Number.isFinite(measuredVelocity) && previousHeightM[x] > 0) {
				verticalVelocityMPerS[x] = (verticalVelocityMPerS[x] + measuredVelocity) * 0.5;
			}
		}
		return transfers;
	}

	deriveColumns();
	const columns: FluidColumns = {
		volumeM3,
		surfaceHeightM,
		verticalVelocityMPerS,
	};
	function reset(): void {
		previousHeightM.fill(0);
		deriveColumns();
	}
	return { step, deriveColumns, reset, columns };
}
