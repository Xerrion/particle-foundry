import { bindControls } from "./controls";
import { createSandbox } from "./sandbox";
import { createSimulationClock } from "./simulation-clock";

function requiredElement<T extends Element>(selector: string): T {
	const element = document.querySelector<T>(selector);
	if (!element) throw new Error(`Required element not found: ${selector}`);
	return element;
}

const canvas = requiredElement<HTMLCanvasElement>("#sandbox");
const context = canvas.getContext("2d", { alpha: false });
if (!context) throw new Error("Canvas 2D context is unavailable");
const ctx: CanvasRenderingContext2D = context;

const sandbox = createSandbox(canvas.width, canvas.height);
const controls = bindControls(canvas, sandbox);
const clock = createSimulationClock();
let lastTimestamp: number | undefined;
let frameCount = 0;
let fpsSampleStarted: number | undefined;

function animationLoop(timestamp: number): void {
	if (!Number.isFinite(timestamp) || timestamp < 0) {
		throw new RangeError("Animation timestamp must be finite and nonnegative");
	}
	const steps = clock.advance(lastTimestamp === undefined ? 0 : timestamp - lastTimestamp, {
		speed: controls.getSpeed(),
		isPaused: document.hidden || controls.isPaused(),
	});
	lastTimestamp = timestamp;

	for (let step = 0; step < steps; step += 1) {
		controls.paintHeldTool();
		sandbox.step();
	}

	sandbox.render(ctx, controls.getPointer());
	controls.updateProbe();
	fpsSampleStarted ??= timestamp;
	frameCount += 1;
	if (timestamp - fpsSampleStarted >= 500) {
		const fps = Math.round((frameCount * 1000) / (timestamp - fpsSampleStarted));
		frameCount = 0;
		fpsSampleStarted = timestamp;
		controls.updateStats(sandbox.getParticleCount(), fps, sandbox.getDiagnostics());
	}

	requestAnimationFrame(animationLoop);
}

document.addEventListener("visibilitychange", () => {
	clock.reset();
	lastTimestamp = undefined;
	frameCount = 0;
	fpsSampleStarted = undefined;
});

sandbox.seed();
sandbox.render(ctx, controls.getPointer());
controls.updateProbe();
requestAnimationFrame(animationLoop);
