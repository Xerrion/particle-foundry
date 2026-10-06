import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createEvidenceDirectory } from "./evidence-output";

const root = resolve(import.meta.dir, "../..");
const web = join(root, "web");
if (process.argv.length > 3) {
	throw new Error("Usage: test-reporting.ts [new-evidence-directory]");
}
const output = await createEvidenceDirectory(root, process.argv[2], "reporting");
const site = join(output, "site");
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
				? "web/tests/browser/glitchtip-smoke.ts"
				: "engine/crates/wasm/src/reporting.rs";
		const sourceFrame = exception.stacktrace?.frames?.find(
			(frame: Record<string, unknown>) =>
				frame.filename === sourceFile &&
				frame.in_app === true &&
				typeof frame.context_line === "string" &&
				frame.context_line.includes(project === "web" ? "throw new Error" : "panic!("),
		);
		requireCondition(
			sourceFrame?.lineno > 0 && sourceFrame?.colno > 0,
			"Missing actionable source location and code context",
		);
		if (/^[a-f0-9]{40}$/.test(event.tags?.build_revision ?? "")) {
			requireCondition(
				typeof sourceFrame.source_link === "string" &&
					sourceFrame.source_link.includes(
						`/blob/${event.tags.build_revision}/${sourceFile}#L${sourceFrame.lineno}`,
					),
				"Missing exact revision source link",
			);
		}
		requireCondition(
			!["reporting-fixture-user", "reporting-fixture-extra", "reporting-fixture-query"].some(
				(value) => JSON.stringify(event).includes(value),
			),
			"Event retained synthetic private data",
		);
		eventIds.add(event.event_id);
		events.push({ project, exceptionType: exception.type, sourceFile, line: sourceFrame.lineno });
	}
}

const build = Bun.spawn(
	[
		process.execPath,
		join(web, "node_modules/vite/bin/vite.js"),
		"build",
		"--mode",
		"glitchtip-smoke",
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
		if (request.method === "GET" && url.pathname.startsWith("/glitchtip-smoke/")) {
			const path = resolve(
				site,
				decodeURIComponent(url.pathname.slice("/glitchtip-smoke/".length)),
			);
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
const url = new URL("/glitchtip-smoke/tests/browser/glitchtip-smoke.html", receiver.url);
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
