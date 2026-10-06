import { afterEach, expect, spyOn, test } from "bun:test";
import { type BrowserOptions, type ErrorEvent, getGlobalScope } from "@sentry/browser";
import { errorReporting } from "../../src/observability/init";
import { type ErrorReporting, initializeErrorReporting } from "../../src/observability/reporting";

const previousWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
const target = new EventTarget();
let reporter: ErrorReporting | undefined;
const events: { project: string; event: ErrorEvent }[] = [];
const transport: NonNullable<BrowserOptions["transport"]> = (options) => ({
	send(envelope) {
		for (const [header, payload] of envelope[1]) {
			if (header.type === "event") {
				events.push({ project: new URL(options.url).pathname, event: payload as ErrorEvent });
			}
		}
		return Promise.resolve({ statusCode: 200 });
	},
	flush: async () => true,
});

function configured(): ErrorReporting {
	Object.defineProperty(globalThis, "window", { configurable: true, value: target });
	return initializeErrorReporting({
		webDsn: "https://public@errors.invalid/1",
		simDsn: "https://public@errors.invalid/2",
		release: "test-release",
		environment: "test",
		transport,
	});
}

afterEach(async () => {
	await reporter?.close();
	reporter = undefined;
	events.length = 0;
	getGlobalScope().setUser(null);
	getGlobalScope().setExtra("private-data", undefined);
	getGlobalScope().clearBreadcrumbs();
	if (previousWindow) Object.defineProperty(globalThis, "window", previousWindow);
	else Reflect.deleteProperty(globalThis, "window");
});

test("empty DSNs leave both projects disabled", async () => {
	reporter = initializeErrorReporting({ transport });
	expect(reporter.capture(new Error("disabled"), "web")).toBeUndefined();
	expect(reporter.capture(new Error("disabled"), "sim")).toBeUndefined();
	expect(await reporter.flush(1_000)).toBe(true);
	expect(events).toHaveLength(0);
});

test("Bun tests leave application reporting disabled despite local Vite configuration", () => {
	expect(errorReporting.capture(new Error("unit fixture"), "web")).toBeUndefined();
	expect(errorReporting.capture(new Error("unit fixture"), "sim")).toBeUndefined();
});

test("routes browser and handled simulation errors to separate projects", async () => {
	reporter = configured();
	reporter.capture(new Error("browser failure"), "web", "startup");
	reporter.capture("simulation rejection", "sim", "advance");
	expect(await reporter.flush(1_000)).toBe(true);
	expect(events.map(({ project }) => project)).toEqual(["/api/1/envelope/", "/api/2/envelope/"]);
	expect(events[0]?.event.release).toBe("test-release");
	expect(events[1]?.event.tags?.operation).toBe("advance");
});

test("reports the Rust panic once and suppresses its following WASM trap", async () => {
	reporter = configured();
	target.dispatchEvent(
		new CustomEvent("particle-foundry:rust-panic", {
			detail: new Error("panic at solver.rs:42: numerical failure"),
		}),
	);
	const trap = new WebAssembly.RuntimeError("unreachable");
	reporter.capture(trap, "sim", "advance");
	reporter.capture(trap, "web", "global-handler");
	expect(await reporter.flush(1_000)).toBe(true);
	expect(events).toHaveLength(1);
	expect(events[0]?.project).toBe("/api/2/envelope/");
	expect(events[0]?.event.tags?.operation).toBe("rust-panic");
});

test("preserves independent traps after a reported Rust panic", async () => {
	reporter = configured();
	target.dispatchEvent(
		new CustomEvent("particle-foundry:rust-panic", { detail: new Error("Rust panic") }),
	);
	reporter.capture(new WebAssembly.RuntimeError("out of bounds"), "sim", "independent-trap");
	reporter.capture(new WebAssembly.RuntimeError("unreachable"), "sim", "panic-trap");
	reporter.capture(new WebAssembly.RuntimeError("unreachable"), "sim", "new-trap");
	expect(await reporter.flush(1_000)).toBe(true);
	expect(events).toHaveLength(3);
	expect(events.map(({ event }) => event.tags?.operation)).toEqual([
		"rust-panic",
		"independent-trap",
		"new-trap",
	]);
});

test("suppresses the WebKit panic trap while preserving other failures", async () => {
	reporter = configured();
	target.dispatchEvent(
		new CustomEvent("particle-foundry:rust-panic", { detail: new Error("Rust panic") }),
	);
	reporter.capture(
		new WebAssembly.RuntimeError(
			"Unreachable code should not be executed (evaluating 'wasm.advance()')",
		),
		"sim",
		"advance",
	);
	reporter.capture(new WebAssembly.RuntimeError("Out of bounds memory access"), "sim", "new-trap");
	expect(await reporter.flush(1_000)).toBe(true);
	expect(events).toHaveLength(2);
});

test("invalid SDK keys disable reporting without logging the configured DSN", async () => {
	const warning = spyOn(console, "warn").mockImplementation(() => {});
	const sdkError = spyOn(console, "error").mockImplementation(() => {});
	try {
		reporter = initializeErrorReporting({
			webDsn: "https://private-key@errors.invalid/1",
			transport,
		});
		expect(reporter.capture(new Error("invalid configuration"), "web")).toBeUndefined();
		expect(await reporter.flush(1_000)).toBe(true);
		expect(warning).toHaveBeenCalledWith("GlitchTip reporting disabled for an invalid DSN.");
		expect(sdkError).not.toHaveBeenCalled();
		expect(events).toHaveLength(0);
	} finally {
		warning.mockRestore();
		sdkError.mockRestore();
	}
});

test("ignores unrelated custom events and reports a WASM trap without a panic hook", async () => {
	reporter = configured();
	target.dispatchEvent(new CustomEvent("particle-foundry:rust-panic", { detail: "untrusted" }));
	reporter.capture(new WebAssembly.RuntimeError("out of bounds"), "sim", "advance");
	expect(await reporter.flush(1_000)).toBe(true);
	expect(events).toHaveLength(1);
});

test("removes identity and request data from transmitted errors", async () => {
	reporter = configured();
	getGlobalScope().setUser({ id: "synthetic-user", email: "synthetic@example.invalid" });
	getGlobalScope().setExtra("private-data", "synthetic-extra");
	getGlobalScope().addBreadcrumb({
		message: "synthetic request",
		data: { url: "https://example.invalid/?secret=fake" },
	});
	reporter.capture(new Error("privacy check"), "web");
	expect(await reporter.flush(1_000)).toBe(true);
	const event = events[0]?.event;
	expect(event).toBeDefined();
	expect(event?.user).toBeUndefined();
	expect(event?.request).toBeUndefined();
	expect(event?.breadcrumbs).toBeUndefined();
	expect(event?.extra).toBeUndefined();
});

test("closing the reporter removes the WASM panic listener", async () => {
	reporter = configured();
	await reporter.close();
	target.dispatchEvent(
		new CustomEvent("particle-foundry:rust-panic", { detail: new Error("late panic") }),
	);
	expect(events).toHaveLength(0);
});

test("removes query parameters and fragments from error stack URLs", async () => {
	reporter = configured();
	const error = new Error("stack URL privacy check");
	error.stack =
		"Error: stack URL privacy check\n    at app (https://example.invalid/app.js?token=synthetic#fragment:1:1)";
	reporter.capture(error, "web");
	expect(await reporter.flush(1_000)).toBe(true);
	const serialized = JSON.stringify(events);
	expect(serialized).toContain("https://example.invalid/app.js");
	expect(serialized).not.toContain("token=synthetic");
	expect(serialized).not.toContain("#fragment");
});
