import { materialNames } from "../materials";
import type { PhysicalTotals } from "../simulation/diagnostics";
import type { PointerState, Sandbox } from "../simulation/sandbox";
import { requiredElement } from "./dom";
import { bindMaterialPicker } from "./material-picker";
import { mapCanvasPointer } from "./pointer-mapping";
import { bindViewControls } from "./view-controls";
import { MAX_ZOOM, type Viewport } from "./viewport";

export interface Controls {
	/** Returns a detached pointer reading; callers cannot change drawing state. */
	getPointer(): PointerState;
	getSpeed(): number;
	isPaused(): boolean;
	/** Applies any held brush once per running simulation step. */
	paintHeldTool(): void;
	/** Refreshes the cell readout, or clears it when the pointer is outside. */
	updateProbe(): void;
	updateStats(particleTotal: number, framesPerSecond: number, diagnostics?: PhysicalTotals): void;
}

/** Commands and readings used by the controls; rendering and stepping stay with the app. */
export type ControlsSandbox = Pick<
	Sandbox,
	| "clear"
	| "getCell"
	| "getCellPhysics"
	| "paintCircle"
	| "paintLine"
	| "seed"
	| "setBrushSize"
	| "setMaterial"
	| "setViewMode"
	| "setWavesEnabled"
>;

export function bindControls(
	canvas: HTMLCanvasElement,
	sandbox: ControlsSandbox,
	viewport: Viewport,
): Controls {
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
	const liveRegion = requiredElement<HTMLElement>("#liveRegion");
	const canvasHint = requiredElement<HTMLElement>("#canvasHint");
	const canvasFrame = requiredElement<HTMLElement>("#canvasFrame");
	const probeMaterial = requiredElement<HTMLElement>("#probeMaterial");
	const probeTemperature = requiredElement<HTMLElement>("#probeTemperature");
	const probePressure = requiredElement<HTMLElement>("#probePressure");
	const probeVelocity = requiredElement<HTMLElement>("#probeVelocity");
	const waveToggle = requiredElement<HTMLInputElement>("#waveToggle");
	const sidebar = requiredElement<HTMLElement>(".tool-panel");
	requiredElement<HTMLButtonElement>("#showInteractionGuide").addEventListener("click", () => {
		const details = requiredElement<HTMLDetailsElement>(".selection-details");
		details.open = true;
		requiredElement<HTMLElement>(".sidebar-content").scrollTop = 0;
		details.querySelector("summary")?.focus();
	});
	const tabs = [...sidebar.querySelectorAll<HTMLButtonElement>("[role=tab]")];
	function activateTab(tab: HTMLButtonElement): void {
		for (const entry of tabs) {
			const active = entry === tab;
			entry.setAttribute("aria-selected", String(active));
			entry.tabIndex = active ? 0 : -1;
			requiredElement<HTMLElement>(`#${entry.dataset.panel}`).hidden = !active;
		}
	}
	for (const [index, tab] of tabs.entries()) {
		tab.addEventListener("click", () => activateTab(tab));
		tab.addEventListener("keydown", (event) => {
			const next =
				event.key === "ArrowRight"
					? (index + 1) % tabs.length
					: event.key === "ArrowLeft"
						? (index + tabs.length - 1) % tabs.length
						: event.key === "Home"
							? 0
							: event.key === "End"
								? tabs.length - 1
								: -1;
			if (next < 0) return;
			event.preventDefault();
			activateTab(tabs[next]);
			tabs[next].focus();
		});
	}
	const clearButton = requiredElement<HTMLButtonElement>("#clearButton");
	const resetButton = requiredElement<HTMLButtonElement>("#resetButton");
	const fullscreenButton = requiredElement<HTMLButtonElement>("#fullscreenButton");
	const zoomInButton = requiredElement<HTMLButtonElement>("#zoomInButton");
	const zoomOutButton = requiredElement<HTMLButtonElement>("#zoomOutButton");
	const zoomResetButton = requiredElement<HTMLButtonElement>("#zoomResetButton");
	const zoomLevel = requiredElement<HTMLElement>("#zoomLevel");
	const picker = bindMaterialPicker(sandbox.setMaterial);
	const views = bindViewControls(sandbox.setViewMode);

	let paused = false;
	let drawing = false;
	let panning = false;
	let panX = 0;
	let panY = 0;
	let pointer: PointerState = { isInside: false, x: -1, y: -1 };
	let lastPaintX = -1;
	let lastPaintY = -1;
	let hasInteracted = false;

	function viewportPoint(clientX: number, clientY: number): { u: number; v: number } | undefined {
		const rect = canvas.getBoundingClientRect();
		const point = mapCanvasPointer(
			{ x: clientX, y: clientY },
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
		return point.isInside ? { u: point.x / canvas.width, v: point.y / canvas.height } : undefined;
	}

	function pointerToGrid(event: PointerEvent): PointerState {
		const point = viewportPoint(event.clientX, event.clientY);
		const cell = point && viewport.worldAt(point.u, point.v);
		return cell ? { isInside: true, ...cell } : { isInside: false, x: -1, y: -1 };
	}

	function updateZoomLabel(): void {
		zoomLevel.textContent = `${viewport.zoom.toFixed(viewport.zoom % 1 ? 1 : 0)}×`;
		zoomOutButton.disabled = viewport.zoom <= 1;
		zoomInButton.disabled = viewport.zoom >= MAX_ZOOM;
	}

	function zoomBy(factor: number, u = 0.5, v = 0.5): void {
		viewport.zoomAt(factor, u, v);
		updateZoomLabel();
		liveRegion.textContent = `Zoom ${zoomLevel.textContent}`;
	}

	function hideCanvasHint(): void {
		if (hasInteracted) return;
		hasInteracted = true;
		canvasHint.classList.add("hidden");
	}

	function stopDrawing(): void {
		drawing = false;
		panning = false;
		lastPaintX = -1;
		lastPaintY = -1;
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
		if (event.button === 1 || event.button === 2 || event.shiftKey) {
			stopDrawing();
			panning = true;
			panX = event.clientX;
			panY = event.clientY;
			pointer.isInside = false;
			hideCanvasHint();
			return;
		}
		if (event.button !== 0) return;
		drawing = true;
		lastPaintX = pointer.x;
		lastPaintY = pointer.y;
		sandbox.paintCircle(pointer.x, pointer.y);
		hideCanvasHint();
	});

	canvas.addEventListener("pointermove", (event) => {
		if (panning) {
			const rect = canvas.getBoundingClientRect();
			const scale =
				getComputedStyle(canvas).objectFit === "contain"
					? Math.min(rect.width / canvas.width, rect.height / canvas.height)
					: 0;
			const contentWidth = scale ? canvas.width * scale : rect.width;
			const contentHeight = scale ? canvas.height * scale : rect.height;
			viewport.panBy((event.clientX - panX) / contentWidth, (event.clientY - panY) / contentHeight);
			panX = event.clientX;
			panY = event.clientY;
			pointer.isInside = false;
			return;
		}
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
	canvas.addEventListener(
		"wheel",
		(event) => {
			const point = viewportPoint(event.clientX, event.clientY);
			if (!point) return;
			event.preventDefault();
			stopDrawing();
			zoomBy(event.deltaY < 0 ? 1.25 : 1 / 1.25, point.u, point.v);
			pointer = { isInside: false, x: -1, y: -1 };
		},
		{ passive: false },
	);
	zoomInButton.addEventListener("click", () => zoomBy(1.25));
	zoomOutButton.addEventListener("click", () => zoomBy(1 / 1.25));
	zoomResetButton.addEventListener("click", () => {
		viewport.reset();
		updateZoomLabel();
		liveRegion.textContent = "View fitted to world";
	});
	updateZoomLabel();

	pauseButton.addEventListener("click", togglePause);

	brushInput.addEventListener("input", () => {
		const brushSize = Number(brushInput.value);
		sandbox.setBrushSize(brushSize);
		brushOutput.textContent = `${brushSize} cells`;
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
		if (!event.repeat && views.selectShortcut(event.code)) return;
		picker.selectShortcut(event.code);
	});

	brushInput.dispatchEvent(new Event("input"));
	picker.select("sand");

	return {
		getPointer: () => ({ ...pointer }),
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
				probePressure.textContent = "--";
				probeVelocity.textContent = "--";
				return;
			}
			const cell = sandbox.getCell(pointer.x, pointer.y);
			probeMaterial.textContent = materialNames[cell.material];
			probeTemperature.textContent = `${cell.temperature.toFixed(1)} °C`;
			const physics = sandbox.getCellPhysics(pointer.x, pointer.y);
			probePressure.textContent =
				physics.pressurePa > 0 ? `${(physics.pressurePa / 1000).toFixed(1)} kPa` : "--";
			probeVelocity.textContent = `${Math.hypot(physics.velocityX, physics.velocityY).toFixed(2)} m/s (${physics.velocityX.toFixed(2)}, ${physics.velocityY.toFixed(2)})`;
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
