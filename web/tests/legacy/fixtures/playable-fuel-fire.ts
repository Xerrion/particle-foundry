import { energyAtTemperature } from "../../../src/legacy/physics/thermal";
import { createPhysics } from "../../../src/legacy/simulation/physics";
import { createWorld } from "../../../src/legacy/simulation/world";
import { createBrush } from "../../../src/legacy/tools/brush";
import {
  combustionProfile,
  FIRE,
  materialsById,
  OIL,
  STONE,
  type WOOD,
} from "../../../src/materials";

/** Air-fed strips use funded preheating; three-cell-deep fixtures use one Fire click. */
export function playableFuelFire(
  material: typeof WOOD | typeof OIL,
  seed = 42,
  options: { readonly strip?: boolean; readonly boundariesEnabled?: boolean } = {},
) {
  const world = createWorld(32, 32, {
    seed,
    boundariesEnabled: options.boundariesEnabled ?? false,
  });
  const firstX = 12;
  const lastX = 19;
  const bottomY = 27;
  const surfaceY = options.strip ? bottomY : bottomY - 2;
  for (let x = firstX - 1; x <= lastX + 1; x++) {
    world.setCell(x + (bottomY + 1) * world.width, STONE);
  }
  if (material === OIL) {
    for (let y = surfaceY; y <= bottomY; y++) {
      world.setCell(firstX - 1 + y * world.width, STONE);
      world.setCell(lastX + 1 + y * world.width, STONE);
    }
  }
  for (let y = surfaceY; y <= bottomY; y++) {
    for (let x = firstX; x <= lastX; x++) world.setCell(x + y * world.width, material);
  }
  const brush = createBrush(world, () => {});
  brush.setMaterial(FIRE);
  brush.setSize(1);
  function ignite(): void {
    brush.paintCircle(16, surfaceY);
  }
  if (options.strip) {
    for (let x = firstX; x <= lastX; x++) {
      const index = x + surfaceY * world.width;
      world.addExternalEnergy(
        index,
        energyAtTemperature(
          material,
          materialsById[material].ignitionTemperatureC +
            combustionProfile.ignitionMarginC,
          undefined,
          world.massKg[index],
        ) - world.energy[index],
      );
    }
  } else ignite();
  return { world, physics: createPhysics(world), ignite, surfaceY };
}
