import { describe, expect, test } from "bun:test";
import { pressureColor, velocityColor } from "../src/field-map";
import {
	EMPTY,
	LAVA,
	METAL,
	MOLTEN_METAL,
	OIL,
	palettes,
	SAND,
	STONE,
	WATER,
} from "../src/materials";
import { createPhysics } from "../src/physics";
import { createRenderer, type PointerState } from "../src/renderer";
import { TEMPERATURE_SCALE, temperatureColor } from "../src/temperature-map";
import { energyAtTemperature } from "../src/thermal";
import { createVisualWaves } from "../src/visual-waves";
import { createWorld } from "../src/world";

type MinimalCanvas = Pick<
	CanvasRenderingContext2D,
	"putImageData" | "save" | "restore" | "beginPath" | "arc" | "stroke" | "strokeStyle" | "lineWidth"
> & { createImageData(width: number, height: number): ImageData };

function canvasMock() {
	const buffers: Uint8ClampedArray[] = [];
	const images: ImageData[] = [];
	const origins: number[][] = [];
	const arcs: number[][] = [];
	const calls: string[] = [];
	let channelWrites = 0;
	const context: MinimalCanvas = {
		strokeStyle: "",
		lineWidth: 1,
		createImageData(width: number, height: number): ImageData {
			const buffer = new Uint8ClampedArray(width * height * 4);
			buffers.push(buffer);
			const data = new Proxy(buffer, {
				set(target, property, value: unknown): boolean {
					if (typeof value !== "number" || !Number.isFinite(value)) {
						throw new RangeError(`Nonfinite pixel channel: ${String(value)}`);
					}
					channelWrites += 1;
					return Reflect.set(target, property, value);
				},
			});
			return { width, height, colorSpace: "srgb", data };
		},
		putImageData(image: ImageData, x: number, y: number): void {
			images.push(image);
			origins.push([x, y]);
			calls.push("putImageData");
		},
		save(): void {
			calls.push("save");
		},
		restore(): void {
			calls.push("restore");
		},
		beginPath(): void {
			calls.push("beginPath");
		},
		arc(x: number, y: number, radius: number, start: number, end: number): void {
			arcs.push([x, y, radius, start, end]);
			calls.push("arc");
		},
		stroke(): void {
			calls.push("stroke");
		},
	};
	return {
		// Only the canvas operations used by the renderer exist in this headless mock.
		ctx: context as CanvasRenderingContext2D,
		buffers,
		images,
		origins,
		arcs,
		calls,
		channelWrites: () => channelWrites,
	};
}

const outside: PointerState = { isInside: false, x: -1, y: -1 };

function pixel(buffer: Uint8ClampedArray, width: number, x: number, y: number): number[] {
	const start = (x + y * width) * 4;
	return Array.from(buffer.slice(start, start + 4));
}

describe("headless world renderer", () => {
	test("pressure and speed maps use model fields without changing them", () => {
		const world = createWorld(3, 1);
		world.setCell(1, WATER);
		world.setCell(2, STONE);
		world.pressurePa.set([101_325, 111_325, 0]);
		world.velocityX[1] = 0.3;
		world.velocityY[1] = 0.4;
		const beforePressure = world.pressurePa.slice();
		const beforeX = world.velocityX.slice();
		const beforeY = world.velocityY.slice();
		const renderer = createRenderer(world, createVisualWaves(world));
		const canvas = canvasMock();
		renderer.setViewMode("pressure");
		renderer.render(canvas.ctx, outside, 1);
		expect(pixel(canvas.buffers[0], 3, 0, 0)).toEqual([...pressureColor(101_325), 255]);
		expect(pixel(canvas.buffers[0], 3, 1, 0)).toEqual([...pressureColor(111_325), 255]);
		expect(pixel(canvas.buffers[0], 3, 2, 0)).toEqual([18, 25, 30, 255]);
		renderer.setViewMode("velocity");
		renderer.render(canvas.ctx, outside, 1);
		expect(pixel(canvas.buffers[0], 3, 1, 0)).toEqual([...velocityColor(0.3, 0.4), 255]);
		expect(world.pressurePa).toEqual(beforePressure);
		expect(world.velocityX).toEqual(beforeX);
		expect(world.velocityY).toEqual(beforeY);
	});

	test("a moving lava parcel keeps its shade instead of blinking between palette entries", () => {
		const world = createWorld(7, 1);
		world.setCell(0, LAVA);
		const waves = createVisualWaves(world);
		waves.setEnabled(false);
		const renderer = createRenderer(world, waves);
		const canvas = canvasMock();
		renderer.render(canvas.ctx, outside, 1);
		const before = pixel(canvas.buffers[0], 7, 0, 0);
		for (let x = 1; x < 7; x++) {
			world.swap(x - 1, x);
			renderer.render(canvas.ctx, outside, 1);
			expect(pixel(canvas.buffers[0], 7, x, 0)).toEqual(before);
		}
	});

	test("a settled lava pool renders identically across physics ticks, including its partial top row", () => {
		const world = createWorld(12, 6);
		for (let y = 0; y < 6; y++) {
			for (let x = 0; x < 12; x++) {
				const material = y === 5 ? STONE : y >= 3 || (y === 2 && x % 2 === 0) ? LAVA : EMPTY;
				const index = x + y * 12;
				world.setCell(index, material);
				// Isolate movement and rendering from genuine cooling. Both rock
				// phases are stable at their respective latent-heat endpoints.
				world.energy[index] = energyAtTemperature(material, 1200);
			}
		}
		const physics = createPhysics(world);
		const waves = createVisualWaves(world);
		waves.update();
		const renderer = createRenderer(world, waves);
		const canvas = canvasMock();
		renderer.render(canvas.ctx, outside, 1);
		const before = canvas.buffers[0].slice();
		for (let tick = 0; tick < 120; tick++) {
			physics.step();
			waves.update();
			renderer.render(canvas.ctx, outside, 1);
			expect(canvas.buffers[0]).toEqual(before);
		}
	});

	test("temperature mode includes air, matches the legend and never mutates simulation state", () => {
		const world = createWorld(TEMPERATURE_SCALE.length, 1);
		for (let i = 0; i < world.size; i++) {
			world.setCell(i, i % 2 ? WATER : EMPTY);
			world.energy[i] = energyAtTemperature(world.grid[i], TEMPERATURE_SCALE[i].celsius);
		}
		const energy = world.energy.slice();
		const grid = world.grid.slice();
		const renderer = createRenderer(world, createVisualWaves(world));
		const canvas = canvasMock();
		renderer.render(canvas.ctx, outside, 1);
		const ordinary = canvas.buffers[0].slice();
		renderer.setTemperatureMapEnabled(true);
		renderer.render(canvas.ctx, outside, 1);
		for (let i = 0; i < world.size; i++) {
			expect(pixel(canvas.buffers[0], world.width, i, 0)).toEqual([
				...TEMPERATURE_SCALE[i].color,
				255,
			]);
		}
		expect(world.energy).toEqual(energy);
		expect(world.grid).toEqual(grid);
		renderer.setTemperatureMapEnabled(false);
		renderer.render(canvas.ctx, outside, 1);
		expect(canvas.buffers[0]).toEqual(ordinary);
		expect(temperatureColor(-1000)).toEqual(TEMPERATURE_SCALE[0].color);
		expect(temperatureColor(5000)).toEqual(TEMPERATURE_SCALE[TEMPERATURE_SCALE.length - 1].color);
		expect(temperatureColor(61)).toEqual([142, 208, 125]);
		expect(() => temperatureColor(NaN)).toThrow(RangeError);
	});
	test("imports and constructs without browser APIs, allocates lazily and reuses its image", () => {
		expect(typeof document).toBe("undefined");
		expect(typeof ImageData).toBe("undefined");
		const world = createWorld(3, 2);
		const canvas = canvasMock();
		const renderer = createRenderer(world, createVisualWaves(world));
		expect(canvas.buffers).toHaveLength(0);
		renderer.render(canvas.ctx, outside, 4);
		renderer.render(canvas.ctx, outside, 4);
		expect(canvas.buffers).toHaveLength(1);
		expect(canvas.images[0]).toBe(canvas.images[1]);
		expect(canvas.images[0].width).toBe(3);
		expect(canvas.images[0].height).toBe(2);
		expect(canvas.origins).toEqual([
			[0, 0],
			[0, 0],
		]);
		expect(canvas.channelWrites()).toBe(world.size * 4 * 2);
		expect(pixel(canvas.buffers[0], 3, 0, 0)).toEqual([6, 13, 18, 255]);
		expect(pixel(canvas.buffers[0], 3, 1, 0)).toEqual([5, 12, 17, 255]);
	});

	test("preserves all palettes, variation indexing and opaque background noise without NaNs", () => {
		const world = createWorld(MOLTEN_METAL + 1, 4);
		for (let i = 0; i < world.size; i += 1) {
			const material = i % world.width;
			world.setCell(i, material);
			world.energy[i] = energyAtTemperature(material, 22);
			world.variation[i] = Math.floor(i / world.width);
		}
		const waves = createVisualWaves(world);
		waves.setEnabled(false);
		const canvas = canvasMock();
		createRenderer(world, waves).render(canvas.ctx, outside, 4);
		for (let y = 0; y < world.height; y += 1) {
			for (let x = 0; x < world.width; x += 1) {
				const noise = (x * 13 + y * 7) % 19 === 0 ? 1 : 0;
				const color = x === EMPTY ? [5 + noise, 12 + noise, 17 + noise] : palettes[x][y % 4];
				expect(pixel(canvas.buffers[0], world.width, x, y)).toEqual([...color, 255]);
			}
		}
		expect(canvas.channelWrites()).toBe(world.size * 4);
	});

	test.each([
		[METAL, 499, [151, 170, 175, 255]],
		[METAL, 500, [151, 170, 175, 255]],
		[METAL, 1100, [198, 132, 115, 255]],
		[METAL, 1300, [214, 119, 95, 255]],
		[METAL, 1400, [229, 212, 138, 255]],
		[METAL, 1700, [255, 226, 125, 255]],
		[MOLTEN_METAL, 1700, [255, 226, 125, 255]],
		[MOLTEN_METAL, 3000, [255, 226, 125, 255]],
		[SAND, 3000, [244, 207, 102, 255]],
	] satisfies [number, number, number[]][])(
		"preserves heat glow for material %i at %i C",
		(material, temperature, expected) => {
			const world = createWorld(1, 1);
			world.setCell(0, material);
			world.variation[0] = 0;
			world.energy[0] = energyAtTemperature(material, temperature);
			const canvas = canvasMock();
			createRenderer(world, createVisualWaves(world)).render(canvas.ctx, outside, 1);
			expect(Array.from(canvas.buffers[0])).toEqual(expected);
		},
	);

	test.each([WATER, OIL, LAVA])(
		"lights real surfaces without inventing crests for material %i",
		(material) => {
			const world = createWorld(3, 6);
			for (let i = 9; i < world.size; i += 1) world.setCell(i, material);
			world.variation.fill(0);
			const waves = createVisualWaves(world);
			waves.update();
			const renderer = createRenderer(world, waves);
			const canvas = canvasMock();
			const base = palettes[material][0];
			const highlight = [
				Math.min(255, base[0] + 18),
				Math.min(255, base[1] + 24),
				Math.min(255, base[2] + 28),
				255,
			];
			renderer.render(canvas.ctx, outside, 1);
			expect(pixel(canvas.buffers[0], 3, 1, 3)).toEqual(highlight);
			expect(pixel(canvas.buffers[0], 3, 1, 4)).toEqual([...base, 255]);
			waves.disturb(1, material, -1, 0);
			waves.update();
			renderer.render(canvas.ctx, outside, 1);
			expect(pixel(canvas.buffers[0], 3, 1, 2)).toEqual([5, 12, 17, 255]);
			expect(pixel(canvas.buffers[0], 3, 1, 3)[1]).toBeGreaterThan(base[1]);
			world.setCell(7, STONE);
			world.variation[7] = 0;
			renderer.render(canvas.ctx, outside, 1);
			expect(pixel(canvas.buffers[0], 3, 1, 2)).toEqual([...palettes[STONE][0], 255]);
			waves.setEnabled(false);
			renderer.render(canvas.ctx, outside, 1);
			expect(pixel(canvas.buffers[0], 3, 1, 3)).toEqual([...base, 255]);
		},
	);

	test("never cuts visual wave troughs into actual water", () => {
		const world = createWorld(1, 5);
		world.setCell(2, WATER);
		world.setCell(3, WATER);
		world.setCell(4, WATER);
		world.variation.fill(0);
		const waves = createVisualWaves(world);
		waves.update();
		waves.disturb(0, WATER, 1, 0);
		waves.update();
		const canvas = canvasMock();
		createRenderer(world, waves).render(canvas.ctx, outside, 1);
		expect(pixel(canvas.buffers[0], 1, 0, 2)[2]).toBeGreaterThan(palettes[WATER][0][2]);
		expect(pixel(canvas.buffers[0], 1, 0, 3)).toEqual([...palettes[WATER][0], 255]);
		expect(world.grid[2]).toBe(WATER);
	});

	test("rendering changes neither world fields nor visual motion", () => {
		const world = createWorld(3, 6);
		for (let i = 9; i < world.size; i += 1) world.setCell(i, WATER);
		const waves = createVisualWaves(world);
		waves.update();
		waves.disturb(1, WATER, -1, 0);
		waves.update();
		const fields = [world.grid, world.energy, world.lifetime, world.variation, world.moved];
		const before = fields.map((field) => Array.from(field));
		const canvas = canvasMock();
		const renderer = createRenderer(world, waves);
		renderer.render(canvas.ctx, outside, 4);
		const firstPixels = Array.from(canvas.buffers[0]);
		for (let frame = 0; frame < 5; frame += 1) renderer.render(canvas.ctx, outside, 4);
		expect(Array.from(canvas.buffers[0])).toEqual(firstPixels);
		expect(fields.map((field) => Array.from(field))).toEqual(before);
		expect(waves.isSurface(1, 3, WATER)).toBe(true);
	});

	test("draws the unchanged cursor style at the supplied brush radius", () => {
		const world = createWorld(3, 3);
		const canvas = canvasMock();
		const renderer = createRenderer(world, createVisualWaves(world));
		const pointer: PointerState = { isInside: true, x: 1, y: 2 };
		renderer.render(canvas.ctx, pointer, 4);
		expect(canvas.calls).toEqual(["putImageData", "save", "beginPath", "arc", "stroke", "restore"]);
		expect(canvas.arcs).toEqual([[1.5, 2.5, 4.5, 0, Math.PI * 2]]);
		expect(canvas.ctx.strokeStyle).toBe("rgba(255,255,255,0.78)");
		expect(canvas.ctx.lineWidth).toBe(0.7);
		expect(pointer).toEqual({ isInside: true, x: 1, y: 2 });
		renderer.render(canvas.ctx, { isInside: true, x: -1, y: 0 }, 4);
		renderer.render(canvas.ctx, outside, 4);
		expect(canvas.arcs).toHaveLength(1);
		renderer.render(canvas.ctx, pointer, 2);
		expect(canvas.arcs[1][2]).toBe(2.5);
	});

	test("renderer instances own separate image buffers and see current world edits", () => {
		const first = createWorld(1, 1);
		const second = createWorld(2, 1);
		first.setCell(0, SAND);
		first.variation[0] = 0;
		const firstCanvas = canvasMock();
		const secondCanvas = canvasMock();
		const firstRenderer = createRenderer(first, createVisualWaves(first));
		const secondRenderer = createRenderer(second, createVisualWaves(second));
		firstRenderer.render(firstCanvas.ctx, outside, 1);
		secondRenderer.render(secondCanvas.ctx, outside, 1);
		expect(firstCanvas.images[0]).not.toBe(secondCanvas.images[0]);
		expect(firstCanvas.buffers[0]).toHaveLength(4);
		expect(secondCanvas.buffers[0]).toHaveLength(8);
		expect(Array.from(firstCanvas.buffers[0])).toEqual([244, 207, 102, 255]);
		first.clear();
		firstRenderer.render(firstCanvas.ctx, outside, 1);
		expect(Array.from(firstCanvas.buffers[0])).toEqual([6, 13, 18, 255]);
		expect(pixel(secondCanvas.buffers[0], 2, 1, 0)).toEqual([5, 12, 17, 255]);
	});

	test("propagates invalid world energy instead of silently rendering corrupt colors", () => {
		const world = createWorld(1, 1);
		world.setCell(0, METAL);
		world.energy[0] = NaN;
		const canvas = canvasMock();
		expect(() =>
			createRenderer(world, createVisualWaves(world)).render(canvas.ctx, outside, 1),
		).toThrow(RangeError);
		expect(canvas.images).toHaveLength(0);
	});
});
