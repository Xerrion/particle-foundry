export interface SimulationClock {
	/**
	 * Returns whole 60 Hz steps, retaining fractional steps across calls and speeds.
	 * Elapsed time must be finite and nonnegative; speed must be 0.5, 1, 2 or 4.
	 * Invalid inputs throw RangeError without changing state, even when paused.
	 * Pausing discards elapsed time and the retained fraction.
	 * Only 100 ms per call is accepted (at most 24 steps at 4x); excess time is
	 * discarded so overload slows simulation instead of building a catch-up backlog.
	 */
	advance(elapsedMilliseconds: number, options: { speed: number; isPaused: boolean }): number;
	/** Discards all retained time; the next advance starts a fresh interval. */
	reset(): void;
}

const STEPS_PER_SECOND = 60;
const MAX_ELAPSED_MILLISECONDS = 100;
const MAX_STEPS_PER_ADVANCE = 24;
const STEP_ROUNDING_TOLERANCE = 1e-9;

/** Creates an independent clock with no browser, wall-clock or scheduling side effects. */
export function createSimulationClock(): SimulationClock {
	let pendingSteps = 0;

	return {
		advance(elapsedMilliseconds, { speed, isPaused }): number {
			if (!Number.isFinite(elapsedMilliseconds) || elapsedMilliseconds < 0) {
				throw new RangeError("Elapsed milliseconds must be finite and nonnegative");
			}
			if (speed !== 0.5 && speed !== 1 && speed !== 2 && speed !== 4) {
				throw new RangeError("Simulation speed must be 0.5, 1, 2 or 4");
			}
			if (isPaused) {
				pendingSteps = 0;
				return 0;
			}
			if (elapsedMilliseconds === 0) return 0;

			pendingSteps +=
				(Math.min(elapsedMilliseconds, MAX_ELAPSED_MILLISECONDS) * speed * STEPS_PER_SECOND) / 1000;
			// Fractional frame intervals can land just below an integer step.
			// Keep any tiny rounding debt so repeated calls cannot manufacture time.
			const steps = Math.max(
				0,
				Math.min(MAX_STEPS_PER_ADVANCE, Math.floor(pendingSteps + STEP_ROUNDING_TOLERANCE)),
			);
			pendingSteps -= steps;
			return steps;
		},
		reset(): void {
			pendingSteps = 0;
		},
	};
}
