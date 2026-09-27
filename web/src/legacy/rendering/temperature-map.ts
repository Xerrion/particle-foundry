import type { Color } from "../../materials";

/** Fixed, piecewise scale: warm/cold comparisons do not change as the scene heats. */
export const TEMPERATURE_SCALE: readonly { readonly celsius: number; readonly color: Color }[] = [
	{ celsius: -200, color: [38, 35, 135] },
	{ celsius: 0, color: [42, 130, 225] },
	{ celsius: 22, color: [42, 198, 174] },
	{ celsius: 100, color: [242, 217, 75] },
	{ celsius: 500, color: [247, 132, 39] },
	{ celsius: 1500, color: [219, 48, 67] },
	{ celsius: 3000, color: [255, 237, 226] },
];

export function temperatureColor(celsius: number): Color {
	if (!Number.isFinite(celsius))
		throw new RangeError("Temperature map requires a finite temperature");
	if (celsius <= TEMPERATURE_SCALE[0].celsius) return TEMPERATURE_SCALE[0].color;
	for (let i = 1; i < TEMPERATURE_SCALE.length; i += 1) {
		const upper = TEMPERATURE_SCALE[i];
		if (celsius > upper.celsius) continue;
		const lower = TEMPERATURE_SCALE[i - 1];
		const fraction = (celsius - lower.celsius) / (upper.celsius - lower.celsius);
		return [
			Math.round(lower.color[0] + (upper.color[0] - lower.color[0]) * fraction),
			Math.round(lower.color[1] + (upper.color[1] - lower.color[1]) * fraction),
			Math.round(lower.color[2] + (upper.color[2] - lower.color[2]) * fraction),
		];
	}
	return TEMPERATURE_SCALE[TEMPERATURE_SCALE.length - 1].color;
}
