import { type ActivityLayout, CLOSURE_ROUNDS, COUNTERS } from "./schema";

function replaceOnce(source: string, anchor: string, replacement: string): string {
	if (source.split(anchor).length !== 2) {
		throw new Error(`Transport activity shader anchor changed: ${anchor.slice(0, 70)}`);
	}
	return source.replace(anchor, replacement);
}

function declarations(layout: ActivityLayout): string {
	return `
// Benchmark observations use a separate group. Physical bindings stay unchanged.
@group(1) @binding(0) var<storage, read_write> activity: array<atomic<u32>>;

fn activity_begin(x: u32, y: u32, axis: u32) -> u32 {
    if atomicLoad(&activity[0]) == 0u { return 0xffffffffu; }
    if params.width != ${layout.width}u || params.height != ${layout.height}u {
        atomicAdd(&activity[1], 1u);
        return 0xffffffffu;
    }
    let face = select(y * ${layout.width - 1}u + x - 1u,
        ${layout.uInteriorFaces}u + (y - 1u) * ${layout.width}u + x, axis == 1u);
    let visit = atomicAdd(&activity[${layout.visitsOffset}u + face], 1u);
    if visit >= ${CLOSURE_ROUNDS * 8}u { atomicAdd(&activity[2], 1u); }
    return visit % ${CLOSURE_ROUNDS}u;
}

fn activity_count(round: u32, category: u32) {
    if round == 0xffffffffu { return; }
    atomicAdd(&activity[${layout.roundsOffset}u + round * ${COUNTERS.length}u + category], 1u);
}

fn activity_tile(cell: u32) -> u32 {
    return (cell / ${layout.width}u / ${layout.tileSize}u) * ${layout.tileColumns}u
        + (cell % ${layout.width}u) / ${layout.tileSize}u;
}

fn activity_tiles(round: u32, negative: u32, positive: u32, changed: bool) {
    if round == 0xffffffffu { return; }
    let offset = select(${layout.tileWritesOffset}u, ${layout.tileChangesOffset}u, changed)
        + round * ${layout.tiles}u;
    let first = activity_tile(negative);
    let second = activity_tile(positive);
    atomicAdd(&activity[offset + first], 1u);
    if second != first { atomicAdd(&activity[offset + second], 1u); }
}
`;
}

/** Add counters to the current shader. Physical arithmetic, status writes and order stay intact. */
export function instrumentTransportPairs(source: string, layout: ActivityLayout): string {
	let fingerprint = 2166136261;
	for (let index = 0; index < source.length; index += 1) {
		fingerprint = Math.imul(fingerprint ^ source.charCodeAt(index), 16777619) >>> 0;
	}
	// Pin the complete authored shader, including returns not selected by the anchors below.
	if (source.length !== 4203 || fingerprint !== 0xa48481fa) {
		throw new Error(
			"Transport activity shader source changed. Review the observation anchors before capture",
		);
	}
	if (source.includes("@group(1)") || source.includes("activity_begin")) {
		throw new Error("Transport activity shader has an unexpected diagnostic binding");
	}
	let code = replaceOnce(
		source,
		"const CORRECTION_LIMIT: u32 = 16u;",
		`${declarations(layout)}\nconst CORRECTION_LIMIT: u32 = 16u;`,
	);
	code = replaceOnce(
		code,
		"    let positive_cell = y * params.width + x;",
		"    let positive_cell = y * params.width + x;\n    let activity_round = activity_begin(x, y, axis);",
	);
	const exits = [
		[
			"    if headers[negative_cell].fixed_wall != 0u || headers[positive_cell].fixed_wall != 0u {",
			0,
		],
		["    if open <= 0.0 {", 1],
		["    if delta == 0.0 {", 2],
		["    if !finite(requested_cfl) {", 3],
		[
			"    if !(finite(corrected) && finite(correction_cfl)\n         && abs(correction_cfl) <= MAX_CORRECTION_CFL) {",
			3,
		],
	] as const;
	for (const [anchor, category] of exits) {
		code = replaceOnce(
			code,
			`${anchor}\n`,
			`${anchor}\n        activity_count(activity_round, ${category}u);\n`,
		);
	}
	code = replaceOnce(
		code,
		"    residual[positive_cell] = positive + actual;",
		`    residual[positive_cell] = positive + actual;
    activity_count(activity_round, 4u);
    activity_tiles(activity_round, negative_cell, positive_cell, false);
    if bitcast<u32>(corrected) != bitcast<u32>(previous)
        || bitcast<u32>(residual[negative_cell]) != bitcast<u32>(negative)
        || bitcast<u32>(residual[positive_cell]) != bitcast<u32>(positive) {
        activity_count(activity_round, 5u);
        activity_tiles(activity_round, negative_cell, positive_cell, true);
    }`,
	);
	return code;
}
