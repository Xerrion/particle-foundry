import { mkdir, mkdtemp, readdir, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const engine = resolve(import.meta.dir, "../../engine");
const check = process.argv[2] === "--check";
const reportingSmoke = process.argv[2] === "--glitchtip-smoke";
if (process.argv.length > 3 || (process.argv[2] && !check && !reportingSmoke)) {
	throw new Error("Usage: build-wasm.ts [--check | --glitchtip-smoke]");
}
const generated = resolve(
	import.meta.dir,
	reportingSmoke ? "../generated/wasm-glitchtip-smoke" : "../generated/wasm",
);
// Feature builds must not replace the artifact while normal bindings are generated in parallel.
const target = resolve(
	engine,
	process.env.CARGO_TARGET_DIR ?? "target",
	reportingSmoke ? "glitchtip-smoke" : ".",
);

// Keep the CLI and Cargo library paired. A mismatched global installation is an error.
const version = Bun.spawnSync(["wasm-bindgen", "--version"]);
if (version.exitCode !== 0 || version.stdout.toString().trim() !== "wasm-bindgen 0.2.108") {
	throw new Error(
		"Install the pinned bindings tool: cargo install wasm-bindgen-cli --version 0.2.108 --locked",
	);
}
const build = Bun.spawn(
	[
		"cargo",
		"build",
		"--locked",
		"-p",
		"particle-wasm",
		"--target",
		"wasm32-unknown-unknown",
		"--release",
		...(reportingSmoke ? ["--features", "glitchtip-smoke"] : []),
	],
	{
		cwd: engine,
		env: { ...process.env, CARGO_TARGET_DIR: target },
		stdout: "inherit",
		stderr: "inherit",
	},
);
if ((await build.exited) !== 0) throw new Error("WASM target build failed");
const output = check ? await mkdtemp(join(tmpdir(), "particle-foundry-bindings-")) : generated;
try {
	await mkdir(output, { recursive: true });
	const bindings = Bun.spawn(
		[
			"wasm-bindgen",
			join(target, "wasm32-unknown-unknown/release/particle_wasm.wasm"),
			"--target",
			"web",
			"--out-dir",
			output,
		],
		{ stdout: "inherit", stderr: "inherit" },
	);
	if ((await bindings.exited) !== 0) throw new Error("WASM bindings generation failed");
	if (check) {
		const files = (await readdir(output)).sort();
		if (JSON.stringify(files) !== JSON.stringify((await readdir(generated)).sort())) {
			throw new Error("Generated WASM file set is stale; run mise run build:wasm");
		}
		for (const file of files) {
			if (!(await readFile(join(output, file))).equals(await readFile(join(generated, file)))) {
				throw new Error(`Generated ${file} is stale; run mise run build:wasm`);
			}
		}
		console.log(`Verified ${files.length} generated WASM assets byte-for-byte`);
	}
} finally {
	if (check) await rm(output, { recursive: true, force: true });
}
