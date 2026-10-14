import type { ProfileBuffer, ProfileDevice, ProfileGlobals, QuerySet } from "./profile-types";

type Counter = { calls: number; milliseconds: number };
type Pending = {
	buffer: ProfileBuffer;
	resolved: ProfileBuffer;
	query: QuerySet;
	labels: string[];
};

/** Diagnostic hooks run separately from every baseline timing run. */
export function installProfile(sampleEvery = 64) {
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
		submit.call(this, commands);
		record(cpu, "submit", started);
	};
	api.GPUBuffer.prototype.mapAsync = async function (...args: Parameters<typeof map>) {
		if (!active) return map.apply(this, args);
		const started = performance.now();
		await map.apply(this, args);
		record(waits, this.label, started);
	};
	api.GPUDevice.prototype.createCommandEncoder = function (descriptor) {
		const encoder = create.call(this, descriptor);
		if (!active) return encoder;
		const device = this;
		let query: QuerySet | undefined;
		const labels: string[] = [];
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
			if (desc.timestampWrites || labels.length >= 64)
				throw new Error("Profile query capacity or ownership conflict");
			query ??= device.createQuerySet({ type: "timestamp", count: 128 });
			const index = labels.length * 2;
			labels.push(label);
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
			if (query && labels.length) {
				const size = labels.length * 16;
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
				encoder.resolveQuerySet(query, 0, 2 * labels.length, resolved, 0);
				encoder.copyBufferToBuffer(resolved, 0, buffer, 0, size);
				pending.push({ buffer, resolved, query, labels });
			}
			return finish(desc);
		};
		return encoder;
	};
	return {
		start() {
			active = true;
		},
		stop() {
			active = false;
		},
		async finish() {
			active = false;
			for (const item of pending) {
				await map.call(item.buffer, api.GPUMapMode.READ);
				const values = new BigUint64Array(item.buffer.getMappedRange());
				item.labels.forEach((label, index) => {
					const seconds = Number(values[2 * index + 1] - values[2 * index]) / 1e9;
					if (seconds < 0 || !Number.isFinite(seconds))
						throw new Error("Invalid GPU timestamp interval");
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
				item.buffer.unmap();
			}
			if (errors.length) throw new Error(`WebGPU profile validation failed: ${errors.join("; ")}`);
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
				measurement:
					"Separate instrumented run. GPU values cover sampled passes only. Inclusive map waits overlap GPU execution. CPU durations include hook overhead. Requested bytes are cumulative, not peak memory.",
			};
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
