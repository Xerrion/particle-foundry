import loadWasm, {
	type BrowserGpuScene,
	create_browser_gpu_scene,
	type Engine,
	initialize,
} from "../../generated/wasm/particle_wasm";

// Low-level boundary exports let the browser fixture check Rust input validation and MIME.
export { initialize as initializeWasmEngine };
export const wasmAssetUrl = new URL("../../generated/wasm/particle_wasm_bg.wasm", import.meta.url);

let bindings: ReturnType<typeof loadWasm> | undefined;

export interface BrowserGpuSceneSession {
	backend(): string;
	status_json(): string;
	render(): boolean;
	resize(width: number, height: number): void;
	advance(maxSubsteps: number): Promise<string>;
	paint(centerX: number, centerY: number, radius: number, material: "water" | "air"): string;
	probe(x: number, y: number): Promise<string>;
	checkpoint_prototype(): Promise<Uint8Array>;
	reset(): string;
	dispose(): void;
	free(): void;
}

function positiveSafeInteger(value: number, label: string, maximum: number): void {
	if (!Number.isSafeInteger(value) || value < 1 || value > maximum) {
		throw new RangeError(`${label} must be an integer between 1 and ${maximum}`);
	}
}

function nonnegativeSafeInteger(value: number, label: string, maximum: number): void {
	if (!Number.isSafeInteger(value) || value < 0 || value > maximum) {
		throw new RangeError(`${label} must be an integer between 0 and ${maximum}`);
	}
}

function checkedBrowserScene(raw: BrowserGpuScene): BrowserGpuSceneSession {
	return {
		backend: () => raw.backend(),
		status_json: () => raw.status_json(),
		render: () => raw.render(),
		resize(width, height) {
			positiveSafeInteger(width, "GPU canvas width", 0xffff_ffff);
			positiveSafeInteger(height, "GPU canvas height", 0xffff_ffff);
			raw.resize(width, height);
		},
		advance(maxSubsteps) {
			positiveSafeInteger(maxSubsteps, "GPU substep budget", 8);
			return raw.advance(maxSubsteps) as Promise<string>;
		},
		paint(centerX, centerY, radius, material) {
			nonnegativeSafeInteger(centerX, "GPU paint x", 479);
			nonnegativeSafeInteger(centerY, "GPU paint y", 269);
			nonnegativeSafeInteger(radius, "GPU paint radius", 16);
			if (material !== "water" && material !== "air") {
				throw new RangeError("GPU paint material must be water or air");
			}
			return raw.paint(centerX, centerY, radius, material);
		},
		probe(x, y) {
			nonnegativeSafeInteger(x, "GPU probe x", 479);
			nonnegativeSafeInteger(y, "GPU probe y", 269);
			return raw.probe(x, y) as Promise<string>;
		},
		checkpoint_prototype: () => raw.checkpoint_prototype() as Promise<Uint8Array>,
		reset: () => raw.reset(),
		dispose: () => raw.dispose(),
		free: () => raw.free(),
	};
}

async function ready(): Promise<void> {
	bindings ??= loadWasm().catch((error: unknown) => {
		bindings = undefined;
		throw error;
	});
	await bindings;
}

/** Creates the opt-in browser scene before any 2D context is selected. */
export async function initializeBrowserGpuScene(
	canvas: HTMLCanvasElement,
): Promise<BrowserGpuSceneSession> {
	if (canvas.width < 1 || canvas.height < 1) {
		throw new RangeError("GPU canvas backing dimensions must be positive");
	}
	await ready();
	return checkedBrowserScene(await create_browser_gpu_scene(canvas));
}

/** Initializes the experimental lifecycle boundary; it does not replace a legacy scene. */
export async function initializeEngine(
	width: number,
	height: number,
	execution: "cpu-reference" | "wgpu",
): Promise<Engine> {
	if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1) {
		throw new RangeError("Engine dimensions must be positive integers");
	}
	await ready();
	return initialize(width, height, execution);
}
