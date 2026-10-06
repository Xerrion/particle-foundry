import {
	BrowserClient,
	type BrowserOptions,
	browserApiErrorsIntegration,
	dedupeIntegration,
	defaultStackParser,
	eventFiltersIntegration,
	globalHandlersIntegration,
	makeFetchTransport,
	Scope,
	setCurrentClient,
} from "@sentry/browser";
import { annotateRustOrigin } from "./rust-origin";
import { resolveSourceContext } from "./source-resolution";

export interface ReportingConfiguration {
	webDsn?: string;
	simDsn?: string;
	release?: string;
	environment?: string;
	transport?: BrowserOptions["transport"];
	baseUrl?: string;
	revision?: string;
}

export interface ErrorReporting {
	capture(error: unknown, project: "web" | "sim", context?: string): string | undefined;
	flush(timeoutMs: number): Promise<boolean>;
	close(): Promise<void>;
}

function configuredDsn(value: string | undefined): string | undefined {
	if (!value?.trim()) return undefined;
	try {
		const url = new URL(value.trim());
		if (
			(url.protocol === "https:" || url.protocol === "http:") &&
			/^\w+$/.test(url.username) &&
			!url.password &&
			/^\/\d+$/.test(url.pathname) &&
			!url.search &&
			!url.hash
		)
			return url.href;
	} catch {
		// A configuration failure must not stop the sandbox or expose its DSN.
	}
	console.warn("GlitchTip reporting disabled for an invalid DSN.");
	return undefined;
}

/** Starts error reporting at the host boundary. Empty DSNs disable their project. */
export function initializeErrorReporting(config: ReportingConfiguration): ErrorReporting {
	let lastPanicAt = Number.NEGATIVE_INFINITY;
	const reportedTraps = new WeakSet<WebAssembly.RuntimeError>();
	const scopes = new Map<"web" | "sim", Scope>();
	const clients: BrowserClient[] = [];
	let sourceEnricher:
		| Promise<ReturnType<typeof import("./source-context")["createSourceEnricher"]>>
		| undefined;
	function isReportedTrap(error: unknown): boolean {
		if (!(error instanceof WebAssembly.RuntimeError)) return false;
		if (reportedTraps.has(error)) return true;
		if (
			(error.message !== "unreachable" &&
				error.message !== "unreachable executed" &&
				!/^Unreachable code should not be executed(?: \(evaluating .*\))?$/.test(error.message)) ||
			Date.now() - lastPanicAt >= 1_000
		)
			return false;
		lastPanicAt = Number.NEGATIVE_INFINITY;
		reportedTraps.add(error);
		return true;
	}

	for (const project of ["web", "sim"] as const) {
		const dsn = configuredDsn(project === "web" ? config.webDsn : config.simDsn);
		if (!dsn) continue;
		const integrations = [
			eventFiltersIntegration(),
			dedupeIntegration(),
			...(project === "web" ? [globalHandlersIntegration(), browserApiErrorsIntegration()] : []),
		];
		const client = new BrowserClient({
			dsn,
			release: config.release || undefined,
			environment: config.environment || "production",
			transport: config.transport ?? makeFetchTransport,
			stackParser: defaultStackParser,
			integrations,
			maxBreadcrumbs: 0,
			sendClientReports: false,
			dataCollection: {
				userInfo: false,
				cookies: false,
				httpHeaders: false,
				httpBodies: [],
				urlQueryParams: false,
				stackFrameVariables: false,
				frameContextLines: 0,
			},
			async beforeSend(event, hint) {
				if (isReportedTrap(hint.originalException)) return null;
				annotateRustOrigin(event, hint.originalException, config.revision);
				event = await resolveSourceContext(event, hint.originalException, () => {
					sourceEnricher ??= import("./source-context").then(({ createSourceEnricher }) =>
						createSourceEnricher({ baseUrl: config.baseUrl, revision: config.revision }),
					);
					return sourceEnricher;
				});
				event.tags = { ...event.tags, project: `particle-foundry-${project}` };
				if (config.revision) event.tags.build_revision = config.revision;
				delete event.user;
				delete event.request;
				delete event.breadcrumbs;
				delete event.extra;
				const traces = [
					...(event.exception?.values?.map((exception) => exception.stacktrace) ?? []),
					...(event.threads?.values?.map((thread) => thread.stacktrace) ?? []),
				];
				for (const trace of traces) {
					for (const frame of trace?.frames ?? []) {
						if (frame.filename) frame.filename = frame.filename.split(/[?#]/, 1)[0];
						if (frame.abs_path) frame.abs_path = frame.abs_path.split(/[?#]/, 1)[0];
					}
				}
				for (const image of event.debug_meta?.images ?? []) {
					if ("code_file" in image && typeof image.code_file === "string") {
						image.code_file = image.code_file.split(/[?#]/, 1)[0];
					}
				}
				return event;
			},
			beforeSendLog: () => null,
			beforeSendMetric: () => null,
		});
		const scope = new Scope();
		scope.setClient(client);
		scope.setTag("project", `particle-foundry-${project}`);
		scopes.set(project, scope);
		clients.push(client);
		if (project === "web") setCurrentClient(client);
		client.init();
	}

	const reporting: ErrorReporting = {
		capture(error, project, context) {
			if (isReportedTrap(error)) return undefined;
			const scope = scopes.get(project);
			if (!scope) return undefined;
			const exception = error instanceof Error ? error : new Error(String(error));
			return scope.captureException(exception, {
				captureContext: { tags: { operation: context ?? "unknown" } },
			});
		},
		async flush(timeoutMs) {
			return (await Promise.all(clients.map((client) => client.flush(timeoutMs)))).every(Boolean);
		},
		async close() {
			if (scopes.has("sim")) window.removeEventListener("particle-foundry:rust-panic", onPanic);
			await Promise.all(clients.map((client) => client.close(5_000)));
		},
	};

	function onPanic(event: Event): void {
		const error: unknown = (event as CustomEvent<unknown>).detail;
		if (!(error instanceof Error)) return;
		lastPanicAt = Date.now();
		reporting.capture(error, "sim", "rust-panic");
	}
	if (scopes.has("sim")) window.addEventListener("particle-foundry:rust-panic", onPanic);
	return reporting;
}
