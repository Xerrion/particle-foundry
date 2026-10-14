import type { ProfileBuffer, ProfileDevice, ProfileGlobals, QuerySet } from "./profile-types";
import {
	type AdvanceContext,
	boundedEvents,
	type GpuEvent,
	type HostEvent,
	type ProfileTimeline,
} from "./timeline";

type Counter = { calls: number; milliseconds: number };
type Pending = {
	buffer: ProfileBuffer;
	resolved: ProfileBuffer;
	query: QuerySet;
	passes: { label: string; sequence: number; context: AdvanceContext | null }[];
};

/** Diagnostic hooks run separately from every baseline timing run. */
export function installProfile(sampleEvery = 64, timelineCapacity = 8192) {
	const api = globalThis as unknown as ProfileGlobals;
	if (!api.GPUAdapter || !api.GPUDevice || !api.GPUBuffer)
		throw new Error("WebGPU profile hooks are unavailable");
	const cpu: Record<string, Counter> = {};
	const waits: Record<string, Counter> = {};
	const gpu: Record<
		string,
		{
			sampledPasses: number;
			sampledSeconds: number;
			minimumSeconds: number;
			maximumSeconds: number;
		}
	> = {};
	const totalPasses: Record<string, number> = {};
	const bufferAllocations: Record<string, { calls: number; requestedBytes: number }> = {};
	const pending: Pending[] = [];
	const errors: string[] = [];
	const devices: ProfileDevice[] = [];
	const hostEvents = boundedEvents<HostEvent>(timelineCapacity);
	const gpuEvents = boundedEvents<GpuEvent>(timelineCapacity);
	let hostSequence = 0;
	let gpuSequence = 0;
	let hostOriginMs = 0;
	let context: AdvanceContext | null = null;
	let advanceEvent: HostEvent | null = null;
	let completedTimestampBatches = 0;
	let finishPromise: Promise<ReturnType<typeof snapshot>> | undefined;
	let active = false;
	let timestampsAvailable = false;
	let adapterInfo: unknown;
	const request = api.GPUAdapter.prototype.requestDevice;
	const create = api.GPUDevice.prototype.createCommandEncoder;
	const createBuffer = api.GPUDevice.prototype.createBuffer;
	const createBindGroup = api.GPUDevice.prototype.createBindGroup;
	const setPipeline = api.GPUComputePassEncoder.prototype.setPipeline;
	const setBindGroup = api.GPUComputePassEncoder.prototype.setBindGroup;
	const dispatch = api.GPUComputePassEncoder.prototype.dispatchWorkgroups;
	const submit = api.GPUQueue.prototype.submit;
	const map = api.GPUBuffer.prototype.mapAsync;
	const validationError: EventListener = (event) => {
		const error = (event as Event & { error?: { message?: string } }).error;
		errors.push(error?.message ?? "Uncaptured WebGPU error");
	};
	function record(table: Record<string, Counter>, key: string, started: number) {
		const row = table[key] ?? { calls: 0, milliseconds: 0 };
		table[key] = row;
		row.calls += 1;
		row.milliseconds += performance.now() - started;
	}
	function span(type: HostEvent["type"], label: string, startedMs: number) {
		return hostEvents.add({
			sequence: ++hostSequence,
			type,
			label,
			startedMs,
			completedMs: startedMs,
			outcome: "complete",
			context,
		});
	}
	function complete(event: HostEvent | null, outcome: HostEvent["outcome"] = "complete") {
		if (event) {
			event.completedMs = performance.now();
			event.outcome = outcome;
		}
	}
	api.GPUAdapter.prototype.requestDevice = async function (descriptor) {
		timestampsAvailable = this.features.has("timestamp-query");
		adapterInfo = {
			...this.info,
			vendor: this.info.vendor,
			architecture: this.info.architecture,
			description: this.info.description,
			isFallbackAdapter: this.info.isFallbackAdapter,
		};
		const requiredFeatures = new Set(descriptor?.requiredFeatures ?? []);
		if (timestampsAvailable) requiredFeatures.add("timestamp-query");
		const device = await request.call(this, { ...descriptor, requiredFeatures });
		device.addEventListener("uncapturederror", validationError);
		devices.push(device);
		return device;
	};
	api.GPUDevice.prototype.createBuffer = function (descriptor) {
		if (!active) return createBuffer.call(this, descriptor);
		const started = performance.now();
		const buffer = createBuffer.call(this, descriptor);
		record(cpu, "createBuffer", started);
		const label = descriptor.label ?? "unlabelled";
		const row = bufferAllocations[label] ?? { calls: 0, requestedBytes: 0 };
		bufferAllocations[label] = row;
		row.calls += 1;
		row.requestedBytes += Number(descriptor.size);
		return buffer;
	};
	api.GPUDevice.prototype.createBindGroup = function (descriptor) {
		if (!active) return createBindGroup.call(this, descriptor);
		const started = performance.now();
		const result = createBindGroup.call(this, descriptor);
		record(cpu, "createBindGroup", started);
		return result;
	};
	api.GPUComputePassEncoder.prototype.setPipeline = function (pipeline) {
		if (!active) return setPipeline.call(this, pipeline);
		const started = performance.now();
		setPipeline.call(this, pipeline);
		record(cpu, "setPipeline", started);
	};
	api.GPUComputePassEncoder.prototype.setBindGroup = function (
		...args: Parameters<typeof setBindGroup>
	) {
		if (!active) return setBindGroup.apply(this, args);
		const started = performance.now();
		setBindGroup.apply(this, args);
		record(cpu, "setBindGroup", started);
	};
	api.GPUComputePassEncoder.prototype.dispatchWorkgroups = function (x, y, z) {
		if (!active) return dispatch.call(this, x, y, z);
		const started = performance.now();
		dispatch.call(this, x, y, z);
		record(cpu, "dispatchWorkgroups", started);
	};
	api.GPUQueue.prototype.submit = function (commands) {
		if (!active) return submit.call(this, commands);
		const started = performance.now();
		const event = span("submit", "queue", started);
		try {
			submit.call(this, commands);
			complete(event);
			record(cpu, "submit", started);
		} catch (error) {
			complete(event, "error");
			throw error;
		}
	};
	api.GPUBuffer.prototype.mapAsync = async function (...args: Parameters<typeof map>) {
		if (!active) return map.apply(this, args);
		const started = performance.now();
		const event = span("map", this.label, started);
		try {
			await map.apply(this, args);
			complete(event);
			record(waits, this.label, started);
		} catch (error) {
			complete(event, "error");
			throw error;
		}
	};
	api.GPUDevice.prototype.createCommandEncoder = function (descriptor) {
		const encoder = create.call(this, descriptor);
		if (!active) return encoder;
		const device = this;
		let query: QuerySet | undefined;
		const passes: Pending["passes"] = [];
		const compute = encoder.beginComputePass.bind(encoder);
		const render = encoder.beginRenderPass.bind(encoder);
		const finish = encoder.finish.bind(encoder);
		function timestamp<T extends { label?: string; timestampWrites?: unknown }>(
			kind: string,
			desc: T,
		): T {
			const label = `${kind}:${desc.label ?? "unlabelled"}`;
			totalPasses[label] = (totalPasses[label] ?? 0) + 1;
			if (!timestampsAvailable || totalPasses[label] % sampleEvery !== 1 % sampleEvery) return desc;
			if (desc.timestampWrites || passes.length >= 64)
				throw new Error("Profile query capacity or ownership conflict");
			query ??= device.createQuerySet({ type: "timestamp", count: 128 });
			const index = passes.length * 2;
			passes.push({ label, sequence: ++gpuSequence, context });
			return {
				...desc,
				timestampWrites: {
					querySet: query,
					beginningOfPassWriteIndex: index,
					endOfPassWriteIndex: index + 1,
				},
			};
		}
		encoder.beginComputePass = (desc = {}) => compute(timestamp("compute", desc));
		encoder.beginRenderPass = (desc) => render(timestamp("render", desc));
		encoder.finish = (desc) => {
			if (query && passes.length) {
				const size = passes.length * 16;
				// Use original allocation calls so profiling storage does not enter engine API counts.
				const resolved = createBuffer.call(device, {
					label: "activity-profile-resolve",
					size,
					usage: api.GPUBufferUsage.QUERY_RESOLVE | api.GPUBufferUsage.COPY_SRC,
				});
				const buffer = createBuffer.call(device, {
					label: "activity-profile-readback",
					size,
					usage: api.GPUBufferUsage.MAP_READ | api.GPUBufferUsage.COPY_DST,
				});
				encoder.resolveQuerySet(query, 0, 2 * passes.length, resolved, 0);
				encoder.copyBufferToBuffer(resolved, 0, buffer, 0, size);
				pending.push({ buffer, resolved, query, passes });
			}
			return finish(desc);
		};
		return encoder;
	};
	function snapshot() {
		const timeline: ProfileTimeline = {
			version: 1,
			host: {
				clockDomain: "browser-performance-now",
				originMs: hostOriginMs,
				...hostEvents.snapshot(),
			},
			gpu: {
				clockDomain: "gpu-timestamp",
				originTimestampNs: gpuEvents.snapshot().events[0]?.beginTimestampNs ?? null,
				...gpuEvents.snapshot(),
			},
			note: "Host milliseconds use performance.now. GPU nanoseconds use the device timestamp clock. These clocks have no measured alignment. Relative origins are independent. Do not subtract GPU timestamps from host timestamps or infer host queue delay from their horizontal positions.",
		};
		return {
			adapterInfo,
			timestampsAvailable,
			timestampStatus: timestampsAvailable ? "available" : "unavailable",
			sampleEvery,
			gpu,
			totalPasses,
			cpu,
			inclusiveMapWaits: waits,
			bufferAllocations,
			timeline,
			timestampCollection: {
				completedBatches: completedTimestampBatches,
				pendingBatches: pending.length - completedTimestampBatches,
				validationErrors: [...errors],
			},
			measurement:
				"Separate instrumented run. GPU values cover sampled passes only. Inclusive map waits overlap GPU execution. CPU durations include hook overhead. Requested bytes are cumulative, not peak memory.",
		};
	}
	async function collectTimestamps() {
		active = false;
		for (const item of pending) {
			await map.call(item.buffer, api.GPUMapMode.READ);
			try {
				const values = new BigUint64Array(item.buffer.getMappedRange());
				item.passes.forEach(({ label, sequence, context }, index) => {
					const begin = values[2 * index];
					const end = values[2 * index + 1];
					const seconds = Number(end - begin) / 1e9;
					if (seconds < 0 || !Number.isFinite(seconds))
						throw new Error("Invalid GPU timestamp interval");
					gpuEvents.add({
						sequence,
						label,
						beginTimestampNs: String(begin),
						endTimestampNs: String(end),
						context,
					});
					const row = gpu[label] ?? {
						sampledPasses: 0,
						sampledSeconds: 0,
						minimumSeconds: Infinity,
						maximumSeconds: 0,
					};
					gpu[label] = row;
					row.sampledPasses += 1;
					row.sampledSeconds += seconds;
					row.minimumSeconds = Math.min(row.minimumSeconds, seconds);
					row.maximumSeconds = Math.max(row.maximumSeconds, seconds);
				});
				completedTimestampBatches += 1;
			} finally {
				item.buffer.unmap();
			}
		}
		if (errors.length) throw new Error(`WebGPU profile validation failed: ${errors.join("; ")}`);
		return snapshot();
	}
	return {
		start() {
			hostOriginMs = performance.now();
			active = true;
		},
		beforeAdvance(nextContext: AdvanceContext) {
			context = nextContext;
			advanceEvent = span("advance", "advance(8)", performance.now());
		},
		afterAdvance(outcome: HostEvent["outcome"] = "complete") {
			complete(advanceEvent, outcome);
			if (advanceEvent) {
				performance.measure("pf.advance", {
					start: advanceEvent.startedMs,
					end: advanceEvent.completedMs,
					detail: context,
				});
				performance.clearMeasures("pf.advance");
			}
			advanceEvent = null;
		},
		stop() {
			active = false;
		},
		snapshot,
		finish() {
			finishPromise ??= collectTimestamps();
			return finishPromise;
		},
		restore() {
			active = false;
			api.GPUAdapter.prototype.requestDevice = request;
			api.GPUDevice.prototype.createCommandEncoder = create;
			api.GPUDevice.prototype.createBuffer = createBuffer;
			api.GPUDevice.prototype.createBindGroup = createBindGroup;
			api.GPUComputePassEncoder.prototype.setPipeline = setPipeline;
			api.GPUComputePassEncoder.prototype.setBindGroup = setBindGroup;
			api.GPUComputePassEncoder.prototype.dispatchWorkgroups = dispatch;
			api.GPUQueue.prototype.submit = submit;
			api.GPUBuffer.prototype.mapAsync = map;
			for (const item of pending) {
				item.buffer.destroy();
				item.resolved.destroy();
				item.query.destroy();
			}
			for (const device of devices) device.removeEventListener("uncapturederror", validationError);
		},
	};
}
