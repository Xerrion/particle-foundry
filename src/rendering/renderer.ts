import {
	type Color,
	combustionProfile,
	EMPTY,
	materialsById,
	palettes,
	STONE,
	surfaceAppearance,
} from "../materials";
import { isGas, isLiquid } from "../materials/queries";
import { AMBIENT_PRESSURE_PA } from "../simulation/physical-scale";
import type { World } from "../simulation/world";
import { pressureColor, velocityColor } from "./field-map";
import { temperatureColor } from "./temperature-map";
import type { VisualWaves } from "./visual-waves";

export type PointerState = {
	isInside: boolean;
	x: number;
	y: number;
};

export type ViewMode = "materials" | "temperature" | "pressure" | "velocity";
export type VisibleWorldRect = { left: number; top: number; width: number; height: number };

const MAX_OVERLAY_OPACITY = 0.6;

function blendOverlay(base: Color, tint: Color, opacity: number): Color {
	return [
		Math.round(base[0] + (tint[0] - base[0]) * opacity),
		Math.round(base[1] + (tint[1] - base[1]) * opacity),
		Math.round(base[2] + (tint[2] - base[2]) * opacity),
	];
}

/** Short direction markers sample the simulated gas velocity, including invisible air. */
function drawWindVectors(world: World, pixels: Uint8ClampedArray): void {
	function mark(x: number, y: number): void {
		const column = Math.round(x);
		const row = Math.round(y);
		if (column < 0 || column >= world.width || row < 0 || row >= world.height) return;
		const index = column + row * world.width;
		if (!isGas(world.grid[index])) return;
		const pixel = index * 4;
		const color = blendOverlay(
			[pixels[pixel], pixels[pixel + 1], pixels[pixel + 2]],
			[214, 246, 255],
			0.75,
		);
		pixels[pixel] = color[0];
		pixels[pixel + 1] = color[1];
		pixels[pixel + 2] = color[2];
	}

	function line(fromX: number, fromY: number, toX: number, toY: number): void {
		const steps = Math.ceil(Math.hypot(toX - fromX, toY - fromY) * 2);
		for (let step = 0; step <= steps; step += 1) {
			const fraction = step / steps;
			mark(fromX + (toX - fromX) * fraction, fromY + (toY - fromY) * fraction);
		}
	}

	for (let y = 3; y < world.height; y += 6) {
		for (let x = 3; x < world.width; x += 6) {
			const index = x + y * world.width;
			if (!isGas(world.grid[index])) continue;
			const vx = world.velocityX[index];
			const vy = world.velocityY[index];
			const speed = Math.hypot(vx, vy);
			if (speed < 0.15) continue;
			const ux = vx / speed;
			const uy = vy / speed;
			const length = Math.min(4, 2 + speed * 0.4);
			const endX = x + (ux * length) / 2;
			const endY = y + (uy * length) / 2;
			line(x - (ux * length) / 2, y - (uy * length) / 2, endX, endY);
			line(endX, endY, endX - ux + uy * 0.7, endY - uy - ux * 0.7);
			line(endX, endY, endX - ux - uy * 0.7, endY - uy + ux * 0.7);
		}
	}
}

function heatColor(base: Color, temperature: number, material: number): Color {
	const profile = materialsById[material]?.heatGlow;
	if (!profile || temperature < profile.startsC) {
		return base;
	}
	const glow = Math.min(1, (temperature - profile.startsC) / (profile.fullC - profile.startsC));
	const target: Color =
		temperature > profile.brightAboveC ? profile.brightColor : profile.warmColor;
	return [
		Math.round(base[0] + (target[0] - base[0]) * glow),
		Math.round(base[1] + (target[1] - base[1]) * glow),
		Math.round(base[2] + (target[2] - base[2]) * glow),
	];
}

/**
 * Draws a trusted world and its visual waves without changing either.
 * No browser APIs are used until render creates its reusable image through ctx.
 * Canvas and world temperature errors propagate to the caller.
 */
export function createRenderer(
	world: World,
	waves: VisualWaves,
): {
	render(
		ctx: CanvasRenderingContext2D,
		pointer: PointerState,
		brushSize: number,
		visible?: VisibleWorldRect,
	): void;
	setTemperatureMapEnabled(enabled: boolean): void;
	setViewMode(mode: ViewMode): void;
} {
	let image: ImageData | undefined;
	let viewMode: ViewMode = "materials";

	function render(
		ctx: CanvasRenderingContext2D,
		pointer: PointerState,
		brushSize: number,
		visible?: VisibleWorldRect,
	): void {
		image ??= ctx.createImageData(world.width, world.height);
		const pixels = image.data;
		const left = visible ? Math.max(0, Math.floor(visible.left)) : 0;
		const top = visible ? Math.max(0, Math.floor(visible.top)) : 0;
		const right = visible
			? Math.min(world.width, Math.ceil(visible.left + visible.width))
			: world.width;
		const bottom = visible
			? Math.min(world.height, Math.ceil(visible.top + visible.height))
			: world.height;
		for (let i = 0; i < world.size; i += 1) {
			const x = i % world.width;
			const y = Math.floor(i / world.width);
			if (x < left || x >= right || y < top || y >= bottom) continue;
			const material = waves.materialAt(x, y, world.grid[i]);
			const pixel = i * 4;
			let color: Color;
			let temperature: number | undefined;
			if (material === EMPTY) {
				const noise = (x * 13 + y * 7) % 19 === 0 ? 1 : 0;
				color = [5 + noise, 12 + noise, 17 + noise];
			} else {
				const palette = palettes[material] || palettes[STONE];
				// Variation belongs to the parcel and travels with it. Including x
				// re-coloured every sideways step, making flowing lava flash.
				color = palette[world.variation[i] % palette.length];
				if (world.burning[i]) {
					color =
						combustionProfile.emberPalette[
							world.variation[i] % combustionProfile.emberPalette.length
						];
				}
				if (waves.isSurface(x, y, material)) {
					const shimmer = Math.round(
						waves.shimmerAt(x, y, material) * surfaceAppearance.shimmerAmplitude,
					);
					color = [
						Math.min(255, color[0] + surfaceAppearance.highlight[0] + shimmer),
						Math.min(255, color[1] + surfaceAppearance.highlight[1] + shimmer),
						Math.min(255, color[2] + surfaceAppearance.highlight[2] + shimmer),
					];
				}
				temperature = world.temperatureAt(i);
				color = heatColor(color, temperature, material);
			}

			if (viewMode === "temperature") {
				temperature ??= world.temperatureAt(i);
				const opacity = Math.min(
					MAX_OVERLAY_OPACITY,
					(Math.abs(temperature - 22) / 80) * MAX_OVERLAY_OPACITY,
				);
				color = blendOverlay(color, temperatureColor(temperature), opacity);
			} else if (viewMode === "pressure" && (isGas(world.grid[i]) || isLiquid(world.grid[i]))) {
				const pressure = world.pressurePa[i];
				const opacity = Math.min(
					MAX_OVERLAY_OPACITY,
					(Math.abs(pressure - AMBIENT_PRESSURE_PA) / 10_000) * MAX_OVERLAY_OPACITY,
				);
				color = blendOverlay(color, pressureColor(pressure), opacity);
			} else if (viewMode === "velocity") {
				const vx = world.velocityX[i];
				const vy = world.velocityY[i];
				const opacity = Math.min(MAX_OVERLAY_OPACITY, Math.hypot(vx, vy) * MAX_OVERLAY_OPACITY);
				color = blendOverlay(color, velocityColor(vx, vy), opacity);
			}
			pixels[pixel] = color[0];
			pixels[pixel + 1] = color[1];
			pixels[pixel + 2] = color[2];
			pixels[pixel + 3] = 255;
		}
		if (viewMode === "velocity") drawWindVectors(world, pixels);
		if (visible) ctx.putImageData(image, 0, 0, left, top, right - left, bottom - top);
		else ctx.putImageData(image, 0, 0);

		if (pointer.isInside && pointer.x >= 0) {
			ctx.save();
			ctx.strokeStyle = "rgba(255,255,255,0.78)";
			ctx.lineWidth = 0.7;
			ctx.beginPath();
			ctx.arc(pointer.x + 0.5, pointer.y + 0.5, brushSize + 0.5, 0, Math.PI * 2);
			ctx.stroke();
			ctx.restore();
		}
	}

	return {
		render,
		setTemperatureMapEnabled: (enabled) => {
			viewMode = enabled ? "temperature" : "materials";
		},
		setViewMode: (mode) => {
			viewMode = mode;
		},
	};
}
