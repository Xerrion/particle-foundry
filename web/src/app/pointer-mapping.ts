import type { PointerState } from "../engine-client";

/**
 * Maps a viewport point through a centered canvas image into integer grid cells.
 * Supports fill and contain sizing. Letterboxes, outer edges and unmeasurable
 * geometry return an outside pointer, never a clamped edge cell.
 */
export function mapCanvasPointer(
	point: { x: number; y: number },
	geometry: {
		left: number;
		top: number;
		width: number;
		height: number;
		columns: number;
		rows: number;
		fit: "fill" | "contain";
	},
): PointerState {
	const { left, top, width, height, columns, rows, fit } = geometry;
	if (
		![point.x, point.y, left, top, width, height, columns, rows].every(Number.isFinite) ||
		width <= 0 ||
		height <= 0 ||
		!Number.isInteger(columns) ||
		!Number.isInteger(rows) ||
		columns <= 0 ||
		rows <= 0
	)
		return { isInside: false, x: -1, y: -1 };
	const scale = Math.min(width / columns, height / rows);
	const contentWidth = fit === "contain" ? columns * scale : width;
	const contentHeight = fit === "contain" ? rows * scale : height;
	const contentLeft = left + (width - contentWidth) / 2;
	const contentTop = top + (height - contentHeight) / 2;
	const x = Math.floor(((point.x - contentLeft) / contentWidth) * columns);
	const y = Math.floor(((point.y - contentTop) / contentHeight) * rows);
	if (x < 0 || x >= columns || y < 0 || y >= rows) {
		return { isInside: false, x: -1, y: -1 };
	}
	return { isInside: true, x, y };
}
