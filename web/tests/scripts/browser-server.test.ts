import { describe, expect, test } from "bun:test";

import { waitForBrowserServer } from "../../scripts/browser-server";

describe("browser fixture startup", () => {
	test("retries when the first HTTP request stalls but the server becomes ready", async () => {
		let requests = 0;
		const server = Bun.serve({
			hostname: "127.0.0.1",
			port: 0,
			fetch() {
				requests += 1;
				if (requests === 1) return new Promise<Response>(() => {});
				return new Response("ready");
			},
		});
		try {
			await waitForBrowserServer(new URL(`http://127.0.0.1:${server.port}`), () => null, {
				timeoutMs: 1000,
				requestTimeoutMs: 50,
				pollIntervalMs: 10,
			});
			expect(requests).toBeGreaterThanOrEqual(2);
		} finally {
			await server.stop(true);
		}
	});

	test("rejects within the overall deadline when every request stalls", async () => {
		let requests = 0;
		const server = Bun.serve({
			hostname: "127.0.0.1",
			port: 0,
			fetch() {
				requests += 1;
				return new Promise<Response>(() => {});
			},
		});
		try {
			const start = performance.now();
			await expect(
				waitForBrowserServer(new URL(`http://127.0.0.1:${server.port}`), () => null, {
					timeoutMs: 200,
					requestTimeoutMs: 50,
					pollIntervalMs: 10,
				}),
			).rejects.toThrow("did not become ready in 0.2 seconds");
			expect(performance.now() - start).toBeLessThan(1000);
			expect(requests).toBeGreaterThanOrEqual(2);
		} finally {
			await server.stop(true);
		}
	});

	test("reports a non-success HTTP status instead of declaring readiness", async () => {
		const server = Bun.serve({
			hostname: "127.0.0.1",
			port: 0,
			fetch: () => new Response("missing", { status: 404 }),
		});
		try {
			await expect(
				waitForBrowserServer(new URL(`http://127.0.0.1:${server.port}`), () => null, {
					timeoutMs: 200,
					requestTimeoutMs: 50,
					pollIntervalMs: 10,
				}),
			).rejects.toThrow("HTTP 404");
		} finally {
			await server.stop(true);
		}
	});

	test("rejects an exited server before probing HTTP", async () => {
		await expect(waitForBrowserServer(new URL("http://127.0.0.1:1"), () => 1)).rejects.toThrow(
			"server exited",
		);
	});
});
