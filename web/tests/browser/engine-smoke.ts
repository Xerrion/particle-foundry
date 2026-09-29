import { create_browser_gpu_scene } from "../../generated/wasm/particle_wasm";
import {
	initializeWasmEngine as initialize,
	initializeBrowserGpuScene,
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

type GpuAbiReport = {
	backend: string;
	maxStorageBufferBindingSize: number;
	maxBufferSize: number;
	maxComputeWorkgroupsPerDimension: number;
	maxComputeInvocationsPerWorkgroup: number;
	sentinelBytes: number;
	coreFieldBytes: number;
};

type GpuSceneReport = {
	backend: string;
	epoch: number;
	tick: number;
	acceptedTimeS: number;
	acceptedSubsteps: number;
	scaledResidual: number;
	scaledDivergence: number;
	encodedTimeErrorS: number;
	rejectionKeptCommittedState: boolean;
};

function parseGpuAbiReport(json: string, backend: string): GpuAbiReport {
	const value: unknown = JSON.parse(json);
	if (!value || typeof value !== "object") throw new Error("GPU ABI report is not an object");
	const report = value as Partial<GpuAbiReport>;
	const limits = [
		report.maxStorageBufferBindingSize,
		report.maxBufferSize,
		report.maxComputeWorkgroupsPerDimension,
		report.maxComputeInvocationsPerWorkgroup,
		report.sentinelBytes,
		report.coreFieldBytes,
	];
	if (
		report.backend !== backend ||
		limits.some(
			(limit) => typeof limit !== "number" || !Number.isSafeInteger(limit) || limit < 1,
		) ||
		(report.maxStorageBufferBindingSize ?? 0) < (report.sentinelBytes ?? 0) ||
		(report.maxBufferSize ?? 0) < (report.sentinelBytes ?? 0) ||
		report.coreFieldBytes !== 3_113_416
	) {
		throw new Error("GPU ABI report has invalid backend or limits");
	}
	return report as GpuAbiReport;
}

function parseGpuSceneReport(json: string, backend: string): GpuSceneReport {
	const value: unknown = JSON.parse(json);
	if (!value || typeof value !== "object") throw new Error("GPU scene report is not an object");
	const report = value as Partial<GpuSceneReport>;
	if (
		report.backend !== backend ||
		report.epoch !== 2 ||
		report.tick !== 8 ||
		typeof report.acceptedTimeS !== "number" ||
		!Number.isFinite(report.acceptedTimeS) ||
		Math.abs(report.acceptedTimeS - 8 / 60) > 1e-6 ||
		typeof report.acceptedSubsteps !== "number" ||
		!Number.isSafeInteger(report.acceptedSubsteps) ||
		report.acceptedSubsteps < 1 ||
		report.acceptedSubsteps > 8 ||
		typeof report.scaledResidual !== "number" ||
		!Number.isFinite(report.scaledResidual) ||
		report.scaledResidual < 0 ||
		report.scaledResidual >= 1e-5 ||
		typeof report.scaledDivergence !== "number" ||
		!Number.isFinite(report.scaledDivergence) ||
		report.scaledDivergence < 0 ||
		report.scaledDivergence >= 1e-5 ||
		typeof report.encodedTimeErrorS !== "number" ||
		!Number.isFinite(report.encodedTimeErrorS) ||
		Math.abs(report.encodedTimeErrorS) > 1e-6 ||
		report.rejectionKeptCommittedState !== true
	) {
		throw new Error("GPU scene fixture did not complete one safe outer tick");
	}
	return report as GpuSceneReport;
}

async function settleValidationAfterDispose(
	pending: Promise<unknown>,
	backend: string,
	label: string,
	parse: (json: string, backend: string) => unknown,
): Promise<"completed" | "rejected"> {
	let timeout: ReturnType<typeof setTimeout> | undefined;
	try {
		const outcome = await Promise.race([
			pending.then(
				(value) => ({ status: "completed" as const, value }),
				(error: unknown) => ({ status: "rejected" as const, error }),
			),
			new Promise<never>((_, reject) => {
				timeout = setTimeout(() => reject(new Error(`${label} did not settle`)), 10_000);
			}),
		]);
		if (outcome.status === "rejected") {
			if (
				typeof outcome.error !== "string" ||
				!outcome.error ||
				/recursive|borrow/i.test(outcome.error)
			) {
				throw new Error(`${label} did not reject cleanly: ${String(outcome.error)}`);
			}
			return "rejected";
		}
		if (typeof outcome.value !== "string") throw new Error(`${label} is not a string`);
		parse(outcome.value, backend);
		return "completed";
	} finally {
		clearTimeout(timeout);
	}
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

	let gpu: {
		status: "pass" | "unavailable";
		backend?: string;
		abi?: GpuAbiReport;
		scene?: GpuSceneReport;
		disposeDuringValidation?: "completed" | "rejected";
		sceneDisposeDuringValidation?: "completed" | "rejected";
		canvas?: {
			initialEpoch: number;
			initialTick: number;
			initialRendered: boolean;
			resetEpoch: number;
			staleAdvanceRejected: boolean;
			acceptedSubsteps: number;
			acceptedTimeS: number;
			scaledResidual: number;
			scaledDivergence: number;
			secondScaledResidual: number;
			secondScaledDivergence: number;
			probedCells: number;
			checkpointBytes: number;
			readbackBytesFirstTwo: number;
			paintRevisionAdvanced: boolean;
			staleProbeRejected: boolean;
			staleCheckpointRejected: boolean;
			sustained?: {
				completedTicks: number;
				acceptedSubsteps: number;
				simulatedSeconds: number;
				wallSeconds: number;
				simulatedSecondsPerWallSecond: number;
				p50AdvanceMs: number;
				p95AdvanceMs: number;
				readbackBytes: number;
			};
		};
		error?: string;
	};
	try {
		const engine = await initializeEngine(480, 270, "wgpu");
		let freed = false;
		try {
			const backend = engine.backend();
			if (backend !== "BrowserWebGpu")
				throw new Error(`Unexpected browser GPU backend: ${backend}`);
			const abiJson: unknown = await engine.validate_gpu_abi();
			if (typeof abiJson !== "string") throw new Error("GPU ABI result is not a string");
			const abi = parseGpuAbiReport(abiJson, backend);
			const sceneJson: unknown = await engine.validate_gpu_scene_fixture();
			if (typeof sceneJson !== "string") throw new Error("GPU scene result is not a string");
			const scene = parseGpuSceneReport(sceneJson, backend);
			const pending: unknown = engine.validate_gpu_abi();
			if (!(pending instanceof Promise))
				throw new Error("GPU ABI validation did not return a Promise");
			engine.dispose();
			engine.dispose();
			freed = true;
			engine.free();
			const disposeDuringValidation = await settleValidationAfterDispose(
				pending,
				backend,
				"GPU ABI validation",
				parseGpuAbiReport,
			);
			const sceneEngine = await initializeEngine(480, 270, "wgpu");
			let sceneFreed = false;
			let sceneDisposeDuringValidation: "completed" | "rejected";
			try {
				const scenePending: unknown = sceneEngine.validate_gpu_scene_fixture();
				if (!(scenePending instanceof Promise))
					throw new Error("GPU scene validation did not return a Promise");
				sceneEngine.dispose();
				sceneEngine.dispose();
				sceneFreed = true;
				sceneEngine.free();
				sceneDisposeDuringValidation = await settleValidationAfterDispose(
					scenePending,
					backend,
					"GPU scene validation",
					parseGpuSceneReport,
				);
			} finally {
				if (!sceneFreed) sceneEngine.free();
			}
			const canvas = document.createElement("canvas");
			canvas.width = 960;
			canvas.height = 540;
			document.body.append(canvas);
			let canvasReport: NonNullable<typeof gpu.canvas>;
			const browserScene = await initializeBrowserGpuScene(canvas);
			try {
				if (browserScene.backend() !== backend) {
					throw new Error("Canvas scene selected the wrong browser GPU backend");
				}
				const initial = JSON.parse(browserScene.status_json()) as {
					epoch: number;
					tick: number;
					acceptedTimeS: number;
					busy: boolean;
				};
				if (
					initial.epoch !== 1 ||
					initial.tick !== 0 ||
					initial.acceptedTimeS !== 0 ||
					initial.busy
				) {
					throw new Error("Canvas scene initial committed stamp is wrong");
				}
				const initialRendered = browserScene.render();
				if (!initialRendered) throw new Error("Canvas scene did not acquire its first frame");
				for (const [x, y, expectedWall, expectedLiquid] of [
					[240, 248, false, true],
					[240, 100, false, false],
					[240, 255, true, false],
				] as const) {
					const sample = JSON.parse(await browserScene.probe(x, y)) as {
						x: number;
						y: number;
						liquidMassKg: number;
						carrierMassKg: number;
						fixedWall: boolean;
						epoch: number;
						stateRevision: number;
					};
					if (
						sample.x !== x ||
						sample.y !== y ||
						sample.fixedWall !== expectedWall ||
						sample.epoch !== 1 ||
						sample.stateRevision !== 0 ||
						(expectedLiquid ? sample.liquidMassKg < 0.000999 : sample.liquidMassKg !== 0) ||
						(expectedWall ? sample.carrierMassKg !== 0 : sample.carrierMassKg < 0)
					) {
						throw new Error("Canvas scene probe returned the wrong committed cell");
					}
				}
				const stale = browserScene.advance(1) as Promise<unknown>;
				const pendingReset = JSON.parse(browserScene.reset()) as {
					busy: boolean;
					resetPending: boolean;
				};
				if (!pendingReset.busy || !pendingReset.resetPending) {
					throw new Error("Canvas scene did not queue reset during advance");
				}
				let staleAdvanceRejected = false;
				try {
					await stale;
				} catch (error) {
					staleAdvanceRejected = String(error).includes("reset during advance");
				}
				if (!staleAdvanceRejected) throw new Error("Canvas scene published a stale advance");
				const reset = JSON.parse(browserScene.status_json()) as {
					epoch: number;
					tick: number;
					acceptedTimeS: number;
					busy: boolean;
				};
				if (reset.epoch !== 2 || reset.tick !== 0 || reset.acceptedTimeS !== 0 || reset.busy) {
					throw new Error("Canvas scene reset did not start a clean epoch");
				}
				if (!browserScene.render()) throw new Error("Canvas scene did not render after reset");
				for (const invalid of [0, -1, 1.5, 2 ** 32 + 1, Number.NaN, Number.POSITIVE_INFINITY]) {
					await rejects(
						async () => browserScene.advance(invalid),
						`adapter GPU advance ${invalid}`,
					);
					await rejects(
						async () => browserScene.resize(invalid, 540),
						`adapter GPU width ${invalid}`,
					);
					await rejects(
						async () => browserScene.resize(960, invalid),
						`adapter GPU height ${invalid}`,
					);
				}
				const tick = JSON.parse((await browserScene.advance(8)) as string) as {
					completedOuterTick: boolean;
					acceptedSubsteps: number;
					epoch: number;
					tick: number;
					acceptedTimeS: number;
					remainingOuterS: number;
					readbackBytes: number;
					scaledResidual: number;
					scaledDivergence: number;
				};
				if (
					!tick.completedOuterTick ||
					!Number.isSafeInteger(tick.acceptedSubsteps) ||
					tick.acceptedSubsteps < 1 ||
					tick.acceptedSubsteps > 8 ||
					tick.epoch !== 2 ||
					tick.tick !== 1 ||
					Math.abs(tick.acceptedTimeS - 1 / 60) > 1e-6 ||
					tick.remainingOuterS !== 0 ||
					!Number.isFinite(tick.scaledResidual) ||
					tick.scaledResidual >= 1e-5 ||
					!Number.isFinite(tick.scaledDivergence) ||
					tick.scaledDivergence >= 1e-5 ||
					!Number.isSafeInteger(tick.readbackBytes) ||
					tick.readbackBytes < 92
				) {
					throw new Error("Canvas scene did not accept a bounded full-size tick");
				}
				const settled = JSON.parse(browserScene.status_json()) as {
					epoch: number;
					tick: number;
					acceptedTimeS: number;
					busy: boolean;
				};
				if (
					settled.epoch !== tick.epoch ||
					settled.tick !== tick.tick ||
					settled.acceptedTimeS !== tick.acceptedTimeS ||
					settled.busy
				) {
					throw new Error("Canvas scene status disagreed with accepted tick");
				}
				if (!browserScene.render())
					throw new Error("Canvas scene did not render its accepted tick");
				const second = JSON.parse((await browserScene.advance(8)) as string) as typeof tick;
				if (
					!second.completedOuterTick ||
					second.acceptedSubsteps < 1 ||
					second.epoch !== 2 ||
					second.tick !== 2 ||
					Math.abs(second.acceptedTimeS - 2 / 60) > 1e-6 ||
					second.remainingOuterS !== 0 ||
					!Number.isFinite(second.scaledResidual) ||
					second.scaledResidual >= 1e-5 ||
					!Number.isFinite(second.scaledDivergence) ||
					second.scaledDivergence >= 1e-5 ||
					!Number.isSafeInteger(second.readbackBytes) ||
					second.readbackBytes < 92
				) {
					throw new Error("Canvas scene did not accept its second full-size tick");
				}
				if (!browserScene.render())
					throw new Error("Canvas scene did not render its second accepted tick");
				const checkpoint = await browserScene.checkpoint_prototype();
				const header = new DataView(
					checkpoint.buffer,
					checkpoint.byteOffset,
					checkpoint.byteLength,
				);
				const expectedReadbackBytes = 3_631_800;
				if (
					!(checkpoint instanceof Uint8Array) ||
					checkpoint.byteLength !== 108 + expectedReadbackBytes ||
					String.fromCharCode(...checkpoint.slice(0, 4)) !== "PFCP" ||
					header.getUint32(4, true) !== 0 ||
					header.getUint32(8, true) !== 480 ||
					header.getUint32(12, true) !== 270 ||
					header.getBigUint64(16, true) !== 2n ||
					header.getBigUint64(24, true) !== 2n ||
					header.getBigUint64(52, true) !== 2n ||
					header.getBigUint64(60, true) !== BigInt(expectedReadbackBytes)
				) {
					throw new Error("Explicit GPU checkpoint prototype has an invalid stamp or size");
				}
				let sustained: NonNullable<NonNullable<typeof gpu.canvas>["sustained"]> | undefined;
				if (new URL(location.href).searchParams.has("sustained-gpu")) {
					const started = performance.now();
					let completedTicks = 2;
					let acceptedSubsteps = 0;
					let readbackBytes = 0;
					let lastAcceptedTimeS = second.acceptedTimeS;
					const advanceMs: number[] = [];
					for (let attempt = 0; completedTicks < 60 && attempt < 1_000; attempt += 1) {
						const stepStarted = performance.now();
						let reply: string;
						try {
							reply = (await browserScene.advance(8)) as string;
						} catch (error) {
							throw new Error(
								`Sustained GPU advance after tick ${completedTicks} and ${lastAcceptedTimeS} s failed: ${String(error)}`,
							);
						}
						const next = JSON.parse(reply) as typeof tick;
						advanceMs.push(performance.now() - stepStarted);
						if (
							next.epoch !== 2 ||
							next.tick < completedTicks ||
							next.tick > completedTicks + 1 ||
							next.acceptedTimeS < lastAcceptedTimeS ||
							!Number.isSafeInteger(next.acceptedSubsteps) ||
							next.acceptedSubsteps < 1 ||
							next.acceptedSubsteps > 8 ||
							!Number.isSafeInteger(next.readbackBytes) ||
							next.readbackBytes < next.acceptedSubsteps * 92
						) {
							throw new Error("Sustained GPU advance returned invalid committed progress");
						}
						acceptedSubsteps += next.acceptedSubsteps;
						readbackBytes += next.readbackBytes;
						lastAcceptedTimeS = next.acceptedTimeS;
						if (next.completedOuterTick) {
							completedTicks += 1;
							if (
								next.tick !== completedTicks ||
								!Number.isFinite(next.scaledResidual) ||
								next.scaledResidual >= 1e-5 ||
								!Number.isFinite(next.scaledDivergence) ||
								next.scaledDivergence >= 1e-5 ||
								!browserScene.render()
							) {
								throw new Error(`Sustained GPU tick ${completedTicks} failed its gates or render`);
							}
						} else if (next.tick !== completedTicks || next.remainingOuterS <= 0) {
							throw new Error("Sustained GPU pause reported invalid remaining time");
						}
					}
					if (completedTicks !== 60 || Math.abs(lastAcceptedTimeS - 1) > 1e-6) {
						throw new Error("Sustained GPU run did not complete one simulated second");
					}
					const wallSeconds = (performance.now() - started) / 1_000;
					advanceMs.sort((left, right) => left - right);
					const percentile = (fraction: number) =>
						advanceMs[Math.ceil(fraction * advanceMs.length) - 1];
					sustained = {
						completedTicks,
						acceptedSubsteps,
						simulatedSeconds: lastAcceptedTimeS - second.acceptedTimeS,
						wallSeconds,
						simulatedSecondsPerWallSecond: (lastAcceptedTimeS - second.acceptedTimeS) / wallSeconds,
						p50AdvanceMs: percentile(0.5),
						p95AdvanceMs: percentile(0.95),
						readbackBytes,
					};
				}
				const beforePaint = JSON.parse(browserScene.status_json()) as {
					epoch: number;
					tick: number;
					acceptedTimeS: number;
					stateRevision: number;
				};
				const painted = JSON.parse(browserScene.paint(240, 100, 2, "water")) as typeof beforePaint;
				const paintedProbe = JSON.parse(await browserScene.probe(240, 100)) as {
					liquidMassKg: number;
					carrierMassKg: number;
					stateRevision: number;
				};
				const paintRevisionAdvanced =
					painted.epoch === beforePaint.epoch &&
					painted.tick === beforePaint.tick &&
					painted.acceptedTimeS === beforePaint.acceptedTimeS &&
					painted.stateRevision === beforePaint.stateRevision + 1 &&
					paintedProbe.stateRevision === painted.stateRevision &&
					paintedProbe.liquidMassKg > 0.000999 &&
					paintedProbe.carrierMassKg === 0 &&
					browserScene.render();
				if (!paintRevisionAdvanced)
					throw new Error("Canvas scene paint did not commit at zero time");
				const pendingProbe = browserScene.probe(240, 248);
				browserScene.reset();
				let staleProbeRejected = false;
				try {
					await pendingProbe;
				} catch (error) {
					staleProbeRejected = String(error).includes("reset during probe");
				}
				if (!staleProbeRejected) throw new Error("Canvas scene published a stale probe");
				const pendingCheckpoint = browserScene.checkpoint_prototype();
				browserScene.reset();
				let staleCheckpointRejected = false;
				try {
					await pendingCheckpoint;
				} catch (error) {
					staleCheckpointRejected = String(error).includes("reset during checkpoint");
				}
				if (!staleCheckpointRejected) throw new Error("Canvas scene published a stale checkpoint");
				canvasReport = {
					initialEpoch: initial.epoch,
					initialTick: initial.tick,
					initialRendered,
					resetEpoch: reset.epoch,
					staleAdvanceRejected,
					acceptedSubsteps: tick.acceptedSubsteps,
					acceptedTimeS: second.acceptedTimeS,
					scaledResidual: tick.scaledResidual,
					scaledDivergence: tick.scaledDivergence,
					secondScaledResidual: second.scaledResidual,
					secondScaledDivergence: second.scaledDivergence,
					probedCells: 3,
					checkpointBytes: checkpoint.byteLength,
					readbackBytesFirstTwo: tick.readbackBytes + second.readbackBytes,
					paintRevisionAdvanced,
					staleProbeRejected,
					staleCheckpointRejected,
					sustained,
				};
			} finally {
				browserScene.dispose();
				browserScene.free();
				canvas.remove();
			}
			const rawCanvas = document.createElement("canvas");
			rawCanvas.width = 960;
			rawCanvas.height = 540;
			document.body.append(rawCanvas);
			const rawScene = await create_browser_gpu_scene(rawCanvas);
			try {
				const beforeInvalid = rawScene.status_json();
				for (const invalid of [0, -1, 1.5, 2 ** 32 + 1, Number.NaN, Number.POSITIVE_INFINITY]) {
					await rejects(async () => rawScene.advance(invalid), `raw GPU advance ${invalid}`);
					await rejects(async () => rawScene.resize(invalid, 540), `raw GPU width ${invalid}`);
					await rejects(async () => rawScene.resize(960, invalid), `raw GPU height ${invalid}`);
				}
				if (rawScene.status_json() !== beforeInvalid) {
					throw new Error("Invalid raw GPU command changed committed status");
				}
			} finally {
				rawScene.dispose();
				rawScene.free();
				rawCanvas.remove();
			}
			gpu = {
				status: "pass",
				backend,
				abi,
				scene,
				disposeDuringValidation,
				sceneDisposeDuringValidation,
				canvas: canvasReport,
			};
		} finally {
			if (!freed) engine.free();
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

const reportToken = new URL(location.href).searchParams.get("report-token");
if (reportToken) {
	if (!/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(reportToken)) {
		throw new Error("Invalid browser smoke report token");
	}
	await fetch("http://127.0.0.1:4175/report", {
		method: "POST",
		body: JSON.stringify({ token: reportToken, report: result.textContent }),
	});
}
