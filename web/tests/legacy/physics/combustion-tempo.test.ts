import { describe, expect, test } from "bun:test";
import { createReactions } from "../../../src/legacy/physics/reactions";
import { energyAtTemperature } from "../../../src/legacy/physics/thermal";
import {
  compareConservation,
  measureWorld,
} from "../../../src/legacy/simulation/diagnostics";
import { createWorld } from "../../../src/legacy/simulation/world";
import {
  combustionProfile,
  materialsById,
  OIL,
  OXYGEN,
  SMOKE,
  STONE,
  WATER,
  WOOD,
} from "../../../src/materials";
import { playableFuelFire } from "../fixtures/playable-fuel-fire";

describe("playable Wood and Oil burn tempo", () => {
  test.each([WOOD, OIL] as const)(
    "air-fed fuel strip %i exhausts fuel within 10 model seconds",
    (material) => {
      const { world, physics } = playableFuelFire(material, 42, { strip: true });
      const before = measureWorld(world);
      for (let tick = 0; tick < 600; tick++) physics.step();
      const after = measureWorld(world);
      expect(after.chemicalEnergyKj).toBeLessThanOrEqual(1e-9);
      expect(world.countMaterial(material)).toBe(0);
      expect(compareConservation(before, after, true)).toMatchObject({
        massWithinTolerance: true,
        energyWithinTolerance: true,
      });
    },
  );

  for (const material of [WOOD, OIL] as const) {
    test.each([1, 42, 123])(
      `${materialsById[material].name} spreads from one click and spends at least 95 percent fuel within 30 model seconds (seed %i)`,
      (seed) => {
        const { world, physics, surfaceY } = playableFuelFire(material, seed);
        const before = measureWorld(world);
        // The ignition brush touches four cells; the other twenty start cold.
        expect(world.temperatureAt(12 + surfaceY * world.width)).toBeCloseTo(22, 10);
        let burningSpread = false;
        for (let tick = 0; tick < 1800; tick++) {
          physics.step();
          burningSpread ||= world.burning.reduce((sum, value) => sum + value, 0) > 4;
        }
        const after = measureWorld(world);
        expect(burningSpread).toBe(true);
        expect(after.chemicalEnergyKj).toBeLessThanOrEqual(
          before.chemicalEnergyKj * 0.05,
        );
        expect(world.countMaterial(material)).toBeLessThan(24);
        expect(compareConservation(before, after, true)).toMatchObject({
          massWithinTolerance: true,
          energyWithinTolerance: true,
        });
      },
    );

    test.each(["held Fire", "default boundary flag"])(
      `${materialsById[material].name} keeps a finite playable burn after %s`,
      (control) => {
        const { world, physics, ignite } = playableFuelFire(material, 42, {
          boundariesEnabled: control === "default boundary flag",
        });
        const before = measureWorld(world);
        for (let tick = 0; tick < 180; tick++) {
          if (control === "held Fire") ignite();
          physics.step();
        }
        const fuelAtRelease = measureWorld(world).chemicalEnergyKj;
        expect(fuelAtRelease).toBeLessThan(before.chemicalEnergyKj);
        for (let tick = 180; tick < 1800; tick++) physics.step();
        const after = measureWorld(world);
        expect(after.chemicalEnergyKj).toBeLessThanOrEqual(fuelAtRelease);
        expect(after.chemicalEnergyKj).toBeLessThanOrEqual(
          before.chemicalEnergyKj * 0.05,
        );
        expect(compareConservation(before, after, true)).toMatchObject({
          massWithinTolerance: true,
          energyWithinTolerance: true,
        });
      },
    );
  }

  test.each([WOOD, OIL] as const)(
    "air-fed fuel %i records bounded product heat outflow at its ceiling",
    (material) => {
      const world = createWorld(5, 5);
      const index = 12;
      world.setCell(index, material);
      world.energy[index] = energyAtTemperature(
        SMOKE,
        combustionProfile.maximumProductTemperatureC,
        undefined,
        world.massKg[index],
      );
      // Defer emission so this check isolates reaction/outflow accounting.
      world.random.next = () => 0.99;
      const before = measureWorld(world);
      const energy = world.energy[index];
      const reactions = createReactions(world);
      reactions.beginStep();
      reactions.update(index);
      const after = measureWorld(world);
      expect(world.energy[index]).toBeCloseTo(energy, 10);
      expect(before.chemicalEnergyKj - after.chemicalEnergyKj).toBeCloseTo(
        combustionProfile.heatPerTick / 1000,
        10,
      );
      expect(
        after.ledger.externalEnergyRemoved - before.ledger.externalEnergyRemoved,
      ).toBeCloseTo(combustionProfile.heatPerTick, 10);
      expect(world.burning[index]).toBe(1);
      expect(compareConservation(before, after, true)).toMatchObject({
        massWithinTolerance: true,
        energyWithinTolerance: true,
      });
    },
  );

  for (const material of [WOOD, OIL] as const) {
    test.each(["enclosed oxygen at ceiling", "no oxygen", "water", "cooled fuel"])(
      `${materialsById[material].name} preserves fuel and records no product outflow for %s`,
      (control) => {
        const world = createWorld(5, 5);
        const index = 12;
        if (control === "enclosed oxygen at ceiling" || control === "no oxygen") {
          for (let i = 0; i < world.size; i++) world.setCell(i, STONE);
        }
        world.setCell(index, material);
        if (control === "enclosed oxygen at ceiling") world.setCell(index + 1, OXYGEN);
        if (control === "water") world.setCell(index + 1, WATER);
        world.energy[index] =
          control === "enclosed oxygen at ceiling"
            ? energyAtTemperature(
                SMOKE,
                combustionProfile.maximumProductTemperatureC,
                undefined,
                world.massKg[index],
              )
            : energyAtTemperature(
                material,
                control === "cooled fuel" ? 100 : 500,
                undefined,
                world.massKg[index],
              );
        const fuel = world.chemicalEnergyKj[index];
        const oxygen = world.oxygenKg.slice();
        const ledger = { ...world.ledger };
        const reactions = createReactions(world);
        reactions.beginStep();
        for (let tick = 0; tick < 60; tick++) reactions.update(index);
        expect(world.grid[index]).toBe(material);
        expect(world.chemicalEnergyKj[index]).toBe(fuel);
        expect(world.ledger).toEqual(ledger);
        if (control === "enclosed oxygen at ceiling")
          expect(world.oxygenKg).toEqual(oxygen);
        else expect(world.burning[index]).toBe(0);
      },
    );
  }
});
