import { type BrowserGpuSceneSession, initializeBrowserGpuScene } from "../src/engine-client/wasm";
import { failedProfileReport } from "./gpu-activity/failure-report";
import { fixtureDescriptions } from "./gpu-activity/fixtures";
import { installActivityObserver } from "./gpu-activity/observer";
import { readOptions, reporter } from "./gpu-activity/page";
import { installProfile } from "./gpu-activity/profile";
import type { PressureJournal } from "./gpu-activity/timeline";
import { measureActivityFixture, type RunOptions } from "./gpu-activity/timing";

type AdapterIdentity = {
	vendor: string;
	architecture: string;
	description: string;
	isFallbackAdapter: boolean;
};
type BrowserGpu = {
	requestAdapter(options?: object): Promise<{ info: AdapterIdentity } | null>;
};

const params = new URL(location.href).searchParams;
const output = document.getElementById("result");
const canvas = document.getElementById("scene");
if (!output || !(canvas instanceof HTMLCanvasElement))
	throw new Error("Activity benchmark DOM is missing");
const receiver = reporter(params);
let report: Record<string, unknown> = {
	status: "fail",
	error: "Activity benchmark did not complete",
};
let scene: BrowserGpuSceneSession | undefined;
let observer: ReturnType<typeof installActivityObserver> | undefined;
let profile: ReturnType<typeof installProfile> | undefined;
let pressureJournal: PressureJournal | undefined;
let pressureRecordingEnabled = false;
try {
	const options = readOptions(params);
	const browserTraceEnabled = params.get("browser-trace") === "true";
	if (options.mode === "activity")
		observer = installActivityObserver({ width: 480, height: 270, tileSize: 16 });
	if (options.mode === "profile") profile = installProfile(options.sampleEvery);
	const gpu = (navigator as unknown as { gpu?: BrowserGpu }).gpu;
	if (!gpu) throw new Error("Activity investigation requires WebGPU");
	const adapterRequests: { options?: object; info: AdapterIdentity | null }[] = [];
	const originalRequest = gpu.requestAdapter;
	gpu.requestAdapter = async function (requestOptions) {
		const adapter = await originalRequest.call(this, requestOptions);
		adapterRequests.push({
			options: requestOptions,
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
	try {
		scene = await initializeBrowserGpuScene(canvas);
	} finally {
		gpu.requestAdapter = originalRequest;
	}
	const selectedAdapter = adapterRequests.at(-1)?.info;
	const adapter = JSON.parse(scene.adapter_info_json()) as {
		name?: string;
		deviceType?: string;
		backend?: string;
	};
	if (scene.backend() !== "BrowserWebGpu" || selectedAdapter?.isFallbackAdapter !== false) {
		throw new Error("Activity investigation requires a browser hardware GPU");
	}
	const warmup =
		options.warmupTicks === 0
			? null
			: await measureActivityFixture(scene, {
					fixture: options.fixture,
					ticks: options.warmupTicks,
					pacing: false,
				});
	const runs: Awaited<ReturnType<typeof measureActivityFixture>>[] = [];
	const measuredScene = scene;
	const run = (name: string, pacing: boolean, diagnostic = false) => {
		let armed = false;
		const diagnosticHooks: Pick<
			RunOptions,
			"beforeTimer" | "afterTimer" | "beforeAdvance" | "afterAdvance"
		> = diagnostic
			? {
					beforeTimer: () => {
						if (profile) {
							profile.start();
							measuredScene.enable_pressure_diagnostics(8192);
							pressureRecordingEnabled = true;
						}
					},
					afterTimer: () => {
						if (profile) {
							profile.stop();
							pressureJournal = JSON.parse(
								measuredScene.take_pressure_diagnostics_json(),
							) as PressureJournal;
							measuredScene.disable_pressure_diagnostics();
							pressureRecordingEnabled = false;
						}
					},
					beforeAdvance: (last, context) => {
						profile?.beforeAdvance(context);
						if (observer && !armed && last.tick >= options.sampleTick) {
							observer.arm(options.samplePasses);
							armed = true;
						}
					},
					afterAdvance: (_context, outcome) => {
						profile?.afterAdvance(outcome);
					},
				}
			: {};
		return measureActivityFixture(measuredScene, {
			fixture: options.fixture,
			ticks: options.ticks,
			pacing,
			saveCheckpoint: (phase, bytes) => receiver.checkpoint(`${name}-${phase}`, bytes),
			...diagnosticHooks,
		});
	};
	if (options.mode === "baseline") {
		for (let index = 0; index < options.repeats; index += 1)
			runs.push(await run(`throughput-${index + 1}`, false));
		for (let index = 0; index < options.repeats; index += 1)
			runs.push(await run(`pacing-${index + 1}`, true));
	} else {
		runs.push(await run(options.mode, false, true));
	}
	const activity = await observer?.finish();
	const profiling = await profile?.finish();
	const reference = runs[0];
	if (
		runs.some(
			(item) =>
				item.before.fieldSha256 !== reference.before.fieldSha256 ||
				item.after.fieldSha256 !== reference.after.fieldSha256,
		)
	) {
		throw new Error("Repeated fixture runs disagree at equal accepted time");
	}
	report = {
		status: "pass",
		options,
		fixtureDescription: fixtureDescriptions[options.fixture],
		adapter,
		adapterRequests,
		selectedAdapter,
		browserTraceEnabled,
		measurementMode:
			options.mode === "baseline" && !browserTraceEnabled
				? "uninstrumented-baseline"
				: "diagnostic",
		userAgent: navigator.userAgent,
		width: 480,
		height: 270,
		measurement:
			"Warmup duration is recorded in options. Each measured run resets the scene. Paint, checkpoints and checkpoint transfer are outside timing. Final queue probe is included." +
			(browserTraceEnabled ? " CDP browser tracing adds overhead. This run is diagnostic." : ""),
		baselineHooks:
			options.mode === "baseline"
				? "Adapter identification during initialization only. No timing hooks."
				: "Adapter identification and diagnostic mode hooks",
		displayedFpsQualified: false,
		warmup,
		runs,
		activity,
		profile: profiling,
		pressureJournal,
	};
} catch (error) {
	report = {
		status: "fail",
		error: error instanceof Error ? (error.stack ?? error.message) : String(error),
		userAgent: navigator.userAgent,
	};
	try {
		Object.assign(
			report,
			await failedProfileReport(error, {
				scene,
				profile,
				pressureJournal,
				pressureRecordingEnabled,
			}),
		);
	} catch (exportError) {
		report.diagnosticExport = { partial: true, error: String(exportError) };
	}
} finally {
	const cleanupErrors: { stage: string; error: string }[] = [];
	for (const [stage, cleanup] of [
		["scene-dispose", () => scene?.dispose()],
		["scene-free", () => scene?.free()],
		["activity-hooks", () => observer?.restore()],
		["profile-hooks", () => profile?.restore()],
	] as const) {
		try {
			cleanup();
		} catch (error) {
			cleanupErrors.push({ stage, error: String(error) });
		}
	}
	if (cleanupErrors.length) {
		report = {
			...report,
			status: "fail",
			error: report.error ?? "Activity cleanup failed",
			cleanupErrors,
		};
	}
}
output.textContent = JSON.stringify(report);
await receiver.report(report);
