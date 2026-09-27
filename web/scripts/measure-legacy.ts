import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { createPhysics } from "../src/legacy/simulation/physics";
import { createWorld, type World } from "../src/legacy/simulation/world";
import { EMPTY, SAND, STONE, WATER } from "../src/materials";

// Instrument only an ephemeral bundle. No production hot-loop counters or source edits.
const counters = {
	pressureDerivations: 0,
	outletSearches: 0,
	outletDequeues: 0,
	outletNeighborVisits: 0,
};
Object.assign(globalThis, { __baselineCounters: counters });
const anchors: Record<string, [string, string][]> = {
	"gas-dynamics.ts": [["function derivePressure(): void {", "pressureDerivations"]],
	"motion.ts": [
		[
			"function findLiquidOutlet(target: number, granular: number, throughGrains: boolean): number {",
			"outletSearches",
		],
		["const cell = liquidQueue[head++];", "outletDequeues"],
		["function visit(neighbor: number): number {", "outletNeighborVisits"],
	],
};
const instrumented = await Bun.build({
	entrypoints: [new URL("../src/legacy/simulation/physics.ts", import.meta.url).pathname],
	target: "bun",
	format: "esm",
	plugins: [
		{
			name: "e00-call-counters",
			setup(builder) {
				builder.onLoad({ filter: /(?:gas-dynamics|motion)\.ts$/ }, async ({ path }) => {
					let contents = await readFile(path, "utf8");
					for (const [anchor, field] of anchors[path.split("/").at(-1) ?? ""]) {
						if (contents.split(anchor).length !== 2)
							throw new Error(`Instrumentation anchor changed: ${path}: ${anchor}`);
						contents = contents.replace(
							anchor,
							`${anchor}\nglobalThis.__baselineCounters.${field}++;`,
						);
					}
					return { contents, loader: "ts" };
				});
			},
		},
	],
});
if (!instrumented.success)
	throw new AggregateError(instrumented.logs, "Cannot instrument legacy physics");
const { createPhysics: countedPhysics } = (await import(
	URL.createObjectURL(instrumented.outputs[0])
)) as { createPhysics: typeof createPhysics };

function fixture(scene: string): World {
	const world = createWorld(480, 270, { seed: 20260926, boundariesEnabled: true });
	if (scene === "ambient") return world;
	for (let y = 0; y < world.height; y++)
		for (let x = 0; x < world.width; x++) {
			const wall = x === 0 || x === world.width - 1 || y === world.height - 1;
			world.setCell(
				x + y * world.width,
				wall
					? STONE
					: y >= 180
						? WATER
						: scene === "sand-over-water" && y === 179 && x % 4 === 0
							? SAND
							: EMPTY,
			);
		}
	return world;
}

function fingerprint(world: World): string {
	const hash = createHash("sha256");
	for (const [name, value] of Object.entries(world).sort(([a], [b]) => a.localeCompare(b))) {
		if (ArrayBuffer.isView(value))
			hash.update(name).update(new Uint8Array(value.buffer, value.byteOffset, value.byteLength));
	}
	return hash.update(JSON.stringify(world.ledger)).digest("hex");
}

const results = [];
for (const scene of process.argv.includes("--counts-only")
	? []
	: ["ambient", "pool", "sand-over-water"]) {
	const repeats = [];
	for (let repeat = 0; repeat < 3; repeat++) {
		const world = fixture(scene);
		const physics = createPhysics(world);
		for (let step = 0; step < 5; step++) physics.step();
		const elapsed = [];
		for (let step = 0; step < 20; step++) {
			const start = performance.now();
			physics.step();
			elapsed.push(performance.now() - start);
		}
		elapsed.sort((a, b) => a - b);
		const wallMs = elapsed.reduce((a, b) => a + b, 0);
		repeats.push({
			p50Ms: elapsed[9],
			p95Ms: elapsed[18],
			wallMs,
			acceptedSeconds: 20 / 60,
			simulatedSecondsPerWallSecond: 20 / 60 / (wallMs / 1000),
			stateHash: fingerprint(world),
		});
	}
	const world = fixture(scene);
	const physics = countedPhysics(world);
	for (const key of Object.keys(counters) as (keyof typeof counters)[]) counters[key] = 0;
	for (let step = 0; step < 25; step++) physics.step();
	const matches = fingerprint(world) === repeats[0].stateHash;
	if (!matches) throw new Error(`Instrumentation changed ${scene} state`);
	results.push({
		scene,
		width: 480,
		height: 270,
		seed: 20260926,
		warmupTicks: 5,
		sampledTicks: 20,
		outerDtSeconds: 1 / 60,
		repeats,
		countersOver25Ticks: { ...counters },
		instrumentedStateMatches: matches,
	});
}
const report = {
	kind: "legacy-headless-completed-tick-baseline",
	rendering: "excluded",
	instrumentation: "separate untimed pass, array and ledger hash parity",
	results,
};
const ventWorld = createWorld(7, 7, { boundariesEnabled: false, seed: 20260926 });
const ventPhysics = countedPhysics(ventWorld);
ventWorld.addExternalEnergy(0, 1);
for (const key of Object.keys(counters) as (keyof typeof counters)[]) counters[key] = 0;
ventPhysics.step();
Object.assign(report, { openHotEdgeOneTick: { ...counters } });
if (counters.pressureDerivations !== 5)
	throw new Error("Open hot edge did not exercise the fifth pressure derivation");
if (process.argv[2]) await Bun.write(process.argv[2], `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify(report, null, 2));
