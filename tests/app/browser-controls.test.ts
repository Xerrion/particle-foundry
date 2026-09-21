import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { Window } from "happy-dom";
import { bindControls } from "../../src/app/controls";
import { createViewport } from "../../src/app/viewport";
import { BLAST, GUNPOWDER, HYDROCHLORIC_ACID, WATER } from "../../src/materials";
import type { Sandbox } from "../../src/simulation/sandbox";

const window = new Window({ url: "http://localhost/" });
const previousDescriptors = new Map<PropertyKey, PropertyDescriptor | undefined>();

function installGlobal(key: PropertyKey, value: unknown): void {
	previousDescriptors.set(key, Object.getOwnPropertyDescriptor(globalThis, key));
	Object.defineProperty(globalThis, key, { configurable: true, writable: true, value });
}

beforeAll(async () => {
	window.document.write(await Bun.file(new URL("../../index.html", import.meta.url)).text());
	for (const [key, value] of Object.entries({
		window,
		document: window.document,
		Element: window.Element,
		HTMLElement: window.HTMLElement,
		HTMLButtonElement: window.HTMLButtonElement,
		HTMLInputElement: window.HTMLInputElement,
		HTMLCanvasElement: window.HTMLCanvasElement,
		PointerEvent: window.PointerEvent,
		KeyboardEvent: window.KeyboardEvent,
		Event: window.Event,
		getComputedStyle: window.getComputedStyle.bind(window),
	})) {
		installGlobal(key, value);
	}
});

afterAll(() => {
	for (const [key, descriptor] of previousDescriptors) {
		if (descriptor) Object.defineProperty(globalThis, key, descriptor);
		else Reflect.deleteProperty(globalThis, key);
	}
	window.close();
});

describe("browser control flow", () => {
	test("material, pause, speed, reset, clear, waves and touch drawing are repeatable", () => {
		function element<T extends Element>(selector: string): T {
			const match = window.document.querySelector(selector);
			if (!match) throw new Error(`DOM fixture is missing: ${selector}`);
			return match as unknown as T;
		}
		const domEvent = (event: unknown): Event => event as Event;
		const calls = {
			materials: [] as number[],
			sizes: [] as number[],
			circles: 0,
			circlePositions: [] as [number, number][],
			lines: 0,
			clears: 0,
			seeds: 0,
			waves: [] as boolean[],
			viewModes: [] as string[],
		};
		const sandbox: Sandbox = {
			clear: () => {
				calls.clears += 1;
			},
			getCell: () => ({ material: WATER, temperature: 20, energy: 417.6 }),
			getCellPhysics: () => ({ pressurePa: 102_000, velocityX: 0.3, velocityY: -0.4 }),
			getDiagnostics: () => ({
				totalMassKg: 1,
				ledger: {
					massAddedKg: 0,
					massRemovedKg: 0,
					externalEnergyAdded: 0,
					externalEnergyRemoved: 0,
				},
				matterMassKg: 1,
				gasMassKg: 0,
				thermalEnergy: 2,
				chemicalEnergyKj: 3,
				kineticEnergyKj: 4,
				potentialEnergyKj: 5,
				totalTrackedEnergyKj: 14,
				maximumPressurePa: 101_325,
				maximumSpeedMPerS: 0,
			}),
			getParticleCount: () => 1,
			paintCircle: (x, y) => {
				calls.circles += 1;
				calls.circlePositions.push([x, y]);
			},
			paintLine: () => {
				calls.lines += 1;
			},
			render: () => {},
			seed: () => {
				calls.seeds += 1;
			},
			setBrushSize: (size) => calls.sizes.push(size),
			setCellDynamic: () => {},
			setMaterial: (material) => calls.materials.push(material),
			setWavesEnabled: (enabled) => calls.waves.push(enabled),
			setTemperatureMapEnabled: (enabled) =>
				calls.viewModes.push(enabled ? "temperature" : "materials"),
			setViewMode: (mode) => calls.viewModes.push(mode),
			step: () => {},
		};
		const canvas = element<HTMLCanvasElement>("#sandbox");
		Object.defineProperty(canvas, "getBoundingClientRect", {
			configurable: true,
			value: () => ({
				x: 0,
				y: 0,
				left: 0,
				top: 0,
				right: 240,
				bottom: 135,
				width: 240,
				height: 135,
				toJSON: () => ({}),
			}),
		});
		Object.defineProperty(canvas, "setPointerCapture", {
			configurable: true,
			value: () => {},
		});

		const viewport = createViewport(480, 270);
		const controls = bindControls(canvas, sandbox, viewport);
		controls.getPointer().isInside = true;
		expect(controls.getPointer().isInside).toBe(false);
		element<HTMLButtonElement>("#zoomInButton").click();
		expect(viewport.zoom).toBe(1.25);
		element<HTMLButtonElement>("#zoomResetButton").click();
		expect(viewport.zoom).toBe(1);
		expect(window.document.querySelectorAll(".element-tile")).toHaveLength(118);
		expect(window.document.querySelectorAll(".element-tile:not([hidden])")).toHaveLength(8);
		element<HTMLButtonElement>("#elementsTab").click();
		expect(element<HTMLElement>("#elementsPanel").hidden).toBe(false);
		element<HTMLButtonElement>("#elementGrid [data-material='hydrogen']").click();
		expect(element<HTMLElement>("#selectedMaterialLabel").textContent).toContain("Hydrogen");
		expect(element<HTMLElement>("#selectionInteractions").textContent).toContain("2 H₂ + O₂");
		element<HTMLButtonElement>("#showInteractionGuide").click();
		expect(element<HTMLDetailsElement>(".selection-details").open).toBe(true);
		const reference = element<HTMLInputElement>("#showReferenceElements");
		reference.checked = true;
		reference.dispatchEvent(domEvent(new window.Event("change")));
		const elementSearch = element<HTMLInputElement>("#elementSearch");
		elementSearch.value = "oganesson";
		elementSearch.dispatchEvent(domEvent(new window.Event("input")));
		expect(window.document.querySelectorAll(".element-tile:not([hidden])")).toHaveLength(1);
		expect(element<HTMLElement>("#elementResultCount").textContent).toContain("1 of 118");

		const unmodeled = element<HTMLButtonElement>("#elementGrid [data-atomic-number='118']");
		expect(unmodeled.disabled).toBe(true);
		const beforeUnavailable = calls.materials.length;
		unmodeled.click();
		expect(calls.materials.length).toBe(beforeUnavailable);
		element<HTMLButtonElement>("#materialsTab").click();
		expect(element<HTMLElement>("#elementsPanel").hidden).toBe(true);
		const materialSearch = element<HTMLInputElement>("#materialSearch");
		materialSearch.value = "gunpowder";
		materialSearch.dispatchEvent(domEvent(new window.Event("input")));
		expect(window.document.querySelectorAll(".material:not([hidden])")).toHaveLength(1);
		const materialCategory = element<HTMLSelectElement>("#materialCategory");
		materialSearch.value = "";
		materialSearch.dispatchEvent(domEvent(new window.Event("input")));
		materialCategory.value = "tools";
		materialCategory.dispatchEvent(domEvent(new window.Event("change")));
		expect(window.document.querySelectorAll(".material:not([hidden])")).toHaveLength(4);
		materialCategory.value = "chemicals";
		materialCategory.dispatchEvent(domEvent(new window.Event("change")));
		expect(window.document.querySelectorAll(".material:not([hidden])")).toHaveLength(3);
		materialCategory.value = "all";
		materialCategory.dispatchEvent(domEvent(new window.Event("change")));
		element<HTMLButtonElement>("[data-material='hydrochloricAcid']").click();
		expect(calls.materials.at(-1)).toBe(HYDROCHLORIC_ACID);
		element<HTMLButtonElement>("[data-material='water']").click();
		expect(calls.materials.at(-1)).toBe(WATER);
		element<HTMLButtonElement>("[data-material='gunpowder']").click();
		expect(calls.materials.at(-1)).toBe(GUNPOWDER);
		window.document.dispatchEvent(new window.KeyboardEvent("keydown", { code: "KeyB" }));
		expect(calls.materials.at(-1)).toBe(BLAST);

		element<HTMLButtonElement>("#pauseButton").click();
		expect(controls.isPaused()).toBe(true);
		window.document.dispatchEvent(new window.KeyboardEvent("keydown", { code: "Space" }));
		expect(controls.isPaused()).toBe(false);

		const speed = element<HTMLSelectElement>("#speedSelect");
		speed.value = "2";
		speed.dispatchEvent(domEvent(new window.Event("change")));
		expect(controls.getSpeed()).toBe(2);

		const waves = element<HTMLInputElement>("#waveToggle");
		waves.checked = false;
		waves.dispatchEvent(domEvent(new window.Event("change")));
		expect(calls.waves.at(-1)).toBe(false);

		element<HTMLButtonElement>("#clearButton").click();
		element<HTMLButtonElement>("#resetButton").click();
		expect([calls.clears, calls.seeds]).toEqual([1, 1]);

		canvas.dispatchEvent(
			domEvent(
				new window.PointerEvent("pointerdown", {
					clientX: 20,
					clientY: 20,
					pointerId: 1,
					pointerType: "touch",
				}),
			),
		);
		canvas.dispatchEvent(
			domEvent(
				new window.PointerEvent("pointermove", {
					clientX: 30,
					clientY: 25,
					pointerId: 1,
					pointerType: "touch",
				}),
			),
		);
		canvas.dispatchEvent(domEvent(new window.PointerEvent("pointerup", { pointerId: 1 })));
		expect(calls.circles).toBe(1);
		expect(calls.lines).toBe(1);

		// No pointermove is sent during these holds. Every selectable material
		// and tool must emit on each simulation tick, for mouse and touch alike.
		for (const pointerType of ["mouse", "touch"]) {
			for (const button of window.document.querySelectorAll("[data-material]")) {
				(button as unknown as HTMLButtonElement).click();
				const before = calls.circles;
				canvas.dispatchEvent(
					domEvent(
						new window.PointerEvent("pointerdown", {
							clientX: 20,
							clientY: 20,
							pointerId: 2,
							pointerType,
						}),
					),
				);
				for (let tick = 0; tick < 5; tick++) controls.paintHeldTool();
				expect(calls.circles).toBe(before + 6);
				element<HTMLButtonElement>("#pauseButton").click();
				controls.paintHeldTool();
				expect(calls.circles).toBe(before + 6);
				canvas.dispatchEvent(domEvent(new window.PointerEvent("pointerup", { pointerId: 2 })));
				element<HTMLButtonElement>("#pauseButton").click();
				controls.paintHeldTool();
				expect(calls.circles).toBe(before + 6);
			}
		}
		const temperatureToggle = element<HTMLInputElement>("#temperatureToggle");
		temperatureToggle.checked = true;
		temperatureToggle.dispatchEvent(domEvent(new window.Event("change")));
		expect(calls.viewModes.at(-1)).toBe("temperature");
		expect(element<HTMLElement>("#temperatureLegend").hidden).toBe(false);
		expect(element<HTMLElement>("#temperatureLegend").textContent).toContain("includes air");
		expect(element<HTMLElement>("#temperatureLegend .temperature-labels").children).toHaveLength(7);
		window.document.dispatchEvent(new window.KeyboardEvent("keydown", { code: "KeyT" }));
		expect(calls.viewModes.at(-1)).toBe("materials");
		expect(element<HTMLElement>("#temperatureLegend").hidden).toBe(true);
		expect(temperatureToggle.checked).toBe(false);
		const pressureToggle = element<HTMLInputElement>("#pressureToggle");
		pressureToggle.checked = true;
		pressureToggle.dispatchEvent(domEvent(new window.Event("change")));
		expect(calls.viewModes.at(-1)).toBe("pressure");
		expect(element<HTMLElement>("#pressureLegend").hidden).toBe(false);
		expect(element<HTMLElement>("#pressureLegend .field-labels").children).toHaveLength(5);
		window.document.dispatchEvent(new window.KeyboardEvent("keydown", { code: "KeyV" }));
		expect(calls.viewModes.at(-1)).toBe("velocity");
		expect(pressureToggle.checked).toBe(false);
		expect(element<HTMLElement>("#velocityLegend").hidden).toBe(false);
		expect(element<HTMLElement>("#velocityLegend .field-labels").children).toHaveLength(4);
		window.document.dispatchEvent(new window.KeyboardEvent("keydown", { code: "KeyP" }));
		expect(calls.viewModes.at(-1)).toBe("pressure");
		expect(element<HTMLElement>("#velocityLegend").hidden).toBe(true);
		window.document.dispatchEvent(
			new window.KeyboardEvent("keydown", { code: "KeyP", repeat: true }),
		);
		expect(calls.viewModes.at(-1)).toBe("pressure");

		controls.updateProbe();
		controls.updateStats(1, 60, sandbox.getDiagnostics());
		expect(window.document.querySelector("#probeMaterial")?.textContent).toBe("Water");
		expect(window.document.querySelector("#probePressure")?.textContent).toBe("102.0 kPa");
		expect(window.document.querySelector("#probeVelocity")?.textContent).toContain("0.50 m/s");
		expect(window.document.querySelector("#totalMass")?.textContent).toBe("1.000 kg matter");

		viewport.zoomAt(2, 0.5, 0.5);
		canvas.dispatchEvent(
			domEvent(
				new window.PointerEvent("pointerdown", {
					clientX: 20,
					clientY: 20,
					pointerId: 3,
					pointerType: "mouse",
				}),
			),
		);
		expect(calls.circlePositions.at(-1)).toEqual([140, 87]);
		canvas.dispatchEvent(domEvent(new window.PointerEvent("pointerup", { pointerId: 3 })));
		const beforePan = calls.circles;
		const leftBeforePan = viewport.left;
		canvas.dispatchEvent(
			domEvent(
				new window.PointerEvent("pointerdown", {
					clientX: 60,
					clientY: 30,
					pointerId: 4,
					shiftKey: true,
				}),
			),
		);
		canvas.dispatchEvent(
			domEvent(
				new window.PointerEvent("pointermove", {
					clientX: 100,
					clientY: 30,
					pointerId: 4,
					shiftKey: true,
				}),
			),
		);
		canvas.dispatchEvent(domEvent(new window.PointerEvent("pointerup", { pointerId: 4 })));
		expect(viewport.left).toBeLessThan(leftBeforePan);
		expect(calls.circles).toBe(beforePan);
	});
});
