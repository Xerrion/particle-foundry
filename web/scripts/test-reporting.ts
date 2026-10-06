import { readFileSync } from "node:fs";
import { mkdtemp, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { SourceMapConsumer } from "source-map-js";
import { createEvidenceDirectory } from "./evidence-output";
import { prepareReportingBuild } from "./prepare-reporting";

const root = resolve(import.meta.dir, "../..");
const web = join(root, "web");
if (process.argv.length > 3) {
	throw new Error("Usage: test-reporting.ts [new-evidence-directory]");
}
const output = await createEvidenceDirectory(root, process.argv[2], "reporting");
const site = join(output, "site");
const archive = join(output, "private-maps");
const binary =
	process.env.CHROME_BIN ??
	(process.platform === "darwin"
		? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
		: "/usr/bin/google-chrome");
const expectedRelease = "particle-foundry-reporting-smoke";
const expectedEnvironment = "verification";
const events: {
	project: "web" | "sim";
	exceptionType: string;
	sourceFile: string;
	line: number;
}[] = [];
const failures: string[] = [];
const eventIds = new Set<string>();
const reportToken = crypto.randomUUID();
let finish: (report: Record<string, unknown>) => void;
const completion = new Promise<Record<string, unknown>>((resolve) => {
	finish = resolve;
});

class EnvelopeFailure extends Error {}

function requireCondition(condition: unknown, failure: string): asserts condition {
	if (!condition) throw new EnvelopeFailure(failure);
}

function recordEnvelope(body: string, project: "web" | "sim"): void {
	const lines = body.trimEnd().split("\n");
	requireCondition(lines.length >= 3 && lines.length % 2 === 1, "Invalid envelope structure");
	JSON.parse(lines[0] ?? "");
	for (let index = 1; index < lines.length; index += 2) {
		const header = JSON.parse(lines[index] ?? "");
		requireCondition(header.type === "event", "Unexpected envelope item type");
		const event = JSON.parse(lines[index + 1] ?? "");
		requireCondition(event && typeof event === "object", "Invalid event payload");
		for (const field of ["user", "request", "extra", "breadcrumbs"]) {
			requireCondition(!Object.hasOwn(event, field), `Event retained ${field} data`);
		}
		requireCondition(event.release === expectedRelease, "Incorrect release");
		requireCondition(event.environment === expectedEnvironment, "Incorrect environment");
		requireCondition(
			event.tags?.project === `particle-foundry-${project}`,
			"Incorrect project tag",
		);
		requireCondition(event.platform === "javascript", "Incorrect event platform");
		requireCondition(
			typeof event.event_id === "string" && /^[a-f0-9]{32}$/.test(event.event_id),
			"Invalid event identifier",
		);
		requireCondition(!eventIds.has(event.event_id), "Duplicate event identifier");
		const exception = event.exception?.values?.at(-1);
		requireCondition(
			exception?.type === (project === "web" ? "Error" : "RustPanic"),
			"Incorrect exception type",
		);
		requireCondition(
			typeof exception.value === "string" &&
				exception.value.includes(
					project === "web"
						? "Particle Foundry browser reporting verification"
						: "Particle Foundry Rust WASM reporting verification",
				),
			"Incorrect exception message",
		);
		if (project === "web") {
			requireCondition(exception.mechanism?.handled === false, "Browser error was not unhandled");
		}
		const sourceFile =
			project === "web"
				? "web/tests/browser/sentry-smoke.ts"
				: "engine/crates/wasm/src/reporting.rs";
		let sourceLine: number;
		if (project === "web") {
			const frame = exception.stacktrace?.frames?.at(-1);
			requireCondition(
				frame?.filename && frame.lineno > 0 && frame.colno > 0,
				"Missing generated browser frame",
			);
			const codeFile = new URL(frame.filename).pathname;
			const map = JSON.parse(
				readFileSync(join(archive, "files", "assets", `${basename(codeFile)}.map`), "utf8"),
			);
			requireCondition(
				event.debug_meta?.images?.some(
					(image: Record<string, unknown>) =>
						image.type === "sourcemap" &&
						image.debug_id === map.debug_id &&
						typeof image.code_file === "string" &&
						new URL(image.code_file).pathname === codeFile,
				),
				"Missing matching source map debug ID",
			);
			const original = new SourceMapConsumer(map).originalPositionFor({
				line: frame.lineno,
				column: frame.colno - 1,
			});
			requireCondition(
				original.source?.endsWith("tests/browser/sentry-smoke.ts") && original.line,
				"Incorrect original browser source",
			);
			const sourceIndex = map.sources.indexOf(original.source);
			const source = map.sourcesContent[sourceIndex];
			requireCondition(
				typeof source === "string" &&
					source
						.split("\n")
						[original.line - 1]?.includes(
							'throw new Error("Particle Foundry browser reporting verification")',
						),
				"Incorrect browser throw line or source context",
			);
			sourceLine = original.line;
		} else {
			const frame = exception.stacktrace?.frames?.find(
				(value: Record<string, unknown>) =>
					value.filename === sourceFile && value.platform === "rust",
			);
			requireCondition(
				frame?.lineno > 0 && frame.colno > 0 && frame.in_app === true,
				"Missing Rust panic source location",
			);
			const source = readFileSync(join(root, sourceFile), "utf8");
			requireCondition(
				source.split("\n")[frame.lineno - 1]?.includes("panic!("),
				"Incorrect Rust panic origin",
			);
			if (/^[a-f0-9]{40}$/.test(event.tags?.build_revision ?? "")) {
				requireCondition(
					frame.source_link?.includes(
						`/blob/${event.tags.build_revision}/${sourceFile}#L${frame.lineno}`,
					),
					"Missing exact revision source link",
				);
			}
			sourceLine = frame.lineno;
		}
		requireCondition(
			!["reporting-fixture-user", "reporting-fixture-extra", "reporting-fixture-query"].some(
				(value) => JSON.stringify(event).includes(value),
			),
			"Event retained synthetic private data",
		);
		eventIds.add(event.event_id);
		events.push({ project, exceptionType: exception.type, sourceFile, line: sourceLine });
	}
}

const build = Bun.spawn(
	[
		process.execPath,
		join(web, "node_modules/vite/bin/vite.js"),
		"build",
		"--mode",
		"sentry-smoke",
		"--outDir",
		site,
	],
	{
		cwd: web,
		stdout: Bun.file(join(output, "build.log")),
		stderr: Bun.file(join(output, "build-error.log")),
	},
);
if ((await build.exited) !== 0) throw new Error(`Reporting fixture build failed. See ${output}`);
await prepareReportingBuild(site, archive, { VITE_SENTRY_RELEASE: expectedRelease });
const maps = await readdir(join(archive, "files", "assets"));
const privateMap = maps.find((file) => file.endsWith(".js.map"));
if (!privateMap) throw new Error("Reporting fixture has no private source maps");

const receiver = Bun.serve({
	hostname: "127.0.0.1",
	port: 0,
	maxRequestBodySize: 262_144,
	async fetch(request) {
		const url = new URL(request.url);
		const envelope = /^\/api\/(1|2)\/envelope\/$/.exec(url.pathname);
		if (request.method === "POST" && envelope) {
			try {
				recordEnvelope(await request.text(), envelope[1] === "1" ? "web" : "sim");
				return Response.json({});
			} catch (error) {
				const failure = error instanceof EnvelopeFailure ? error.message : "Invalid envelope JSON";
				failures.push(failure);
				return new Response("Invalid reporting envelope", { status: 400 });
			}
		}
		if (request.method === "POST" && url.pathname === "/report") {
			const payload = await request.json();
			if (
				request.headers.get("origin") !== url.origin ||
				payload?.token !== reportToken ||
				!payload.report ||
				typeof payload.report !== "object"
			) {
				return new Response("Invalid fixture report", { status: 400 });
			}
			finish(payload.report);
			return new Response("ok");
		}
		if (request.method === "GET" && url.pathname.startsWith("/sentry-smoke/")) {
			const path = resolve(site, decodeURIComponent(url.pathname.slice("/sentry-smoke/".length)));
			if (!path.startsWith(`${site}/`)) return new Response("Invalid asset path", { status: 400 });
			const file = Bun.file(path);
			return (await file.exists())
				? new Response(file)
				: new Response("Missing asset", { status: 404 });
		}
		if (url.pathname.startsWith("/api/")) failures.push("Unexpected reporting route");
		return new Response("Unknown fixture route", { status: 404 });
	},
});
const mapResponse = await fetch(new URL(`/sentry-smoke/assets/${privateMap}`, receiver.url));
requireCondition(mapResponse.status === 404, "Source map was publicly served");
const url = new URL("/sentry-smoke/tests/browser/sentry-smoke.html", receiver.url);
url.searchParams.set("report-token", reportToken);
url.searchParams.set("fixture", "reporting-fixture-query");
const profile = await mkdtemp(join(tmpdir(), "particle-foundry-reporting-"));
let browser: ReturnType<typeof Bun.spawn> | undefined;
let timeout: ReturnType<typeof setTimeout> | undefined;
try {
	browser = Bun.spawn(
		[
			binary,
			"--headless=new",
			`--user-data-dir=${profile}`,
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
	const report = await Promise.race([
		completion,
		browser.exited.then((code) => {
			throw new Error(`Browser exited before reporting (exit ${code}). See ${output}`);
		}),
		new Promise<never>((_, reject) => {
			timeout = setTimeout(
				() => reject(new Error(`Reporting smoke timed out after 30 seconds. See ${output}`)),
				30_000,
			);
		}),
	]);
	if (events.filter((event) => event.project === "web").length !== 1) {
		failures.push("Expected exactly one browser event");
	}
	if (events.filter((event) => event.project === "sim").length !== 1) {
		failures.push("Expected exactly one Rust WASM panic event");
	}
	const result = {
		status: report.status === "pass" && failures.length === 0 ? "pass" : "fail",
		browser: report,
		events,
		privacy: failures.length === 0 ? "verified" : "failed",
		release: expectedRelease,
		environment: expectedEnvironment,
		failures,
	};
	await Bun.write(join(output, "result.json"), `${JSON.stringify(result, null, 2)}\n`);
	console.log(JSON.stringify(result));
	if (result.status !== "pass") throw new Error(`Reporting smoke failed. See ${output}`);
} finally {
	clearTimeout(timeout);
	await receiver.stop(true);
	if (browser?.exitCode === null) browser.kill("SIGKILL");
	await browser?.exited;
	await rm(profile, { recursive: true, force: true });
}
