import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { readOptions } from "../benchmarks/gpu-activity/page";
import { waitForBrowserServer } from "./browser-server";
import { createEvidenceDirectory } from "./evidence-output";

const root = resolve(import.meta.dir, "../..");
const web = join(root, "web");
const args = process.argv.slice(2);
const requested = args[0] && !args[0].startsWith("--") ? args.shift() : undefined;
const params = new URLSearchParams();
const allowed = new Set([
	"mode",
	"fixture",
	"ticks",
	"warmup-ticks",
	"repeats",
	"sample-tick",
	"sample-passes",
	"sample-every",
	"timeout-seconds",
]);
for (let index = 0; index < args.length; index += 2) {
	const key = args[index]?.slice(2);
	const value = args[index + 1];
	if (!args[index]?.startsWith("--") || !allowed.has(key) || !value || params.has(key)) {
		throw new Error(
			"Usage: run-gpu-activity.ts [new-evidence-directory] [--mode baseline|activity|profile] [--fixture rest|brush|dense] [--ticks 60] [--warmup-ticks 10] [--repeats 3] [--sample-tick 10] [--sample-passes 2] [--sample-every 64] [--timeout-seconds 900]",
		);
	}
	params.set(key, value);
}
const options = readOptions(params);
const timeoutSeconds = Number(params.get("timeout-seconds") ?? 900);
if (!Number.isSafeInteger(timeoutSeconds) || timeoutSeconds < 900 || timeoutSeconds > 7200) {
	throw new Error("Activity timeout must be between 900 and 7200 seconds");
}
const output = await createEvidenceDirectory(root, requested, "gpu-activity");
const profileDirectory = await mkdtemp(join(tmpdir(), "particle-foundry-activity-"));
const binary =
	process.env.CHROME_BIN ??
	(process.platform === "darwin"
		? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
		: "/usr/bin/google-chrome");
const reservation = Bun.serve({
	hostname: "127.0.0.1",
	port: 0,
	fetch: () => new Response("reserved"),
});
const port = reservation.port;
await reservation.stop(true);
const url = new URL(`http://127.0.0.1:${port}/gpu-activity/gpu-activity.html`);
for (const [key, value] of params) url.searchParams.set(key, value);
const token = crypto.randomUUID();
let finish!: (report: Record<string, unknown>) => void;
const completion = new Promise<Record<string, unknown>>((resolve) => {
	finish = resolve;
});
const savedCheckpoints = new Set<string>();
const receiver = Bun.serve({
	hostname: "127.0.0.1",
	port: 0,
	maxRequestBodySize: 16 * 1024 * 1024,
	async fetch(request) {
		const headers = { "Access-Control-Allow-Origin": url.origin };
		if (
			request.method !== "POST" ||
			new URL(request.url).pathname !== "/report" ||
			request.headers.get("origin") !== url.origin
		) {
			return new Response("Unexpected activity report", { status: 400, headers });
		}
		try {
			const payload = await request.json();
			if (payload?.token !== token)
				return new Response("Invalid report token", { status: 400, headers });
			if (payload.kind === "checkpoint") {
				if (
					typeof payload.name !== "string" ||
					!/^(throughput-[1-9][0-9]*|pacing-[1-9][0-9]*|activity|profile)-(before|after)$/.test(
						payload.name,
					) ||
					savedCheckpoints.has(payload.name) ||
					typeof payload.bytes !== "string"
				) {
					return new Response("Invalid checkpoint name", { status: 400, headers });
				}
				const bytes = Buffer.from(payload.bytes, "base64");
				if (
					bytes.length !== 108 + (5 * 480 * 270 + 481 * 270 + 480 * 271) * 4 ||
					bytes.subarray(0, 4).toString() !== "PFCP"
				) {
					return new Response("Invalid checkpoint payload", { status: 400, headers });
				}
				savedCheckpoints.add(payload.name);
				await Bun.write(join(output, `${payload.name}.pfcp`), bytes);
			} else if (
				payload.kind === "result" &&
				payload.report &&
				typeof payload.report === "object"
			) {
				finish(payload.report);
			} else return new Response("Invalid report payload", { status: 400, headers });
			return new Response("ok", { headers });
		} catch {
			return new Response("Invalid report body", { status: 400, headers });
		}
	},
});
url.searchParams.set("report-token", token);
url.searchParams.set("report-url", `http://127.0.0.1:${receiver.port}/report`);
let server: ReturnType<typeof Bun.spawn> | undefined;
let browser: ReturnType<typeof Bun.spawn> | undefined;
let timeout: ReturnType<typeof setTimeout> | undefined;
try {
	const source = Bun.spawn(["git", "rev-parse", "HEAD"], {
		cwd: root,
		stdout: "pipe",
		stderr: "pipe",
	});
	const revision = (await new Response(source.stdout).text()).trim();
	if (await source.exited) throw new Error("Cannot identify the source revision");
	const wasm = await Bun.file(join(web, "generated/wasm/particle_wasm_bg.wasm")).arrayBuffer();
	const wasmSha256 = new Bun.CryptoHasher("sha256").update(wasm).digest("hex");
	await Bun.write(
		join(output, "protocol.json"),
		`${JSON.stringify(
			{
				revision,
				wasmSha256,
				options,
				timeoutSeconds,
				startedAt: new Date().toISOString(),
				bun: Bun.version,
				note: "Revision alone does not identify uncommitted changes. Preserve the working-tree patch with this evidence.",
			},
			null,
			2,
		)}\n`,
	);
	const vite = [process.execPath, join(web, "node_modules/vite/bin/vite.js")];
	const build = Bun.spawn(
		[...vite, "build", "--mode", "gpu-activity", "--outDir", join(output, "site")],
		{
			cwd: web,
			stdout: Bun.file(join(output, "build.log")),
			stderr: Bun.file(join(output, "build-error.log")),
		},
	);
	if (await build.exited) throw new Error(`Activity fixture build failed. See ${output}`);
	const serverProcess = Bun.spawn(
		[
			...vite,
			"preview",
			"--mode",
			"gpu-activity",
			"--outDir",
			join(output, "site"),
			"--host",
			"127.0.0.1",
			"--port",
			String(port),
			"--strictPort",
		],
		{
			cwd: web,
			stdout: Bun.file(join(output, "server.log")),
			stderr: Bun.file(join(output, "server-error.log")),
		},
	);
	server = serverProcess;
	await waitForBrowserServer(url, () => serverProcess.exitCode);
	const browserProcess = Bun.spawn(
		[
			binary,
			"--headless=new",
			"--enable-gpu",
			`--user-data-dir=${profileDirectory}`,
			"--no-first-run",
			"--no-default-browser-check",
			"--disable-background-networking",
			url.href,
		],
		{
			stdout: Bun.file(join(output, "browser.log")),
			stderr: Bun.file(join(output, "browser-error.log")),
		},
	);
	browser = browserProcess;
	const report = await Promise.race([
		completion,
		browserProcess.exited.then((code) => {
			throw new Error(`Activity browser exited before reporting: ${code}`);
		}),
		new Promise<never>((_, reject) => {
			timeout = setTimeout(
				() => reject(new Error(`Activity benchmark timed out after ${timeoutSeconds} seconds`)),
				timeoutSeconds * 1000,
			);
		}),
	]);
	await Bun.write(join(output, "result.json"), `${JSON.stringify(report, null, 2)}\n`);
	if (report.status !== "pass") throw new Error(`Activity benchmark failed: ${report.error}`);
	console.log(
		JSON.stringify({ status: "pass", output, options, checkpoints: savedCheckpoints.size }),
	);
} catch (error) {
	await Bun.write(
		join(output, "runner-error.json"),
		`${JSON.stringify({ status: "fail", error: String(error) }, null, 2)}\n`,
	);
	throw error;
} finally {
	clearTimeout(timeout);
	await receiver.stop(true);
	for (const child of [browser, server]) {
		if (child?.exitCode === null) child.kill("SIGKILL");
		await child?.exited;
	}
	await rm(profileDirectory, { recursive: true, force: true });
}
