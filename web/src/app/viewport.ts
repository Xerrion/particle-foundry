/** A camera over a fixed simulation grid. Coordinates are measured in cells. */
export interface Viewport {
	readonly worldWidth: number;
	readonly worldHeight: number;
	readonly zoom: number;
	readonly left: number;
	readonly top: number;
	readonly width: number;
	readonly height: number;
	worldAt(u: number, v: number): { x: number; y: number } | undefined;
	zoomAt(factor: number, u: number, v: number): void;
	panBy(du: number, dv: number): void;
	reset(): void;
}

export const MAX_ZOOM = 8;

export function createViewport(worldWidth: number, worldHeight: number): Viewport {
	if (
		!Number.isSafeInteger(worldWidth) ||
		!Number.isSafeInteger(worldHeight) ||
		worldWidth < 1 ||
		worldHeight < 1
	) {
		throw new RangeError("Viewport dimensions must be positive integers");
	}
	let zoom = 1;
	let centerX = worldWidth / 2;
	let centerY = worldHeight / 2;

	function clampCenter(): void {
		const halfWidth = worldWidth / (2 * zoom);
		const halfHeight = worldHeight / (2 * zoom);
		centerX = Math.max(halfWidth, Math.min(worldWidth - halfWidth, centerX));
		centerY = Math.max(halfHeight, Math.min(worldHeight - halfHeight, centerY));
	}

	return {
		worldWidth,
		worldHeight,
		get zoom() {
			return zoom;
		},
		get left() {
			return centerX - worldWidth / (2 * zoom);
		},
		get top() {
			return centerY - worldHeight / (2 * zoom);
		},
		get width() {
			return worldWidth / zoom;
		},
		get height() {
			return worldHeight / zoom;
		},
		worldAt(u, v) {
			if (!Number.isFinite(u) || !Number.isFinite(v) || u < 0 || u >= 1 || v < 0 || v >= 1)
				return undefined;
			const x = Math.floor(centerX - worldWidth / (2 * zoom) + (u * worldWidth) / zoom);
			const y = Math.floor(centerY - worldHeight / (2 * zoom) + (v * worldHeight) / zoom);
			if (x < 0 || x >= worldWidth || y < 0 || y >= worldHeight) return undefined;
			return { x, y };
		},
		zoomAt(factor, u, v) {
			if (
				!Number.isFinite(factor) ||
				factor <= 0 ||
				!Number.isFinite(u) ||
				!Number.isFinite(v) ||
				u < 0 ||
				u > 1 ||
				v < 0 ||
				v > 1
			) {
				throw new RangeError("Invalid viewport zoom or anchor");
			}
			const nextZoom = Math.max(1, Math.min(MAX_ZOOM, zoom * factor));
			const anchorX = centerX + (u - 0.5) * (worldWidth / zoom);
			const anchorY = centerY + (v - 0.5) * (worldHeight / zoom);
			zoom = nextZoom;
			centerX = anchorX + (0.5 - u) * (worldWidth / zoom);
			centerY = anchorY + (0.5 - v) * (worldHeight / zoom);
			clampCenter();
		},
		panBy(du, dv) {
			if (!Number.isFinite(du) || !Number.isFinite(dv))
				throw new RangeError("Invalid viewport pan");
			centerX -= (du * worldWidth) / zoom;
			centerY -= (dv * worldHeight) / zoom;
			clampCenter();
		},
		reset() {
			zoom = 1;
			centerX = worldWidth / 2;
			centerY = worldHeight / 2;
		},
	};
}
