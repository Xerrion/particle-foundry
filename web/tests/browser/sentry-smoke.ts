import { getCurrentScope, makeFetchTransport } from "@sentry/browser";
import { verifyRustReportingPanic } from "../../src/engine-client/reporting-smoke";
import { initializeErrorReporting } from "../../src/observability/reporting";

const remoteReplies: { status: number; path: string; eventId?: string }[] = [];
const reporting = initializeErrorReporting({
	webDsn: __PF_VERIFY_LIVE__
		? import.meta.env.VITE_SENTRY_WEB_DSN
		: `http://public@${location.host}/1`,
	simDsn: __PF_VERIFY_LIVE__
		? import.meta.env.VITE_SENTRY_SIM_DSN
		: `http://public@${location.host}/2`,
	release: "particle-foundry-reporting-smoke",
	environment: "verification",
	revision: import.meta.env.VITE_SENTRY_REVISION,
	transport: (options) =>
		makeFetchTransport(options, async (input, init) => {
			const response = await fetch(input, init);
			const body = await response
				.clone()
				.json()
				.catch(() => ({}));
			remoteReplies.push({
				status: response.status,
				path: new URL(String(input)).pathname,
				eventId: body.id,
			});
			return response;
		}),
});

async function verify(): Promise<Record<string, unknown>> {
	// Synthetic values prove that the receiver does not retain ambient scope data.
	getCurrentScope().setUser({ id: "reporting-fixture-user" });
	getCurrentScope().setExtra("fixture", "reporting-fixture-extra");
	const browserError = new Promise<boolean>((resolve) => {
		window.addEventListener(
			"error",
			(event) =>
				resolve(event.error?.message === "Particle Foundry browser reporting verification"),
			{ once: true },
		);
		setTimeout(() => {
			throw new Error("Particle Foundry browser reporting verification");
		}, 0);
	});
	const browserErrorObserved = await browserError;
	let rustPanicObserved = false;
	window.addEventListener(
		"particle-foundry:rust-panic",
		(event) => {
			const error: unknown = (event as CustomEvent<unknown>).detail;
			rustPanicObserved = error instanceof Error && error.name === "RustPanic";
		},
		{ once: true },
	);
	let wasmTrapObserved = false;
	let wasmTrapDeduplicated = false;
	try {
		await verifyRustReportingPanic();
	} catch (error) {
		if (!(error instanceof WebAssembly.RuntimeError)) throw error;
		wasmTrapObserved = true;
		wasmTrapDeduplicated = reporting.capture(error, "sim", "verification-trap") === undefined;
	}
	const flushCompleted = await reporting.flush(5_000);
	await reporting.close();
	const deliveryAccepted =
		!__PF_VERIFY_LIVE__ ||
		(remoteReplies.length === 2 &&
			remoteReplies.every((reply) => reply.status === 200) &&
			new Set(remoteReplies.map((reply) => reply.path)).size === 2);
	const passed =
		browserErrorObserved &&
		rustPanicObserved &&
		wasmTrapObserved &&
		wasmTrapDeduplicated &&
		flushCompleted &&
		deliveryAccepted;
	return {
		status: passed ? "pass" : "fail",
		browserErrorObserved,
		rustPanicObserved,
		wasmTrapObserved,
		wasmTrapDeduplicated,
		flushCompleted,
		deliveryAccepted,
	};
}

let report: Record<string, unknown>;
try {
	report = await verify();
} catch (error) {
	// Report the failure class without exposing error details or configuration.
	report = { status: "fail", errorType: error instanceof Error ? error.name : "UnknownError" };
}
document.querySelector("#result")?.replaceChildren(JSON.stringify(report, null, 2));
report.remoteReplies = remoteReplies;
await fetch("/report", {
	method: "POST",
	headers: { "Content-Type": "application/json" },
	body: JSON.stringify({ token: new URL(location.href).searchParams.get("report-token"), report }),
});
