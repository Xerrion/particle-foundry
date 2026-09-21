import { createBrush } from "./brush";
import { measureWorld, type PhysicalTotals } from "./diagnostics";
import { createPhysics } from "./physics";
import { createRenderer, type PointerState } from "./renderer";
import { seedStarterScene } from "./starter-scene";
import { createVisualWaves } from "./visual-waves";
import { type CellReading, createWorld, type WorldOptions } from "./world";

export type { PointerState } from "./renderer";
export type { CellReading } from "./world";

export interface Sandbox {
	clear(): void;
	/** Returns a detached reading; invalid or out-of-bounds coordinates throw RangeError. */
	getCell(x: number, y: number): CellReading;
	getParticleCount(): number;
	getDiagnostics(): PhysicalTotals;
	/** Paints at integer coordinates; outside centers are ignored, invalid coordinates throw. */
	paintCircle(x: number, y: number): void;
	/** Paints a stroke; outside endpoints are ignored, invalid coordinates throw. */
	paintLine(fromX: number, fromY: number, toX: number, toY: number): void;
	render(ctx: CanvasRenderingContext2D, pointer: PointerState): void;
	seed(): void;
	/** Sets radius in integer 1..12; other values throw RangeError. */
	setBrushSize(size: number): void;
	/** Selects a known matter or tool ID; other values throw RangeError. */
	setMaterial(material: number): void;
	/** Opts an existing solid cell into or out of rigid-parcel mechanics. */
	setCellDynamic(x: number, y: number, movable: boolean): void;
	setWavesEnabled(enabled: boolean): void;
	setTemperatureMapEnabled(enabled: boolean): void;
	/** Advances one fixed 1/60-second model tick, independent of drawing. */
	step(): void;
}

/**
 * Composes a cellular world, physics, editing tools and independent visualization.
 * Dimensions must be integers in 1..32767. Passive steps conserve stored energy;
 * painting and combustion are explicit sources/sinks. No browser needed until render.
 */
export function createSandbox(width: number, height: number, options: WorldOptions = {}): Sandbox {
	const world = createWorld(width, height, options);
	const physics = createPhysics(world);
	const waves = createVisualWaves(world);
	const renderer = createRenderer(world, waves);
	const brush = createBrush(world, waves.disturb);

	function clear(): void {
		world.clear();
		physics.reset();
		waves.reset();
	}

	function seed(): void {
		seedStarterScene(world);
		physics.reset();
		waves.reset();
		waves.update();
	}

	function step(): void {
		physics.step();
		waves.update();
	}

	return {
		clear,
		seed,
		step,
		getCell: world.getCell,
		getParticleCount: world.getParticleCount,
		getDiagnostics: () => measureWorld(world),
		paintCircle: brush.paintCircle,
		paintLine: brush.paintLine,
		setBrushSize: brush.setSize,
		setMaterial: brush.setMaterial,
		setCellDynamic: (x, y, movable) => {
			if (!world.inBounds(x, y)) throw new RangeError("Dynamic cell is outside the world");
			world.setDynamic(x + y * world.width, movable);
		},
		setWavesEnabled: waves.setEnabled,
		setTemperatureMapEnabled: renderer.setTemperatureMapEnabled,
		render: (ctx, pointer) => renderer.render(ctx, pointer, brush.getSize()),
	};
}
