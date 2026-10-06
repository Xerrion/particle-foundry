import * as bindings from "../../generated/wasm/particle_wasm";

/** Runs the deliberate panic available only in the separate reporting fixture build. */
export async function verifyRustReportingPanic(): Promise<void> {
	await bindings.default();
	const panic: unknown = Reflect.get(bindings, "verify_reporting_panic");
	if (typeof panic !== "function") {
		throw new Error("The reporting fixture requires its separate WASM feature build.");
	}
	panic();
}
