import type { PhysicalTotals } from "./diagnostics";
import {
	materialDefinitions,
	materialNames,
	phaseFamilies,
	pickerIds,
	SAND,
	selectableMaterials,
	toolDefinitions,
} from "./materials";
import type { PointerState, Sandbox } from "./sandbox";
import { TEMPERATURE_SCALE } from "./temperature-map";

function requiredElement<T extends Element>(selector: string): T {
	const element = document.querySelector<T>(selector);
	if (!element) throw new Error(`Required element not found: ${selector}`);
	return element;
}

export interface Controls {
	getPointer(): PointerState;
	getSpeed(): number;
	isPaused(): boolean;
	/** Applies any held brush once per running simulation step. */
	paintHeldTool(): void;
	/** Refreshes the cell readout, or clears it when the pointer is outside. */
	updateProbe(): void;
	updateStats(particleTotal: number, framesPerSecond: number, diagnostics?: PhysicalTotals): void;
}

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

export function bindControls(canvas: HTMLCanvasElement, sandbox: Sandbox): Controls {
	const pauseButton = requiredElement<HTMLButtonElement>("#pauseButton");
	const stateText = requiredElement<HTMLElement>("#stateText");
	const stateLight = requiredElement<HTMLElement>("#stateLight");
	const particleCount = requiredElement<HTMLElement>("#particleCount");
	const fpsCounter = requiredElement<HTMLElement>("#fpsCounter");
	const totalMass = requiredElement<HTMLElement>("#totalMass");
	const totalEnergy = requiredElement<HTMLElement>("#totalEnergy");
	const maxPressure = requiredElement<HTMLElement>("#maxPressure");
	const brushInput = requiredElement<HTMLInputElement>("#brushSize");
	const brushOutput = requiredElement<HTMLOutputElement>("#brushOutput");
	const speedSelect = requiredElement<HTMLSelectElement>("#speedSelect");
	const selectedMaterialLabel = requiredElement<HTMLElement>("#selectedMaterialLabel");
	const liveRegion = requiredElement<HTMLElement>("#liveRegion");
	const canvasHint = requiredElement<HTMLElement>("#canvasHint");
	const canvasFrame = requiredElement<HTMLElement>("#canvasFrame");
	const probeMaterial = requiredElement<HTMLElement>("#probeMaterial");
	const probeTemperature = requiredElement<HTMLElement>("#probeTemperature");
	const waveToggle = requiredElement<HTMLInputElement>("#waveToggle");
	const temperatureToggle = requiredElement<HTMLInputElement>("#temperatureToggle");
	const temperatureLegend = requiredElement<HTMLElement>("#temperatureLegend");
	const materialGrid = requiredElement<HTMLElement>("#materialGrid");
	const clearButton = requiredElement<HTMLButtonElement>("#clearButton");
	const resetButton = requiredElement<HTMLButtonElement>("#resetButton");
	const fullscreenButton = requiredElement<HTMLButtonElement>("#fullscreenButton");
	const phaseNotes = requiredElement<HTMLElement>("#phaseNotes");
	phaseNotes.replaceChildren();
	for (const family of Object.values(phaseFamilies)) {
		for (let i = 0; i < family.transitions.length; i += 1) {
			const transition = family.transitions[i];
			const note = document.createElement("p");
			note.textContent = `${materialNames[family.phases[i]]} ↔ ${materialNames[family.phases[i + 1]]} at ${transition.temperature} °C.`;
			phaseNotes.append(note);
		}
	}
	for (const material of Object.values(materialDefinitions)) {
		if (!material.transforms) continue;
		const note = document.createElement("p");
		note.textContent = `${material.name} → ${materialNames[material.transforms.target]} at ${material.transforms.temperature} °C.`;
		phaseNotes.append(note);
	}

	// The picker, names, shortcuts and swatches come from the same registry
	// as the simulation. A new selectable material needs no HTML/CSS entry.
	materialGrid.replaceChildren();
	const pickerEntries = [...selectableMaterials, ...toolDefinitions];
	for (const entry of pickerEntries) {
		const button = document.createElement("button");
		button.type = "button";
		button.className = entry.id === SAND ? "material active" : "material";
		button.dataset.material = entry.key;
		button.setAttribute("aria-pressed", String(entry.id === SAND));
		button.title = entry.name;
		const swatch = document.createElement("span");
		swatch.className = "swatch";
		const colors = entry.palette.map((color) => `rgb(${color.join(",")})`);
		swatch.style.background = `linear-gradient(135deg, ${colors[0]} 0 50%, ${colors[1] ?? colors[0]} 50%)`;
		const label = document.createElement("span");
		label.textContent = entry.name;
		button.append(swatch, label);
		if (entry.shortcut) {
			const shortcut = document.createElement("kbd");
			shortcut.textContent = entry.shortcut;
			button.setAttribute("aria-keyshortcuts", entry.shortcut);
			button.append(shortcut);
		}
		materialGrid.append(button);
	}
	const legendScale = document.createElement("div");
	legendScale.className = "temperature-scale";
	legendScale.style.background = `linear-gradient(to right, ${TEMPERATURE_SCALE.map((stop, i) => `rgb(${stop.color.join(",")}) ${(100 * i) / (TEMPERATURE_SCALE.length - 1)}%`).join(",")})`;
	const legendLabels = document.createElement("div");
	legendLabels.className = "temperature-labels";
	for (const stop of TEMPERATURE_SCALE) {
		const label = document.createElement("span");
		label.textContent = String(stop.celsius);
		legendLabels.append(label);
	}
	const legendTitle = document.createElement("div");
	legendTitle.textContent = "Temperature °C · fixed, nonlinear scale · includes air";
	temperatureLegend.replaceChildren(legendTitle, legendScale, legendLabels);

	function setTemperatureMap(enabled: boolean): void {
		temperatureToggle.checked = enabled;
		sandbox.setTemperatureMapEnabled(enabled);
		temperatureLegend.hidden = !enabled;
		canvasFrame.classList.toggle("temperature-map", enabled);
		liveRegion.textContent = enabled ? "Temperature map enabled" : "Material colours enabled";
	}
	temperatureToggle.addEventListener("change", () => setTemperatureMap(temperatureToggle.checked));

	let paused = false;
	let drawing = false;
	let pointer: PointerState = { isInside: false, x: -1, y: -1 };
	let lastPaintX = -1;
	let lastPaintY = -1;
	let hasInteracted = false;

	function pointerToGrid(event: PointerEvent): PointerState {
		const rect = canvas.getBoundingClientRect();
		return mapCanvasPointer(
			{ x: event.clientX, y: event.clientY },
			{
				left: rect.left,
				top: rect.top,
				width: rect.width,
				height: rect.height,
				columns: canvas.width,
				rows: canvas.height,
				fit: getComputedStyle(canvas).objectFit === "contain" ? "contain" : "fill",
			},
		);
	}

	function hideCanvasHint(): void {
		if (hasInteracted) return;
		hasInteracted = true;
		canvasHint.classList.add("hidden");
	}

	function stopDrawing(): void {
		drawing = false;
		lastPaintX = -1;
		lastPaintY = -1;
	}

	function selectMaterial(key: string): void {
		if (!Object.hasOwn(pickerIds, key)) return;
		const material = pickerIds[key];
		if (material === undefined) return;
		if (typeof material !== "number") return;
		sandbox.setMaterial(material);
		materialGrid.querySelectorAll<HTMLButtonElement>(".material").forEach((button) => {
			const isActive = button.dataset.material === key;
			button.classList.toggle("active", isActive);
			button.setAttribute("aria-pressed", String(isActive));
		});
		const name = materialNames[material];
		selectedMaterialLabel.textContent = `${name} selected`;
		liveRegion.textContent = `${name} selected`;
	}

	function togglePause(): void {
		paused = !paused;
		pauseButton.classList.toggle("paused", paused);
		pauseButton.setAttribute("aria-label", paused ? "Resume simulation" : "Pause simulation");
		pauseButton.title = paused ? "Resume (Space)" : "Pause (Space)";
		stateText.textContent = paused ? "Paused" : "Simulating";
		stateLight.classList.toggle("paused", paused);
		liveRegion.textContent = paused ? "Simulation paused" : "Simulation resumed";
	}

	canvas.addEventListener("pointerdown", (event) => {
		event.preventDefault();
		pointer = pointerToGrid(event);
		if (!pointer.isInside) return;
		canvas.setPointerCapture(event.pointerId);
		drawing = true;
		lastPaintX = pointer.x;
		lastPaintY = pointer.y;
		sandbox.paintCircle(pointer.x, pointer.y);
		hideCanvasHint();
	});

	canvas.addEventListener("pointermove", (event) => {
		pointer = pointerToGrid(event);
		if (!pointer.isInside) {
			stopDrawing();
			return;
		}
		if (!drawing) return;
		sandbox.paintLine(lastPaintX, lastPaintY, pointer.x, pointer.y);
		lastPaintX = pointer.x;
		lastPaintY = pointer.y;
	});

	canvas.addEventListener("pointerup", stopDrawing);
	canvas.addEventListener("lostpointercapture", stopDrawing);
	canvas.addEventListener("pointercancel", () => {
		pointer.isInside = false;
		stopDrawing();
	});
	canvas.addEventListener("pointerenter", (event) => {
		pointer = pointerToGrid(event);
	});
	canvas.addEventListener("pointerleave", () => {
		pointer.isInside = false;
		stopDrawing();
	});
	function invalidatePointer(): void {
		pointer.isInside = false;
		stopDrawing();
	}
	window.addEventListener("blur", invalidatePointer);
	window.addEventListener("resize", invalidatePointer);
	document.addEventListener("fullscreenchange", invalidatePointer);
	canvas.addEventListener("contextmenu", (event) => event.preventDefault());

	materialGrid.addEventListener("click", (event) => {
		if (!(event.target instanceof Element)) return;
		const button = event.target.closest(".material");
		if (!(button instanceof HTMLButtonElement) || !materialGrid.contains(button)) return;
		const material = button.dataset.material;
		if (!material) return;
		selectMaterial(material);
	});

	pauseButton.addEventListener("click", togglePause);

	brushInput.addEventListener("input", () => {
		const brushSize = Number(brushInput.value);
		sandbox.setBrushSize(brushSize);
		brushOutput.textContent = `${brushSize} px`;
		brushInput.style.setProperty("--range-progress", `${((brushSize - 1) / 11) * 100}%`);
	});

	speedSelect.addEventListener("change", () => {
		liveRegion.textContent = `Simulation speed ${speedSelect.options[speedSelect.selectedIndex].text}`;
	});

	waveToggle.addEventListener("change", () => {
		sandbox.setWavesEnabled(waveToggle.checked);
		liveRegion.textContent = waveToggle.checked
			? "Surface shimmer enabled"
			: "Surface shimmer disabled";
	});

	clearButton.addEventListener("click", () => {
		sandbox.clear();
		liveRegion.textContent = "Canvas cleared";
	});

	resetButton.addEventListener("click", () => {
		sandbox.seed();
		liveRegion.textContent = "Starter scene restored";
	});

	fullscreenButton.addEventListener("click", async () => {
		try {
			if (document.fullscreenElement) await document.exitFullscreen();
			else await canvasFrame.requestFullscreen();
		} catch {
			liveRegion.textContent = "Fullscreen is not available in this browser";
		}
	});

	document.addEventListener("keydown", (event) => {
		if (event.ctrlKey || event.altKey || event.metaKey) return;
		if (
			event.target instanceof Element &&
			event.target.closest(
				"input, textarea, select, [contenteditable]:not([contenteditable='false'])",
			)
		)
			return;
		if (event.code === "Space") {
			if (event.target instanceof Element && event.target.closest("button")) return;
			event.preventDefault();
			togglePause();
			return;
		}
		if (event.code === "KeyT" && !event.repeat) {
			setTemperatureMap(!temperatureToggle.checked);
			return;
		}
		const entry = pickerEntries.find(
			(entry) =>
				entry.shortcut &&
				event.code === (/^\d$/.test(entry.shortcut) ? "Digit" : "Key") + entry.shortcut,
		);
		if (entry) selectMaterial(entry.key);
	});

	brushInput.dispatchEvent(new Event("input"));

	return {
		getPointer: () => pointer,
		getSpeed: () => Number(speedSelect.value),
		isPaused: () => paused,
		paintHeldTool: () => {
			if (paused || !drawing || !pointer.isInside) return;
			sandbox.paintCircle(pointer.x, pointer.y);
		},
		updateProbe: () => {
			if (!pointer.isInside) {
				probeMaterial.textContent = "--";
				probeTemperature.textContent = "--";
				return;
			}
			const cell = sandbox.getCell(pointer.x, pointer.y);
			probeMaterial.textContent = materialNames[cell.material];
			probeTemperature.textContent = `${cell.temperature.toFixed(1)} °C`;
		},
		updateStats: (particleTotal, framesPerSecond, diagnostics) => {
			fpsCounter.textContent = `${framesPerSecond} FPS`;
			particleCount.textContent = `${particleTotal.toLocaleString()} particles`;
			if (diagnostics) {
				totalMass.textContent = `${diagnostics.matterMassKg.toFixed(3)} kg matter`;
				totalEnergy.textContent = `${diagnostics.totalTrackedEnergyKj.toFixed(1)} kJ tracked`;
				maxPressure.textContent = `${(diagnostics.maximumPressurePa / 1000).toFixed(0)} kPa max`;
			}
		},
	};
}
