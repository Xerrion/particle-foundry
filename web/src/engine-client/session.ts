/** Experimental E03 command boundary. An owner belongs to exactly one scene epoch. */
export type EngineCommand =
	| { kind: "paint"; x: number; y: number; materialId: number; radius: number }
	| { kind: "set-boundaries"; enabled: boolean };

export interface SceneRequest {
	width: number;
	height: number;
	model: "lowMach" | "compressible";
	execution: "cpu-reference" | "wgpu";
	materialIds: readonly number[];
	boundaries: "fixed" | "open";
}

export interface SceneCapabilities {
	model: SceneRequest["model"];
	execution: SceneRequest["execution"];
	materialIds: readonly number[];
	boundaries: readonly SceneRequest["boundaries"][];
	maxCells: number;
}

/** Reject before allocating or replacing an owner. A device alone grants no scene support. */
export function preflightScene(scene: SceneRequest, support: SceneCapabilities): void {
	if (
		!Number.isSafeInteger(scene.width) ||
		!Number.isSafeInteger(scene.height) ||
		scene.width < 1 ||
		scene.height < 1 ||
		scene.width > 4096 ||
		scene.height > 4096 ||
		scene.width * scene.height > Math.min(support.maxCells, 1_048_576)
	) {
		throw new RangeError("Scene dimensions exceed supported capacity");
	}
	if (scene.model !== support.model || scene.execution !== support.execution) {
		throw new RangeError("Scene model or execution is unsupported");
	}
	if (!support.boundaries.includes(scene.boundaries)) {
		throw new RangeError("Scene boundaries are unsupported");
	}
	if (
		!scene.materialIds.length ||
		scene.materialIds.length > 16 ||
		new Set(scene.materialIds).size !== scene.materialIds.length ||
		scene.materialIds.some((id) => !Number.isSafeInteger(id) || !support.materialIds.includes(id))
	) {
		throw new RangeError("Scene materials are unsupported or duplicated");
	}
}

export interface Stamp {
	epoch: number;
	tick: number;
}

export interface SequencedCommand {
	sequence: number;
	command: EngineCommand;
}

export interface SmallProbe extends Stamp {
	x: number;
	y: number;
	materialId: number;
}

export interface EpochOwner {
	/** Commit at most requestedTicks. A failed batch must leave the prior commit intact. */
	apply(
		commands: readonly SequencedCommand[],
		requestedTicks: number,
		signal: AbortSignal,
	): Promise<number>;
	probe(x: number, y: number, signal: AbortSignal): Promise<SmallProbe>;
	dispose(): void;
}

export interface SessionStatus extends Stamp {
	acceptedTimeSeconds: number;
	lastCommittedSequence: number;
	queuedCommands: number;
	state: "ready" | "busy" | "faulted" | "disposed";
}

export interface QueueReceipt {
	epoch: number;
	firstSequence: number;
	lastSequence: number;
	queued: number;
}

export interface AdvanceReceipt {
	epoch: number;
	requestedTicks: number;
	/** Submission is not proof of completed physical time. */
	accepted: true;
}

export interface EngineSession {
	enqueue(commands: readonly EngineCommand[]): QueueReceipt;
	advance(requestedTicks: number): AdvanceReceipt;
	latestStatus(): SessionStatus;
	probe(x: number, y: number, signal?: AbortSignal): Promise<SmallProbe>;
	/** Replaces the owner atomically and invalidates all older replies. */
	reset(): void;
	dispose(): void;
}

const MAX_QUEUED_COMMANDS = 64;
const MAX_TICKS_PER_BATCH = 8;
const OUTER_DT_SECONDS = 1 / 60;

function validateCommand(command: EngineCommand, scene: SceneRequest): void {
	if (command.kind === "set-boundaries") {
		if (typeof command.enabled !== "boolean") throw new RangeError("Invalid boundary setting");
		if ((command.enabled ? "fixed" : "open") !== scene.boundaries) {
			throw new RangeError("Boundary change needs a supported scene conversion");
		}
		return;
	}
	if (command.kind !== "paint") throw new RangeError("Unknown engine command");
	if (
		!Number.isSafeInteger(command.x) ||
		!Number.isSafeInteger(command.y) ||
		command.x < 0 ||
		command.y < 0 ||
		command.x >= scene.width ||
		command.y >= scene.height ||
		!Number.isSafeInteger(command.radius) ||
		command.radius < 1 ||
		command.radius > 12 ||
		!scene.materialIds.includes(command.materialId)
	) {
		throw new RangeError("Invalid paint command");
	}
}

/** A contract host for one owner per epoch; production scene support remains gated. */
export function createEngineSession(
	scene: SceneRequest,
	support: SceneCapabilities,
	createOwner: (epoch: number) => EpochOwner,
): EngineSession {
	preflightScene(scene, support);
	const selectedScene = { ...scene, materialIds: [...scene.materialIds] };
	let epoch = 1;
	let owner = createOwner(epoch);
	let controller = new AbortController();
	let tick = 0;
	let sequence = 0;
	let committedSequence = 0;
	let queue: SequencedCommand[] = [];
	let state: SessionStatus["state"] = "ready";

	function requireReady(): void {
		if (state === "disposed" || state === "faulted") throw new Error(`Engine session is ${state}`);
	}

	return {
		enqueue(commands) {
			requireReady();
			if (
				!commands.length ||
				commands.length + queue.length > MAX_QUEUED_COMMANDS ||
				sequence + commands.length > Number.MAX_SAFE_INTEGER
			) {
				throw new RangeError("Command queue capacity exceeded");
			}
			for (const command of commands) validateCommand(command, selectedScene);
			const firstSequence = sequence + 1;
			for (const command of commands) queue.push({ sequence: ++sequence, command: { ...command } });
			return { epoch, firstSequence, lastSequence: sequence, queued: queue.length };
		},
		advance(requestedTicks) {
			requireReady();
			if (state === "busy") throw new Error("Engine backpressure: batch in flight");
			if (
				!Number.isSafeInteger(requestedTicks) ||
				requestedTicks < 0 ||
				requestedTicks > MAX_TICKS_PER_BATCH ||
				(requestedTicks === 0 && queue.length === 0) ||
				tick + requestedTicks > Number.MAX_SAFE_INTEGER
			) {
				throw new RangeError("Invalid tick request");
			}
			const batch = queue;
			queue = [];
			const submittedEpoch = epoch;
			const submittedOwner = owner;
			const signal = controller.signal;
			state = "busy";
			void Promise.resolve()
				.then(() => submittedOwner.apply(batch, requestedTicks, signal))
				.then((acceptedTicks) => {
					if (submittedEpoch !== epoch || state === "disposed") return;
					if (
						!Number.isSafeInteger(acceptedTicks) ||
						acceptedTicks < 0 ||
						acceptedTicks > requestedTicks
					) {
						state = "faulted";
						return;
					}
					tick += acceptedTicks;
					committedSequence = batch.at(-1)?.sequence ?? committedSequence;
					state = "ready";
				})
				.catch(() => {
					if (submittedEpoch === epoch && state !== "disposed") state = "faulted";
				});
			return { epoch: submittedEpoch, requestedTicks, accepted: true };
		},
		latestStatus() {
			return {
				epoch,
				tick,
				acceptedTimeSeconds: tick * OUTER_DT_SECONDS,
				lastCommittedSequence: committedSequence,
				queuedCommands: queue.length,
				state,
			};
		},
		async probe(x, y, signal) {
			requireReady();
			if (
				!Number.isSafeInteger(x) ||
				!Number.isSafeInteger(y) ||
				x < 0 ||
				y < 0 ||
				x >= selectedScene.width ||
				y >= selectedScene.height
			)
				throw new RangeError("Invalid probe coordinates");
			if (signal?.aborted) throw new Error("Probe cancelled");
			const requestedEpoch = epoch;
			const requestOwner = owner;
			const combined = signal ? AbortSignal.any([signal, controller.signal]) : controller.signal;
			let onAbort: (() => void) | undefined;
			const cancelled = new Promise<never>((_, reject) => {
				onAbort = () => reject(new Error("Probe cancelled"));
				combined.addEventListener("abort", onAbort, { once: true });
			});
			try {
				const reading = await Promise.race([requestOwner.probe(x, y, combined), cancelled]);
				if (
					epoch !== requestedEpoch ||
					reading.epoch !== epoch ||
					reading.tick !== tick ||
					reading.x !== x ||
					reading.y !== y ||
					!selectedScene.materialIds.includes(reading.materialId)
				) {
					throw new Error("Stale probe result");
				}
				return { ...reading };
			} finally {
				if (onAbort) combined.removeEventListener("abort", onAbort);
			}
		},
		reset() {
			if (state === "disposed") throw new Error("Engine session is disposed");
			if (epoch === Number.MAX_SAFE_INTEGER) throw new RangeError("Epoch exhausted");
			const nextOwner = createOwner(epoch + 1);
			controller.abort();
			owner.dispose();
			owner = nextOwner;
			epoch++;
			controller = new AbortController();
			tick = 0;
			sequence = 0;
			committedSequence = 0;
			queue = [];
			state = "ready";
		},
		dispose() {
			if (state === "disposed") return;
			controller.abort();
			owner.dispose();
			queue = [];
			state = "disposed";
		},
	};
}
