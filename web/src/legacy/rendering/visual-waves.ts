import { materialsById, WAVE_MATERIALS, type WaveProfile } from "../../materials";
import { isGas } from "../../materials/queries";
import type { World } from "../simulation/world";

/** Rendering-only surface lighting. Lookups use in-bounds integer coordinates. */
export interface VisualWaves {
	/** Erases detected surfaces and motion without changing the enabled setting or world. */
	reset(): void;
	/** Detects current fluid surfaces and advances visual motion only while enabled. */
	update(): void;
	/**
	 * Adds a tapered visual impulse; unsupported materials and disabled waves are ignored.
	 * Invalid integer coordinates/radius, nonfinite strength or Float32 overflow throw RangeError.
	 */
	disturb(x: number, material: number, strength: number, radius?: number): void;
	/** Disabling erases motion; enabling immediately detects current surfaces. */
	setEnabled(enabled: boolean): void;
	isEnabled(): boolean;
	/** Always preserves actual occupancy, including interfaces between different liquids. */
	materialAt(x: number, y: number, material: number): number;
	/** Identifies an actual gas/liquid surface, not a displaced visual boundary. */
	isSurface(x: number, y: number, material: number): boolean;
	/** Bounded lighting variation on real free surfaces, never virtual occupancy. */
	shimmerAt(x: number, y: number, material: number): number;
}

type WaveMaterial = (typeof WAVE_MATERIALS)[number];

interface WaveState {
	surface: Int16Array;
	nextSurface: Int16Array;
	displacement: Float32Array;
	velocity: Float32Array;
	nextVelocity: Float32Array;
}

const waveProfiles: Readonly<Record<number, WaveProfile>> = Object.fromEntries(
	Object.values(materialsById).flatMap((material) =>
		material.wave ? [[material.id, material.wave]] : [],
	),
);

function isWaveMaterial(material: number): material is WaveMaterial {
	return !!materialsById[material]?.wave;
}

function createWaveState(width: number): WaveState {
	return {
		surface: new Int16Array(width).fill(-1),
		nextSurface: new Int16Array(width).fill(-1),
		displacement: new Float32Array(width),
		velocity: new Float32Array(width),
		nextVelocity: new Float32Array(width),
	};
}

/** Creates independent, enabled visual ripples with no detected surfaces and no world mutations. */
export function createVisualWaves(world: World): VisualWaves {
	const { width, height, grid } = world;
	const states: Record<WaveMaterial, WaveState> = Object.fromEntries(
		WAVE_MATERIALS.map((material) => [material, createWaveState(width)]),
	);
	let hasEnabledWaves = true;

	function reset(): void {
		for (const material of WAVE_MATERIALS) {
			const state = states[material];
			state.surface.fill(-1);
			state.nextSurface.fill(-1);
			state.displacement.fill(0);
			state.velocity.fill(0);
			state.nextVelocity.fill(0);
		}
	}

	function disturb(x: number, material: number, strength: number, radius = 5): void {
		if (!hasEnabledWaves || !isWaveMaterial(material)) return;
		if (
			!Number.isSafeInteger(x) ||
			!Number.isSafeInteger(radius) ||
			radius < 0 ||
			!Number.isFinite(strength)
		) {
			throw new RangeError(
				"Visual impulses require an integer center, a nonnegative integer radius and finite strength",
			);
		}
		const state = states[material];
		const firstX = Math.max(0, x - radius);
		const lastX = Math.min(width - 1, x + radius);
		for (let targetX = firstX; targetX <= lastX; targetX += 1) {
			const falloff = 1 - Math.abs(targetX - x) / (radius + 1);
			const velocity = Math.fround(state.velocity[targetX] + strength * falloff);
			if (!Number.isFinite(velocity)) {
				throw new RangeError("Visual impulse exceeds finite Float32 velocity");
			}
			state.velocity[targetX] = velocity;
		}
	}

	function detectSurfaces(material: WaveMaterial, state: WaveState): void {
		state.nextSurface.fill(-1);
		for (let x = 0; x < width; x += 1) {
			for (let y = 0; y < height; y += 1) {
				const index = x + y * width;
				if (grid[index] !== material || (y > 0 && !isGas(grid[index - width]))) continue;
				const hasSupport =
					(y < height - 1 && grid[index + width] === material) ||
					(x > 0 && grid[index - 1] === material) ||
					(x < width - 1 && grid[index + 1] === material);
				if (hasSupport) {
					state.nextSurface[x] = y;
					break;
				}
			}
		}
	}

	function trackSurfaceChanges(state: WaveState): void {
		for (let x = 0; x < width; x += 1) {
			const previousY = state.surface[x];
			const nextY = state.nextSurface[x];
			if (nextY < 0) {
				state.displacement[x] *= 0.72;
				state.velocity[x] *= 0.72;
				continue;
			}
			if (previousY >= 0) {
				const surfaceShift = Math.max(-3, Math.min(3, previousY - nextY));
				state.velocity[x] -= surfaceShift * 0.22;
			}
		}
		state.surface.set(state.nextSurface);
	}

	function updateVelocities(material: WaveMaterial, state: WaveState): void {
		const profile = waveProfiles[material];
		for (let x = 0; x < width; x += 1) {
			const surfaceY = state.surface[x];
			if (surfaceY < 0) {
				state.nextVelocity[x] = 0;
				continue;
			}
			const current = state.displacement[x];
			const isLeftConnected =
				x > 0 && state.surface[x - 1] >= 0 && Math.abs(state.surface[x - 1] - surfaceY) <= 3;
			const isRightConnected =
				x < width - 1 &&
				state.surface[x + 1] >= 0 &&
				Math.abs(state.surface[x + 1] - surfaceY) <= 3;
			const left = isLeftConnected ? state.displacement[x - 1] : current;
			const right = isRightConnected ? state.displacement[x + 1] : current;
			const acceleration =
				(left + right - current * 2) * profile.tension - current * profile.restoring;
			state.nextVelocity[x] = (state.velocity[x] + acceleration) * profile.damping;
		}
	}

	function updateDisplacements(material: WaveMaterial, state: WaveState): void {
		const { limit } = waveProfiles[material];
		for (let x = 0; x < width; x += 1) {
			if (state.surface[x] < 0) continue;
			state.velocity[x] = state.nextVelocity[x];
			state.displacement[x] = Math.max(
				-limit,
				Math.min(limit, state.displacement[x] + state.velocity[x]),
			);
		}
	}

	function update(): void {
		if (!hasEnabledWaves) return;
		for (const material of WAVE_MATERIALS) {
			const state = states[material];
			detectSurfaces(material, state);
			trackSurfaceChanges(state);
			updateVelocities(material, state);
			updateDisplacements(material, state);
		}
	}

	function materialAt(_x: number, _y: number, material: number): number {
		// Surface displacement used to invent floating crests and cut holes below
		// oil. Rendering must always agree with the transported material grid.
		return material;
	}

	function isSurface(x: number, y: number, material: number): boolean {
		if (!hasEnabledWaves || !isWaveMaterial(material)) return false;
		const state = states[material];
		const baseSurface = state.surface[x];
		return (
			baseSurface >= 0 &&
			y === baseSurface &&
			grid[x + y * width] === material &&
			(y === 0 || isGas(grid[x + (y - 1) * width]))
		);
	}

	function shimmerAt(x: number, y: number, material: number): number {
		if (!isWaveMaterial(material) || !isSurface(x, y, material)) return 0;
		return Math.max(
			-1,
			Math.min(1, states[material].displacement[x] / waveProfiles[material].limit),
		);
	}

	function setEnabled(enabled: boolean): void {
		hasEnabledWaves = enabled;
		if (!enabled) reset();
		else update();
	}

	return {
		reset,
		update,
		disturb,
		setEnabled,
		isEnabled: () => hasEnabledWaves,
		materialAt,
		isSurface,
		shimmerAt,
	};
}
