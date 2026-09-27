import { seedStarterScene } from "../src/legacy/scenes/starter-scene";
import { createPhysics } from "../src/legacy/simulation/physics";
import { createWorld } from "../src/legacy/simulation/world";
import { EMPTY, FIRE, SMOKE, STONE, WATER } from "../src/materials";

const WIDTH = 240;
const HEIGHT = 135;
const STEPS = 60;
const FRAME_BUDGET_MS = 1000 / 60;

function measure(name: string, materialAt: (index: number) => number): object {
	const world = createWorld(WIDTH, HEIGHT, { seed: 20260920 });
	for (let index = 0; index < world.size; index += 1) {
		world.setCell(index, materialAt(index));
	}
	const physics = createPhysics(world);
	for (let warmup = 0; warmup < 5; warmup += 1) physics.step();
	const started = performance.now();
	for (let step = 0; step < STEPS; step += 1) physics.step();
	const elapsedMs = performance.now() - started;
	const millisecondsPerStep = elapsedMs / STEPS;
	return {
		name,
		cells: world.size,
		steps: STEPS,
		millisecondsPerStep: Number(millisecondsPerStep.toFixed(3)),
		budgetMilliseconds: Number(FRAME_BUDGET_MS.toFixed(3)),
		withinSingleFrameBudget: millisecondsPerStep <= FRAME_BUDGET_MS,
	};
}

function measureLargeStarter(): object {
	const width = 480;
	const height = 270;
	const world = createWorld(width, height, { seed: 20260920 });
	seedStarterScene(world);
	const physics = createPhysics(world);
	for (let warmup = 0; warmup < 5; warmup += 1) physics.step();
	const started = performance.now();
	for (let step = 0; step < STEPS; step += 1) physics.step();
	const millisecondsPerStep = (performance.now() - started) / STEPS;
	return {
		name: "starter-480",
		cells: world.size,
		steps: STEPS,
		millisecondsPerStep: Number(millisecondsPerStep.toFixed(3)),
		budgetMilliseconds: Number(FRAME_BUDGET_MS.toFixed(3)),
		withinSingleFrameBudget: millisecondsPerStep <= FRAME_BUDGET_MS,
	};
}

const results = [
	measureLargeStarter(),
	measure("empty", () => EMPTY),
	measure("filled", () => WATER),
	measure("fragmented", (index) => ((index + Math.floor(index / WIDTH)) % 3 === 0 ? STONE : WATER)),
	measure("gas-plumes", (index) => (index % 5 === 0 ? FIRE : index % 3 === 0 ? SMOKE : EMPTY)),
];

console.log(JSON.stringify(results, null, 2));
