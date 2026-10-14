const CELLS = 480 * 270;
export const FIELD_BYTES = (5 * CELLS + 481 * 270 + 480 * 271) * 4;
export type Advance = {
	epoch: number;
	tick: number;
	acceptedTimeS: number;
	stateRevision: number;
	completedOuterTick: boolean;
	acceptedSubsteps: number;
	attemptedCandidates: number;
	refinementRetries: number;
	readbackBytes: number;
	rejectedReadbackBytes: number;
	pressureIterations: number | null;
	scaledResidual: number | null;
	scaledDivergence: number | null;
};
export type CommittedStamp = Pick<Advance, "epoch" | "tick" | "acceptedTimeS" | "stateRevision">;
function inventory(bytes: Uint8Array, expected: CommittedStamp) {
	const fields = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	if (
		bytes.byteLength !== 108 + FIELD_BYTES ||
		String.fromCharCode(...bytes.slice(0, 4)) !== "PFCP" ||
		fields.getUint32(4, true) !== 0 ||
		fields.getUint32(8, true) !== 480 ||
		fields.getUint32(12, true) !== 270 ||
		fields.getBigUint64(16, true) !== BigInt(expected.epoch) ||
		fields.getBigUint64(24, true) !== BigInt(expected.tick) ||
		fields.getFloat64(32, true) !== expected.acceptedTimeS ||
		fields.getFloat64(40, true) !== 0 ||
		fields.getBigUint64(52, true) !== BigInt(expected.stateRevision) ||
		fields.getBigUint64(60, true) !== BigInt(FIELD_BYTES)
	) {
		throw new Error("GPU benchmark checkpoint has an invalid layout or committed stamp");
	}
	const totals = [0, 0, 0, 0];
	let maxRelativeVolumeError = 0;
	let maxAbsoluteVelocityMPerS = 0;
	for (let cell = 0; cell < CELLS; cell += 1) {
		const values = totals.map((_, field) =>
			fields.getFloat32(108 + 4 * (field * CELLS + cell), true),
		);
		if (
			values.some((value) => !Number.isFinite(value) || value < 0) ||
			(values[0] === 0 && values[2] !== 0) ||
			(values[1] === 0 && values[3] !== 0)
		) {
			throw new Error(`GPU benchmark has invalid inventory in cell ${cell}`);
		}
		for (let field = 0; field < 4; field += 1) totals[field] += values[field];
		const density = fields.getFloat32(108 + 4 * (4 * CELLS + cell), true);
		if (!Number.isFinite(density) || density <= 0) {
			throw new Error(`GPU benchmark has invalid density in cell ${cell}`);
		}
		if (values[0] + values[1] > 0) {
			const volume = values[0] / 1000 + values[1] / 1.2;
			maxRelativeVolumeError = Math.max(maxRelativeVolumeError, Math.abs(volume / 1e-6 - 1));
		}
	}
	for (let offset = 108 + 4 * 5 * CELLS; offset < bytes.byteLength; offset += 4) {
		const velocity = fields.getFloat32(offset, true);
		if (!Number.isFinite(velocity)) throw new Error("GPU benchmark has nonfinite face velocity");
		maxAbsoluteVelocityMPerS = Math.max(maxAbsoluteVelocityMPerS, Math.abs(velocity));
	}
	// The f64 diagnostic division also exposes the f32 cell-volume rounding.
	if (maxRelativeVolumeError > 1.01e-5)
		throw new Error("GPU benchmark cell volume exceeds its gate");
	return { totals, maxRelativeVolumeError, maxAbsoluteVelocityMPerS };
}

export async function checkedFields(bytes: Uint8Array, expected: CommittedStamp) {
	const checked = inventory(bytes, expected);
	const digest = await crypto.subtle.digest("SHA-256", bytes.slice(108));
	const fieldSha256 = Array.from(new Uint8Array(digest), (byte) =>
		byte.toString(16).padStart(2, "0"),
	).join("");
	return { ...checked, fieldSha256 };
}
