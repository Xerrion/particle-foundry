import loadWasm, { type Engine, initialize } from "../../generated/wasm/particle_wasm";

// Low-level boundary exports let the browser fixture check Rust input validation and MIME.
export { initialize as initializeWasmEngine };
export const wasmAssetUrl = new URL("../../generated/wasm/particle_wasm_bg.wasm", import.meta.url);

let bindings: ReturnType<typeof loadWasm> | undefined;

/** Initializes the experimental lifecycle boundary; it does not replace a legacy scene. */
export async function initializeEngine(
	width: number,
	height: number,
	execution: "cpu-reference" | "wgpu",
): Promise<Engine> {
	if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1) {
		throw new RangeError("Engine dimensions must be positive integers");
	}
	bindings ??= loadWasm().catch((error: unknown) => {
		bindings = undefined;
		throw error;
	});
	await bindings;
	return initialize(width, height, execution);
}
