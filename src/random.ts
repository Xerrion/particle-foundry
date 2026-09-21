export interface RandomSource {
	next(): number;
	reset(): void;
}

/** Deterministic Mulberry32 stream. Seed 0 is valid and replayable. */
export function createSeededRandom(seed = 0x5eed1234): RandomSource {
	if (!Number.isSafeInteger(seed)) throw new RangeError("Random seed must be a safe integer");
	const initial = seed >>> 0;
	let state = initial;
	function next(): number {
		state = (state + 0x6d2b79f5) >>> 0;
		let value = state;
		value = Math.imul(value ^ (value >>> 15), value | 1);
		value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
		return ((value ^ (value >>> 14)) >>> 0) / 0x100000000;
	}
	return {
		next,
		reset: () => {
			state = initial;
		},
	};
}
