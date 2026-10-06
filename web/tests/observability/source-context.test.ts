import { expect, test } from "bun:test";
import type { ErrorEvent } from "@sentry/browser";
import { SourceMapGenerator } from "source-map-js";
import { createSourceEnricher } from "../../src/observability/source-context";

const revision = "a".repeat(40);
const baseUrl = "https://sandbox.example.invalid/preview/";
const assetUrl = `${baseUrl}assets/app.js`;

function event(type = "Error"): ErrorEvent {
	return {
		type: undefined,
		exception: {
			values: [
				{
					type,
					value: "failure",
					stacktrace: { frames: [{ filename: assetUrl, lineno: 1, colno: 21 }] },
				},
			],
		},
	};
}

function map(source = "../../src/app/viewport.ts") {
	const generator = new SourceMapGenerator({ file: "app.js" });
	generator.addMapping({
		generated: { line: 1, column: 20 },
		original: { line: 3, column: 4 },
		source,
		name: "zoomAt",
	});
	generator.setSourceContent(
		source,
		"export function zoomAt() {\n  const size = 0;\n    throw new RangeError('invalid viewport');\n}\n",
	);
	return generator.toJSON();
}

test("maps a minified frame to authored source, correct columns, context and revision", async () => {
	const requests: string[] = [];
	const enricher = createSourceEnricher({
		baseUrl,
		revision,
		fetch: async (input, init) => {
			requests.push(String(input));
			expect(init?.credentials).toBe("omit");
			expect(init?.referrerPolicy).toBe("no-referrer");
			return Response.json(map());
		},
	});
	const report = event();
	report.exception?.values?.[0]?.stacktrace?.frames?.push({
		filename: `${assetUrl}?private=synthetic`,
		lineno: 1,
		colno: 21,
	});
	await enricher.enrich(report, new Error("failure"));
	const frame = report.exception?.values?.[0]?.stacktrace?.frames?.[0];
	expect(frame).toMatchObject({
		filename: "web/src/app/viewport.ts",
		lineno: 3,
		colno: 5,
		function: "zoomAt",
		in_app: true,
		context_line: "    throw new RangeError('invalid viewport');",
		source_link: `https://github.com/Xerrion/particle-foundry/blob/${revision}/web/src/app/viewport.ts#L3`,
	});
	expect(report.tags?.source_frames).toBe("2");
	expect(requests).toEqual([`${assetUrl}.map`]);
});

test("preserves an error when a map is missing or malformed", async () => {
	for (const response of [
		new Response("missing", { status: 404 }),
		new Response("invalid JSON"),
		Response.json({ version: 3 }),
	]) {
		const enricher = createSourceEnricher({ baseUrl, fetch: async () => response });
		const report = event();
		await enricher.enrich(report, new Error("failure"));
		expect(report.exception?.values?.[0]?.stacktrace?.frames?.[0]).toMatchObject({
			filename: assetUrl,
			lineno: 1,
			colno: 21,
		});
		expect(report.tags?.source_frames).toBe("0");
	}
});

test("does not fetch frames outside the application origin or base path", async () => {
	let calls = 0;
	const enricher = createSourceEnricher({
		baseUrl,
		fetch: async () => {
			calls++;
			return Response.json(map());
		},
	});
	const report = event();
	const frames = report.exception?.values?.[0]?.stacktrace?.frames;
	if (!frames) throw new Error("Missing fixture frames");
	frames[0] = { filename: "https://outside.example.invalid/app.js", lineno: 1, colno: 21 };
	frames.push({ filename: "https://sandbox.example.invalid/private/app.js", lineno: 1, colno: 21 });
	await enricher.enrich(report, new Error("failure"));
	expect(calls).toBe(0);
});

test("does not include source snippets from dependencies", async () => {
	const enricher = createSourceEnricher({
		baseUrl,
		fetch: async () => Response.json(map("../../node_modules/dependency/src/private.ts")),
	});
	const report = event();
	await enricher.enrich(report, new Error("failure"));
	const frame = report.exception?.values?.[0]?.stacktrace?.frames?.[0];
	expect(frame?.in_app).toBe(false);
	expect(frame?.context_line).toBeUndefined();
});

test("adds a precise Rust panic origin and source context", async () => {
	const origin = "engine/crates/wasm/src/reporting.rs";
	const requests: string[] = [];
	const enricher = createSourceEnricher({
		baseUrl,
		revision,
		fetch: async (input) => {
			requests.push(String(input));
			return String(input).endsWith(".map")
				? new Response("missing", { status: 404 })
				: Response.json({
						file: origin,
						lines: ["fn verification() {", '    panic!("verification");', "}"],
					});
		},
	});
	const error = Object.assign(new Error("Rust panic"), {
		name: "RustPanic",
		rustLocation: { file: "crates/wasm/src/reporting.rs", line: 2, column: 5 },
	});
	const report = event("RustPanic");
	await enricher.enrich(report, error);
	const exception = report.exception?.values?.[0];
	expect(exception?.stacktrace?.frames?.at(-1)).toMatchObject({
		filename: origin,
		lineno: 2,
		colno: 5,
		in_app: true,
		context_line: '    panic!("verification");',
		source_link: `https://github.com/Xerrion/particle-foundry/blob/${revision}/${origin}#L2`,
	});
	expect(exception?.mechanism).toEqual({ type: "rust-panic", handled: false });
	expect(requests).toContain(`${baseUrl}reporting-sources/${origin}.json`);
});

test("preserves a Rust location when source fetching fails and excludes dirty source links", async () => {
	const enricher = createSourceEnricher({
		baseUrl,
		revision: `${revision}-dirty`,
		fetch: async () => {
			throw new Error("Network unavailable");
		},
	});
	const report = event("RustPanic");
	await enricher.enrich(
		report,
		Object.assign(new Error("panic"), {
			name: "RustPanic",
			rustLocation: { file: "crates/wasm/src/reporting.rs", line: 2, column: 5 },
		}),
	);
	const frame = report.exception?.values?.[0]?.stacktrace?.frames?.at(-1);
	expect(frame).toMatchObject({
		filename: "engine/crates/wasm/src/reporting.rs",
		lineno: 2,
		colno: 5,
	});
	expect(frame).not.toHaveProperty("source_link");
});

test("rejects malformed and traversal panic locations", async () => {
	let calls = 0;
	const enricher = createSourceEnricher({
		baseUrl,
		fetch: async () => {
			calls++;
			return Response.json({});
		},
	});
	const report: ErrorEvent = {
		type: undefined,
		exception: { values: [{ type: "RustPanic", value: "panic" }] },
	};
	await enricher.enrich(
		report,
		Object.assign(new Error("panic"), {
			name: "RustPanic",
			rustLocation: { file: "crates/../../.env", line: 1, column: 1 },
		}),
	);
	expect(calls).toBe(0);
	expect(report.exception?.values?.[0]?.stacktrace).toBeUndefined();
});

test("retries a source map after a transient fetch failure", async () => {
	let calls = 0;
	const enricher = createSourceEnricher({
		baseUrl,
		fetch: async () => {
			calls++;
			return calls === 1
				? new Response("temporary failure", { status: 404 })
				: Response.json(map());
		},
	});
	const first = event();
	await enricher.enrich(first, new Error("first error"));
	expect(first.tags?.source_frames).toBe("0");
	const second = event();
	await enricher.enrich(second, new Error("second error"));
	expect(second.exception?.values?.[0]?.stacktrace?.frames?.[0].filename).toBe(
		"web/src/app/viewport.ts",
	);
	expect(calls).toBe(2);
});
