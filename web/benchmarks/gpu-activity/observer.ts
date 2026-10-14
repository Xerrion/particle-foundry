import type {
	ActivityBuffer,
	ActivityDevice,
	ActivityPass,
	ActivityPipeline,
	ActivityShader,
	PipelineDescriptor,
	ShaderDescriptor,
} from "./gpu-types";
import { activityReport } from "./report";
import { ACTIVITY_LABEL, type ActivityReport, activityLayout } from "./schema";
import { instrumentTransportPairs } from "./shader";

const USAGE = { MAP_READ: 1, COPY_SRC: 4, COPY_DST: 8, STORAGE: 128 } as const;
type PipelineBindings = { disabled: object; captured?: object };

/**
 * Install before scene construction. Arm once after setup to capture bounded complete passes.
 * Await finish after the workload. Dispose the scene before restore. Never reuse that scene.
 * Baseline measurements must not install this observer.
 */
export function installActivityObserver(options: {
	width: number;
	height: number;
	tileSize?: number;
}): {
	arm(maxPasses: number): void;
	disarm(): void;
	finish(): Promise<ActivityReport>;
	restore(): void;
} {
	const layout = activityLayout(options);
	const globals = globalThis as unknown as {
		GPUDevice?: { prototype: ActivityDevice };
		GPUComputePassEncoder?: { prototype: ActivityPass };
	};
	const devicePrototype = globals.GPUDevice?.prototype;
	const passPrototype = globals.GPUComputePassEncoder?.prototype;
	if (!devicePrototype || !passPrototype) {
		throw new Error("Transport activity capture requires browser WebGPU prototypes");
	}
	const originalShader = devicePrototype.createShaderModule;
	const originalPipeline = devicePrototype.createComputePipeline;
	const originalAsyncPipeline = devicePrototype.createComputePipelineAsync;
	const originalSetPipeline = passPrototype.setPipeline;
	const modules = new WeakSet<ActivityShader>();
	const pipelines = new Map<ActivityPipeline, PipelineBindings>();
	const seenPasses = new WeakMap<ActivityPass, object>();
	const errors: string[] = [];
	const compilationChecks: Promise<void>[] = [];
	let device: ActivityDevice | undefined;
	let disabled: ActivityBuffer | undefined;
	let captured: ActivityBuffer | undefined;
	let restored = false;
	let armed = false;
	let armUsed = false;
	let finishUsed = false;
	let limit = 0;
	let totalTransportPasses = 0;
	let sampledTransportPasses = 0;
	const onError: EventListener = (event) => {
		const error = (event as Event & { error?: { message?: string } }).error;
		errors.push(error?.message ?? "Unknown uncaptured WebGPU error");
	};

	function selectDevice(selected: ActivityDevice): void {
		if (device && device !== selected) {
			throw new Error("Transport activity capture supports one scene device per install");
		}
		if (device) return;
		device = selected;
		device.addEventListener("uncapturederror", onError);
		disabled = device.createBuffer({
			label: "disabled transport activity",
			size: 4,
			usage: USAGE.STORAGE | USAGE.COPY_DST,
		});
	}

	function bind(pipeline: ActivityPipeline, buffer: ActivityBuffer): object {
		if (!device) throw new Error("Transport activity device is missing");
		return device.createBindGroup({
			label: "transport activity observations",
			layout: pipeline.getBindGroupLayout(1),
			entries: [{ binding: 0, resource: { buffer } }],
		});
	}

	function register(pipeline: ActivityPipeline): ActivityPipeline {
		if (!disabled) throw new Error("Transport activity disabled buffer is missing");
		pipelines.set(pipeline, {
			disabled: bind(pipeline, disabled),
			captured: captured ? bind(pipeline, captured) : undefined,
		});
		return pipeline;
	}

	function isTarget(descriptor: PipelineDescriptor): boolean {
		if (!modules.has(descriptor.compute.module)) return false;
		if (descriptor.layout !== "auto" || descriptor.compute.entryPoint !== "pair_faces") {
			throw new Error("Transport activity pipeline layout or entry point changed");
		}
		return true;
	}

	devicePrototype.createShaderModule = function (descriptor: ShaderDescriptor) {
		if (descriptor.label !== ACTIVITY_LABEL) return originalShader.call(this, descriptor);
		selectDevice(this);
		const code = instrumentTransportPairs(descriptor.code, layout);
		const module = originalShader.call(this, { ...descriptor, code });
		modules.add(module);
		compilationChecks.push(
			module
				.getCompilationInfo()
				.then((info) => {
					for (const message of info.messages) {
						if (message.type === "error") errors.push(message.message);
					}
				})
				.catch((error: unknown) => {
					errors.push(String(error));
				}),
		);
		return module;
	};
	devicePrototype.createComputePipeline = function (descriptor: PipelineDescriptor) {
		const target = isTarget(descriptor);
		const pipeline = originalPipeline.call(this, descriptor);
		return target ? register(pipeline) : pipeline;
	};
	devicePrototype.createComputePipelineAsync = async function (descriptor: PipelineDescriptor) {
		const target = isTarget(descriptor);
		const pipeline = await originalAsyncPipeline.call(this, descriptor);
		return target ? register(pipeline) : pipeline;
	};
	passPrototype.setPipeline = function (pipeline: ActivityPipeline) {
		originalSetPipeline.call(this, pipeline);
		const bindings = pipelines.get(pipeline);
		if (!bindings) return;
		if (this.label !== "transport stage") {
			throw new Error(
				"Transport activity pipeline moved outside the expected complete transport pass",
			);
		}
		let selected = seenPasses.get(this);
		if (!selected) {
			selected = bindings.disabled;
			if (armed) {
				totalTransportPasses += 1;
				if (sampledTransportPasses < limit) {
					if (!bindings.captured) throw new Error("Transport activity capture binding is missing");
					selected = bindings.captured;
					sampledTransportPasses += 1;
				}
			}
			seenPasses.set(this, selected);
		}
		this.setBindGroup(1, selected);
	};

	function assertHealthy(): void {
		if (errors.length > 0) throw new Error(`Transport activity GPU errors: ${errors.join("; ")}`);
	}

	return {
		arm(maxPasses) {
			if (restored || armUsed || finishUsed || !device || pipelines.size === 0) {
				throw new Error("Arm transport activity once, after the instrumented scene is ready");
			}
			if (!Number.isInteger(maxPasses) || maxPasses < 1 || maxPasses > 8) {
				throw new Error("Transport activity sample limit must be an integer from 1 to 8");
			}
			assertHealthy();
			captured = device.createBuffer({
				label: "transport activity counters",
				size: layout.words * 4,
				usage: USAGE.STORAGE | USAGE.COPY_SRC | USAGE.COPY_DST,
			});
			device.queue.writeBuffer(captured, 0, new Uint32Array([1]));
			for (const [pipeline, bindings] of pipelines) bindings.captured = bind(pipeline, captured);
			limit = maxPasses;
			armUsed = true;
			armed = true;
		},
		disarm() {
			armed = false;
		},
		async finish() {
			if (restored || finishUsed || !armUsed || !device || !captured) {
				throw new Error("Finish transport activity once, after an armed workload");
			}
			finishUsed = true;
			armed = false;
			await Promise.all(compilationChecks);
			await device.queue.onSubmittedWorkDone();
			assertHealthy();
			const readback = device.createBuffer({
				label: "transport activity readback",
				size: layout.words * 4,
				usage: USAGE.MAP_READ | USAGE.COPY_DST,
			});
			let mapped = false;
			try {
				const encoder = device.createCommandEncoder({ label: "read transport activity" });
				encoder.copyBufferToBuffer(captured, 0, readback, 0, layout.words * 4);
				device.queue.submit([encoder.finish()]);
				await readback.mapAsync(1);
				mapped = true;
				assertHealthy();
				return activityReport(
					new Uint32Array(readback.getMappedRange()),
					layout,
					totalTransportPasses,
					sampledTransportPasses,
				);
			} finally {
				if (mapped) readback.unmap();
				readback.destroy();
			}
		},
		restore() {
			if (restored) return;
			restored = true;
			armed = false;
			devicePrototype.createShaderModule = originalShader;
			devicePrototype.createComputePipeline = originalPipeline;
			devicePrototype.createComputePipelineAsync = originalAsyncPipeline;
			passPrototype.setPipeline = originalSetPipeline;
			device?.removeEventListener("uncapturederror", onError);
			captured?.destroy();
			disabled?.destroy();
		},
	};
}
