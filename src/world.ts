import { materialsById } from "./material-physics";
import { EMPTY, isMatterId } from "./materials";
import { AMBIENT_TEMPERATURE_C, CELL_VOLUME_M3 } from "./physical-scale";
import { createSeededRandom, type RandomSource } from "./random";
import {
	energyAtTemperature,
	initialTemperature,
	phaseFromEnergy,
	temperatureFromEnergy,
} from "./thermal";

export interface CellReading {
	readonly material: number;
	readonly temperature: number;
	readonly energy: number;
}

export interface WorldLedger {
	massAddedKg: number;
	massRemovedKg: number;
	externalEnergyAdded: number;
	externalEnergyRemoved: number;
}

export interface WorldOptions {
	readonly seed?: number;
}

/** Internal simulation state. Arrays are owned here; physics modules share them, never mirror them. */
export interface World {
	readonly width: number;
	readonly height: number;
	readonly size: number;
	readonly grid: Uint8Array;
	readonly energy: Float64Array;
	readonly lifetime: Uint16Array;
	readonly variation: Uint8Array;
	readonly moved: Uint8Array;
	/** Authoritative transported physical fields; all move atomically with grid cells. */
	readonly massKg: Float64Array;
	readonly volumeM3: Float64Array;
	readonly velocityX: Float64Array;
	readonly velocityY: Float64Array;
	readonly chemicalEnergyKj: Float64Array;
	readonly oxygenKg: Float64Array;
	readonly pressurePa: Float64Array;
	readonly dynamic: Uint8Array;
	/** Burning fuel retains its solid/liquid identity until its chemical store is spent. */
	readonly burning: Uint8Array;
	/** Brush-created ignition gas fades back to air; it contains no soot-producing fuel. */
	readonly ignitionFlame: Uint8Array;
	readonly ledger: WorldLedger;
	readonly random: RandomSource;
	readonly visualRandom: RandomSource;
	inBounds(x: number, y: number): boolean;
	temperatureAt(index: number): number;
	/** Replaces matter without changing stored energy. Only trusted indices and matter IDs are accepted internally. */
	changeMaterial(index: number, material: number): void;
	/** Initializes matter and energy at its spawn temperature. */
	setCell(index: number, material: number): void;
	/** Marks a solid parcel as movable; terrain remains anchored by default. */
	setDynamic(index: number, movable: boolean): void;
	/** Replaces matter with ambient air, recording both the outgoing and incoming mass. */
	removeMatter(index: number): void;
	/** Records energy delivered by a brush or another explicit external source. */
	addExternalEnergy(index: number, delta: number): void;
	applyPhase(index: number): void;
	/** Swaps all particle properties, including air energy, and marks both positions processed. */
	swap(a: number, b: number): void;
	clear(): void;
	getCell(x: number, y: number): CellReading;
	getParticleCount(): number;
}

/** Rejects fractional or nonfinite coordinates before any brush or probe accesses the grid. */
export function requireCoordinates(x: number, y: number): void {
	if (!Number.isSafeInteger(x) || !Number.isSafeInteger(y)) {
		throw new RangeError("Cell coordinates must be safe integers");
	}
}

function lifetimeFor(material: number, random: RandomSource): number {
	const range = materialsById[material].lifetimeTicks;
	return range ? range[0] + Math.floor(random.next() * (range[1] - range[0])) : 0;
}

function initialMass(material: number): number {
	return materialsById[material].densityKgPerM3 * CELL_VOLUME_M3;
}

/** Creates an ambient-air grid; dimensions outside integer 1..32767 throw RangeError. */
export function createWorld(width: number, height: number, options: WorldOptions = {}): World {
	if (
		!Number.isInteger(width) ||
		!Number.isInteger(height) ||
		width < 1 ||
		height < 1 ||
		width > 32767 ||
		height > 32767
	) {
		throw new RangeError("World dimensions must be integers in 1..32767");
	}
	const size = width * height;
	const grid = new Uint8Array(size);
	const energy = new Float64Array(size).fill(energyAtTemperature(EMPTY, 22));
	const lifetime = new Uint16Array(size);
	const variation = new Uint8Array(size);
	const moved = new Uint8Array(size);
	const massKg = new Float64Array(size);
	const volumeM3 = new Float64Array(size);
	const velocityX = new Float64Array(size);
	const velocityY = new Float64Array(size);
	const chemicalEnergyKj = new Float64Array(size);
	const oxygenKg = new Float64Array(size);
	const pressurePa = new Float64Array(size);
	const dynamic = new Uint8Array(size);
	const burning = new Uint8Array(size);
	const ignitionFlame = new Uint8Array(size);
	const seed = options.seed ?? 0x5eed1234;
	const random = createSeededRandom(seed);
	const visualRandom = createSeededRandom(seed ^ 0x9e3779b9);
	const ledger: WorldLedger = {
		massAddedKg: 0,
		massRemovedKg: 0,
		externalEnergyAdded: 0,
		externalEnergyRemoved: 0,
	};

	function resetCellPhysics(index: number, material: number): void {
		massKg[index] = initialMass(material);
		volumeM3[index] = CELL_VOLUME_M3;
		velocityX[index] = 0;
		velocityY[index] = 0;
		chemicalEnergyKj[index] = massKg[index] * materialsById[material].chemicalEnergyKjPerKg;
		oxygenKg[index] = massKg[index] * materialsById[material].oxygenMassFraction;
		pressurePa[index] = 0;
		dynamic[index] = 0;
		burning[index] = 0;
		ignitionFlame[index] = 0;
	}

	for (let index = 0; index < size; index += 1) resetCellPhysics(index, EMPTY);

	function inBounds(x: number, y: number): boolean {
		return x >= 0 && x < width && y >= 0 && y < height;
	}

	function temperatureAt(index: number): number {
		return temperatureFromEnergy(grid[index], energy[index]);
	}

	function changeMaterial(index: number, material: number): void {
		grid[index] = material;
		burning[index] = 0;
		ignitionFlame[index] = 0;
		lifetime[index] = lifetimeFor(material, random);
		variation[index] = Math.floor(visualRandom.next() * 4);
		const density = materialsById[material].densityKgPerM3;
		volumeM3[index] = density > 0 ? massKg[index] / density : 0;
	}

	function setCell(index: number, material: number): void {
		if (!Number.isInteger(index) || index < 0 || index >= size || !isMatterId(material)) {
			throw new RangeError("Cell initialization requires an in-bounds index and a matter ID");
		}
		const previousMass = massKg[index];
		const initialEnergy = energyAtTemperature(material, initialTemperature(material));
		resetCellPhysics(index, material);
		changeMaterial(index, material);
		energy[index] = initialEnergy;
		const deltaMass = massKg[index] - previousMass;
		if (deltaMass >= 0) ledger.massAddedKg += deltaMass;
		else ledger.massRemovedKg -= deltaMass;
	}

	function setDynamic(index: number, movable: boolean): void {
		if (!Number.isSafeInteger(index) || index < 0 || index >= size) {
			throw new RangeError("Dynamic state requires an in-bounds index");
		}
		dynamic[index] = movable ? 1 : 0;
	}

	function removeMatter(index: number): void {
		if (!Number.isSafeInteger(index) || index < 0 || index >= size) {
			throw new RangeError("Matter removal requires an in-bounds index");
		}
		ledger.massRemovedKg += massKg[index];
		resetCellPhysics(index, EMPTY);
		changeMaterial(index, EMPTY);
		ledger.massAddedKg += massKg[index];
	}

	function addExternalEnergy(index: number, delta: number): void {
		if (!Number.isSafeInteger(index) || index < 0 || index >= size || !Number.isFinite(delta)) {
			throw new RangeError("External energy requires an in-bounds index and finite delta");
		}
		energy[index] += delta;
		if (delta >= 0) ledger.externalEnergyAdded += delta;
		else ledger.externalEnergyRemoved -= delta;
	}

	function applyPhase(index: number): void {
		const pressure = pressurePa[index] > 0 ? pressurePa[index] : undefined;
		const phase = phaseFromEnergy(grid[index], energy[index], pressure);
		if (phase !== grid[index]) changeMaterial(index, phase);
	}

	function swap(a: number, b: number): void {
		const material = grid[a];
		const age = lifetime[a];
		const cellEnergy = energy[a];
		const shade = variation[a];
		const physicalFields: Array<Float64Array | Uint8Array> = [
			massKg,
			volumeM3,
			velocityX,
			velocityY,
			chemicalEnergyKj,
			oxygenKg,
			pressurePa,
			dynamic,
			burning,
			ignitionFlame,
		];
		for (const field of physicalFields) {
			const value = field[a];
			field[a] = field[b];
			field[b] = value;
		}
		grid[a] = grid[b];
		lifetime[a] = lifetime[b];
		energy[a] = energy[b];
		variation[a] = variation[b];
		grid[b] = material;
		lifetime[b] = age;
		energy[b] = cellEnergy;
		variation[b] = shade;
		moved[a] = 1;
		moved[b] = 1;
	}

	function clear(): void {
		random.reset();
		visualRandom.reset();
		grid.fill(EMPTY);
		energy.fill(energyAtTemperature(EMPTY, AMBIENT_TEMPERATURE_C));
		lifetime.fill(0);
		variation.fill(0);
		moved.fill(0);
		for (let index = 0; index < size; index += 1) resetCellPhysics(index, EMPTY);
		ledger.massAddedKg = 0;
		ledger.massRemovedKg = 0;
		ledger.externalEnergyAdded = 0;
		ledger.externalEnergyRemoved = 0;
	}

	function getCell(x: number, y: number): CellReading {
		requireCoordinates(x, y);
		if (!inBounds(x, y)) throw new RangeError(`Cell (${x}, ${y}) is outside the world`);
		const index = x + y * width;
		return {
			material: grid[index],
			energy: energy[index],
			temperature: temperatureAt(index),
		};
	}

	function getParticleCount(): number {
		let count = 0;
		for (const material of grid) if (material !== EMPTY) count += 1;
		return count;
	}

	return {
		width,
		height,
		size,
		grid,
		energy,
		lifetime,
		variation,
		moved,
		massKg,
		volumeM3,
		velocityX,
		velocityY,
		chemicalEnergyKj,
		oxygenKg,
		pressurePa,
		dynamic,
		burning,
		ignitionFlame,
		ledger,
		random,
		visualRandom,
		inBounds,
		temperatureAt,
		changeMaterial,
		setCell,
		setDynamic,
		removeMatter,
		addExternalEnergy,
		applyPhase,
		swap,
		clear,
		getCell,
		getParticleCount,
	};
}
