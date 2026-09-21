import type { Color } from "./materials";
import { AMBIENT_PRESSURE_PA } from "./physical-scale";

export const PRESSURE_SCALE: readonly { readonly gaugeKPa: number; readonly color: Color }[] = [
	{ gaugeKPa: -20, color: [71, 83, 188] },
	{ gaugeKPa: 0, color: [36, 137, 155] },
	{ gaugeKPa: 10, color: [225, 196, 73] },
	{ gaugeKPa: 50, color: [231, 98, 61] },
	{ gaugeKPa: 100, color: [247, 224, 205] },
];

export const VELOCITY_SCALE: readonly {
	readonly metersPerSecond: number;
	readonly color: Color;
}[] = [
	{ metersPerSecond: 0, color: [19, 35, 45] },
	{ metersPerSecond: 0.25, color: [45, 118, 173] },
	{ metersPerSecond: 1, color: [80, 214, 201] },
	{ metersPerSecond: 3, color: [246, 209, 82] },
	{ metersPerSecond: 6, color: [240, 93, 67] },
];

const pressureStops = PRESSURE_SCALE.map((stop) => ({ value: stop.gaugeKPa, color: stop.color }));
const velocityStops = VELOCITY_SCALE.map((stop) => ({
	value: stop.metersPerSecond,
	color: stop.color,
}));

function colorAt(
	value: number,
	stops: readonly { readonly value: number; readonly color: Color }[],
): Color {
	if (!Number.isFinite(value)) throw new RangeError("Field map requires a finite value");
	if (value <= stops[0].value) return stops[0].color;
	for (let i = 1; i < stops.length; i += 1) {
		const upper = stops[i];
		if (value > upper.value) continue;
		const lower = stops[i - 1];
		const fraction = (value - lower.value) / (upper.value - lower.value);
		return [
			Math.round(lower.color[0] + (upper.color[0] - lower.color[0]) * fraction),
			Math.round(lower.color[1] + (upper.color[1] - lower.color[1]) * fraction),
			Math.round(lower.color[2] + (upper.color[2] - lower.color[2]) * fraction),
		];
	}
	return stops[stops.length - 1].color;
}

export function pressureColor(pressurePa: number): Color {
	return colorAt((pressurePa - AMBIENT_PRESSURE_PA) / 1000, pressureStops);
}

export function velocityColor(velocityX: number, velocityY: number): Color {
	return colorAt(Math.hypot(velocityX, velocityY), velocityStops);
}
