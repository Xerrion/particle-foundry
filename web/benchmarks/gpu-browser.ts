import { type BrowserGpuSceneSession, initializeBrowserGpuScene } from "../src/engine-client/wasm";

const CELLS = 480 * 270;
const FIELD_BYTES = (5 * CELLS + 481 * 270 + 480 * 271) * 4;
const INVENTORY_DRIFT_LIMIT = 1e-5;
type Advance = {
	epoch: number;
	tick: number;
	acceptedTimeS: number;
	stateRevision: number;
	completedOuterTick: boolean;
	acceptedSubsteps: number;
	attemptedCandidates: number;
	refinementRetries: number;
	readbackBytes: number;
	rejectedReadbackBytes: number;
	pressureIterations: number | null;
	scaledResidual: number | null;
	scaledDivergence: number | null;
};
type AdapterInfo = {
	vendor: string;
	architecture: string;
	description: string;
	isFallbackAdapter: boolean;
};
type ComputePass = {
	label: string;
	dispatchWorkgroups(x: number, y?: number, z?: number): void;
};

function inventory(bytes: Uint8Array, expected: Advance) {
	const fields = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	if (
		bytes.byteLength !== 108 + FIELD_BYTES ||
		String.fromCharCode(...bytes.slice(0, 4)) !== "PFCP" ||
		fields.getUint32(4, true) !== 0 ||
		fields.getUint32(8, true) !== 480 ||
		fields.getUint32(12, true) !== 270 ||
		fields.getBigUint64(16, true) !== BigInt(expected.epoch) ||
		fields.getBigUint64(24, true) !== BigInt(expected.tick) ||
		fields.getFloat64(32, true) !== expected.acceptedTimeS ||
		fields.getFloat64(40, true) !== 0 ||
		fields.getBigUint64(52, true) !== BigInt(expected.stateRevision) ||
		fields.getBigUint64(60, true) !== BigInt(FIELD_BYTES)
	) {
		throw new Error("GPU benchmark checkpoint has an invalid layout or committed stamp");
	}
	const totals = [0, 0, 0, 0];
	let maxRelativeVolumeError = 0;
	let maxAbsoluteVelocityMPerS = 0;
	for (let cell = 0; cell < CELLS; cell += 1) {
		const values = totals.map((_, field) =>
			fields.getFloat32(108 + 4 * (field * CELLS + cell), true),
		);
		if (
			values.some((value) => !Number.isFinite(value) || value < 0) ||
			(values[0] === 0 && values[2] !== 0) ||
			(values[1] === 0 && values[3] !== 0)
		) {
			throw new Error(`GPU benchmark has invalid inventory in cell ${cell}`);
		}
		for (let field = 0; field < 4; field += 1) totals[field] += values[field];
		const density = fields.getFloat32(108 + 4 * (4 * CELLS + cell), true);
		if (!Number.isFinite(density) || density <= 0) {
			throw new Error(`GPU benchmark has invalid density in cell ${cell}`);
		}
		if (values[0] + values[1] > 0) {
			const volume = values[0] / 1000 + values[1] / 1.2;
			maxRelativeVolumeError = Math.max(maxRelativeVolumeError, Math.abs(volume / 1e-6 - 1));
		}
	}
	for (let offset = 108 + 4 * 5 * CELLS; offset < bytes.byteLength; offset += 4) {
		const velocity = fields.getFloat32(offset, true);
		if (!Number.isFinite(velocity)) throw new Error("GPU benchmark has nonfinite face velocity");
		maxAbsoluteVelocityMPerS = Math.max(maxAbsoluteVelocityMPerS, Math.abs(velocity));
	}
	// The f64 diagnostic division also exposes the f32 cell-volume rounding.
	if (maxRelativeVolumeError > 1.01e-5)
		throw new Error("GPU benchmark cell volume exceeds its gate");
	return { totals, maxRelativeVolumeError, maxAbsoluteVelocityMPerS };
}

async function measure(scene: BrowserGpuSceneSession, ticks: number) {
	scene.reset();
	const painted = JSON.parse(scene.paint(240, 100, 4, "water")) as Advance;
	if (painted.tick !== 0 || painted.acceptedTimeS !== 0 || painted.stateRevision !== 1) {
		throw new Error("GPU benchmark brush did not commit at zero model time");
	}
	if (!scene.render()) throw new Error("GPU benchmark cannot render the painted scene");
	const before = inventory(await scene.checkpoint_prototype(), painted);
	const run = {
		completedTicks: 0,
		acceptedTimeS: 0,
		acceptedSubsteps: 0,
		attemptedCandidates: 0,
		refinementRetries: 0,
		readbackBytes: 0,
		rejectedReadbackBytes: 0,
		renderedFrames: 0,
		maxPressureIterations: 0,
		maxScaledResidual: 0,
		maxScaledDivergence: 0,
	};
	let last = painted;
	const started = performance.now();
	for (let calls = 0; run.completedTicks < ticks && calls < ticks * 64; calls += 1) {
		const next = JSON.parse(await scene.advance(8)) as Advance;
		if (
			next.epoch !== painted.epoch ||
			!Number.isFinite(next.acceptedTimeS) ||
			next.acceptedTimeS <= last.acceptedTimeS ||
			next.tick !== last.tick + Number(next.completedOuterTick) ||
			next.attemptedCandidates !== next.acceptedSubsteps + next.refinementRetries
		) {
			throw new Error("GPU benchmark advance has invalid accepted-time or attempt accounting");
		}
		for (const key of [
			"acceptedSubsteps",
			"attemptedCandidates",
			"refinementRetries",
			"readbackBytes",
			"rejectedReadbackBytes",
		] as const) {
			if (!Number.isSafeInteger(next[key]) || next[key] < 0) {
				throw new Error(`GPU benchmark has invalid ${key}`);
			}
			run[key] += next[key];
		}
		if (next.completedOuterTick) {
			if (
				next.pressureIterations === null ||
				next.scaledResidual === null ||
				next.scaledDivergence === null ||
				!Number.isSafeInteger(next.pressureIterations) ||
				next.pressureIterations < 0 ||
				next.pressureIterations > 512 ||
				![next.scaledResidual, next.scaledDivergence].every(
					(value) => Number.isFinite(value) && value >= 0 && value <= 1e-5,
				)
			) {
				throw new Error("GPU benchmark completed tick exceeds reference pressure gates");
			}
			run.maxPressureIterations = Math.max(run.maxPressureIterations, next.pressureIterations);
			run.maxScaledResidual = Math.max(run.maxScaledResidual, next.scaledResidual);
			run.maxScaledDivergence = Math.max(run.maxScaledDivergence, next.scaledDivergence);
		}
		if (!scene.render()) throw new Error("GPU benchmark cannot render committed progress");
		run.renderedFrames += 1;
		run.completedTicks = next.tick;
		run.acceptedTimeS = next.acceptedTimeS;
		last = next;
	}
	// This bounded 16-byte probe waits for the last submitted render on the same queue.
	await scene.probe(240, 100);
	const wallSeconds = (performance.now() - started) / 1000;
	if (run.completedTicks !== ticks || Math.abs(run.acceptedTimeS - ticks / 60) > 1e-6) {
		throw new Error("GPU benchmark did not complete the requested model time");
	}
	const after = inventory(await scene.checkpoint_prototype(), last);
	const relativeInventoryDrift = after.totals.map(
		(total, field) =>
			Math.abs(total - before.totals[field]) / Math.max(before.totals[field], 1e-12),
	);
	if (relativeInventoryDrift.some((drift) => drift > INVENTORY_DRIFT_LIMIT)) {
		throw new Error("GPU benchmark inventory drift exceeds its gate");
	}
	return {
		...run,
		wallSeconds,
		modelSecondsPerWallSecond: run.acceptedTimeS / wallSeconds,
		finalQueueProbeReadbackBytes: 16,
		explicitCheckpointReadbackBytes: FIELD_BYTES * 2,
		before,
		after,
		relativeInventoryDrift,
	};
}

async function benchmark(canvas: HTMLCanvasElement) {
	const gpu = navigator.gpu;
	if (!gpu) throw new Error("GPU benchmark requires WebGPU");
	const adapterRequests: { options: unknown; info: AdapterInfo | null }[] = [];
	const originalRequest = gpu.requestAdapter;
	gpu.requestAdapter = async (options) => {
		const adapter = await originalRequest.call(gpu, options);
		adapterRequests.push({
			options,
			info: adapter
				? {
						vendor: adapter.info.vendor,
						architecture: adapter.info.architecture,
						description: adapter.info.description,
						isFallbackAdapter: adapter.info.isFallbackAdapter,
					}
				: null,
		});
		return adapter;
	};
	let scene: BrowserGpuSceneSession;
	try {
		scene = await initializeBrowserGpuScene(canvas);
	} finally {
		gpu.requestAdapter = originalRequest;
	}
	try {
		const selectedAdapter = adapterRequests.at(-1)?.info;
		if (selectedAdapter?.isFallbackAdapter !== false) {
			throw new Error("GPU benchmark requires an identified hardware adapter");
		}
		const warmup = await measure(scene, 10);
		const runs = [];
		for (let run = 0; run < 3; run += 1) runs.push(await measure(scene, 60));
		const prototype = (
			globalThis as typeof globalThis & { GPUComputePassEncoder?: { prototype: ComputePass } }
		).GPUComputePassEncoder?.prototype;
		if (!prototype) throw new Error("GPU benchmark dispatch tracing is unavailable");
		const dispatches: Record<string, number> = {};
		const originalDispatch = prototype.dispatchWorkgroups;
		prototype.dispatchWorkgroups = function (x, y, z) {
			dispatches[this.label] = (dispatches[this.label] ?? 0) + 1;
			originalDispatch.call(this, x, y, z);
		};
		let tracedRun: Awaited<ReturnType<typeof measure>>;
		try {
			tracedRun = await measure(scene, 60);
		} finally {
			prototype.dispatchWorkgroups = originalDispatch;
		}
		const sustainedRun = await measure(scene, 300);
		return {
			status: "pass",
			targetMet:
				runs.every((run) => run.modelSecondsPerWallSecond >= 1) &&
				sustainedRun.modelSecondsPerWallSecond >= 1,
			fixture: { width: 480, height: 270, brush: { x: 240, y: 100, radius: 4, material: "water" } },
			quality: {
				pressureIterations: 512,
				pressureBatchIterations: 64,
				pressureTolerance: 1e-5,
				closureRounds: 256,
			},
			measurement:
				"10-tick warm-up; fresh scene per run; render per advance; final queue probe included; checkpoints excluded",
			pressureObservationScope: "last accepted substep of each completed tick",
			adapter: JSON.parse(scene.adapter_info_json()) as unknown,
			adapterRequests,
			warmup,
			runs,
			sustainedRun,
			trace: {
				measurement: "separate instrumented run; duration is excluded from targetMet",
				dispatches,
				run: tracedRun,
			},
			userAgent: navigator.userAgent,
		};
	} finally {
		scene.dispose();
		scene.free();
	}
}

const result = document.getElementById("result");
const canvas = document.getElementById("scene");
if (!result || !(canvas instanceof HTMLCanvasElement))
	throw new Error("GPU benchmark DOM is missing");
try {
	result.textContent = JSON.stringify(await benchmark(canvas));
} catch (error) {
	result.textContent = JSON.stringify({ status: "fail", error: String(error) });
}
const token = new URL(location.href).searchParams.get("report-token");
if (token) {
	if (!/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(token)) {
		throw new Error("Invalid GPU benchmark report token");
	}
	await fetch("http://127.0.0.1:4175/report", {
		method: "POST",
		body: JSON.stringify({ token, report: result.textContent }),
	});
}
