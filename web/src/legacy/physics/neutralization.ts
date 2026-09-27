import { NEUTRAL_SOLUTION } from "../../materials";
import { materialsById } from "../../materials/physical-properties";
import type { World } from "../simulation/world";

/** Pairs adjacent equal-strength aqueous presets once per tick. */
export function createNeutralization(world: World) {
	const reacted = new Uint8Array(world.size);
	const acidIds = Object.values(materialsById)
		.filter((material) => material.neutralization?.role === "acid")
		.map((material) => material.id);
	const baseIds = Object.values(materialsById)
		.filter((material) => material.neutralization?.role === "base")
		.map((material) => material.id);

	function step(): number {
		if (
			!acidIds.some((id) => world.countMaterial(id) > 0) ||
			!baseIds.some((id) => world.countMaterial(id) > 0)
		)
			return 0;
		reacted.fill(0);
		let heatJ = 0;
		for (let index = 0; index < world.size; index += 1) {
			if (reacted[index]) continue;
			const acid = materialsById[world.grid[index]].neutralization;
			if (acid?.role !== "acid") continue;
			const x = index % world.width;
			for (const neighbor of [
				x > 0 ? index - 1 : -1,
				x + 1 < world.width ? index + 1 : -1,
				index >= world.width ? index - world.width : -1,
				index + world.width < world.size ? index + world.width : -1,
			]) {
				if (neighbor < 0 || reacted[neighbor]) continue;
				const base = materialsById[world.grid[neighbor]].neutralization;
				if (base?.role !== "base") continue;
				// One cell is 1 mL at the default scale; volumes remain explicit for future changes.
				const acidMol = acid.equivalentsMolPerL * world.volumeM3[index] * 1000;
				const baseMol = base.equivalentsMolPerL * world.volumeM3[neighbor] * 1000;
				const requestedJ = Math.min(acidMol, baseMol) * (acid.heatJPerMol ?? 0);
				// A parcel has no concentration field yet, so only a fully funded pair can react.
				if (requestedJ <= 0 || world.chemicalEnergyKj[index] * 1000 + 1e-9 < requestedJ) continue;
				const releasedJ = requestedJ;
				world.chemicalEnergyKj[index] = Math.max(
					0,
					world.chemicalEnergyKj[index] - releasedJ / 1000,
				);
				world.energy[index] += releasedJ / 2;
				world.energy[neighbor] += releasedJ / 2;
				world.changeMaterial(index, NEUTRAL_SOLUTION);
				world.changeMaterial(neighbor, NEUTRAL_SOLUTION);
				reacted[index] = 1;
				reacted[neighbor] = 1;
				heatJ += releasedJ;
				break;
			}
		}
		return heatJ;
	}
	return { step };
}
