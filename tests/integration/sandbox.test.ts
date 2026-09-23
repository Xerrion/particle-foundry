import { describe, expect, test } from "bun:test";
import {
	COOLER,
	EMPTY,
	ERASER,
	FIRE,
	GLASS,
	HEATER,
	ICE,
	LAVA,
	METAL,
	MOLTEN_METAL,
	materialsById,
	SAND,
	SMOKE,
	STEAM,
	STONE,
	WATER,
	WOOD,
} from "../../src/materials";
import { boilingTemperatureAtPressure, energyAtTemperature } from "../../src/physics/thermal";
import { createSandbox, type Sandbox } from "../../src/simulation/sandbox";

const spawnMass = (material: number) => materialsById[material].densityKgPerM3 * 0.01 ** 3;
const spawnScale = (material: number) => spawnMass(material) / 0.001;
const ambientEnergy = energyAtTemperature(EMPTY, 22, undefined, spawnMass(EMPTY));

function paint(world: Sandbox, material: number, x = 0, y = 0): void {
	world.setMaterial(material);
	world.paintCircle(x, y);
}

function singleCell(material: number): Sandbox {
	const world = createSandbox(1, 1);
	world.setBrushSize(1);
	paint(world, material);
	return world;
}

function brush(world: Sandbox, tool: number, strokes: number): void {
	world.setMaterial(tool);
	for (let i = 0; i < strokes; i += 1) world.paintCircle(0, 0);
}

function trackedEnergy(world: Sandbox, _width: number, _height: number): number {
	return world.getDiagnostics().totalTrackedEnergyKj * 1000;
}

function totalEnergy(world: Sandbox, width: number, height: number): number {
	let total = 0;
	for (let y = 0; y < height; y += 1) {
		for (let x = 0; x < width; x += 1) total += world.getCell(x, y).energy;
	}
	return total;
}

describe("thermal tools and integrated phases", () => {
	test("works without a browser and starts with ambient air", () => {
		const world = createSandbox(3, 2);
		expect(world.getParticleCount()).toBe(0);
		expect(world.getCell(2, 1)).toEqual({
			material: EMPTY,
			temperature: 22,
			energy: ambientEnergy,
		});
		world.step();
		expect(totalEnergy(world, 3, 2)).toBeCloseTo(6 * ambientEnergy, 12);
	});

	test("heat and cool tools affect air without creating particles", () => {
		const world = singleCell(EMPTY);
		brush(world, HEATER, 1);
		expect(world.getCell(0, 0)).toEqual({
			material: EMPTY,
			temperature: 122,
			energy: energyAtTemperature(EMPTY, 122, undefined, spawnMass(EMPTY)),
		});
		brush(world, COOLER, 1);
		expect(world.getCell(0, 0).temperature).toBeCloseTo(22, 10);
		expect(world.getParticleCount()).toBe(0);
	});

	test("brushes complete latent transitions even while simulation is paused", () => {
		const world = singleCell(WATER);
		expect(world.getCell(0, 0).temperature).toBeCloseTo(22, 10);
		const initial = world.getCell(0, 0).energy;
		brush(world, HEATER, 4);
		expect(world.getCell(0, 0).temperature).toBeCloseTo(
			boilingTemperatureAtPressure(world.getCellPhysics(0, 0).pressurePa),
			10,
		);
		expect(world.getCell(0, 0).material).toBe(WATER);
		expect(world.getCell(0, 0).energy).toBeCloseTo(initial + 400 * spawnScale(WATER), 10);
		brush(world, HEATER, 22);
		expect(world.getCell(0, 0).material).toBe(STEAM);
		expect(world.getCell(0, 0).temperature).toBeGreaterThan(100);
		expect(world.getCell(0, 0).energy).toBeCloseTo(initial + 2600 * spawnScale(WATER), 10);
		brush(world, COOLER, 25);
		expect(world.getCell(0, 0).material).toBe(WATER);
		brush(world, COOLER, 6);
		expect(world.getCell(0, 0).material).toBe(ICE);
		expect(world.getCell(0, 0).temperature).toBeLessThan(0);
		expect(world.getParticleCount()).toBe(1);
	});

	test("ice absorbs latent energy at zero before melting", () => {
		const world = singleCell(ICE);
		brush(world, HEATER, 1);
		expect(world.getCell(0, 0).material).toBe(ICE);
		expect(world.getCell(0, 0).temperature).toBe(0);
		expect(world.getCell(0, 0).energy).toBeCloseTo(58 * spawnScale(ICE), 10);
		brush(world, HEATER, 3);
		expect(world.getCell(0, 0).material).toBe(WATER);
		expect(world.getCell(0, 0).energy).toBeCloseTo(358 * spawnScale(ICE), 10);
	});

	test("metal melts with latent heat and refreezes without losing energy", () => {
		const world = singleCell(METAL);
		brush(world, HEATER, 7);
		expect(world.getCell(0, 0).material).toBe(METAL);
		expect(world.getCell(0, 0).temperature).toBe(1538);
		brush(world, HEATER, 3);
		expect(world.getCell(0, 0).material).toBe(MOLTEN_METAL);
		brush(world, COOLER, 4);
		expect(world.getCell(0, 0).material).toBe(METAL);
		expect(world.getCell(0, 0).energy).toBeCloseTo(609.9 * spawnScale(METAL), 10);
	});

	test("lava solidifies and sand vitrifies irreversibly", () => {
		const lava = singleCell(LAVA);
		brush(lava, COOLER, 7);
		expect(lava.getCell(0, 0).material).toBe(STONE);
		expect(lava.getCell(0, 0).energy).toBeCloseTo(960 * spawnScale(LAVA), 10);
		brush(lava, HEATER, 7);
		expect(lava.getCell(0, 0).material).toBe(LAVA);

		const sand = singleCell(SAND);
		brush(sand, HEATER, 14);
		expect(sand.getCell(0, 0).material).toBe(GLASS);
		expect(sand.getCell(0, 0).energy).toBeCloseTo(1417.6 * spawnScale(SAND), 10);
		brush(sand, COOLER, 14);
		expect(sand.getCell(0, 0).material).toBe(GLASS);
		expect(sand.getCell(0, 0).temperature).toBeCloseTo(22, 10);
	});

	test("tools stop at their temperature bounds without corrupting the material ID", () => {
		for (const material of [EMPTY, WATER, METAL, SAND]) {
			const world = singleCell(material);
			brush(world, HEATER, 200);
			expect(world.getCell(0, 0).temperature).toBeCloseTo(3000, 10);
			brush(world, COOLER, 200);
			expect(world.getCell(0, 0).temperature).toBeCloseTo(-200, 10);
			expect(world.getCell(0, 0).material).toBeLessThan(15);
		}
	});

	test("steam does not disappear on a lifetime timer or cool in an insulated cell", () => {
		const world = singleCell(STEAM);
		const before = world.getCell(0, 0);
		for (let i = 0; i < 500; i += 1) world.step();
		expect(world.getCell(0, 0)).toEqual(before);
		expect(world.getParticleCount()).toBe(1);
	});
});

describe("conservative world integration", () => {
	test("diffusion actually warms neighbors and cools its source", () => {
		const world = createSandbox(5, 1);
		world.setBrushSize(1);
		paint(world, METAL, 1, 0);
		paint(world, METAL, 4, 0);
		paint(world, HEATER, 0, 0);
		const initial = trackedEnergy(world, 5, 1);
		const hot = world.getCell(1, 0).temperature;
		world.step();
		expect(world.getCell(1, 0).temperature).toBeLessThan(hot);
		expect(world.getCell(2, 0).temperature).toBeGreaterThan(22);
		expect(trackedEnergy(world, 5, 1)).toBeCloseTo(initial, 9);
	});

	test("hot gas traverses adjacent air without losing its displaced energy", () => {
		const world = createSandbox(1, 8);
		world.setBrushSize(1);
		paint(world, HEATER, 0, 4);
		paint(world, STEAM, 0, 7);
		const initial = trackedEnergy(world, 1, 8);
		world.step();
		expect(world.getCell(0, 4).material).toBe(STEAM);
		expect(world.getParticleCount()).toBe(2);
		expect(trackedEnergy(world, 1, 8)).toBeCloseTo(initial, 9);
	});

	test("steam cannot jump through a one-cell solid ceiling", () => {
		const world = createSandbox(1, 6);
		world.setBrushSize(1);
		paint(world, STONE, 0, 1);
		paint(world, ERASER, 0, 0);
		paint(world, STEAM, 0, 4);
		expect(world.getCell(0, 2).material).toBe(STONE);
		const initial = trackedEnergy(world, 1, 6);
		for (let i = 0; i < 40; i += 1) world.step();
		expect(world.getCell(0, 0).material).toBe(EMPTY);
		expect(world.getCell(0, 1).material).toBe(EMPTY);
		expect(world.getParticleCount()).toBe(4);
		expect(trackedEnergy(world, 1, 6)).toBeCloseTo(initial, 8);
	});

	test("mixed passive movement and phases conserve mass, heat and occupied cells", () => {
		const width = 24;
		const height = 16;
		const world = createSandbox(width, height);
		world.setBrushSize(1);
		const materials = [SAND, WATER, WATER, STONE, METAL, ICE, GLASS, MOLTEN_METAL, LAVA, STEAM];
		for (let i = 0; i < materials.length; i += 1) {
			paint(world, materials[i], 2 + (i % 5) * 4, 2 + Math.floor(i / 5) * 6);
		}
		const initial = trackedEnergy(world, width, height);
		const initialDiagnostics = world.getDiagnostics();
		const count = world.getParticleCount();
		for (let i = 0; i < 160; i += 1) {
			world.step();
			expect(world.getParticleCount()).toBe(count);
			const diagnostics = world.getDiagnostics();
			expect(diagnostics.totalMassKg).toBeCloseTo(initialDiagnostics.totalMassKg ?? 0, 12);
		}
		expect(trackedEnergy(world, width, height)).toBeCloseTo(initial, 4);
	});

	test("molten metal moves downward and stays in the metal phase family", () => {
		const world = createSandbox(1, 8);
		world.setBrushSize(1);
		paint(world, MOLTEN_METAL, 0, 0);
		const initial = trackedEnergy(world, 1, 8);
		world.step();
		expect(world.getCell(0, 2).material).toBe(MOLTEN_METAL);
		expect(world.getParticleCount()).toBe(2);
		expect(trackedEnergy(world, 1, 8)).toBeCloseTo(initial, 9);
	});

	test("a cold metal reservoir solidifies molten metal without energy loss", () => {
		const world = createSandbox(1, 8);
		world.setBrushSize(1);
		paint(world, MOLTEN_METAL, 0, 0);
		for (const y of [3, 5, 7]) paint(world, METAL, 0, y);
		const initial = trackedEnergy(world, 1, 8);
		for (let i = 0; i < 1000; i += 1) world.step();
		let solidCells = 0;
		for (let y = 0; y < 8; y += 1) {
			const cell = world.getCell(0, y);
			expect(cell.material).not.toBe(MOLTEN_METAL);
			if (cell.material === METAL) solidCells += 1;
		}
		expect(solidCells).toBe(8);
		expect(world.getParticleCount()).toBe(8);
		expect(trackedEnergy(world, 1, 8)).toBeCloseTo(initial, 8);
	});

	test("visual wave toggles change neither matter nor energy", () => {
		const world = createSandbox(8, 8);
		paint(world, WATER, 4, 4);
		const initial = trackedEnergy(world, 8, 8);
		const count = world.getParticleCount();
		world.setWavesEnabled(false);
		world.setWavesEnabled(true);
		expect(trackedEnergy(world, 8, 8)).toBe(initial);
		expect(world.getParticleCount()).toBe(count);
	});

	test("vented smoke carries its heat away while combustion adds explicit source energy", () => {
		const smoke = singleCell(SMOKE);
		for (let i = 0; i < 300; i += 1) smoke.step();
		expect(smoke.getCell(0, 0)).toEqual({
			material: EMPTY,
			temperature: 22,
			energy: ambientEnergy,
		});
		const wood = singleCell(WOOD);
		brush(wood, HEATER, 6);
		const initial = wood.getCell(0, 0).energy;
		wood.step();
		expect(wood.getCell(0, 0).material).toBe(WOOD);
		expect(wood.getCell(0, 0).energy).toBeCloseTo(initial + 100, 9);
		wood.step();
		expect(wood.getCell(0, 0).energy).toBeCloseTo(initial + 200, 9);
	});
});

describe("world API and reset", () => {
	test("cell APIs reject invalid coordinate pairs before they can alias another cell", () => {
		const sandbox = createSandbox(3, 3);
		const before = sandbox.getDiagnostics();
		for (const [x, y] of [
			[0.5, 0.5],
			[1.5, 0.5],
			[-1, 1],
			[3, 0],
			[NaN, 0],
			[0, Infinity],
		]) {
			expect(() => sandbox.getCell(x, y)).toThrow(RangeError);
			expect(() => sandbox.getCellPhysics(x, y)).toThrow(RangeError);
			expect(() => sandbox.setCellDynamic(x, y, true)).toThrow(RangeError);
		}
		expect(sandbox.getDiagnostics()).toEqual(before);
	});

	test("counts paint and erase immediately, independently of traversal", () => {
		const world = createSandbox(8, 8);
		world.setBrushSize(1);
		paint(world, SAND, 3, 3);
		expect(world.getParticleCount()).toBe(5);
		world.step();
		expect(world.getParticleCount()).toBe(5);
		world.clear();
		expect(world.getParticleCount()).toBe(0);
		expect(totalEnergy(world, 8, 8)).toBeCloseTo(64 * ambientEnergy, 12);
		paint(world, SAND, 3, 3);
		paint(world, ERASER, 3, 3);
		expect(world.getParticleCount()).toBe(0);
	});

	test("reset scene immediately supplies an accurate count and clears prior energy", () => {
		const world = createSandbox(240, 135);
		world.seed();
		const count = world.getParticleCount();
		const energy = totalEnergy(world, 240, 135);
		expect(count).toBeGreaterThan(1000);
		paint(world, HEATER, 100, 100);
		world.step();
		world.seed();
		expect(world.getParticleCount()).toBe(count);
		expect(totalEnergy(world, 240, 135)).toBe(energy);
	});

	test("opening the top and sides preserves the starter floor across resets", () => {
		const world = createSandbox(240, 135);
		world.seed();
		const ambientMaximumPressure = world.getDiagnostics().maximumPressurePa;
		expect(world.getCell(0, 134).material).toBe(STONE);
		world.setBoundariesEnabled(false);
		expect(world.getCell(0, 134).material).toBe(STONE);
		expect(world.getDiagnostics().maximumPressurePa).toBeLessThan(ambientMaximumPressure * 2);
		world.step();
		expect(world.getDiagnostics().maximumPressurePa).toBeLessThan(ambientMaximumPressure * 2);
		world.seed();
		expect(world.getCell(0, 134).material).toBe(STONE);
		world.setBoundariesEnabled(true);
		expect(world.getCell(0, 134).material).toBe(STONE);
		world.clear();
		world.setBoundariesEnabled(false);
		world.setBoundariesEnabled(true);
		expect(world.getCell(0, 134).material).toBe(EMPTY);
	});

	test("cell readings are detached and separate worlds cannot share state", () => {
		const first = singleCell(WATER);
		const second = singleCell(WATER);
		const reading = first.getCell(0, 0);
		Object.assign(reading, { material: FIRE, energy: Infinity });
		brush(first, HEATER, 1);
		expect(first.getCell(0, 0).material).toBe(WATER);
		expect(second.getCell(0, 0).temperature).toBeCloseTo(22, 10);
	});

	test("rejects invalid dimensions, materials, sizes and coordinates before mutation", () => {
		for (const value of [0, -1, 1.5, NaN, Infinity, 32768]) {
			expect(() => createSandbox(value, 1)).toThrow(RangeError);
			expect(() => createSandbox(1, value)).toThrow(RangeError);
		}
		const world = singleCell(SAND);
		for (const value of [-1, 200, 256, 1.5, NaN, Infinity]) {
			expect(() => world.setMaterial(value)).toThrow(RangeError);
		}
		for (const value of [0, -1, 13, 1.5, NaN, Infinity]) {
			expect(() => world.setBrushSize(value)).toThrow(RangeError);
		}
		for (const value of [0.5, NaN, Infinity]) {
			expect(() => world.paintCircle(value, 0)).toThrow(RangeError);
			expect(() => world.paintLine(0, 0, value, 0)).toThrow(RangeError);
		}
		expect(() => world.getCell(-1, 0)).toThrow(RangeError);
		expect(() => world.getCell(1, 0)).toThrow(RangeError);
		world.paintCircle(-1, 0);
		world.paintLine(0, 0, -1, 0);
		expect(world.getCell(0, 0).material).toBe(SAND);
	});
});
