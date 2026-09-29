import { createHash } from "node:crypto";
import { copyFile, open, readFile } from "node:fs/promises";
import { resolve } from "node:path";

import { createEvidenceDirectory } from "./evidence-output";

const root = resolve(import.meta.dir, "../..");
process.chdir(root);

// E00 evidence runner. Historical harnesses run unchanged against fresh bundles.
const output = await createEvidenceDirectory(root, process.argv[2], "p1-m0");
const commands: { argv: string[]; exitCode: number; log: string }[] = [];

async function run(argv: string[], name: string, env = process.env): Promise<number> {
	const log = `${output}/${name}.log`;
	const handle = await open(log, "w");
	let exitCode: number;
	try {
		exitCode = await Bun.spawn(argv, { stdout: handle.fd, stderr: handle.fd, env }).exited;
	} finally {
		await handle.close();
	}
	commands.push({ argv, exitCode, log: `${name}.log` });
	console.log(`${name}: exit ${exitCode}`);
	await Bun.write(`${output}/commands.json`, `${JSON.stringify(commands, null, 2)}\n`);
	return exitCode;
}

async function json(path: string): Promise<unknown> {
	return JSON.parse(await readFile(path, "utf8"));
}

async function hash(path: string): Promise<string> {
	return createHash("sha256")
		.update(await readFile(path))
		.digest("hex");
}

const files = Bun.spawnSync([
	"git",
	"ls-files",
	"-z",
	"web",
	"engine",
	"mise.toml",
	".github",
	"*.json",
]);
if (files.exitCode !== 0) throw new Error("Cannot inventory tracked baseline files");
const hashes: Record<string, string> = {};
for (const path of files.stdout.toString().split("\0").filter(Boolean))
	hashes[path] = await hash(path);
const review = (await json("docs/data/source-review-manifest.json")) as {
	files: Record<string, { sha256: string }>;
};
const sourceDifferences = Object.entries(review.files)
	.filter(([path, record]) => hashes[path] !== record.sha256)
	.map(([path]) => path);
const fire = (await json("docs/data/fire-review-manifest.json")) as {
	files: { path: string; sha256: string }[];
	reviewArchive: { path: string; sha256: string };
};
for (const record of [...fire.files, fire.reviewArchive]) {
	if ((await hash(`docs/${record.path}`)) !== record.sha256)
		throw new Error(`Changed imported fire evidence: ${record.path}`);
}
await Bun.write(
	`${output}/provenance.json`,
	`${JSON.stringify(
		{
			kind: "fresh-checkout-baseline-not-corrected-acceptance",
			createdAt: new Date().toISOString(),
			commit: Bun.spawnSync(["git", "rev-parse", "HEAD"]).stdout.toString().trim(),
			status: Bun.spawnSync([
				"git",
				"status",
				"--porcelain=v2",
				"--untracked-files=all",
			]).stdout.toString(),
			bun: Bun.version,
			node: Bun.spawnSync(["node", "--version"]).stdout.toString().trim(),
			platform: process.platform,
			architecture: process.arch,
			sourceDifferences,
			hashes,
			historicalEvidenceVerified: true,
			rawHarnessProvenance:
				"Archive names and line labels in unchanged harness output are historical; these hashes identify this run.",
		},
		null,
		2,
	)}\n`,
);

if ((await run(["mise", "run", "install"], "install")) !== 0)
	throw new Error("Locked dependency install failed; see install.log");
for (const command of ["lint", "check", "build"]) await run(["mise", "run", command], command);
await run(["mise", "run", "docs:check"], "docs");
const modules = {
	world: "simulation/world",
	physics: "simulation/physics",
	diagnostics: "simulation/diagnostics",
	"physical-scale": "simulation/physical-scale",
	materials: "materials/index",
	"gas-dynamics": "physics/gas-dynamics",
	"solid-mechanics": "physics/solid-mechanics",
	motion: "physics/motion",
	"fluid-solver": "physics/fluid-solver",
	reactions: "physics/reactions",
	electricity: "physics/electricity",
	"element-reactions": "physics/element-reactions",
	neutralization: "physics/neutralization",
	airflow: "physics/airflow",
	thermal: "physics/thermal",
	brush: "tools/brush",
};
for (const [name, source] of Object.entries(modules)) {
	const bundle = await Bun.build({
		entrypoints: [`web/src/${source.startsWith("materials/") ? source : `legacy/${source}`}.ts`],
		target: "node",
		format: "cjs",
	});
	if (!bundle.success) throw new AggregateError(bundle.logs, `Cannot bundle ${source}`);
	await Bun.write(`${output}/audit-build/${name}.js`, bundle.outputs[0]);
	await Bun.write(`${output}/fire/build/${source}.js`, bundle.outputs[0]);
}
await Bun.write(`${output}/audit-build/package.json`, '{"type":"commonjs"}\n');
await Bun.write(`${output}/fire/build/package.json`, '{"type":"commonjs"}\n');
await copyFile(
	"docs/evidence/fire-review-2026-09-23/audit-fire.cjs",
	`${output}/fire/audit-fire.cjs`,
);
await run(
	[
		"node",
		"docs/plans/fluid-gpu-redesign/audit.cjs",
		`${output}/audit-build`,
		`${output}/audit-results.json`,
	],
	"audit",
);
await run(["node", `${output}/fire/audit-fire.cjs`], "fire-characterization", {
	...process.env,
	FIRE_REVIEW_BUILD: `${output}/fire/build`,
});
await run(
	["bun", "web/scripts/fire-corrected-baseline.ts", `${output}/fire/corrected-results.json`],
	"fire-corrected",
);
await run(
	["bun", "web/scripts/measure-legacy.ts", `${output}/legacy-performance.json`],
	"legacy-performance",
);
const fresh = (await json(`${output}/audit-results.json`)) as {
	results: { id: string; passed: boolean }[];
};
const historical = (await json(
	"docs/plans/fluid-gpu-redesign/current-audit-results.json",
)) as typeof fresh;
await Bun.write(
	`${output}/audit-comparison.json`,
	`${JSON.stringify(
		fresh.results.map((result) => ({
			id: result.id,
			historicalPassed: historical.results.find((old) => old.id === result.id)?.passed,
			freshPassed: result.passed,
		})),
		null,
		2,
	)}\n`,
);
console.log(
	`Fresh evidence: ${output}. Audit and corrected-fire failures are reported, never reclassified as passing.`,
);
// Collection success is distinct from acceptance. Unexpected harness/control failures still fail this runner.
const observations = (await json(`${output}/fire/fire-audit-results.json`)) as {
	results: { status: string }[];
};
process.exitCode = Number(
	commands.some(
		(command) =>
			command.exitCode !== 0 && !["audit.log", "fire-corrected.log"].includes(command.log),
	) ||
		fresh.results.some((result) => result.id.startsWith("C") && !result.passed) ||
		observations.results.some((result) => result.status === "harness-failure"),
);
