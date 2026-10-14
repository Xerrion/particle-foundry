import { expect, test } from "bun:test";
import { failedProfileReport } from "../../benchmarks/gpu-activity/failure-report";
import { installProfile } from "../../benchmarks/gpu-activity/profile";
import type { PressureJournal } from "../../benchmarks/gpu-activity/timeline";

for (const failingAdvance of [false, true]) {
	test(
		failingAdvance
			? "failed advance retains map errors and pressure rejection when timestamp collection also fails"
			: "profile hooks preserve sums while bounded chronology retains advance context",
		async () => {
			const completions = new Map<string, () => void>();
			class Buffer {
				constructor(public label: string) {}
				mapAsync() {
					if (failingAdvance && this.label === "pressure-second")
						return Promise.reject(new Error("Original pressure status map failure"));
					if (failingAdvance && this.label === "activity-profile-readback")
						return Promise.reject(new Error("Diagnostic timestamp readback failure"));
					if (this.label.startsWith("pressure"))
						return new Promise<void>((resolve) => completions.set(this.label, resolve));
					return Promise.resolve();
				}
				getMappedRange() {
					return new BigUint64Array([1_000_000_000_000_000_000n, 1_000_000_000_000_001_000n])
						.buffer;
				}
				unmap() {}
				destroy() {}
			}
			class Pass {
				setPipeline() {}
				setBindGroup() {}
				dispatchWorkgroups() {}
			}
			class Encoder {
				beginComputePass() {
					return new Pass();
				}
				beginRenderPass() {
					return {};
				}
				resolveQuerySet() {}
				copyBufferToBuffer() {}
				finish() {
					return {};
				}
			}
			class Device {
				createBuffer(descriptor: { label?: string }) {
					return new Buffer(descriptor.label ?? "");
				}
				createBindGroup() {
					return {};
				}
				createQuerySet() {
					return { destroy() {} };
				}
				createCommandEncoder() {
					return new Encoder();
				}
				addEventListener() {}
				removeEventListener() {}
			}
			class Adapter {
				features = new Set(["timestamp-query"]);
				info = {
					vendor: "test",
					architecture: "test",
					description: "test",
					isFallbackAdapter: false,
				};
				requestDevice() {
					return Promise.resolve(new Device());
				}
			}
			class Queue {
				submit() {}
			}
			const globals = {
				GPUAdapter: Adapter,
				GPUDevice: Device,
				GPUComputePassEncoder: Pass,
				GPUBuffer: Buffer,
				GPUQueue: Queue,
				GPUMapMode: { READ: 1 },
				GPUBufferUsage: { QUERY_RESOLVE: 1, COPY_SRC: 2, COPY_DST: 4, MAP_READ: 8 },
			};
			const previous = new Map(
				Object.keys(globals).map((name) => [
					name,
					Object.getOwnPropertyDescriptor(globalThis, name),
				]),
			);
			for (const [name, value] of Object.entries(globals))
				Object.defineProperty(globalThis, name, { value, configurable: true });
			let profile: ReturnType<typeof installProfile> | undefined;
			try {
				profile = installProfile(1, 4);
				const device = await new Adapter().requestDevice();
				const first = device.createBuffer({ label: "pressure-first" });
				const second = device.createBuffer({ label: "pressure-second" });
				profile.start();
				const context = {
					advanceId: 1,
					epoch: 7,
					outerTick: 1,
					stateRevision: 3,
					acceptedTimeBeforeS: 0,
				};
				profile.beforeAdvance(context);
				const firstMap = first.mapAsync();
				const secondMap = second.mapAsync();
				const encoder = device.createCommandEncoder();
				encoder.beginComputePass();
				encoder.finish();
				new Queue().submit();
				new Queue().submit();
				completions.get("pressure-second")?.();
				let advanceError: unknown;
				try {
					await secondMap;
				} catch (error) {
					advanceError = error;
					profile.afterAdvance("error");
				}
				completions.get("pressure-first")?.();
				await firstMap;
				if (!failingAdvance) profile.afterAdvance();
				profile.stop();
				if (failingAdvance) {
					const calls: string[] = [];
					const pressureJournal: PressureJournal = {
						version: 1,
						clockDomain: "browser-performance-now",
						capacity: 8192,
						droppedEvents: 0,
						events: [
							{
								sequence: 1,
								type: "pressureStatus",
								epoch: 7,
								outerTick: 1,
								candidateId: 1,
								attempt: 5,
								stateRevision: 3,
								acceptedTimeBeforeS: 0,
								encodedDtS: 1 / 960,
								phase: "pcgBatch",
								batchIndex: 1,
								requestedIterations: 32,
								mapStartedMs: 1,
								mapCompletedMs: 2,
								statusWords: [0, 0, 0, 0, 0, 0, 0, 0],
							},
							{
								sequence: 2,
								type: "candidateOutcome",
								epoch: 7,
								outerTick: 1,
								candidateId: 1,
								attempt: 5,
								stateRevision: 3,
								acceptedTimeBeforeS: 0,
								encodedDtS: 1 / 960,
								timestampMs: 2,
								outcome: "terminalRejection",
							},
						],
					};
					const report = await failedProfileReport(advanceError, {
						profile,
						pressureRecordingEnabled: true,
						scene: {
							take_pressure_diagnostics_json() {
								calls.push("drain");
								return JSON.stringify(pressureJournal);
							},
							disable_pressure_diagnostics() {
								calls.push("disable");
							},
						},
					});
					expect(report.status).toBe("fail");
					expect(report.error).toContain("Original pressure status map failure");
					expect(report.error).not.toContain("Diagnostic timestamp readback failure");
					expect(report.diagnosticExport).toMatchObject({
						partial: true,
						pressureStatus: "collected",
						profileStatus: "partial",
					});
					expect(report.diagnosticExport.errors[0]).toMatchObject({
						stage: "profile-timestamps",
						error: "Error: Diagnostic timestamp readback failure",
					});
					expect(report.pressureJournal).toEqual(pressureJournal);
					expect(calls).toEqual(["drain", "disable"]);
					expect(report.profile?.timeline.host.events[0]).toMatchObject({
						type: "advance",
						outcome: "error",
					});
					expect(report.profile?.timeline.host.events[2]).toMatchObject({
						type: "map",
						label: "pressure-second",
						outcome: "error",
					});
					expect(report.profile?.timestampCollection.pendingBatches).toBe(1);
					return;
				}
				const result = await profile.finish();
				expect(await profile.finish()).toBe(result);
				expect(result.cpu.submit.calls).toBe(2);
				expect(result.inclusiveMapWaits["pressure-first"].calls).toBe(1);
				expect(result.timeline.host.droppedEvents).toBe(1);
				expect(result.timeline.host.events.map((event) => event.label)).toEqual([
					"advance(8)",
					"pressure-first",
					"pressure-second",
					"queue",
				]);
				expect(result.timeline.host.events[1].completedMs).toBeGreaterThanOrEqual(
					result.timeline.host.events[2].completedMs,
				);
				expect(result.timeline.host.events.every((event) => event.context?.epoch === 7)).toBe(true);
				expect(result.timeline.gpu.events[0]).toMatchObject({
					beginTimestampNs: "1000000000000000000",
					endTimestampNs: "1000000000000001000",
					context,
				});
				expect(result.gpu["compute:unlabelled"].sampledSeconds).toBeCloseTo(1e-6, 12);
			} finally {
				profile?.restore();
				for (const [name, descriptor] of previous) {
					if (descriptor) Object.defineProperty(globalThis, name, descriptor);
					else Reflect.deleteProperty(globalThis, name);
				}
			}
		},
	);
}
