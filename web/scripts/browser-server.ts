interface StartupOptions {
	timeoutMs?: number;
	requestTimeoutMs?: number;
	pollIntervalMs?: number;
}

/** Poll the fixture server with a separate deadline for each HTTP request. */
export async function waitForBrowserServer(
	url: URL,
	exitCode: () => number | null,
	{ timeoutMs = 15_000, requestTimeoutMs = 500, pollIntervalMs = 100 }: StartupOptions = {},
): Promise<void> {
	const startup = AbortSignal.timeout(timeoutMs);
	let lastAttempt = "No HTTP response";
	while (!startup.aborted) {
		if (exitCode() !== null) {
			throw new Error("Browser test server exited; see server-error.log");
		}
		try {
			const response = await fetch(url, {
				signal: AbortSignal.any([startup, AbortSignal.timeout(requestTimeoutMs)]),
			});
			lastAttempt = `HTTP ${response.status}`;
			await response.body?.cancel();
			if (response.ok) return;
		} catch (error) {
			if (!startup.aborted) {
				lastAttempt = error instanceof Error ? error.name : "Unknown request error";
			}
		}
		if (!startup.aborted) await Bun.sleep(pollIntervalMs);
	}
	throw new Error(
		`Browser test server did not become ready in ${timeoutMs / 1000} seconds (${lastAttempt}); see server.log and server-error.log`,
	);
}
