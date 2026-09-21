import { PRESSURE_SCALE, VELOCITY_SCALE } from "../rendering/field-map";
import type { ViewMode } from "../rendering/renderer";
import { TEMPERATURE_SCALE } from "../rendering/temperature-map";
import { requiredElement } from "./dom";

type FieldMode = Exclude<ViewMode, "materials">;
type LegendStop = {
	readonly position: number;
	readonly label: string;
	readonly color: readonly number[];
};

const fields: readonly {
	readonly mode: FieldMode;
	readonly shortcut: string;
	readonly toggle: string;
	readonly legend: string;
	readonly title: string;
	readonly stops: readonly LegendStop[];
	readonly labels: string;
}[] = [
	{
		mode: "temperature",
		shortcut: "KeyT",
		toggle: "#temperatureToggle",
		legend: "#temperatureLegend",
		title: "Temperature °C · fixed, nonlinear scale · includes air",
		stops: TEMPERATURE_SCALE.map((stop, index) => ({
			position: index / (TEMPERATURE_SCALE.length - 1),
			label: String(stop.celsius),
			color: stop.color,
		})),
		labels: "temperature-labels",
	},
	{
		mode: "pressure",
		shortcut: "KeyP",
		toggle: "#pressureToggle",
		legend: "#pressureLegend",
		title: "Gauge pressure kPa · solids excluded",
		stops: PRESSURE_SCALE.map((stop) => ({
			position:
				(stop.gaugeKPa - PRESSURE_SCALE[0].gaugeKPa) /
				(PRESSURE_SCALE[PRESSURE_SCALE.length - 1].gaugeKPa - PRESSURE_SCALE[0].gaugeKPa),
			label: String(stop.gaugeKPa),
			color: stop.color,
		})),
		labels: "field-labels",
	},
	{
		mode: "velocity",
		shortcut: "KeyV",
		toggle: "#velocityToggle",
		legend: "#velocityLegend",
		title: "Speed m/s · arrows show air and gas direction",
		stops: VELOCITY_SCALE.map((stop) => ({
			position: stop.metersPerSecond / VELOCITY_SCALE[VELOCITY_SCALE.length - 1].metersPerSecond,
			label: stop.metersPerSecond === 0.25 ? "" : String(stop.metersPerSecond),
			color: stop.color,
		})),
		labels: "field-labels",
	},
];

function renderLegend(
	element: HTMLElement,
	title: string,
	stops: readonly LegendStop[],
	labelsClass: string,
): void {
	const heading = document.createElement("div");
	heading.textContent = title;
	const scale = document.createElement("div");
	scale.className = "temperature-scale";
	scale.style.background = `linear-gradient(to right, ${stops.map((stop) => `rgb(${stop.color.join(",")}) ${stop.position * 100}%`).join(",")})`;
	const labels = document.createElement("div");
	labels.className = labelsClass;
	for (const [index, stop] of stops.entries()) {
		if (!stop.label) continue;
		const label = document.createElement("span");
		label.textContent = stop.label;
		if (labelsClass === "field-labels") {
			label.style.left = `${stop.position * 100}%`;
			if (index === 0) label.className = "first";
			if (index === stops.length - 1) label.className = "last";
		}
		labels.append(label);
	}
	element.replaceChildren(heading, scale, labels);
}

/** Keeps field-map toggles, legends, shortcuts and their announcement consistent. */
export function bindViewControls(onChange: (mode: ViewMode) => void): {
	selectShortcut(code: string): boolean;
} {
	const canvasFrame = requiredElement<HTMLElement>("#canvasFrame");
	const liveRegion = requiredElement<HTMLElement>("#liveRegion");
	const controls = fields.map((field) => {
		const toggle = requiredElement<HTMLInputElement>(field.toggle);
		const legend = requiredElement<HTMLElement>(field.legend);
		renderLegend(legend, field.title, field.stops, field.labels);
		return { ...field, toggle, legend };
	});

	let viewMode: ViewMode = "materials";
	function setViewMode(mode: ViewMode): void {
		viewMode = mode;
		onChange(mode);
		for (const field of controls) {
			field.toggle.checked = mode === field.mode;
			field.legend.hidden = mode !== field.mode;
		}
		canvasFrame.classList.toggle("temperature-map", mode !== "materials");
		liveRegion.textContent =
			mode === "materials"
				? "Material colours enabled"
				: `${mode[0].toUpperCase()}${mode.slice(1)} map enabled`;
	}
	function toggle(mode: FieldMode): void {
		setViewMode(viewMode === mode ? "materials" : mode);
	}
	for (const field of controls) {
		field.toggle.addEventListener("change", () =>
			setViewMode(field.toggle.checked ? field.mode : "materials"),
		);
	}
	return {
		selectShortcut: (code) => {
			const field = controls.find((entry) => entry.shortcut === code);
			if (!field) return false;
			toggle(field.mode);
			return true;
		},
	};
}
