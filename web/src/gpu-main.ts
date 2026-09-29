import { type BrowserGpuSceneSession, initializeBrowserGpuScene } from "./engine-client/wasm";

interface SceneStatus {
	epoch: number;
	tick: number;
	acceptedTimeS: number;
	stateRevision: number;
	busy: boolean;
	resetPending: boolean;
}

function requiredElement<T extends HTMLElement>(selector: string): T {
	const element = document.querySelector<T>(selector);
	if (!element) {
		throw new Error(`GPU preview element missing: ${selector}`);
	}
	return element;
}

const canvas = requiredElement<HTMLCanvasElement>("#gpuCanvas");
const pauseButton = requiredElement<HTMLButtonElement>("#gpuPause");
const resetButton = requiredElement<HTMLButtonElement>("#gpuReset");
const stateLight = requiredElement<HTMLSpanElement>("#gpuStateLight");
const statusText = requiredElement<HTMLSpanElement>("#gpuStatus");
const errorText = requiredElement<HTMLParagraphElement>("#gpuError");
const modelTime = requiredElement<HTMLOutputElement>("#gpuModelTime");
const tick = requiredElement<HTMLOutputElement>("#gpuTick");
const epoch = requiredElement<HTMLOutputElement>("#gpuEpoch");
const backend = requiredElement<HTMLOutputElement>("#gpuBackend");
const paintMaterial = requiredElement<HTMLSelectElement>("#gpuPaintMaterial");
const paintRadius = requiredElement<HTMLInputElement>("#gpuPaintRadius");
const paintRadiusValue = requiredElement<HTMLOutputElement>("#gpuPaintRadiusValue");
const paintX = requiredElement<HTMLInputElement>("#gpuPaintX");
const paintY = requiredElement<HTMLInputElement>("#gpuPaintY");
const paintAtButton = requiredElement<HTMLButtonElement>("#gpuPaintAt");
const paintMessage = requiredElement<HTMLParagraphElement>("#gpuPaintMessage");
const probeAtButton = requiredElement<HTMLButtonElement>("#gpuProbeAt");
const probeMessage = requiredElement<HTMLParagraphElement>("#gpuProbeMessage");

let scene: BrowserGpuSceneSession | undefined;
let progress: SceneStatus | undefined;
let running = true;
let activeStep = false;
let activeProbe = false;
let failed = false;
let disposed = false;
let revision = 0;
let pendingResetEpoch: number | undefined;
let scheduledFrame: number | undefined;

function parseStatus(value: string): SceneStatus {
	const parsed: unknown = JSON.parse(value);
	if (typeof parsed !== "object" || parsed === null) {
		throw new Error("GPU scene returned an invalid status");
	}
	const status = parsed as Record<string, unknown>;
	const { epoch, tick, acceptedTimeS, stateRevision, busy, resetPending } = status;
	if (
		typeof epoch !== "number" ||
		!Number.isSafeInteger(epoch) ||
		epoch < 1 ||
		typeof tick !== "number" ||
		!Number.isSafeInteger(tick) ||
		tick < 0 ||
		typeof acceptedTimeS !== "number" ||
		!Number.isFinite(acceptedTimeS) ||
		acceptedTimeS < 0 ||
		typeof stateRevision !== "number" ||
		!Number.isSafeInteger(stateRevision) ||
		stateRevision < 0 ||
		typeof busy !== "boolean" ||
		typeof resetPending !== "boolean"
	) {
		throw new Error("GPU scene returned an invalid status");
	}
	return { epoch, tick, acceptedTimeS, stateRevision, busy, resetPending };
}

function errorMessage(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

function showError(context: string, error: unknown): void {
	failed = true;
	running = false;
	const detail = errorMessage(error).trim();
	const period = /[.!?]$/.test(detail) ? "" : ".";
	const recovery = scene
		? "Reset the scene or reload this page to retry."
		: "Reload this page or return to the sandbox.";
	errorText.textContent = `${context} ${detail}${period} ${recovery}`;
	errorText.hidden = false;
	refreshView();
}

function clearError(): void {
	failed = false;
	errorText.textContent = "";
	errorText.hidden = true;
}

function refreshView(): void {
	pauseButton.disabled = !scene || disposed || failed || activeProbe;
	resetButton.disabled = !scene || disposed;
	paintAtButton.disabled = !scene || disposed || running || activeStep || activeProbe || failed;
	probeAtButton.disabled = !scene || disposed || running || activeStep || activeProbe || failed;
	canvas.dataset.paintable = String(
		Boolean(scene && !disposed && !running && !activeStep && !activeProbe && !failed),
	);
	pauseButton.textContent = running ? "Pause" : "Resume";
	if (progress) {
		modelTime.value = `${progress.acceptedTimeS.toFixed(3)} s`;
		tick.value = String(progress.tick);
		epoch.value = String(progress.epoch);
	}
	if (failed) {
		stateLight.dataset.state = "error";
		statusText.textContent = "GPU preview stopped";
	} else if (!scene) {
		stateLight.dataset.state = "loading";
		statusText.textContent = "Loading GPU scene";
	} else if (progress?.resetPending) {
		stateLight.dataset.state = "resetting";
		statusText.textContent = "Reset waiting for current GPU step";
	} else if (!running && activeStep) {
		stateLight.dataset.state = "paused";
		statusText.textContent = "Pausing after current GPU step";
	} else if (document.hidden) {
		stateLight.dataset.state = "paused";
		statusText.textContent = "Paused while tab is hidden";
	} else if (running) {
		stateLight.dataset.state = "running";
		statusText.textContent = "Simulating";
	} else {
		stateLight.dataset.state = "paused";
		statusText.textContent = "Paused";
	}
}

function paintCell(x: number, y: number): void {
	if (!scene || disposed) {
		return;
	}
	if (running || activeStep || activeProbe) {
		paintMessage.textContent = "Pause and wait for the current GPU step before painting.";
		return;
	}
	if (failed) {
		paintMessage.textContent = "Reset the scene before painting again.";
		return;
	}
	try {
		const material = paintMaterial.value;
		if (material !== "water" && material !== "air") {
			throw new RangeError("Select water or air");
		}
		const radius = Number(paintRadius.value);
		progress = parseStatus(scene.paint(x, y, radius, material));
		scene.render();
		paintMessage.textContent = `${material === "water" ? "Water" : "Air"} brush accepted at cell ${x}, ${y}. Fixed walls stay unchanged.`;
		refreshView();
	} catch (error) {
		paintMessage.textContent = `Paint command failed: ${errorMessage(error)}`;
	}
}

async function probeCell(x: number, y: number): Promise<void> {
	const currentScene = scene;
	if (!currentScene || disposed) return;
	if (running || activeStep || activeProbe || failed) {
		probeMessage.textContent = "Pause and wait for the current GPU operation before inspecting.";
		return;
	}
	activeProbe = true;
	probeMessage.textContent = "Reading one cell from the GPU…";
	refreshView();
	try {
		const sample = JSON.parse(await currentScene.probe(x, y)) as {
			x: number;
			y: number;
			liquidMassKg: number;
			carrierMassKg: number;
			fixedWall: boolean;
			epoch: number;
			stateRevision: number;
		};
		const settled = parseStatus(currentScene.status_json());
		progress = settled;
		if (sample.epoch !== settled.epoch || sample.stateRevision !== settled.stateRevision) {
			probeMessage.textContent = "Cell reading became stale. Inspect it again.";
		} else if (sample.fixedWall) {
			probeMessage.textContent = `Cell ${sample.x}, ${sample.y}: fixed wall.`;
		} else {
			probeMessage.textContent = `Cell ${sample.x}, ${sample.y}: water ${sample.liquidMassKg.toExponential(3)} kg; air ${sample.carrierMassKg.toExponential(3)} kg. Tick ${settled.tick}.`;
		}
	} catch (error) {
		probeMessage.textContent = String(error).includes("reset during probe")
			? "Cell reading canceled by reset."
			: `Cell reading failed: ${errorMessage(error)}`;
	} finally {
		activeProbe = false;
		if (!disposed && scene === currentScene) {
			const previousEpoch = progress?.epoch;
			progress = parseStatus(currentScene.status_json());
			if (progress.epoch !== previousEpoch) {
				pendingResetEpoch = undefined;
				currentScene.render();
			}
			refreshView();
		}
	}
}

function scheduleStep(): void {
	if (
		!scene ||
		!running ||
		failed ||
		disposed ||
		document.hidden ||
		activeStep ||
		activeProbe ||
		scheduledFrame !== undefined
	) {
		return;
	}
	scheduledFrame = requestAnimationFrame(() => {
		scheduledFrame = undefined;
		void advanceOnce();
	});
}

async function advanceOnce(): Promise<void> {
	const currentScene = scene;
	if (!currentScene || !running || failed || disposed || document.hidden || activeStep) {
		return;
	}
	activeStep = true;
	const stepRevision = revision;
	let stepError: unknown;
	let stepFailed = false;
	try {
		await currentScene.advance(8);
	} catch (error) {
		stepFailed = true;
		stepError = error;
	}
	activeStep = false;
	if (disposed) {
		return;
	}

	try {
		progress = parseStatus(currentScene.status_json());
		if (pendingResetEpoch !== undefined) {
			if (progress.resetPending || progress.epoch <= pendingResetEpoch) {
				throw new Error(`Scene reset did not complete. ${errorMessage(stepError)}`);
			}
			pendingResetEpoch = undefined;
		}
		if (stepFailed && stepRevision === revision) {
			showError("GPU step failed.", stepError);
			return;
		}
		currentScene.render();
		refreshView();
		scheduleStep();
	} catch (error) {
		showError("GPU preview failed.", error);
	}
}

pauseButton.addEventListener("click", () => {
	running = !running;
	if (running) {
		clearError();
	}
	refreshView();
	scheduleStep();
});

paintRadius.addEventListener("input", () => {
	paintRadiusValue.value = `${paintRadius.value} cells`;
});

canvas.addEventListener("pointerdown", (event) => {
	if (event.button !== 0) {
		return;
	}
	const bounds = canvas.getBoundingClientRect();
	if (bounds.width <= 0 || bounds.height <= 0) {
		return;
	}
	const x = Math.floor(((event.clientX - bounds.left) / bounds.width) * canvas.width);
	const y = Math.floor(((event.clientY - bounds.top) / bounds.height) * canvas.height);
	if (x < 0 || x >= canvas.width || y < 0 || y >= canvas.height) {
		return;
	}
	event.preventDefault();
	paintX.value = String(x);
	paintY.value = String(y);
	paintCell(x, y);
});

paintAtButton.addEventListener("click", () => {
	if (!paintX.reportValidity() || !paintY.reportValidity()) {
		return;
	}
	paintCell(Number(paintX.value), Number(paintY.value));
});

probeAtButton.addEventListener("click", () => {
	if (!paintX.reportValidity() || !paintY.reportValidity()) return;
	void probeCell(Number(paintX.value), Number(paintY.value));
});

document.addEventListener("visibilitychange", () => {
	if (!document.hidden) {
		refreshView();
		scheduleStep();
		return;
	}
	running = false;
	if (scheduledFrame !== undefined) {
		cancelAnimationFrame(scheduledFrame);
		scheduledFrame = undefined;
	}
	refreshView();
});

resetButton.addEventListener("click", () => {
	if (!scene) {
		return;
	}
	revision += 1;
	const previousEpoch = progress?.epoch;
	clearError();
	try {
		progress = parseStatus(scene.reset());
		pendingResetEpoch = progress.resetPending ? previousEpoch : undefined;
		if (!progress.resetPending) {
			scene.render();
		}
		paintMessage.textContent = "Pause before painting.";
		refreshView();
		scheduleStep();
	} catch (error) {
		showError("GPU reset failed.", error);
	}
});

window.addEventListener("pagehide", () => {
	if (disposed) return;
	disposed = true;
	if (scheduledFrame !== undefined) {
		cancelAnimationFrame(scheduledFrame);
	}
	scene?.dispose();
	scene?.free();
});

window.addEventListener("pageshow", (event) => {
	if (event.persisted) location.reload();
});

async function start(): Promise<void> {
	try {
		const initialized = await initializeBrowserGpuScene(canvas);
		if (disposed) {
			initialized.dispose();
			initialized.free();
			return;
		}
		scene = initialized;
		backend.value = scene.backend();
		progress = parseStatus(scene.status_json());
		if (document.hidden) running = false;
		scene.render();
		refreshView();
		scheduleStep();
	} catch (error) {
		showError("Could not start the GPU preview.", error);
	}
}

void start();
