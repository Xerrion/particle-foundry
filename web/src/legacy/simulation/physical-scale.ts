/** Authoritative model scale. Rendering pixels have no additional physical meaning. */
export const CELL_WIDTH_METERS = 0.01;
export const REPRESENTED_DEPTH_METERS = 0.01;
export const CELL_VOLUME_M3 = CELL_WIDTH_METERS ** 2 * REPRESENTED_DEPTH_METERS;
export const FIXED_TIME_STEP_SECONDS = 1 / 60;
export const GRAVITY_M_PER_S2 = 9.80665;
export const AMBIENT_PRESSURE_PA = 101_325;
export const AMBIENT_TEMPERATURE_C = 22;
export const IDEAL_GAS_CONSTANT = 8.314462618;
export const CONSERVATION_ABSOLUTE_TOLERANCE = 1e-9;
export const CONSERVATION_RELATIVE_TOLERANCE = 1e-10;

export function conservationTolerance(reference: number): number {
	return Math.max(
		CONSERVATION_ABSOLUTE_TOLERANCE,
		Math.abs(reference) * CONSERVATION_RELATIVE_TOLERANCE,
	);
}
