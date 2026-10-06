import type { BrowserGpuSceneSession } from "../../src/engine-client/wasm";

type Inventory = {
	liquidMassKg: number;
	carrierMassKg: number;
	liquidMarker: number;
	carrierMarker: number;
};
type Stamp = {
	epoch: number;
	tick: number;
	acceptedTimeS: number;
	remainingOuterS: number;
	stateRevision: number;
	busy: boolean;
};
type Advance = Stamp & {
	completedOuterTick: boolean;
	acceptedSubsteps: number;
	attemptedCandidates: number;
	refinementRetries: number;
	readbackBytes: number;
	rejectedReadbackBytes: number;
	scaledResidual: number | null;
	scaledDivergence: number | null;
	pressureIterations: number | null;
	encodedTimeErrorS: number | null;
};
type Checks = {
	inventory(checkpoint: Uint8Array, stamp: Stamp): Inventory;
	kineticEnergy(checkpoint: Uint8Array): number;
	attemptAccounting(report: Advance, label: string): void;
	pressureGates(
		report: Advance,
		label: string,
	): asserts report is Advance & {
		scaledResidual: number;
		scaledDivergence: number;
		pressureIterations: number;
	};
};
type Frame = { pixels: Uint8ClampedArray; pngBytes: number };

export type InteractiveGpuReport = {
	initialCondition: string;
	paintedBrush: { x: number; y: number; radius: number; material: string };
	epoch: number;
	completedTicks: number;
	acceptedTimeS: number;
	acceptedSubsteps: number;
	attemptedCandidates: number;
	refinementRetries: number;
	readbackBytes: number;
	rejectedReadbackBytes: number;
	pressureIterationBudget: number;
	pressureObservationScope: string;
	maxPressureIterations: number;
	maxScaledResidual: number;
	maxScaledDivergence: number;
	beforeTotals: Inventory;
	afterTotals: Inventory;
	relativeInventoryDriftLimit: number;
	maxRelativeInventoryDrift: number;
	checkpointTicks: number[];
	kineticEnergySamplesJ: number[];
	maxMeasuredKineticEnergyJ: number;
	maxLiquidInventoryChangeKg: number;
	explicitCheckpointReadbackBytes: number;
	originalPoolResponse: {
		firstWorldRow: number;
		lastWorldRow: number;
		captureTicks: number[];
		changedPixels: number[];
		scope: string;
	};
	visibleResponse: {
		captureMethod: string;
		scope: string;
		paintChangedPixels: number;
		progressChangedPixels: number[];
		captureTicks: number[];
		pngBytes: number[];
	};
};

async function captureFrame(
	scene: BrowserGpuSceneSession,
	canvas: HTMLCanvasElement,
): Promise<Frame> {
	// Capture in the same presentation callback. WebGPU clears its drawing buffer after presentation.
	const png = await new Promise<string>((resolve, reject) => {
		requestAnimationFrame(() => {
			try {
				if (!scene.render())
					throw new Error("Interactive GPU scene did not acquire a capture frame");
				resolve(canvas.toDataURL("image/png"));
			} catch (error) {
				reject(error);
			}
		});
	});
	if (!png.startsWith("data:image/png;base64,")) {
		throw new Error("Interactive GPU canvas did not return a PNG snapshot");
	}
	const image = new Image();
	image.src = png;
	await image.decode();
	const snapshot = document.createElement("canvas");
	snapshot.width = canvas.width;
	snapshot.height = canvas.height;
	const context = snapshot.getContext("2d");
	if (!context) throw new Error("Interactive GPU snapshot context is unavailable");
	context.drawImage(image, 0, 0);
	return {
		pixels: context.getImageData(0, 0, snapshot.width, snapshot.height).data,
		pngBytes: atob(png.slice(png.indexOf(",") + 1)).length,
	};
}

function changedPixels(
	before: Frame,
	after: Frame,
	firstByte = 0,
	lastByte = before.pixels.length,
): number {
	if (before.pixels.length !== after.pixels.length) {
		throw new Error("Interactive GPU snapshots have different dimensions");
	}
	let changed = 0;
	for (let offset = firstByte; offset < lastByte; offset += 4) {
		if (
			[0, 1, 2, 3].some(
				(channel) => before.pixels[offset + channel] !== after.pixels[offset + channel],
			)
		) {
			changed += 1;
		}
	}
	return changed;
}

function liquidInventoryChange(before: Uint8Array, after: Uint8Array): number {
	const initial = new DataView(before.buffer, before.byteOffset, before.byteLength);
	const current = new DataView(after.buffer, after.byteOffset, after.byteLength);
	let changeKg = 0;
	for (let cell = 0; cell < 480 * 270; cell += 1) {
		changeKg += Math.abs(
			current.getFloat32(108 + cell * 4, true) - initial.getFloat32(108 + cell * 4, true),
		);
	}
	return changeKg;
}

/** Qualifies the normal Water brush separately from the hydrostatic-rest fixture. */
export async function qualifyInteractiveGpu(
	scene: BrowserGpuSceneSession,
	canvas: HTMLCanvasElement,
	checks: Checks,
): Promise<InteractiveGpuReport> {
	const previous = JSON.parse(scene.status_json()) as Stamp;
	const fresh = JSON.parse(scene.reset()) as Stamp;
	if (
		fresh.epoch !== previous.epoch + 1 ||
		fresh.tick !== 0 ||
		fresh.acceptedTimeS !== 0 ||
		fresh.remainingOuterS !== 0 ||
		fresh.stateRevision !== 0 ||
		fresh.busy
	) {
		throw new Error("Interactive GPU fixture did not reset to a fresh rest seed");
	}
	const restFrame = await captureFrame(scene, canvas);
	const painted = JSON.parse(scene.paint(240, 100, 4, "water")) as Stamp;
	if (
		painted.epoch !== fresh.epoch ||
		painted.tick !== 0 ||
		painted.acceptedTimeS !== 0 ||
		painted.remainingOuterS !== 0 ||
		painted.stateRevision !== 1 ||
		painted.busy
	) {
		throw new Error("Default Water brush did not commit at zero model time");
	}
	const initial = await scene.checkpoint_prototype();
	const beforeTotals = checks.inventory(initial, painted);
	if (checks.kineticEnergy(initial) !== 0) {
		throw new Error("Default Water brush changed velocity before model time advanced");
	}
	const paintedFrame = await captureFrame(scene, canvas);
	const paintChangedPixels = changedPixels(restFrame, paintedFrame);
	if (paintChangedPixels === 0)
		throw new Error("Default Water brush has no visible canvas response");
	const report: InteractiveGpuReport = {
		initialCondition: "fresh hydrostatic rest seed with a normal Water brush above the pool",
		paintedBrush: { x: 240, y: 100, radius: 4, material: "water" },
		epoch: fresh.epoch,
		completedTicks: 0,
		acceptedTimeS: 0,
		acceptedSubsteps: 0,
		attemptedCandidates: 0,
		refinementRetries: 0,
		readbackBytes: 0,
		rejectedReadbackBytes: 0,
		pressureIterationBudget: 512,
		pressureObservationScope: "last accepted substep of each completed tick",
		maxPressureIterations: 0,
		maxScaledResidual: 0,
		maxScaledDivergence: 0,
		beforeTotals,
		afterTotals: beforeTotals,
		relativeInventoryDriftLimit: 1e-5,
		maxRelativeInventoryDrift: 0,
		checkpointTicks: [],
		kineticEnergySamplesJ: [],
		maxMeasuredKineticEnergyJ: 0,
		maxLiquidInventoryChangeKg: 0,
		explicitCheckpointReadbackBytes: initial.byteLength - 108,
		originalPoolResponse: {
			firstWorldRow: 242,
			lastWorldRow: 254,
			captureTicks: [],
			changedPixels: [],
			scope:
				"pixel differences in original pool rows 242 through 254 prove visible response for this fixture, not a minimum wave amplitude",
		},
		visibleResponse: {
			captureMethod: "GPU canvas PNG in a presentation callback, decoded to RGBA pixels",
			scope: "full canvas pixel differences prove response to paint and committed progress",
			paintChangedPixels,
			progressChangedPixels: [],
			captureTicks: [0],
			pngBytes: [paintedFrame.pngBytes],
		},
	};
	let lastAcceptedTimeS = 0;
	for (let calls = 0; report.completedTicks < 300 && calls < 300 * 16; calls += 1) {
		let next: Advance;
		try {
			next = JSON.parse(await scene.advance(8)) as Advance;
		} catch (error) {
			throw new Error(
				`Default Water brush failed after tick ${report.completedTicks} at ${lastAcceptedTimeS} s: ${String(error)}`,
			);
		}
		checks.attemptAccounting(next, "Default Water brush GPU advance");
		if (
			next.epoch !== fresh.epoch ||
			!Number.isSafeInteger(next.tick) ||
			next.tick < report.completedTicks ||
			next.tick > report.completedTicks + 1 ||
			typeof next.completedOuterTick !== "boolean" ||
			!Number.isFinite(next.acceptedTimeS) ||
			next.acceptedTimeS < lastAcceptedTimeS ||
			!Number.isFinite(next.remainingOuterS) ||
			next.remainingOuterS < 0 ||
			next.remainingOuterS > 1 / 60
		) {
			throw new Error("Default Water brush returned invalid committed progress");
		}
		report.acceptedSubsteps += next.acceptedSubsteps;
		report.attemptedCandidates += next.attemptedCandidates;
		report.refinementRetries += next.refinementRetries;
		report.readbackBytes += next.readbackBytes;
		report.rejectedReadbackBytes += next.rejectedReadbackBytes;
		lastAcceptedTimeS = next.acceptedTimeS;
		if (!next.completedOuterTick) {
			if (
				next.tick !== report.completedTicks ||
				next.remainingOuterS <= 0 ||
				next.scaledResidual !== null ||
				next.scaledDivergence !== null ||
				next.pressureIterations !== null ||
				Math.abs(next.acceptedTimeS + next.remainingOuterS - (next.tick + 1) / 60) > 1e-6
			) {
				throw new Error("Default Water brush pause reported invalid outer-tick accounting");
			}
			continue;
		}
		checks.pressureGates(next, `Default Water brush tick ${next.tick}`);
		report.completedTicks += 1;
		if (
			next.tick !== report.completedTicks ||
			Math.abs(next.acceptedTimeS - next.tick / 60) > 1e-6 ||
			next.remainingOuterS !== 0 ||
			typeof next.encodedTimeErrorS !== "number" ||
			!Number.isFinite(next.encodedTimeErrorS) ||
			Math.abs(next.encodedTimeErrorS) > 1e-6 ||
			!scene.render()
		) {
			throw new Error(`Default Water brush failed its clock or render gate at tick ${next.tick}`);
		}
		report.acceptedTimeS = next.acceptedTimeS;
		report.maxPressureIterations = Math.max(report.maxPressureIterations, next.pressureIterations);
		report.maxScaledResidual = Math.max(report.maxScaledResidual, next.scaledResidual);
		report.maxScaledDivergence = Math.max(report.maxScaledDivergence, next.scaledDivergence);
		if (![1, 50, 125, 200, 300].includes(next.tick)) continue;
		const status = JSON.parse(scene.status_json()) as Stamp;
		if (
			status.epoch !== fresh.epoch ||
			status.tick !== next.tick ||
			status.busy ||
			status.acceptedTimeS !== next.acceptedTimeS ||
			status.stateRevision !== 1 + report.acceptedSubsteps
		) {
			throw new Error("Default Water brush checkpoint status disagrees with accepted progress");
		}
		const checkpoint = await scene.checkpoint_prototype();
		const totals = checks.inventory(checkpoint, status);
		const energy = checks.kineticEnergy(checkpoint);
		if (!Number.isFinite(energy) || energy < 0)
			throw new Error("Default Water brush has invalid kinetic energy");
		report.maxMeasuredKineticEnergyJ = Math.max(report.maxMeasuredKineticEnergyJ, energy);
		report.kineticEnergySamplesJ.push(energy);
		report.maxLiquidInventoryChangeKg = Math.max(
			report.maxLiquidInventoryChangeKg,
			liquidInventoryChange(initial, checkpoint),
		);
		for (const key of Object.keys(beforeTotals) as (keyof Inventory)[]) {
			const drift = Math.abs(totals[key] - beforeTotals[key]);
			if (beforeTotals[key] === 0 && drift !== 0)
				throw new Error(`Default Water brush created ${key}`);
			report.maxRelativeInventoryDrift = Math.max(
				report.maxRelativeInventoryDrift,
				beforeTotals[key] === 0 ? 0 : drift / beforeTotals[key],
			);
		}
		report.afterTotals = totals;
		report.checkpointTicks.push(next.tick);
		report.explicitCheckpointReadbackBytes += checkpoint.byteLength - 108;
		if (next.tick !== 50 && next.tick !== 300) continue;
		const progressedFrame = await captureFrame(scene, canvas);
		report.visibleResponse.progressChangedPixels.push(changedPixels(paintedFrame, progressedFrame));
		// BrowserGpuScene renders the full 480 x 270 world with a top-left origin.
		const band = report.originalPoolResponse;
		const firstByte = Math.floor((canvas.height * band.firstWorldRow) / 270) * canvas.width * 4;
		const lastByte = Math.ceil((canvas.height * (band.lastWorldRow + 1)) / 270) * canvas.width * 4;
		report.originalPoolResponse.changedPixels.push(
			changedPixels(paintedFrame, progressedFrame, firstByte, lastByte),
		);
		report.originalPoolResponse.captureTicks.push(next.tick);
		report.visibleResponse.captureTicks.push(next.tick);
		report.visibleResponse.pngBytes.push(progressedFrame.pngBytes);
	}
	if (
		report.completedTicks !== 300 ||
		report.checkpointTicks.length !== 5 ||
		report.maxRelativeInventoryDrift > report.relativeInventoryDriftLimit ||
		report.maxMeasuredKineticEnergyJ <= 1e-12 ||
		report.maxLiquidInventoryChangeKg <= 1e-7 ||
		report.visibleResponse.progressChangedPixels.length !== 2 ||
		report.visibleResponse.progressChangedPixels.some((changed) => changed === 0) ||
		report.originalPoolResponse.changedPixels.length !== 2 ||
		!report.originalPoolResponse.changedPixels.some((changed) => changed > 0)
	) {
		throw new Error(
			`Default Water brush did not complete 300 physical and visible ticks: ${JSON.stringify(report)}`,
		);
	}
	return report;
}
