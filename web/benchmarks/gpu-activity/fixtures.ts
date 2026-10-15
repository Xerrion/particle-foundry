import type { BrowserGpuSceneSession } from "../../src/engine-client/wasm";
import type { CommittedStamp } from "../gpu-checkpoint";

export const fixtureNames = [
  "rest",
  "brush",
  "dense",
  "water-line",
  "water-line-shift",
  "flat-line",
  "flat-line-shift",
  "flat-line-thin",
  "continuous-line",
  "stroke-line",
] as const;
export type FixtureName = (typeof fixtureNames)[number];

export function prepareFixture(
  scene: Pick<BrowserGpuSceneSession, "reset" | "paint"> &
    Partial<Pick<BrowserGpuSceneSession, "paintStroke">>,
  fixture: FixtureName,
): CommittedStamp {
  let stamp = JSON.parse(scene.reset()) as CommittedStamp;
  let paints = 0;
  if (fixture === "brush") {
    stamp = JSON.parse(scene.paint(240, 100, 4, "water")) as CommittedStamp;
    paints = 1;
  } else if (fixture === "dense") {
    for (let y = 24; y <= 216; y += 48) {
      for (let x = 24; x <= 456; x += 48) {
        stamp = JSON.parse(scene.paint(x, y, 16, "water")) as CommittedStamp;
        paints += 1;
      }
    }
  } else if (fixture === "water-line" || fixture === "water-line-shift") {
    const shift = fixture === "water-line-shift" ? 1 : 0;
    for (let index = 0; index <= 44; index += 1) {
      const x = 118 + Math.round((index * 236) / 44) + shift;
      stamp = JSON.parse(scene.paint(x, 89, 4, "water")) as CommittedStamp;
      paints += 1;
    }
  } else if (fixture === "continuous-line") {
    for (let x = 118; x <= 354; x += 1) {
      stamp = JSON.parse(scene.paint(x, 89, 4, "water")) as CommittedStamp;
      paints += 1;
    }
  } else if (fixture === "stroke-line") {
    if (!scene.paintStroke)
      throw new Error("Stroke fixture requires GPU stroke painting");
    stamp = JSON.parse(
      scene.paintStroke(118, 89, 354, 89, 4, "water"),
    ) as CommittedStamp;
    paints = 1;
  } else if (
    fixture === "flat-line" ||
    fixture === "flat-line-shift" ||
    fixture === "flat-line-thin"
  ) {
    const shift = fixture === "flat-line-shift" ? 1 : 0;
    const halfHeight = fixture === "flat-line-thin" ? 3 : 4;
    for (let y = 89 - halfHeight; y <= 89 + halfHeight; y += 1) {
      for (let x = 114 + shift; x <= 358 + shift; x += 1) {
        stamp = JSON.parse(scene.paint(x, y, 0, "water")) as CommittedStamp;
        paints += 1;
      }
    }
  }
  if (stamp.tick !== 0 || stamp.acceptedTimeS !== 0 || stamp.stateRevision !== paints) {
    throw new Error("Activity fixture must commit all paint at zero accepted time");
  }
  return stamp;
}

export const fixtureDescriptions = {
  rest: "Reset pool without additional paint",
  brush: "Reset pool plus one Water disk at (240,100), radius 4",
  dense:
    "Reset pool plus 50 Water disks, radius 16, x=24..456 and y=24..216 at spacing 48",
  "water-line":
    "Reset pool plus 45 Water disks, radius 4, y=89, x=118+round(i*236/44) for i=0..44",
  "water-line-shift":
    "Reset pool plus 45 Water disks, radius 4, y=89, x=119+round(i*236/44) for i=0..44",
  "flat-line": "Reset pool plus a Water rectangle, x=114..358 and y=85..93 inclusive",
  "flat-line-shift":
    "Reset pool plus a Water rectangle, x=115..359 and y=85..93 inclusive (flat-line shifted one cell right)",
  "flat-line-thin":
    "Reset pool plus a Water rectangle, x=114..358 and y=86..92 inclusive",
  "continuous-line":
    "Reset pool plus 237 Water disks, radius 4, y=89, one disk at every x=118..354",
  "stroke-line": "Reset pool plus one Water stroke from (118,89) to (354,89), radius 4",
} satisfies Record<FixtureName, string>;
