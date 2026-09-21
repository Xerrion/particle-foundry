import { describe, expect, test } from "bun:test";
import { mapCanvasPointer } from "../../src/app/pointer-mapping";

const fullscreen = {
	left: 0,
	top: 0,
	width: 1200,
	height: 1000,
	columns: 240,
	rows: 135,
	fit: "contain" as const,
};

describe("canvas image coordinates", () => {
	test("subtracts fullscreen horizontal letterboxes before mapping rows", () => {
		expect(mapCanvasPointer({ x: 600, y: 200 }, fullscreen)).toEqual({
			isInside: true,
			x: 120,
			y: 7,
		});
		expect(mapCanvasPointer({ x: 0, y: 162.5 }, fullscreen)).toEqual({
			isInside: true,
			x: 0,
			y: 0,
		});
		expect(mapCanvasPointer({ x: 1199, y: 837 }, fullscreen)).toEqual({
			isInside: true,
			x: 239,
			y: 134,
		});
		for (const y of [0, 162.4, 837.5, 999]) {
			expect(mapCanvasPointer({ x: 600, y }, fullscreen).isInside).toBe(false);
		}
	});

	test("rejects vertical pillarboxes on wide screens", () => {
		const wide = { ...fullscreen, width: 1600, height: 675 };
		expect(mapCanvasPointer({ x: 200, y: 0 }, wide)).toEqual({
			isInside: true,
			x: 0,
			y: 0,
		});
		expect(mapCanvasPointer({ x: 800, y: 335 }, wide)).toEqual({
			isInside: true,
			x: 120,
			y: 67,
		});
		for (const x of [0, 199.9, 1400, 1599]) {
			expect(mapCanvasPointer({ x, y: 335 }, wide).isInside).toBe(false);
		}
	});

	test("respects viewport offsets and normal fill sizing", () => {
		const normal = {
			...fullscreen,
			left: 20,
			top: 50,
			width: 480,
			height: 270,
			fit: "fill" as const,
		};
		expect(mapCanvasPointer({ x: 260, y: 184 }, normal)).toEqual({
			isInside: true,
			x: 120,
			y: 67,
		});
		expect(mapCanvasPointer({ x: 20, y: 50 }, normal)).toEqual({
			isInside: true,
			x: 0,
			y: 0,
		});
		expect(mapCanvasPointer({ x: 500, y: 50 }, normal).isInside).toBe(false);
		expect(mapCanvasPointer({ x: 20, y: 320 }, normal).isInside).toBe(false);
		expect(mapCanvasPointer({ x: 19.9, y: 50 }, normal).isInside).toBe(false);
		expect(mapCanvasPointer({ x: 20, y: 49.9 }, normal).isInside).toBe(false);
		expect(mapCanvasPointer({ x: 600, y: 200 }, { ...fullscreen, fit: "fill" }).y).toBe(27);
	});

	test("invalid or hidden geometry cannot produce a usable pointer", () => {
		for (const width of [0, -1, NaN, Infinity]) {
			expect(mapCanvasPointer({ x: 0, y: 0 }, { ...fullscreen, width }).isInside).toBe(false);
		}
		for (const rows of [0, -1, 1.5, NaN, Infinity]) {
			expect(mapCanvasPointer({ x: 0, y: 0 }, { ...fullscreen, rows }).isInside).toBe(false);
		}
		expect(mapCanvasPointer({ x: NaN, y: 200 }, fullscreen).isInside).toBe(false);
	});
});
