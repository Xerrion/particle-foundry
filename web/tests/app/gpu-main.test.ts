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
	render: () => {
		renderCalls++;
		return true;
	},
	resize: () => {},
	advance: async () => status(),
	paint: () => status(),
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

beforeAll(async () => {
	dom.document.write(await Bun.file(new URL("../../gpu.html", import.meta.url)).text());
	for (const [key, value] of Object.entries({
		window: dom,
		document: dom.document,
		location: dom.location,
		requestAnimationFrame: (_callback: FrameRequestCallback): number => 1,
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

test("reloads a freed scene on a back-forward cache restore", () => {
	dom.dispatchEvent(transition("pagehide", true));
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
