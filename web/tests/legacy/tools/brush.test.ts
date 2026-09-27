import { describe, expect, test } from "bun:test";
import {
	energyAtTemperature,
	initialTemperature,
	thermalMassScale,
} from "../../../src/legacy/physics/thermal";
import { createWorld, type World } from "../../../src/legacy/simulation/world";
import { type Brush, createBrush } from "../../../src/legacy/tools/brush";
import {
	COOLER,
	EMPTY,
	ERASER,
	FIRE,
	GUNPOWDER,
	HEATER,
	ICE,
	LAVA,
	METAL,
	MOLTEN_METAL,
	OIL,
	SAND,
	STEAM,
	WATER,
	WAVE_MATERIALS,
} from "../../../src/materials";

type Point = [x: number, y: number];
type Disturbance = [x: number, material: number, strength: number, radius: number];
type ParticleState = Pick<World, "grid" | "energy" | "lifetime" | "variation" | "moved">;

function createFixture(
	width: number = 9,
	height: number = 9,
): {
	world: World;
	brush: Brush;
	disturbances: Disturbance[];
} {
	const world = createWorld(width, height);
	const disturbances: Disturbance[] = [];
	const brush = createBrush(
		world,
		(x: number, material: number, strength: number, radius: number): void => {
			disturbances.push([x, material, strength, radius]);
		},
	);
	return { world, brush, disturbances };
}

function snapshot(world: World): ParticleState {
	return {
		grid: world.grid.slice(),
		energy: world.energy.slice(),
		lifetime: world.lifetime.slice(),
		variation: world.variation.slice(),
		moved: world.moved.slice(),
	};
}

describe("brush geometry and input boundary", (): void => {
	test("defaults to a radius-4 sand circle in the supplied world", (): void => {
		const { world, brush, disturbances } = createFixture(13, 13);
		expect(brush.getSize()).toBe(4);
		brush.paintCircle(6, 6);
		for (let y = 0; y < world.height; y += 1) {
			for (let x = 0; x < world.width; x += 1) {
				const expected = (x - 6) ** 2 + (y - 6) ** 2 <= 16 ? SAND : EMPTY;
				expect(world.getCell(x, y).material).toBe(expected);
			}
		}
		expect(disturbances).toEqual([]);
	});

	const lineCenters: Point[] = [
		[2, 2],
		[3, 3],
		[4, 3],
		[5, 4],
		[6, 4],
		[7, 5],
		[8, 5],
	];
	test.each([false, true])(
		"interpolates rounded line centers and both endpoints (steep=%s)",
		(isSteep: boolean): void => {
			for (const isReversed of [false, true]) {
				const { world, brush } = createFixture(11, 11);
				brush.setSize(1);
				brush.setMaterial(HEATER);
				const centers: Point[] = lineCenters.map(
					([x, y]: Point): Point => (isSteep ? [y, x] : [x, y]),
				);
				const [fromX, fromY] = centers[isReversed ? centers.length - 1 : 0];
				const [toX, toY] = centers[isReversed ? 0 : centers.length - 1];
				brush.paintLine(fromX, fromY, toX, toY);
				for (let y = 0; y < world.height; y += 1) {
					for (let x = 0; x < world.width; x += 1) {
						const hits = centers.filter(
							([cx, cy]: Point): boolean => (x - cx) ** 2 + (y - cy) ** 2 <= 1,
						).length;
						expect(world.getCell(x, y).energy).toBeCloseTo(
							(22 + hits * 100) * thermalMassScale(EMPTY, world.massKg[x + y * world.width]),
							10,
						);
					}
				}
			}
		},
	);

	test("paints a same-endpoint line exactly once", (): void => {
		const { world, brush } = createFixture(1, 1);
		brush.setMaterial(HEATER);
		brush.paintLine(0, 0, 0, 0);
		expect(world.getCell(0, 0)).toEqual({
			material: EMPTY,
			energy: energyAtTemperature(EMPTY, 122, undefined, world.massKg[0]),
			temperature: 122,
		});
	});

	test.each([
		[0, 0],
		[3, 0],
		[0, 3],
		[3, 3],
	])("clips circles at corner (%s, %s)", (x: number, y: number): void => {
		const { world, brush } = createFixture(4, 4);
		brush.setSize(1);
		brush.paintCircle(x, y);
		expect(world.getParticleCount()).toBe(3);
		expect(world.getCell(x, y).material).toBe(SAND);
		brush.setSize(12);
		expect(brush.getSize()).toBe(12);
		brush.paintCircle(x, y);
		expect(world.getParticleCount()).toBe(16);
	});

	test("ignores outside centers and either outside endpoint without clipping the line", (): void => {
		const { world, brush, disturbances } = createFixture();
		brush.setMaterial(WATER);
		const before = snapshot(world);
		const outside: Point[] = [
			[-1, 4],
			[9, 4],
			[4, -1],
			[4, 9],
			[Number.MAX_SAFE_INTEGER, 4],
		];
		for (const [x, y] of outside) {
			brush.paintCircle(x, y);
			brush.paintLine(x, y, 4, 4);
			brush.paintLine(4, 4, x, y);
		}
		expect(snapshot(world)).toEqual(before);
		expect(disturbances).toEqual([]);
	});

	test("rejects invalid coordinates in every argument before changing particle state", (): void => {
		const { world, brush, disturbances } = createFixture();
		world.setCell(0, FIRE);
		world.moved[0] = 1;
		brush.setMaterial(WATER);
		const before = snapshot(world);
		for (const value of [0.5, NaN, Infinity, -Infinity, Number.MAX_SAFE_INTEGER + 1]) {
			expect((): void => brush.paintCircle(value, 4)).toThrow(RangeError);
			expect((): void => brush.paintCircle(4, value)).toThrow(RangeError);
			expect((): void => brush.paintLine(value, 4, 4, 4)).toThrow(RangeError);
			expect((): void => brush.paintLine(4, value, 4, 4)).toThrow(RangeError);
			expect((): void => brush.paintLine(4, 4, value, 4)).toThrow(RangeError);
			expect((): void => brush.paintLine(4, 4, 4, value)).toThrow(RangeError);
			expect((): void => brush.paintLine(-1, 4, value, 4)).toThrow(RangeError);
			expect(snapshot(world)).toEqual(before);
			expect(disturbances).toEqual([]);
		}
	});

	test("invalid sizes and materials preserve the selection, radius, and particle state", (): void => {
		const { world, brush, disturbances } = createFixture();
		brush.setSize(1);
		brush.setMaterial(WATER);
		const before = snapshot(world);
		for (const size of [0, -1, 13, 1.5, NaN, Infinity, -Infinity]) {
			expect((): void => brush.setSize(size)).toThrow(RangeError);
			expect(brush.getSize()).toBe(1);
		}
		for (const material of [-1, 200, 251, 256, 1.5, 253.5, NaN, Infinity, -Infinity]) {
			expect((): void => brush.setMaterial(material)).toThrow(RangeError);
		}
		expect(snapshot(world)).toEqual(before);
		expect(disturbances).toEqual([]);
		brush.paintCircle(4, 4);
		expect(world.getParticleCount()).toBe(5);
		expect(world.getCell(4, 4).material).toBe(WATER);
		expect(disturbances).toEqual([[4, WATER, -0.65, 4]]);
	});
});

describe("brush energy and matter edits", (): void => {
	test("accepts every matter ID; ignition preserves hotter air while painting initializes energy", (): void => {
		const { world, brush } = createFixture(1, 1);
		for (let material = EMPTY; material <= GUNPOWDER; material += 1) {
			world.setCell(0, material === EMPTY ? SAND : EMPTY);
			world.energy[0] = 12345;
			brush.setMaterial(material);
			brush.paintCircle(0, 0);
			expect(world.grid[0]).toBe(material);
			expect(world.energy[0]).toBe(
				material === FIRE
					? 12345
					: energyAtTemperature(material, initialTemperature(material), undefined, world.massKg[0]),
			);
		}
		world.setCell(0, FIRE);
		brush.setMaterial(ERASER);
		brush.paintCircle(0, 0);
		expect(world.getCell(0, 0)).toEqual({
			material: EMPTY,
			energy: energyAtTemperature(EMPTY, 22, undefined, world.massKg[0]),
			temperature: 22,
		});
		expect(world.lifetime[0]).toBe(0);
	});

	test("thermal edits retain partial latent energy and resolve phases without reinitializing it", (): void => {
		const { world, brush, disturbances } = createFixture(1, 1);
		world.setCell(0, ICE);
		const scale = thermalMassScale(ICE, world.massKg[0]);
		const initialEnergy = world.energy[0];
		brush.setMaterial(HEATER);
		brush.paintCircle(0, 0);
		expect(world.getCell(0, 0)).toEqual({
			material: ICE,
			energy: initialEnergy + 100 * scale,
			temperature: 0,
		});
		for (let i = 0; i < 3; i += 1) brush.paintCircle(0, 0);
		expect(world.grid[0]).toBe(WATER);
		expect(world.energy[0]).toBe(initialEnergy + 400 * scale);
		world.energy[0] = 2908 * scale;
		brush.paintCircle(0, 0);
		expect(world.getCell(0, 0)).toEqual({
			material: STEAM,
			energy: 3008 * scale,
			temperature: 100,
		});
		brush.setMaterial(COOLER);
		brush.paintCircle(0, 0);
		expect(world.getCell(0, 0)).toEqual({
			material: STEAM,
			energy: 2908 * scale,
			temperature: 100,
		});
		expect(disturbances).toEqual([]);
	});

	test.each([EMPTY, WATER, METAL, SAND])(
		"repeated thermal tools respect temperature limits and never store tool IDs (matter=%s)",
		(material: number): void => {
			const { world, brush, disturbances } = createFixture(1, 1);
			world.setCell(0, material);
			for (const [tool, temperature] of [
				[HEATER, 3000],
				[COOLER, -200],
			]) {
				brush.setMaterial(tool);
				for (let i = 0; i < 200; i += 1) {
					brush.paintCircle(0, 0);
					expect(world.grid[0]).toBeGreaterThanOrEqual(EMPTY);
					expect(world.grid[0]).toBeLessThanOrEqual(MOLTEN_METAL);
				}
				expect(world.temperatureAt(0)).toBeCloseTo(temperature, 10);
				const atLimit = snapshot(world);
				brush.paintCircle(0, 0);
				expect(snapshot(world)).toEqual(atLimit);
			}
			expect(disturbances).toEqual([]);
		},
	);

	test.each([
		[HEATER, STEAM, 3100],
		[COOLER, ICE, -300],
	])(
		"tool %s does not clamp pre-existing energy beyond its bound",
		(tool: number, material: number, temperature: number): void => {
			const { world, brush } = createFixture(1, 1);
			world.setCell(0, material);
			world.energy[0] = energyAtTemperature(material, temperature, undefined, world.massKg[0]);
			const before = snapshot(world);
			brush.setMaterial(tool);
			brush.paintCircle(0, 0);
			expect(snapshot(world)).toEqual(before);
			brush.setMaterial(tool === HEATER ? COOLER : HEATER);
			brush.paintCircle(0, 0);
			expect(world.energy[0]).toBe(
				before.energy[0] +
					(tool === HEATER ? -100 : 100) * thermalMassScale(material, world.massKg[0]),
			);
		},
	);
});

describe("brush visual disturbance boundary", (): void => {
	test.each([...WAVE_MATERIALS])(
		"painting wave material %s emits one disturbance with the brush radius",
		(material: number): void => {
			const { brush, disturbances } = createFixture();
			brush.setMaterial(material);
			brush.paintCircle(4, 4);
			expect(disturbances).toEqual([[4, material, -0.65, 6]]);
		},
	);

	test.each([SAND, ERASER, EMPTY])(
		"replacement %s disturbs each displaced wave material once, excluding molten metal",
		(replacement: number): void => {
			const { world, brush, disturbances } = createFixture(3, 3);
			world.setCell(1, OIL);
			world.setCell(3, WATER);
			world.setCell(4, WATER);
			world.setCell(5, LAVA);
			world.setCell(7, MOLTEN_METAL);
			brush.setSize(1);
			brush.setMaterial(replacement);
			brush.paintCircle(1, 1);
			expect(disturbances).toEqual([
				[1, OIL, 1.15, 4],
				[1, WATER, 1.15, 4],
				[1, LAVA, 1.15, 4],
			]);
			expect(world.grid[4]).toBe(replacement === ERASER ? EMPTY : replacement);
		},
	);

	test("repainting a fluid is not displacement; a different fluid reports both disturbances", (): void => {
		const { world, brush, disturbances } = createFixture(1, 1);
		world.setCell(0, WATER);
		brush.setMaterial(WATER);
		brush.paintCircle(0, 0);
		expect(disturbances).toEqual([[0, WATER, -0.65, 6]]);
		disturbances.length = 0;
		brush.setMaterial(LAVA);
		brush.paintCircle(0, 0);
		expect(disturbances).toEqual([
			[0, LAVA, -0.65, 6],
			[0, WATER, 1.15, 6],
		]);
	});

	test("molten metal does not emit a painting disturbance", (): void => {
		const { brush, disturbances } = createFixture(1, 1);
		brush.setMaterial(MOLTEN_METAL);
		brush.paintCircle(0, 0);
		expect(disturbances).toEqual([]);
	});
});
