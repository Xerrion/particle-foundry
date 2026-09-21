import {
	type Color,
	combustionProfile,
	EMPTY,
	materialsById,
	palettes,
	STONE,
	surfaceAppearance,
} from "./materials";
import { temperatureColor } from "./temperature-map";
import type { VisualWaves } from "./visual-waves";
import type { World } from "./world";

export type PointerState = {
	isInside: boolean;
	x: number;
	y: number;
};

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
	render(ctx: CanvasRenderingContext2D, pointer: PointerState, brushSize: number): void;
	setTemperatureMapEnabled(enabled: boolean): void;
} {
	let image: ImageData | undefined;
	let temperatureMapEnabled = false;

	function render(ctx: CanvasRenderingContext2D, pointer: PointerState, brushSize: number): void {
		image ??= ctx.createImageData(world.width, world.height);
		const pixels = image.data;
		for (let i = 0; i < world.size; i += 1) {
			const x = i % world.width;
			const y = Math.floor(i / world.width);
			const material = waves.materialAt(x, y, world.grid[i]);
			const pixel = i * 4;
			if (temperatureMapEnabled) {
				const color = temperatureColor(world.temperatureAt(i));
				pixels[pixel] = color[0];
				pixels[pixel + 1] = color[1];
				pixels[pixel + 2] = color[2];
				pixels[pixel + 3] = 255;
				continue;
			}
			if (material === EMPTY) {
				const noise = (x * 13 + y * 7) % 19 === 0 ? 1 : 0;
				pixels[pixel] = 5 + noise;
				pixels[pixel + 1] = 12 + noise;
				pixels[pixel + 2] = 17 + noise;
				pixels[pixel + 3] = 255;
				continue;
			}

			const palette = palettes[material] || palettes[STONE];
			// Variation belongs to the parcel and travels with it. Including x
			// re-coloured every sideways step, making flowing lava flash.
			let color = palette[world.variation[i] % palette.length];
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
			color = heatColor(color, world.temperatureAt(i), material);
			pixels[pixel] = color[0];
			pixels[pixel + 1] = color[1];
			pixels[pixel + 2] = color[2];
			pixels[pixel + 3] = 255;
		}
		ctx.putImageData(image, 0, 0);

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
			temperatureMapEnabled = enabled;
		},
	};
}
