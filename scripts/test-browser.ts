import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

// Isolated headless test process, never the user's running browser/profile.
const output = resolve(process.argv[2] ?? `artifacts/validation/browser/${Date.now()}`);
const requireGpu = process.argv[3] === "--require-gpu";
if (process.argv.length > (requireGpu ? 4 : 3)) {
	throw new Error("Usage: test-browser.ts [new-evidence-directory] [--require-gpu]");
}
await mkdir(dirname(output), { recursive: true });
await mkdir(output); // Never overwrite prior evidence.
const profile = await mkdtemp(join(tmpdir(), "particle-foundry-smoke-"));
const binary =
	process.env.CHROME_BIN ??
	(process.platform === "darwin"
		? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
		: "/usr/bin/google-chrome");
const url = new URL("http://127.0.0.1:4174/engine-smoke/tests/browser/engine-smoke.html");
if (requireGpu) url.searchParams.set("require-gpu", "");
let finish: (report: Record<string, unknown>) => void;
const completion = new Promise<Record<string, unknown>>((resolve) => {
	finish = resolve;
});
const reportPath = `/${crypto.randomUUID()}`;
// Real-time completion: dump-dom/virtual time can finish before GPU promises settle.
const receiver = Bun.serve({
	hostname: "127.0.0.1",
	port: 0,
	maxRequestBodySize: 65_536,
	async fetch(request) {
		if (
			request.method !== "POST" ||
			new URL(request.url).pathname !== reportPath ||
			request.headers.get("origin") !== url.origin
		) {
			return new Response("Unexpected smoke report", { status: 400 });
		}
		const report = await request.json();
		finish(report);
		return new Response("ok", { headers: { "Access-Control-Allow-Origin": url.origin } });
	},
});
url.searchParams.set("report", `http://127.0.0.1:${receiver.port}${reportPath}`);
let server: ReturnType<typeof Bun.spawn> | undefined;
let browser: ReturnType<typeof Bun.spawn> | undefined;
let timeout: ReturnType<typeof setTimeout> | undefined;
try {
	const vite = [process.execPath, "node_modules/vite/bin/vite.js"];
	const build = Bun.spawn(
		[...vite, "build", "--mode", "engine-smoke", "--outDir", `${output}/site`],
		{
			stdout: Bun.file(`${output}/build.log`),
			stderr: Bun.file(`${output}/build-error.log`),
		},
	);
	if ((await build.exited) !== 0) throw new Error(`Browser fixture build failed; see ${output}`);
	server = Bun.spawn(
		[
			...vite,
			"preview",
			"--mode",
			"engine-smoke",
			"--outDir",
			`${output}/site`,
			"--host",
			"127.0.0.1",
			"--port",
			"4174",
			"--strictPort",
		],
		{ stdout: Bun.file(`${output}/server.log`), stderr: Bun.file(`${output}/server-error.log`) },
	);
	let ready = false;
	const startup = AbortSignal.timeout(5000);
	while (!startup.aborted) {
		if (server.exitCode !== null)
			throw new Error("Browser test server exited; see server-error.log");
		try {
			const announced = (await Bun.file(`${output}/server.log`).text()).includes("127.0.0.1:4174");
			ready = announced && (await fetch(url, { signal: startup })).ok;
		} catch {
			/* Startup is bounded below. */
		}
		if (ready) break;
		await Bun.sleep(100);
	}
	if (!ready) throw new Error("Browser test server did not become ready in 5 seconds");
	const browserProcess = Bun.spawn(
		[
			binary,
			"--headless=new",
			"--enable-gpu",
			`--user-data-dir=${profile}`,
			"--no-first-run",
			"--no-default-browser-check",
			"--disable-background-networking",
			url.href,
		],
		{ stdout: Bun.file(`${output}/browser.log`), stderr: Bun.file(`${output}/browser-error.log`) },
	);
	browser = browserProcess;
	const report = await Promise.race([
		completion,
		browserProcess.exited.then((code) => {
			throw new Error(`Browser exited before reporting (exit ${code}); see ${output}`);
		}),
		new Promise<never>((_, reject) => {
			timeout = setTimeout(
				() => reject(new Error(`Browser smoke timed out after 30 seconds; see ${output}`)),
				30_000,
			);
		}),
	]);
	await Bun.write(`${output}/result.json`, `${JSON.stringify(report, null, 2)}\n`);
	if (report.status !== "pass") throw new Error(`Browser smoke failed: ${report.error}`);
	console.log(JSON.stringify(report));
} finally {
	clearTimeout(timeout);
	await receiver.stop(true);
	for (const process of [browser, server]) {
		if (process?.exitCode === null) process.kill("SIGKILL");
		await process?.exited;
	}
	await rm(profile, { recursive: true, force: true });
}
