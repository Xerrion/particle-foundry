import { materialsById } from "./material-physics";
import { FIXED_TIME_STEP_SECONDS } from "./physical-scale";
import type { World } from "./world";

/** A bounded resistor-network model for adjacent conductors and 12 V terminals. */
export function createElectricity(world: World) {
	const visited = new Uint8Array(world.size);
	const queue = new Int32Array(world.size);
	const voltageV = new Float64Array(world.size);

	function resistanceAt(index: number): number | undefined {
		return materialsById[world.grid[index]].electrical?.resistanceOhms;
	}

	function neighbors(index: number): number[] {
		const result: number[] = [];
		const x = index % world.width;
		if (x > 0) result.push(index - 1);
		if (x + 1 < world.width) result.push(index + 1);
		if (index >= world.width) result.push(index - world.width);
		if (index + world.width < world.size) result.push(index + world.width);
		return result;
	}

	function step(): number {
		visited.fill(0);
		voltageV.fill(0);
		let deliveredJ = 0;
		for (let start = 0; start < world.size; start += 1) {
			if (visited[start] || resistanceAt(start) === undefined) continue;
			let head = 0;
			let tail = 1;
			queue[0] = start;
			visited[start] = 1;
			const sources: number[] = [];
			let grounded = false;
			while (head < tail) {
				const index = queue[head++];
				const terminal = materialsById[world.grid[index]].electrical?.terminal;
				if (terminal === "source" && world.chemicalEnergyKj[index] > 0) sources.push(index);
				if (terminal === "ground") grounded = true;
				for (const neighbor of neighbors(index)) {
					if (visited[neighbor] || resistanceAt(neighbor) === undefined) continue;
					visited[neighbor] = 1;
					queue[tail++] = neighbor;
				}
			}
			if (!grounded || sources.length === 0) continue;
			for (const source of sources)
				voltageV[source] = materialsById[world.grid[source]].electrical?.voltageV ?? 0;
			for (let iteration = 0; iteration < 128; iteration += 1) {
				let largestChange = 0;
				for (let cursor = 0; cursor < tail; cursor += 1) {
					const index = queue[cursor];
					const profile = materialsById[world.grid[index]].electrical;
					if (!profile || profile.terminal === "ground") continue;
					if (profile.terminal === "source" && world.chemicalEnergyKj[index] > 0) continue;
					let conductance = 0;
					let weightedVoltage = 0;
					for (const neighbor of neighbors(index)) {
						const neighborResistance = resistanceAt(neighbor);
						if (neighborResistance === undefined) continue;
						const weight = 2 / (profile.resistanceOhms + neighborResistance);
						conductance += weight;
						weightedVoltage += weight * voltageV[neighbor];
					}
					if (conductance === 0) continue;
					const nextVoltage = weightedVoltage / conductance;
					largestChange = Math.max(largestChange, Math.abs(nextVoltage - voltageV[index]));
					voltageV[index] = nextVoltage;
				}
				if (largestChange < 1e-7) break;
			}
			const edges: Array<{ a: number; b: number; heatJ: number }> = [];
			let requestedJ = 0;
			for (let cursor = 0; cursor < tail; cursor += 1) {
				const index = queue[cursor];
				const x = index % world.width;
				for (const neighbor of [
					x + 1 < world.width ? index + 1 : -1,
					index + world.width < world.size ? index + world.width : -1,
				]) {
					if (neighbor < 0) continue;
					const neighborResistance = resistanceAt(neighbor);
					if (neighborResistance === undefined) continue;
					const resistance = ((resistanceAt(index) as number) + neighborResistance) / 2;
					const difference = voltageV[index] - voltageV[neighbor];
					const heatJ = (difference * difference * FIXED_TIME_STEP_SECONDS) / resistance;
					if (heatJ <= 0) continue;
					edges.push({ a: index, b: neighbor, heatJ });
					requestedJ += heatJ;
				}
			}
			if (requestedJ === 0) continue;
			const availableJ = sources.reduce(
				(sum, index) => sum + world.chemicalEnergyKj[index] * 1000,
				0,
			);
			const actualJ = Math.min(requestedJ, availableJ);
			let remainingJ = actualJ;
			for (let offset = 0; offset < sources.length; offset += 1) {
				const index = sources[offset];
				const sourceJ = world.chemicalEnergyKj[index] * 1000;
				const debitJ =
					offset + 1 === sources.length ? remainingJ : actualJ * (sourceJ / availableJ);
				world.chemicalEnergyKj[index] = Math.max(0, world.chemicalEnergyKj[index] - debitJ / 1000);
				remainingJ -= debitJ;
			}
			const fraction = actualJ / requestedJ;
			for (const edge of edges) {
				const halfHeat = (edge.heatJ * fraction) / 2;
				world.energy[edge.a] += halfHeat;
				world.energy[edge.b] += halfHeat;
			}
			deliveredJ += actualJ;
		}
		return deliveredJ;
	}

	return { step, voltageAt: (index: number): number => voltageV[index] };
}
