import {
	initializeWasmEngine as initialize,
	initializeEngine,
	wasmAssetUrl,
} from "../../src/engine-client/wasm";

const result = document.querySelector("#result");
if (!result) throw new Error("Smoke-test result element is missing");

async function rejects(action: () => Promise<unknown>, label: string): Promise<void> {
	try {
		await action();
	} catch {
		return;
	}
	throw new Error(`Invalid input accepted: ${label}`);
}

try {
	const wasm = await fetch(wasmAssetUrl);
	const mime = wasm.headers.get("content-type")?.split(";")[0];
	if (!wasm.ok || mime !== "application/wasm") {
		throw new Error(`WASM asset failed: HTTP ${wasm.status}, Content-Type ${mime}`);
	}
	const wasmBytes = (await wasm.arrayBuffer()).byteLength;
	for (let cycle = 0; cycle < 3; cycle++) {
		const engine = await initializeEngine(480, 270, "cpu-reference");
		if (engine.backend() !== "cpu-reference-bootstrap") throw new Error("Wrong lifecycle owner");
		engine.dispose();
		engine.dispose();
		let rejected = false;
		try {
			engine.backend();
		} catch {
			rejected = true;
		}
		if (!rejected) throw new Error("Disposed engine still accepts observations");
		engine.free();
	}
	let invalidInputs = 0;
	// Check both the thin host and direct generated exports so JS coercion cannot hide a Rust bug.
	for (const init of [initializeEngine, initialize]) {
		for (const invalid of [0, -1, 1.5, Number.NaN, Number.POSITIVE_INFINITY, 4097, 2 ** 32 + 1]) {
			await rejects(() => init(invalid, 270, "cpu-reference"), `width ${invalid}`);
			await rejects(() => init(480, invalid, "cpu-reference"), `height ${invalid}`);
			invalidInputs += 2;
		}
		await rejects(() => init(4096, 4096, "cpu-reference"), "cell limit");
		invalidInputs++;
	}
	await rejects(() => initialize(1, 1, "not-a-backend"), "execution backend");
	invalidInputs++;
	// Concurrent initialization reuses the module, not session state.
	const sessions = await Promise.all([
		initializeEngine(2, 3, "cpu-reference"),
		initializeEngine(4, 5, "cpu-reference"),
	]);
	sessions[0].free();
	if (sessions[1].backend() !== "cpu-reference-bootstrap")
		throw new Error("Session ownership leaked");
	sessions[1].free();

	// A missing WebGPU API must reject, never create a CPU session silently.
	const originalGpu = Object.getOwnPropertyDescriptor(navigator, "gpu");
	try {
		Object.defineProperty(navigator, "gpu", { value: undefined, configurable: true });
		await rejects(() => initializeEngine(480, 270, "wgpu"), "unavailable WebGPU");
	} finally {
		if (originalGpu) Object.defineProperty(navigator, "gpu", originalGpu);
		else Reflect.deleteProperty(navigator, "gpu");
	}

	let gpu: { status: "pass" | "unavailable"; backend?: string; error?: string };
	try {
		const engine = await initializeEngine(480, 270, "wgpu");
		try {
			const backend = engine.backend();
			if (backend !== "BrowserWebGpu")
				throw new Error(`Unexpected browser GPU backend: ${backend}`);
			gpu = { status: "pass", backend };
			engine.dispose();
			engine.dispose();
		} finally {
			engine.free();
		}
	} catch (error) {
		const message = String(error);
		if (
			!message.includes("adapter unavailable:") &&
			!message.includes("device initialization failed:")
		) {
			throw error;
		}
		gpu = { status: "unavailable", error: message };
	}
	if (new URL(location.href).searchParams.has("require-gpu") && gpu.status !== "pass") {
		throw new Error(`Required GPU smoke unavailable: ${gpu.error}`);
	}
	result.textContent = JSON.stringify({
		status: "pass",
		backend: "WASM CPU bootstrap",
		cycles: 3,
		invalidInputs,
		concurrentSessions: 2,
		unavailableGpuRejected: true,
		gpu,
		wasm: { bytes: wasmBytes, mime, url: wasm.url },
		userAgent: navigator.userAgent,
		secureContext: isSecureContext,
	});
} catch (error) {
	result.textContent = JSON.stringify({ status: "fail", error: String(error) });
}

const reportUrl = new URL(location.href).searchParams.get("report");
if (reportUrl) await fetch(reportUrl, { method: "POST", body: result.textContent });
