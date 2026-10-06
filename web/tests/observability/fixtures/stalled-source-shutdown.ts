import { mock } from "bun:test";

mock.module("../../../src/observability/source-context", () => ({
	createSourceEnricher: () => ({ enrich: () => new Promise<void>(() => {}) }),
}));
const { initializeErrorReporting } = await import("../../../src/observability/reporting");
let delivered = 0;
const reporting = initializeErrorReporting({
	webDsn: "https://public@errors.invalid/1",
	transport: () => ({
		send: async () => {
			delivered++;
			return { statusCode: 200 };
		},
		flush: async () => true,
	}),
});
reporting.capture(new Error("shutdown verification"), "web");
await reporting.close();
console.log(JSON.stringify({ delivered }));
