import type { BrowserGpuSceneSession } from "../../src/engine-client/wasm";

type Event = {
	sequence: number;
	type: "candidateBegin" | "pressureStatus" | "candidateOutcome";
	epoch: number;
	outerTick: number;
	candidateId: number;
	attempt: number;
	stateRevision: number;
	acceptedTimeBeforeS: number;
	encodedDtS: number;
	timestampMs?: number;
	phase?: "initialization" | "pcgBatch" | "finalGate";
	batchIndex?: number;
	requestedIterations?: number;
	mapStartedMs?: number;
	mapCompletedMs?: number;
	statusWords?: number[];
	outcome?: "accepted" | "numericalRejection" | "terminalRejection";
};

export function checkEmptyPressureJournal(scene: BrowserGpuSceneSession): void {
	const journal = JSON.parse(scene.take_pressure_diagnostics_json());
	if (
		journal.version !== 1 ||
		journal.clockDomain !== "browser-performance-now" ||
		journal.capacity !== 0 ||
		journal.droppedEvents !== 0 ||
		journal.events.length !== 0
	) {
		throw new Error("Disabled GPU pressure journal retained diagnostic data");
	}
}

export function checkBusyPressureJournal(scene: BrowserGpuSceneSession): void {
	for (const action of [
		() => scene.enable_pressure_diagnostics(256),
		() => scene.disable_pressure_diagnostics(),
		() => scene.take_pressure_diagnostics_json(),
	]) {
		let rejected = false;
		try {
			action();
		} catch (error) {
			rejected = String(error).includes("busy");
		}
		if (!rejected) throw new Error("Busy GPU scene accepted a pressure journal operation");
	}
}

export function checkAcceptedPressureJournal(
	scene: BrowserGpuSceneSession,
	accounting: {
		epoch: number;
		tick: number;
		acceptedSubsteps: number;
		attemptedCandidates: number;
		refinementRetries: number;
		readbackBytes: number;
		rejectedReadbackBytes: number;
	},
): void {
	const journal = JSON.parse(scene.take_pressure_diagnostics_json());
	if (
		journal.version !== 1 ||
		journal.clockDomain !== "browser-performance-now" ||
		journal.capacity !== 8192 ||
		journal.droppedEvents !== 0
	) {
		throw new Error("GPU pressure journal returned the wrong version, clock or capacity");
	}
	const events = journal.events as Event[];
	const begins = events.filter((event) => event.type === "candidateBegin");
	const outcomes = events.filter((event) => event.type === "candidateOutcome");
	const statuses = events.filter((event) => event.type === "pressureStatus");
	if (
		begins.length !== accounting.attemptedCandidates ||
		outcomes.filter((event) => event.outcome === "accepted").length !==
			accounting.acceptedSubsteps ||
		outcomes.filter((event) => event.outcome === "numericalRejection").length !==
			accounting.refinementRetries ||
		outcomes.length !== begins.length ||
		statuses.length * 32 !==
			accounting.readbackBytes +
				accounting.rejectedReadbackBytes -
				accounting.acceptedSubsteps * 8 -
				accounting.attemptedCandidates * 20
	) {
		throw new Error("GPU pressure journal disagreed with candidate and readback accounting");
	}
	let previousSequence = 0;
	for (const event of events) {
		if (
			!Number.isSafeInteger(event.sequence) ||
			event.sequence <= previousSequence ||
			event.epoch !== accounting.epoch ||
			event.outerTick !== accounting.tick ||
			!Number.isSafeInteger(event.candidateId) ||
			event.candidateId < 1 ||
			!Number.isSafeInteger(event.attempt) ||
			event.attempt < 1 ||
			event.attempt > 5 ||
			!Number.isFinite(event.acceptedTimeBeforeS) ||
			!Number.isFinite(event.encodedDtS) ||
			event.encodedDtS <= 0
		) {
			throw new Error("GPU pressure journal returned an invalid candidate identity");
		}
		previousSequence = event.sequence;
		if (event.type === "pressureStatus") {
			if (
				!Number.isFinite(event.mapStartedMs) ||
				!Number.isFinite(event.mapCompletedMs) ||
				(event.mapCompletedMs as number) < (event.mapStartedMs as number) ||
				!event.statusWords ||
				event.statusWords.length !== 8 ||
				event.statusWords.some((word) => !Number.isInteger(word) || word < 0 || word > 0xffff_ffff)
			) {
				throw new Error("GPU pressure journal returned invalid map times or raw status words");
			}
		} else if (!Number.isFinite(event.timestampMs)) {
			throw new Error("GPU pressure journal returned an invalid candidate timestamp");
		}
	}
	const drained = JSON.parse(scene.take_pressure_diagnostics_json());
	if (drained.events.length !== 0 || drained.droppedEvents !== 0 || drained.capacity !== 8192) {
		throw new Error("Draining the GPU pressure journal did not retain its recording configuration");
	}
	scene.disable_pressure_diagnostics();
	checkEmptyPressureJournal(scene);
}
