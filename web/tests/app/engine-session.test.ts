import { describe, expect, test } from "bun:test";
import {
	createEngineSession,
	type EpochOwner,
	preflightScene,
	type SceneCapabilities,
	type SceneRequest,
	type SequencedCommand,
	type SmallProbe,
} from "../../src/engine-client/session";

const scene: SceneRequest = {
	width: 4,
	height: 3,
	model: "lowMach",
	execution: "cpu-reference",
	materialIds: [0, 2, 5],
	boundaries: "fixed",
};
const support: SceneCapabilities = {
	model: "lowMach",
	execution: "cpu-reference",
	materialIds: [0, 2, 5],
	boundaries: ["fixed"],
	maxCells: 12,
};

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

function harness() {
	const owners: Array<
		EpochOwner & {
			batches: { commands: readonly SequencedCommand[]; ticks: number }[];
			pending: ReturnType<typeof deferred<number>>[];
			probes: ReturnType<typeof deferred<SmallProbe>>[];
			disposed: boolean;
		}
	> = [];
	const session = createEngineSession(scene, support, (epoch) => {
		const owner = {
			batches: [] as { commands: readonly SequencedCommand[]; ticks: number }[],
			pending: [] as ReturnType<typeof deferred<number>>[],
			probes: [] as ReturnType<typeof deferred<SmallProbe>>[],
			disposed: false,
			apply(commands: readonly SequencedCommand[], ticks: number) {
				const result = deferred<number>();
				this.batches.push({ commands, ticks });
				this.pending.push(result);
				return result.promise;
			},
			probe() {
				const result = deferred<SmallProbe>();
				this.probes.push(result);
				return result.promise;
			},
			dispose() {
				this.disposed = true;
			},
		};
		owners.push(owner);
		expect(epoch).toBe(owners.length);
		return owner;
	});
	return { session, owners };
}

const paint = { kind: "paint" as const, x: 2, y: 1, materialId: 2, radius: 2 };

describe("E03 scene contract", () => {
	test("preflight rejects unsupported features before constructing an owner", () => {
		const invalid = { ...scene, execution: "wgpu" as const };
		expect(() => preflightScene(invalid, support)).toThrow("unsupported");
		expect(() => preflightScene({ ...scene, materialIds: [0, 8] }, support)).toThrow("materials");
		expect(() => preflightScene({ ...scene, width: 5 }, support)).toThrow("capacity");
		let constructed = false;
		expect(() =>
			createEngineSession(invalid, support, () => {
				constructed = true;
				throw new Error("must not construct");
			}),
		).toThrow("unsupported");
		expect(constructed).toBe(false);
	});

	test("paused paint is queued, bounded, ordered and only committed by a zero-tick flush", async () => {
		const { session, owners } = harness();
		expect(session.enqueue([paint])).toEqual({
			epoch: 1,
			firstSequence: 1,
			lastSequence: 1,
			queued: 1,
		});
		expect(session.latestStatus()).toMatchObject({ tick: 0, queuedCommands: 1 });
		expect(() => session.enqueue(Array(64).fill(paint))).toThrow("capacity");
		expect(() => session.enqueue([paint, { ...paint, materialId: 999 }])).toThrow();
		expect(session.latestStatus().queuedCommands).toBe(1);
		session.advance(0);
		await Promise.resolve();
		expect(owners[0].batches[0]).toEqual({ commands: [{ sequence: 1, command: paint }], ticks: 0 });
		expect(() => session.advance(1)).toThrow("backpressure");
		expect(session.latestStatus()).toMatchObject({ tick: 0, state: "busy" });
		owners[0].pending[0].resolve(0);
		await Bun.sleep(0);
		expect(session.latestStatus()).toMatchObject({
			tick: 0,
			lastCommittedSequence: 1,
			state: "ready",
		});
		session.dispose();
	});

	test("reset cancels old probes and late completions cannot advance the replacement", async () => {
		const { session, owners } = harness();
		const probe = session.probe(1, 1);
		session.advance(2);
		await Promise.resolve();
		session.reset();
		expect(owners[0].disposed).toBe(true);
		await expect(probe).rejects.toThrow("cancelled");
		owners[0].pending[0].resolve(2);
		await Bun.sleep(0);
		expect(session.latestStatus()).toMatchObject({ epoch: 2, tick: 0, state: "ready" });
		session.advance(1);
		await Promise.resolve();
		owners[1].pending[0].resolve(1);
		await Bun.sleep(0);
		expect(session.latestStatus()).toMatchObject({
			epoch: 2,
			tick: 1,
			acceptedTimeSeconds: 1 / 60,
		});
		session.dispose();
	});

	test("stale tick samples are rejected and a failed batch cannot claim completion", async () => {
		const { session, owners } = harness();
		session.advance(1);
		await Promise.resolve();
		owners[0].pending[0].resolve(1);
		await Bun.sleep(0);
		const probe = session.probe(0, 0);
		owners[0].probes[0].resolve({ epoch: 1, tick: 0, x: 0, y: 0, materialId: 0 });
		await expect(probe).rejects.toThrow("Stale");
		session.advance(1);
		await Promise.resolve();
		owners[0].pending[1].resolve(9);
		await Bun.sleep(0);
		expect(session.latestStatus()).toMatchObject({ tick: 1, state: "faulted" });
		expect(() => session.advance(1)).toThrow("faulted");
		session.dispose();
	});

	test("current probes detach their result and caller cancellation settles promptly", async () => {
		const { session, owners } = harness();
		const probe = session.probe(1, 2);
		const ownerReading = { epoch: 1, tick: 0, x: 1, y: 2, materialId: 2 };
		owners[0].probes[0].resolve(ownerReading);
		const reading = await probe;
		ownerReading.materialId = 5;
		expect(reading.materialId).toBe(2);
		const controller = new AbortController();
		const pending = session.probe(2, 2, controller.signal);
		controller.abort();
		await expect(pending).rejects.toThrow("cancelled");
		session.dispose();
	});

	test("failed replacement leaves the current owner available", () => {
		let disposed = false;
		const owner: EpochOwner = {
			apply: async () => 0,
			probe: async () => ({ epoch: 1, tick: 0, x: 0, y: 0, materialId: 0 }),
			dispose: () => {
				disposed = true;
			},
		};
		const session = createEngineSession(scene, support, (epoch) => {
			if (epoch === 2) throw new Error("allocation failed");
			return owner;
		});
		expect(() => session.reset()).toThrow("allocation failed");
		expect(disposed).toBe(false);
		expect(session.latestStatus()).toMatchObject({ epoch: 1, state: "ready" });
		session.dispose();
	});
});
