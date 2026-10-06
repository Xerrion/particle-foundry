import { expect, test } from "bun:test";
import type { ErrorEvent } from "@sentry/browser";
import { annotateRustOrigin } from "../../src/observability/rust-origin";

function event(): ErrorEvent {
	return { type: undefined, exception: { values: [{ type: "RustPanic", value: "verification" }] } };
}

test("preserves the precise panic origin with an immutable revision link", () => {
	const report = event();
	const error = Object.assign(new Error("verification"), {
		name: "RustPanic",
		rustLocation: { file: "crates/wasm/src/reporting.rs", line: 44, column: 5 },
	});
	annotateRustOrigin(report, error, "a".repeat(40));
	annotateRustOrigin(report, error, "a".repeat(40));
	const exception = report.exception?.values?.[0];
	expect(exception?.stacktrace?.frames).toHaveLength(1);
	expect(exception?.stacktrace?.frames?.[0]).toMatchObject({
		filename: "engine/crates/wasm/src/reporting.rs",
		lineno: 44,
		colno: 5,
		in_app: true,
		platform: "rust",
		source_link: `https://github.com/Xerrion/particle-foundry/blob/${"a".repeat(40)}/engine/crates/wasm/src/reporting.rs#L44`,
	});
	expect(exception?.mechanism).toEqual({ type: "rust-panic", handled: false });
});

test("retains the origin without a link when the revision is unknown", () => {
	const report = event();
	annotateRustOrigin(
		report,
		Object.assign(new Error("verification"), {
			name: "RustPanic",
			rustLocation: { file: "crates/wasm/src/reporting.rs", line: 44, column: 5 },
		}),
		"unknown",
	);
	expect(report.exception?.values?.[0]?.stacktrace?.frames?.[0]).not.toHaveProperty("source_link");
	expect(report.exception?.values?.[0]?.stacktrace?.frames?.[0]?.lineno).toBe(44);
});

test("rejects paths and positions that cannot identify an authored Rust file", () => {
	for (const location of [
		{ file: "crates/../../.env", line: 1, column: 1 },
		{ file: "crates/wasm/src/reporting.rs", line: 0, column: 1 },
		{ file: "crates/wasm/src/reporting.rs", line: 1, column: Number.NaN },
	]) {
		const report = event();
		annotateRustOrigin(
			report,
			Object.assign(new Error("verification"), { name: "RustPanic", rustLocation: location }),
		);
		expect(report.exception?.values?.[0]?.stacktrace).toBeUndefined();
	}
});
