import { createElementReactions } from "../src/legacy/physics/element-reactions";
import { createReactions } from "../src/legacy/physics/reactions";
import { energyAtTemperature } from "../src/legacy/physics/thermal";
import { measureWorld } from "../src/legacy/simulation/diagnostics";
import { createPhysics } from "../src/legacy/simulation/physics";
import { createWorld, type World } from "../src/legacy/simulation/world";
import { createBrush } from "../src/legacy/tools/brush";
import { EMPTY, FIRE, HYDROGEN, OXYGEN, STONE, WOOD } from "../src/materials";

// These assert desired contracts against the legacy backend, not the F-check observations.
// A control passing here is only that subset of a FIRE-A gate, never FIRE-M6 acceptance.
const results: {
	id: string;
	scope: string;
	passed: boolean;
	measurements: Record<string, number>;
}[] = [];
const sum = (values: Float64Array) => values.reduce((total, value) => total + value, 0);
const hot = (world: World, index: number, celsius: number) =>
	world.addExternalEnergy(
		index,
		energyAtTemperature(world.grid[index], celsius, undefined, world.massKg[index]) -
			world.energy[index],
	);
const sealed = () => {
	const world = createWorld(7, 7, { seed: 2026, boundariesEnabled: true });
	for (let index = 0; index < world.size; index++) world.setCell(index, STONE);
	return world;
};
for (const enclosure of ["outer-boundary", "stone-enclosure"]) {
	const world =
		enclosure === "stone-enclosure"
			? sealed()
			: createWorld(7, 7, { seed: 2026, boundariesEnabled: true });
	world.setCell(24, WOOD);
	world.setCell(23, EMPTY);
	hot(world, 24, 500);
	world.oxygenKg.fill(0);
	const initialChemical = sum(world.chemicalEnergyKj);
	createPhysics(world).step();
	const releasedJ = 1000 * (initialChemical - sum(world.chemicalEnergyKj));
	const oxygenKg = sum(world.oxygenKg);
	results.push({
		id: `FIRE-A01/${enclosure}`,
		scope: "legacy complete physics tick",
		passed: Math.abs(releasedJ) <= 1e-8 && oxygenKg <= 1e-15,
		measurements: { releasedJ, oxygenKg },
	});
}
{
	const world = sealed();
	for (const [index, material] of [
		[24, HYDROGEN],
		[25, OXYGEN],
		[23, FIRE],
	]) {
		world.setCell(index, material);
		hot(world, index, 22);
	}
	const before = sum(world.chemicalEnergyKj);
	createPhysics(world).step();
	const releasedJ = 1000 * (before - sum(world.chemicalEnergyKj));
	results.push({
		id: "FIRE-A04/cold-label",
		scope: "legacy complete physics tick",
		passed: Math.abs(releasedJ) <= 1e-8,
		measurements: { releasedJ },
	});
}
{
	const world = sealed();
	world.setCell(24, EMPTY);
	const brush = createBrush(world, () => {});
	brush.setMaterial(FIRE);
	brush.setSize(1);
	const before = measureWorld(world);
	brush.paintCircle(3, 3);
	const after = measureWorld(world);
	const energyJ = 1000 * (after.totalTrackedEnergyKj - before.totalTrackedEnergyKj);
	const sourceJ = world.ledger.externalEnergyAdded - before.ledger.externalEnergyAdded;
	const massErrorKg = after.totalMassKg - before.totalMassKg;
	results.push({
		id: "FIRE-A07/funded-brush-control",
		scope: "legacy isolated brush; no queue/retry claim",
		passed:
			Math.abs(massErrorKg) <= 1e-15 &&
			energyJ > 0 &&
			Math.abs(energyJ - sourceJ) <= 1e-8 &&
			world.chemicalEnergyKj[24] === 0,
		measurements: { massErrorKg, energyJ, sourceJ },
	});
}
{
	const world = sealed();
	world.setCell(24, HYDROGEN);
	world.setCell(25, OXYGEN);
	hot(world, 24, 600);
	const before = measureWorld(world);
	const reacted = createElementReactions(world).step();
	const after = measureWorld(world);
	const massErrorKg = after.totalMassKg - before.totalMassKg;
	const energyErrorJ = 1000 * (after.totalTrackedEnergyKj - before.totalTrackedEnergyKj);
	results.push({
		id: "FIRE-A05/explicit-hot-reaction-control",
		scope: "legacy isolated scalar control; constituent gate still unrun",
		passed: reacted === 1 && Math.abs(massErrorKg) <= 1e-12 && Math.abs(energyErrorJ) <= 1e-8,
		measurements: { reacted, massErrorKg, energyErrorJ },
	});
}
{
	const world = sealed();
	world.setCell(24, WOOD);
	world.setCell(23, OXYGEN);
	hot(world, 24, 300);
	const before = measureWorld(world);
	const reactions = createReactions(world);
	reactions.beginStep();
	reactions.update(24);
	const energyErrorJ =
		1000 * (measureWorld(world).totalTrackedEnergyKj - before.totalTrackedEnergyKj);
	results.push({
		id: "FIRE-A05/generic-scalar-control",
		scope: "legacy isolated scalar control; known product defect is not accepted",
		passed: Math.abs(energyErrorJ) <= 1e-8,
		measurements: { energyErrorJ },
	});
}
const report = {
	kind: "corrected-contract-baseline-on-legacy-not-promotion",
	backend: "legacy",
	outerDtSeconds: 1 / 60,
	results,
};
if (process.argv[2]) await Bun.write(process.argv[2], `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify(report, null, 2));
process.exitCode = Number(results.some((result) => !result.passed));
