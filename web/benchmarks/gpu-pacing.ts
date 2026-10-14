import type { BrowserGpuSceneSession } from "../src/engine-client/wasm";
import type { Advance } from "./gpu-browser";

const animationFrame = () =>
	new Promise<number>((resolve) => requestAnimationFrame(() => resolve(performance.now())));
const percentile = (sorted: number[], fraction: number) =>
	sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * fraction) - 1)] ?? 0;

/** Uses the preview's serial animation-frame/advance(8)/render policy.
 * Render submissions and animation callbacks are presentation proxies.
 * They do not measure display scanout or physical input-to-photon latency.
 */
export async function measurePreviewPacing(scene: BrowserGpuSceneSession, ticks: number) {
	const resetStarted = performance.now();
	scene.reset();
	const resetCpuMs = performance.now() - resetStarted;
	if (!scene.render()) throw new Error("GPU pacing cannot render the reset scene");
	await scene.probe(240, 100);
	const resetToGpuCompletionMs = performance.now() - resetStarted;

	const brushStarted = performance.now();
	let last = JSON.parse(scene.paint(240, 100, 4, "water")) as Advance;
	if (last.tick !== 0 || last.acceptedTimeS !== 0 || last.stateRevision !== 1) {
		throw new Error("GPU pacing brush did not commit at zero model time");
	}
	if (!scene.render()) throw new Error("GPU pacing cannot render the brush");
	const brushToRenderSubmissionMs = performance.now() - brushStarted;
	const nextAnimation = animationFrame();
	await scene.probe(240, 100);
	const brushToGpuCompletionMs = performance.now() - brushStarted;
	const brushToNextAnimationMs = (await nextAnimation) - brushStarted;
	const renders: number[] = [];
	const intervals: number[] = [];
	const started = performance.now();
	for (let calls = 0; last.tick < ticks && calls < ticks * 64; calls += 1) {
		await animationFrame();
		const next = JSON.parse(await scene.advance(8)) as Advance;
		if (next.acceptedTimeS <= last.acceptedTimeS || next.epoch !== last.epoch) {
			throw new Error("GPU pacing advance did not preserve accepted-time ordering");
		}
		if (!scene.render()) throw new Error("GPU pacing cannot render committed progress");
		const rendered = performance.now();
		const previous = renders.at(-1);
		if (previous !== undefined) intervals.push(rendered - previous);
		renders.push(rendered);
		last = next;
	}
	await scene.probe(240, 100);
	const wallSeconds = (performance.now() - started) / 1000;
	if (last.tick !== ticks || Math.abs(last.acceptedTimeS - ticks / 60) > 1e-6) {
		throw new Error("GPU pacing did not finish the requested model time");
	}
	const sorted = [...intervals].sort((a, b) => a - b);
	return {
		last,
		policy: "preview requestAnimationFrame, advance(8), render, then schedule the next frame",
		measurementScope:
			"render-submission cadence; display presentation and physical input latency are unqualified",
		resetCpuMs,
		resetToGpuCompletionMs,
		brushToRenderSubmissionMs,
		brushToGpuCompletionMs,
		brushToNextAnimationMs,
		frameSubmissions: renders.length,
		wallSeconds,
		acceptedModelSeconds: last.acceptedTimeS,
		modelSecondsPerWallSecond: last.acceptedTimeS / wallSeconds,
		renderSubmissionsPerSecond: renders.length / wallSeconds,
		frameIntervalP50Ms: percentile(sorted, 0.5),
		frameIntervalP95Ms: percentile(sorted, 0.95),
		frameIntervalMaxMs: sorted.at(-1) ?? 0,
		intervalsOver60FpsBudget: intervals.filter((ms) => ms > 1000 / 60).length,
		intervalsOver33_3Ms: intervals.filter((ms) => ms > 1000 / 30).length,
	};
}
