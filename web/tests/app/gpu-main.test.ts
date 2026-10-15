import { afterAll, beforeAll, expect, mock, test } from "bun:test";
import { Window } from "happy-dom";
import type { BrowserGpuSceneSession } from "../../src/engine-client/wasm";

const dom = new Window({ url: "http://localhost/gpu.html" });
const previousGlobals = new Map<PropertyKey, PropertyDescriptor | undefined>();

function installGlobal(key: PropertyKey, value: unknown): void {
	previousGlobals.set(key, Object.getOwnPropertyDescriptor(globalThis, key));
	Object.defineProperty(globalThis, key, { configurable: true, writable: true, value });
}

let sceneEpoch = 1;
let resetPending = false;
let probeInFlight = false;
let rejectProbe: ((reason: Error) => void) | undefined;
let renderCalls = 0;
let disposeCalls = 0;
let freeCalls = 0;
let reloadCalls = 0;
let frameCallback: FrameRequestCallback | undefined;
let rejectStep: ((reason: Error) => void) | undefined;
const paintCalls: Parameters<BrowserGpuSceneSession["paint"]>[] = [];
const strokeCalls: Parameters<BrowserGpuSceneSession["paintStroke"]>[] = [];

function status(): string {
	return JSON.stringify({
		epoch: sceneEpoch,
		tick: 0,
		acceptedTimeS: 0,
		stateRevision: 0,
		busy: probeInFlight,
		resetPending,
	});
}

const scene: BrowserGpuSceneSession = {
	backend: () => "BrowserWebGpu",
	adapter_info_json: () => "{}",
	status_json: status,
	enable_pressure_diagnostics: () => {},
	disable_pressure_diagnostics: () => {},
	take_pressure_diagnostics_json: () => "{}",
	render: () => {
		renderCalls++;
		return true;
	},
	resize: () => {},
	advance: () =>
		new Promise<string>((_resolve, reject) => {
			rejectStep = reject;
		}),
	paint: (...args) => {
		paintCalls.push(args);
		return status();
	},
	paintStroke: (...args) => {
		strokeCalls.push(args);
		return status();
	},
	probe: () => {
		probeInFlight = true;
		return new Promise<string>((_resolve, reject) => {
			rejectProbe = reject;
		});
	},
	checkpoint_prototype: async () => new Uint8Array(),
	reset: () => {
		if (probeInFlight) resetPending = true;
		else sceneEpoch++;
		return status();
	},
	dispose: () => {
		disposeCalls++;
	},
	free: () => {
		freeCalls++;
	},
};

function element<T extends Element>(selector: string): T {
	const match = dom.document.querySelector(selector);
	if (!match) throw new Error(`GPU page fixture is missing: ${selector}`);
	return match as unknown as T;
}

function transition(name: string, persisted: boolean) {
	const event = new dom.Event(name);
	Object.defineProperty(event, "persisted", { value: persisted });
	return event;
}

function pointer(name: string, x: number, y: number, pointerId = 1, isPrimary = true): void {
	element<HTMLCanvasElement>("#gpuCanvas").dispatchEvent(
		new dom.PointerEvent(name, {
			clientX: x,
			clientY: y,
			pointerId,
			isPrimary,
			button: 0,
			buttons: name === "pointerup" ? 0 : 1,
		}) as unknown as PointerEvent,
	);
}

function preparePainting(): void {
	const pause = element<HTMLButtonElement>("#gpuPause");
	if (pause.textContent === "Pause") pause.click();
	element<HTMLButtonElement>("#gpuReset").click();
	element<HTMLSelectElement>("#gpuPaintMaterial").value = "water";
	element<HTMLInputElement>("#gpuPaintRadius").value = "4";
	paintCalls.length = 0;
	strokeCalls.length = 0;
}

beforeAll(async () => {
	dom.document.write(await Bun.file(new URL("../../gpu.html", import.meta.url)).text());
	const canvas = element<HTMLCanvasElement>("#gpuCanvas");
	canvas.getBoundingClientRect = () => new dom.DOMRect(0, 0, 480, 270) as unknown as DOMRect;
	const capturedPointers = new Set<number>();
	canvas.setPointerCapture = (id) => {
		capturedPointers.add(id);
	};
	canvas.hasPointerCapture = (id) => capturedPointers.has(id);
	canvas.releasePointerCapture = (id) => {
		capturedPointers.delete(id);
	};
	for (const [key, value] of Object.entries({
		window: dom,
		document: dom.document,
		location: dom.location,
		requestAnimationFrame: (callback: FrameRequestCallback): number => {
			frameCallback = callback;
			return 1;
		},
		cancelAnimationFrame: (_handle: number): void => {},
	})) {
		installGlobal(key, value);
	}
	Object.defineProperty(dom.location, "reload", {
		configurable: true,
		value: () => {
			reloadCalls++;
		},
	});
	mock.module("../../src/engine-client/wasm", () => ({
		initializeBrowserGpuScene: async () => scene,
	}));
	await import("../../src/gpu-main");
	await Bun.sleep(0);
});

afterAll(() => {
	mock.restore();
	Reflect.deleteProperty(dom.location, "reload");
	for (const [key, descriptor] of previousGlobals) {
		if (descriptor) Object.defineProperty(globalThis, key, descriptor);
		else Reflect.deleteProperty(globalThis, key);
	}
	dom.close();
});

test("redraws a paused scene after reset cancels an active probe", async () => {
	expect(renderCalls).toBe(1);
	element<HTMLButtonElement>("#gpuPause").click();
	element<HTMLButtonElement>("#gpuProbeAt").click();
	expect(probeInFlight).toBe(true);
	element<HTMLButtonElement>("#gpuReset").click();
	expect(resetPending).toBe(true);
	expect(renderCalls).toBe(1);

	probeInFlight = false;
	resetPending = false;
	sceneEpoch = 2;
	rejectProbe?.(new Error("GPU browser scene reset during probe"));
	await Bun.sleep(0);

	expect(element<HTMLOutputElement>("#gpuEpoch").value).toBe("2");
	expect(renderCalls).toBe(2);
	expect(element<HTMLParagraphElement>("#gpuProbeMessage").textContent).toBe(
		"Cell reading canceled by reset.",
	);
	expect(element<HTMLButtonElement>("#gpuPaintAt").disabled).toBe(false);
});

test("connects sparse pointer events and the final pointer-up with one frozen brush", () => {
	preparePainting();
	const renderedBefore = renderCalls;
	pointer("pointerdown", 118, 89);
	element<HTMLSelectElement>("#gpuPaintMaterial").value = "air";
	element<HTMLInputElement>("#gpuPaintRadius").value = "12";
	pointer("pointermove", 240, 89);
	pointer("pointerup", 354, 89);
	expect(paintCalls).toEqual([[118, 89, 4, "water"]]);
	expect(strokeCalls).toEqual([
		[118, 89, 240, 89, 4, "water"],
		[240, 89, 354, 89, 4, "water"],
	]);
	expect(renderCalls - renderedBefore).toBe(3);
	expect(element<HTMLCanvasElement>("#gpuCanvas").hasPointerCapture(1)).toBe(false);
});

test("ignores secondary pointers and starts no bridge after leaving the canvas", () => {
	preparePainting();
	pointer("pointerdown", 100, 80);
	pointer("pointerdown", 250, 80, 2, false);
	pointer("pointermove", 300, 80, 2, false);
	pointer("pointerup", 300, 80, 2, false);
	pointer("pointermove", 120, 80);
	pointer("pointermove", 500, 80);
	pointer("pointermove", 350, 80);
	pointer("pointerup", 350, 80);
	expect(paintCalls).toEqual([[100, 80, 4, "water"]]);
	expect(strokeCalls).toEqual([[100, 80, 120, 80, 4, "water"]]);
});

test("cancel, lost capture, reset, resume, and hiding the page end the current stroke", () => {
	const canvas = element<HTMLCanvasElement>("#gpuCanvas");
	for (const stop of [
		() => pointer("pointercancel", 100, 80),
		() => pointer("lostpointercapture", 100, 80),
		() => element<HTMLButtonElement>("#gpuReset").click(),
		() => {
			element<HTMLButtonElement>("#gpuPause").click();
			pointer("pointerdown", 200, 80);
			element<HTMLButtonElement>("#gpuPause").click();
		},
		() => {
			Object.defineProperty(dom.document, "hidden", { configurable: true, value: true });
			dom.document.dispatchEvent(new dom.Event("visibilitychange"));
			pointer("pointerdown", 200, 80);
			Object.defineProperty(dom.document, "hidden", { configurable: true, value: false });
			dom.document.dispatchEvent(new dom.Event("visibilitychange"));
		},
	]) {
		preparePainting();
		pointer("pointerdown", 100, 80);
		stop();
		pointer("pointermove", 300, 80);
		pointer("pointerup", 350, 80);
		expect(paintCalls).toHaveLength(1);
		expect(strokeCalls).toHaveLength(0);
		expect(canvas.hasPointerCapture(1)).toBe(false);
	}
});

test("an active probe ends a stroke and blocks painting", async () => {
	preparePainting();
	pointer("pointerdown", 100, 80);
	element<HTMLButtonElement>("#gpuProbeAt").click();
	pointer("pointermove", 200, 80);
	pointer("pointerdown", 250, 80);
	expect(paintCalls).toHaveLength(1);
	expect(strokeCalls).toHaveLength(0);
	probeInFlight = false;
	rejectProbe?.(new Error("Probe canceled by test"));
	await Bun.sleep(0);
	pointer("pointermove", 300, 80);
	expect(strokeCalls).toHaveLength(0);
});

test("pending and failed GPU steps block painting until reset", async () => {
	preparePainting();
	element<HTMLButtonElement>("#gpuPause").click();
	frameCallback?.(0);
	element<HTMLButtonElement>("#gpuPause").click();
	pointer("pointerdown", 100, 80);
	expect(paintCalls).toHaveLength(0);
	rejectStep?.(new Error("GPU test step failed"));
	await Bun.sleep(0);
	pointer("pointerdown", 100, 80);
	expect(paintCalls).toHaveLength(0);
	expect(element<HTMLParagraphElement>("#gpuError").hidden).toBe(false);
	element<HTMLButtonElement>("#gpuReset").click();
	pointer("pointerdown", 200, 80);
	pointer("pointerup", 200, 80);
	expect(paintCalls).toEqual([[200, 80, 4, "water"]]);
});

test("coordinate painting remains available without a pointer stroke", () => {
	preparePainting();
	element<HTMLInputElement>("#gpuPaintX").value = "250";
	element<HTMLInputElement>("#gpuPaintY").value = "70";
	element<HTMLButtonElement>("#gpuPaintAt").click();
	expect(paintCalls).toEqual([[250, 70, 4, "water"]]);
	expect(strokeCalls).toHaveLength(0);
});

test("reloads a freed scene on a back-forward cache restore", () => {
	preparePainting();
	pointer("pointerdown", 100, 80);
	dom.dispatchEvent(transition("pagehide", true));
	pointer("pointermove", 200, 80);
	pointer("pointerdown", 250, 80);
	expect(paintCalls).toHaveLength(1);
	expect(strokeCalls).toHaveLength(0);
	expect(disposeCalls).toBe(1);
	expect(freeCalls).toBe(1);
	dom.dispatchEvent(transition("pageshow", false));
	expect(reloadCalls).toBe(0);
	dom.dispatchEvent(transition("pageshow", true));
	expect(reloadCalls).toBe(1);
	dom.dispatchEvent(transition("pagehide", false));
	expect(disposeCalls).toBe(1);
	expect(freeCalls).toBe(1);
});
