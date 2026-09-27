import { describe, expect, test } from "bun:test";
import { createViewport, MAX_ZOOM } from "../../src/app/viewport";

describe("world viewport", () => {
	test("fits the whole world and maps its edges", () => {
		const view = createViewport(480, 270);
		expect([view.left, view.top, view.width, view.height]).toEqual([0, 0, 480, 270]);
		expect(view.worldAt(0, 0)).toEqual({ x: 0, y: 0 });
		expect(view.worldAt(0.5, 0.5)).toEqual({ x: 240, y: 135 });
		expect(view.worldAt(0.999, 0.999)).toEqual({ x: 479, y: 269 });
		expect(view.worldAt(1, 0)).toBeUndefined();
	});

	test("zoom keeps the selected point under the cursor and pan stays in bounds", () => {
		const view = createViewport(480, 270);
		const anchor = view.worldAt(0.25, 0.75);
		view.zoomAt(2, 0.25, 0.75);
		expect(view.worldAt(0.25, 0.75)).toEqual(anchor);
		expect([view.width, view.height]).toEqual([240, 135]);
		view.panBy(1, 1);
		expect(view.left).toBe(0);
		expect(view.top).toBe(0);
		view.panBy(-10, -10);
		expect(view.left + view.width).toBe(480);
		expect(view.top + view.height).toBe(270);
		view.zoomAt(100, 0.5, 0.5);
		expect(view.zoom).toBe(MAX_ZOOM);
		view.reset();
		expect([view.zoom, view.left, view.top]).toEqual([1, 0, 0]);
	});

	test("rejects invalid geometry and motion", () => {
		expect(() => createViewport(0, 270)).toThrow(RangeError);
		const view = createViewport(10, 10);
		expect(() => view.zoomAt(NaN, 0.5, 0.5)).toThrow(RangeError);
		expect(() => view.panBy(Infinity, 0)).toThrow(RangeError);
	});
});
