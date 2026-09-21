import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { Window } from "happy-dom";
import { bindControls } from "../src/controls";
import { WATER } from "../src/materials";
import type { Sandbox } from "../src/sandbox";

const window = new Window({ url: "http://localhost/" });
const previousDescriptors = new Map<PropertyKey, PropertyDescriptor | undefined>();

function installGlobal(key: PropertyKey, value: unknown): void {
	previousDescriptors.set(key, Object.getOwnPropertyDescriptor(globalThis, key));
	Object.defineProperty(globalThis, key, { configurable: true, writable: true, value });
}

beforeAll(async () => {
	window.document.write(await Bun.file("src/index.html").text());
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
			paintCircle: () => {
				calls.circles += 1;
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

		const controls = bindControls(canvas, sandbox);
		element<HTMLButtonElement>("[data-material='water']").click();
		expect(calls.materials.at(-1)).toBe(WATER);

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
		window.document.dispatchEvent(new window.KeyboardEvent("keydown", { code: "KeyT" }));
		expect(calls.viewModes.at(-1)).toBe("materials");
		expect(element<HTMLElement>("#temperatureLegend").hidden).toBe(true);
		const pressureToggle = element<HTMLInputElement>("#pressureToggle");
		pressureToggle.checked = true;
		pressureToggle.dispatchEvent(domEvent(new window.Event("change")));
		expect(calls.viewModes.at(-1)).toBe("pressure");
		expect(element<HTMLElement>("#pressureLegend").hidden).toBe(false);
		window.document.dispatchEvent(new window.KeyboardEvent("keydown", { code: "KeyV" }));
		expect(calls.viewModes.at(-1)).toBe("velocity");
		expect(pressureToggle.checked).toBe(false);
		expect(element<HTMLElement>("#velocityLegend").hidden).toBe(false);

		controls.updateProbe();
		controls.updateStats(1, 60, sandbox.getDiagnostics());
		expect(window.document.querySelector("#probeMaterial")?.textContent).toBe("Water");
		expect(window.document.querySelector("#probePressure")?.textContent).toBe("102.0 kPa");
		expect(window.document.querySelector("#probeVelocity")?.textContent).toContain("0.50 m/s");
		expect(window.document.querySelector("#totalMass")?.textContent).toBe("1.000 kg matter");
	});
});
