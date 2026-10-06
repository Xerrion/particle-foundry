import { expect, test } from "bun:test";
import { BrowserClient, defaultStackParser, type ErrorEvent, Scope } from "@sentry/browser";
import { resolveSourceContext } from "../../src/observability/source-resolution";

test("the SDK still delivers the error when the source module never loads", async () => {
	let delivered: unknown;
	const client = new BrowserClient({
		dsn: "https://public@errors.invalid/1",
		integrations: [],
		stackParser: defaultStackParser,
		transport: () => ({
			send: async (envelope) => {
				delivered = envelope[1].find(([header]) => header.type === "event")?.[1];
				return { statusCode: 200 };
			},
			flush: async () => true,
		}),
		beforeSend: (event) => resolveSourceContext(event, undefined, () => new Promise(() => {}), 10),
	});
	const scope = new Scope();
	scope.setClient(client);
	client.init();
	scope.captureException(new Error("original failure"));
	expect(await client.flush(500)).toBe(true);
	expect(delivered).toMatchObject({
		tags: { source_resolution: "unavailable" },
		exception: { values: [{ value: "original failure" }] },
	});
	await client.close();
});

test("late diagnostics cannot mutate the event delivered after their deadline", async () => {
	const event: ErrorEvent = {
		type: undefined,
		exception: {
			values: [{ type: "Error", stacktrace: { frames: [{ filename: "original.js", lineno: 1 }] } }],
		},
	};
	let continueDiagnostics: (() => void) | undefined;
	const continuation = new Promise<void>((resolve) => {
		continueDiagnostics = resolve;
	});
	let done: (() => void) | undefined;
	const finished = new Promise<void>((resolve) => {
		done = resolve;
	});
	const result = await resolveSourceContext(
		event,
		undefined,
		async () => ({
			enrich: async (candidate) => {
				await continuation;
				const frame = candidate.exception?.values?.[0]?.stacktrace?.frames?.[0];
				if (frame) frame.filename = "late.ts";
				candidate.tags = { late: "true" };
				done?.();
			},
		}),
		10,
	);
	continueDiagnostics?.();
	await finished;
	expect(result.exception?.values?.[0]?.stacktrace?.frames?.[0].filename).toBe("original.js");
	expect(event.exception?.values?.[0]?.stacktrace?.frames?.[0].filename).toBe("original.js");
	expect(result.tags).toEqual({ source_resolution: "unavailable" });
});

test("closing the production reporter waits for its real diagnostics deadline", async () => {
	const process = Bun.spawn(
		[
			Bun.which("bun") ?? "bun",
			new URL("./fixtures/stalled-source-shutdown.ts", import.meta.url).pathname,
		],
		{ stdout: "pipe", stderr: "pipe" },
	);
	const output = await new Response(process.stdout).text();
	expect(await process.exited).toBe(0);
	expect(JSON.parse(output)).toEqual({ delivered: 1 });
});
