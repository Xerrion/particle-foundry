import {
	createRenderer,
	type PointerState,
	type ViewMode,
	type VisibleWorldRect,
} from "../rendering/renderer";
import { createVisualWaves } from "../rendering/visual-waves";
import { seedStarterScene } from "../scenes/starter-scene";
import { createBrush } from "../tools/brush";
import { measureWorld, type PhysicalTotals } from "./diagnostics";
import { createPhysics } from "./physics";
import { type CellReading, createWorld, type WorldOptions } from "./world";

export type { PointerState } from "../rendering/renderer";
export type { CellReading } from "./world";

export interface Sandbox {
	clear(): void;
	/** Returns a detached reading; invalid or out-of-bounds coordinates throw RangeError. */
	getCell(x: number, y: number): CellReading;
	getCellPhysics(
		x: number,
		y: number,
	): { pressurePa: number; velocityX: number; velocityY: number };
	getParticleCount(): number;
	getDiagnostics(): PhysicalTotals;
	/** Paints at integer coordinates; outside centers are ignored, invalid coordinates throw. */
	paintCircle(x: number, y: number): void;
	/** Paints a stroke; outside endpoints are ignored, invalid coordinates throw. */
	paintLine(fromX: number, fromY: number, toX: number, toY: number): void;
	render(ctx: CanvasRenderingContext2D, pointer: PointerState, visible?: VisibleWorldRect): void;
	seed(): void;
	/** Sets radius in integer 1..12; other values throw RangeError. */
	setBrushSize(size: number): void;
	/** Selects a known matter or tool ID; other values throw RangeError. */
	setMaterial(material: number): void;
	/** Opts a solid cell into rigid-parcel mechanics; invalid/outside coordinates throw. */
	setCellDynamic(x: number, y: number, movable: boolean): void;
	setWavesEnabled(enabled: boolean): void;
	setBoundariesEnabled(enabled: boolean): void;
	setTemperatureMapEnabled(enabled: boolean): void;
	setViewMode(mode: ViewMode): void;
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
	const brush = createBrush(world, waves.disturb, physics.blast);

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

	function setBoundariesEnabled(enabled: boolean): void {
		if (world.boundariesEnabled === enabled) return;
		world.boundariesEnabled = enabled;
		physics.refreshPressure();
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
		getCellPhysics: (x, y) => {
			const index = world.indexAt(x, y);
			return {
				pressurePa: world.pressurePa[index],
				velocityX: world.velocityX[index],
				velocityY: world.velocityY[index],
			};
		},
		getParticleCount: world.getParticleCount,
		getDiagnostics: () => measureWorld(world),
		paintCircle: (x, y) => {
			brush.paintCircle(x, y);
			physics.refreshPressure();
		},
		paintLine: (fromX, fromY, toX, toY) => {
			brush.paintLine(fromX, fromY, toX, toY);
			physics.refreshPressure();
		},
		setBrushSize: brush.setSize,
		setMaterial: brush.setMaterial,
		setCellDynamic: (x, y, movable) => {
			world.setDynamic(world.indexAt(x, y), movable);
		},
		setWavesEnabled: waves.setEnabled,
		setBoundariesEnabled,
		setTemperatureMapEnabled: renderer.setTemperatureMapEnabled,
		setViewMode: renderer.setViewMode,
		render: (ctx, pointer, visible) => renderer.render(ctx, pointer, brush.getSize(), visible),
	};
}
