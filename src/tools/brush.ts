import {
	BLAST,
	CARBON,
	COOLER,
	combustionProfile,
	EMPTY,
	ERASER,
	FIRE,
	HEATER,
	HYDROGEN,
	isMatterId,
	LIQUID_SULFUR,
	materialsById,
	OXYGEN,
	SAND,
	SULFUR,
	SULFUR_VAPOR,
	WAVE_MATERIALS,
} from "../materials";
import { createExplosions } from "../physics/explosions";
import { energyAtTemperature, thermalMassScale } from "../physics/thermal";
import { requireCoordinates, type World } from "../simulation/world";

export interface Brush {
	/** Applies the selected brush. Unsafe or noninteger coordinates throw; outside centers are ignored. */
	paintCircle(x: number, y: number): void;
	/** Paints both endpoints and the points between them. Invalid coordinates throw; outside endpoints are ignored. */
	paintLine(fromX: number, fromY: number, toX: number, toY: number): void;
	/** Sets an integer radius in 1..12; other values throw RangeError. */
	setSize(size: number): void;
	/** Selects a matter or tool ID; unknown or noninteger IDs throw RangeError. */
	setMaterial(material: number): void;
	getSize(): number;
}

const waveMaterials: readonly number[] = WAVE_MATERIALS;

/**
 * Creates a radius-4 sand brush that edits the supplied world without copying it.
 * Paint and erase initialize energy; thermal tools add or remove up to 100 J per reference gram
 * per stroke, bounded by -200/3000 C, and preserve stored energy through phase changes.
 * The fire brush heats fuel or air, creating brief ignition gas without adding soot.
 * Calls onDisturb only for painted or displaced wave materials, after each circle.
 */
export function createBrush(
	world: World,
	onDisturb: (x: number, material: number, strength: number, radius: number) => void,
	onBlast: (x: number, y: number, radius: number) => void = createExplosions(world).blast,
): Brush {
	let selected = SAND;
	let brushSize = 4;

	function applyThermalBrush(index: number): void {
		const material = world.grid[index];
		const pressure = world.pressurePa[index] || undefined;
		const strokeEnergy = 100 * thermalMassScale(material, world.massKg[index]);
		const minimum = energyAtTemperature(material, -200, pressure, world.massKg[index]);
		const maximum = energyAtTemperature(material, 3000, pressure, world.massKg[index]);
		// Bounds restrict only tool input, never passive diffusion or phase changes.
		if (selected === HEATER && world.energy[index] < maximum) {
			world.addExternalEnergy(index, Math.min(strokeEnergy, maximum - world.energy[index]));
		} else if (selected === COOLER && world.energy[index] > minimum) {
			world.addExternalEnergy(index, -Math.min(strokeEnergy, world.energy[index] - minimum));
		}
		world.applyPhase(index);
	}

	function paintCell(x: number, y: number, displacedFluids: Set<number>, isCenter: boolean): void {
		if (!world.inBounds(x, y)) return;
		const index = x + y * world.width;
		if (selected === HEATER || selected === COOLER) {
			applyThermalBrush(index);
			return;
		}
		const existing = world.grid[index];
		if (selected === FIRE) {
			// Oxygen is an oxidizer, not fuel. A spark on its side of a
			// hydrogen/oxygen interface must still heat the reaction contact.
			if (existing === OXYGEN) {
				const touchingHydrogen =
					(x > 0 && world.grid[index - 1] === HYDROGEN) ||
					(x + 1 < world.width && world.grid[index + 1] === HYDROGEN) ||
					(y > 0 && world.grid[index - world.width] === HYDROGEN) ||
					(y + 1 < world.height && world.grid[index + world.width] === HYDROGEN);
				if (touchingHydrogen) {
					const target =
						materialsById[HYDROGEN].ignitionTemperatureC + combustionProfile.ignitionMarginC;
					const needed =
						energyAtTemperature(
							existing,
							target,
							world.pressurePa[index] || undefined,
							world.massKg[index],
						) - world.energy[index];
					if (needed > 0) world.addExternalEnergy(index, needed);
					if (x > 0 && world.grid[index - 1] === HYDROGEN) world.burning[index - 1] = 1;
					if (x + 1 < world.width && world.grid[index + 1] === HYDROGEN)
						world.burning[index + 1] = 1;
					if (y > 0 && world.grid[index - world.width] === HYDROGEN)
						world.burning[index - world.width] = 1;
					if (y + 1 < world.height && world.grid[index + world.width] === HYDROGEN)
						world.burning[index + world.width] = 1;
				}
				return;
			}
			const ignition = materialsById[existing].ignitionTemperatureC;
			if (Number.isFinite(ignition)) {
				// An ignition brush supplies heat; it does not erase the log or oil.
				const needed =
					energyAtTemperature(
						existing,
						ignition + combustionProfile.ignitionMarginC,
						world.pressurePa[index] || undefined,
						world.massKg[index],
					) - world.energy[index];
				if (needed > 0) world.addExternalEnergy(index, needed);
				if (
					existing === HYDROGEN ||
					existing === CARBON ||
					existing === SULFUR ||
					existing === LIQUID_SULFUR ||
					existing === SULFUR_VAPOR
				)
					world.burning[index] = 1;
				return;
			}
			// Do not refresh existing flames or turn smoke/steam into more fire.
			if (existing !== EMPTY) return;
			const profile = materialsById[FIRE].ignitionBrush;
			if (!profile) throw new Error("Fire is missing its ignition brush profile");
			// The center gives reliable feedback on a click. A sparse fringe
			// avoids filling the whole brush disk on every held-input tick.
			if (!isCenter && world.random.next() >= profile.fringeChance) return;
			world.changeMaterial(index, FIRE);
			const added = Math.max(
				0,
				energyAtTemperature(
					FIRE,
					profile.temperatureC,
					world.pressurePa[index] || undefined,
					world.massKg[index],
				) - world.energy[index],
			);
			world.addExternalEnergy(index, added);
			const [minimum, maximum] = profile.lifetimeTicks;
			world.lifetime[index] = minimum + Math.floor(world.random.next() * (maximum - minimum));
			world.ignitionFlame[index] = 1;
			return;
		}
		// Holding a brush fills newly vacated cells without resetting the heat,
		// lifetime or finite fuel store of particles that are already here.
		if (existing === selected) return;
		if (waveMaterials.includes(world.grid[index]) && world.grid[index] !== selected) {
			displacedFluids.add(world.grid[index]);
		}
		world.setCell(index, selected === ERASER ? EMPTY : selected);
	}

	function paintCircle(cx: number, cy: number): void {
		requireCoordinates(cx, cy);
		if (!world.inBounds(cx, cy)) return;
		if (selected === BLAST) {
			onBlast(cx, cy, brushSize);
			return;
		}
		const radiusSquared = brushSize * brushSize;
		const displacedFluids = new Set<number>();
		for (let dy = -brushSize; dy <= brushSize; dy += 1) {
			for (let dx = -brushSize; dx <= brushSize; dx += 1) {
				if (dx * dx + dy * dy > radiusSquared) continue;
				paintCell(cx + dx, cy + dy, displacedFluids, dx === 0 && dy === 0);
			}
		}

		const disturbanceRadius = Math.max(4, brushSize + 2);
		if (waveMaterials.includes(selected)) {
			onDisturb(cx, selected, -0.65, disturbanceRadius);
		}
		for (const fluidType of displacedFluids) {
			onDisturb(cx, fluidType, 1.15, disturbanceRadius);
		}
	}

	function paintLine(fromX: number, fromY: number, toX: number, toY: number): void {
		requireCoordinates(fromX, fromY);
		requireCoordinates(toX, toY);
		if (!world.inBounds(fromX, fromY) || !world.inBounds(toX, toY)) return;
		const distance = Math.max(Math.abs(toX - fromX), Math.abs(toY - fromY));
		if (distance === 0) {
			paintCircle(toX, toY);
			return;
		}
		for (let step = 0; step <= distance; step += 1) {
			const t = step / distance;
			paintCircle(Math.round(fromX + (toX - fromX) * t), Math.round(fromY + (toY - fromY) * t));
		}
	}

	function setSize(size: number): void {
		if (!Number.isInteger(size) || size < 1 || size > 12) {
			throw new RangeError("Brush size must be an integer in 1..12");
		}
		brushSize = size;
	}

	function setMaterial(material: number): void {
		if (
			!Number.isInteger(material) ||
			!(
				isMatterId(material) ||
				material === BLAST ||
				material === HEATER ||
				material === COOLER ||
				material === ERASER
			)
		) {
			throw new RangeError(`Unknown material or tool: ${material}`);
		}
		selected = material;
	}

	return {
		paintCircle,
		paintLine,
		setSize,
		setMaterial,
		getSize: (): number => brushSize,
	};
}
