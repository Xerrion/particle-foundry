import { bindControls } from "./app/controls";
import { requiredElement } from "./app/dom";
import { createSimulationClock } from "./app/simulation-clock";
import { createViewport } from "./app/viewport";
import { createSandbox, selectCanvasContext } from "./engine-client";

const canvas = requiredElement<HTMLCanvasElement>("#sandbox");
const ctx = selectCanvasContext(canvas, "2d");
const fpsCounter = requiredElement<HTMLElement>("#fpsCounter");

const WORLD_WIDTH = 480;
const WORLD_HEIGHT = 270;
const worldCanvas = document.createElement("canvas");
worldCanvas.width = WORLD_WIDTH;
worldCanvas.height = WORLD_HEIGHT;
const worldCtx = selectCanvasContext(worldCanvas, "2d");
const sandbox = createSandbox(WORLD_WIDTH, WORLD_HEIGHT);
const viewport = createViewport(WORLD_WIDTH, WORLD_HEIGHT);
// Start close to the experiments; the fit button reveals the full workspace.
viewport.zoomAt(2, 0.5, 1);
const controls = bindControls(canvas, sandbox, viewport);
const clock = createSimulationClock();
let lastTimestamp: number | undefined;
let frameCount = 0;
let fpsSampleStarted: number | undefined;
let estimatedStepMs = 8;
const PHYSICS_BUDGET_MS = 24;
let sampledPhysicsMs = 0;
let sampledRenderMs = 0;
let sampledSteps = 0;

function render(): void {
	sandbox.render(worldCtx, controls.getPointer(), viewport);
	ctx.imageSmoothingEnabled = false;
	ctx.drawImage(
		worldCanvas,
		viewport.left,
		viewport.top,
		viewport.width,
		viewport.height,
		0,
		0,
		canvas.width,
		canvas.height,
	);
}

function animationLoop(timestamp: number): void {
	if (!Number.isFinite(timestamp) || timestamp < 0) {
		throw new RangeError("Animation timestamp must be finite and nonnegative");
	}
	const steps = clock.advance(lastTimestamp === undefined ? 0 : timestamp - lastTimestamp, {
		speed: controls.getSpeed(),
		isPaused: document.hidden || controls.isPaused(),
	});
	lastTimestamp = timestamp;

	// Drop surplus catch-up steps when the model grows expensive, while still
	// allowing faster simulation speeds in sparse worlds.
	const stepLimit = Math.max(1, Math.floor(PHYSICS_BUDGET_MS / estimatedStepMs));
	const stepsToRun = Math.min(steps, stepLimit);
	const stepStarted = performance.now();
	for (let step = 0; step < stepsToRun; step += 1) {
		controls.paintHeldTool();
		sandbox.step();
	}
	const physicsElapsed = performance.now() - stepStarted;
	if (stepsToRun > 0) {
		sampledPhysicsMs += physicsElapsed;
		sampledSteps += stepsToRun;
		const measured = physicsElapsed / stepsToRun;
		estimatedStepMs = Math.max(0.1, estimatedStepMs * 0.75 + measured * 0.25);
	}

	const renderStarted = performance.now();
	render();
	sampledRenderMs += performance.now() - renderStarted;
	controls.updateProbe();
	fpsSampleStarted ??= timestamp;
	frameCount += 1;
	if (timestamp - fpsSampleStarted >= 500) {
		const fps = Math.round((frameCount * 1000) / (timestamp - fpsSampleStarted));
		const sampledFrames = frameCount;
		frameCount = 0;
		fpsSampleStarted = timestamp;
		controls.updateStats(sandbox.getParticleCount(), fps, sandbox.getDiagnostics());
		fpsCounter.title = `${(sampledPhysicsMs / Math.max(1, sampledSteps)).toFixed(1)} ms/physics step · ${(sampledRenderMs / sampledFrames).toFixed(1)} ms/render`;
		sampledPhysicsMs = 0;
		sampledRenderMs = 0;
		sampledSteps = 0;
	}

	requestAnimationFrame(animationLoop);
}

document.addEventListener("visibilitychange", () => {
	clock.reset();
	lastTimestamp = undefined;
	frameCount = 0;
	fpsSampleStarted = undefined;
	sampledPhysicsMs = 0;
	sampledRenderMs = 0;
	sampledSteps = 0;
});

sandbox.seed();
render();
controls.updateProbe();
requestAnimationFrame(animationLoop);
