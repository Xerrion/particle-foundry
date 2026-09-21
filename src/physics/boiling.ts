import { materialsById } from "../materials";
import { AMBIENT_PRESSURE_PA } from "../simulation/physical-scale";
import type { World } from "../simulation/world";
import { phaseEnthalpyOffset, thermalMassScale } from "./thermal";

const profiles = materialsById.map((material) => {
	const family = material.phaseFamily;
	if (!family || material.state !== "liquid") return undefined;
	const index = family.phases.indexOf(material.id);
	const transition = family.transitions[index];
	if (!transition?.nucleationChancePerTick) return undefined;
	return { transition, vapor: family.phases[index + 1] };
});

/** Coarse-grid phase separation: pool only already-stored latent heat from touching
 * liquid parcels into one complete vapor parcel. No subcell mass, invented heat,
 * or premature temperature threshold. This avoids entire heated rows vaporizing
 * in lockstep while the latent plateau otherwise appears completely motionless. */
export function createBoiling(world: World) {
	const lower = new Float64Array(world.size);
	const upper = new Float64Array(world.size);
	const candidates = new Int32Array(world.size);
	const neighbors = new Int32Array(4);
	function step(tick: number): number {
		let count = 0;
		let bubbles = 0;
		for (let i = 0; i < world.size; i++) {
			const profile = profiles[world.grid[i]];
			if (!profile) continue;
			const pressure = world.pressurePa[i] > 0 ? world.pressurePa[i] : AMBIENT_PRESSURE_PA;
			const offset = phaseEnthalpyOffset(profile.transition, world.grid[i], pressure);
			const scale = thermalMassScale(world.grid[i], world.massKg[i]);
			lower[i] = (profile.transition.lowerCellEnthalpy + offset) * scale;
			upper[i] = (profile.transition.upperCellEnthalpy + offset) * scale;
			if (world.energy[i] > lower[i] && world.energy[i] < upper[i]) candidates[count++] = i;
		}
		for (let cursor = 0; cursor < count; cursor++) {
			const index = candidates[tick % 2 ? count - 1 - cursor : cursor];
			const material = world.grid[index];
			const profile = profiles[material];
			if (!profile || world.energy[index] <= lower[index]) continue;
			const deficit = upper[index] - world.energy[index];
			if (deficit <= 0) continue;
			const x = index % world.width;
			let n = 0;
			if (x > 0) neighbors[n++] = index - 1;
			if (x + 1 < world.width) neighbors[n++] = index + 1;
			if (index >= world.width) neighbors[n++] = index - world.width;
			if (index + world.width < world.size) neighbors[n++] = index + world.width;
			let available = 0;
			for (let j = 0; j < n; j++) {
				const donor = neighbors[j];
				if (world.grid[donor] === material)
					available += Math.max(0, world.energy[donor] - lower[donor]);
			}
			if (
				available < deficit ||
				world.random.next() >= (profile.transition.nucleationChancePerTick ?? 0)
			)
				continue;
			let remaining = deficit;
			for (let j = 0; j < n && remaining > 0; j++) {
				const donor = neighbors[j];
				if (world.grid[donor] !== material) continue;
				const transfer = Math.min(remaining, Math.max(0, world.energy[donor] - lower[donor]));
				world.energy[donor] -= transfer;
				remaining -= transfer;
			}
			world.energy[index] = upper[index];
			world.changeMaterial(index, profile.vapor);
			bubbles++;
		}
		return bubbles;
	}
	return { step };
}
