import type { ErrorEvent } from "@sentry/browser";

interface SourceEnricher {
	enrich(event: ErrorEvent, error: unknown): Promise<void>;
}

/** Bounds optional diagnostics and isolates work that finishes after the deadline. */
export async function resolveSourceContext(
	event: ErrorEvent,
	error: unknown,
	load: () => Promise<SourceEnricher>,
	timeoutMs = 3_000,
): Promise<ErrorEvent> {
	const candidate: ErrorEvent = {
		...event,
		tags: { ...event.tags },
		exception: event.exception
			? {
					...event.exception,
					values: event.exception.values?.map((exception) => ({
						...exception,
						stacktrace: exception.stacktrace
							? {
									...exception.stacktrace,
									frames: exception.stacktrace.frames?.map((frame) => ({ ...frame })),
								}
							: undefined,
					})),
				}
			: undefined,
	};
	let timeout: ReturnType<typeof setTimeout> | undefined;
	const timedOut = new Promise<undefined>((resolve) => {
		timeout = setTimeout(() => resolve(undefined), timeoutMs);
	});
	const resolved = Promise.resolve()
		.then(load)
		.then(async (enricher) => {
			await enricher.enrich(candidate, error);
			return candidate;
		})
		.catch(() => undefined);
	try {
		return (
			(await Promise.race([resolved, timedOut])) ?? {
				...event,
				tags: { ...event.tags, source_resolution: "unavailable" },
			}
		);
	} finally {
		clearTimeout(timeout);
	}
}
