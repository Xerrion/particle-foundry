import { expect, test } from "bun:test";
import { selectCanvasContext } from "../../src/engine-client/canvas";

test("canvas ownership is selected before any context is acquired", () => {
	const calls: string[] = [];
	const context = {} as CanvasRenderingContext2D;
	const canvas = {
		getContext(kind: string) {
			calls.push(kind);
			return context;
		},
	} as unknown as HTMLCanvasElement;
	expect(selectCanvasContext(canvas, "2d")).toBe(context);
	expect(() => selectCanvasContext(canvas, "webgpu")).toThrow("already belongs to 2d");
	expect(calls).toEqual(["2d"]);
});
