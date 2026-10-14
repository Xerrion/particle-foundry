import type { BrowserGpuSceneSession } from "../../src/engine-client/wasm";
import type { CommittedStamp } from "../gpu-checkpoint";

export const fixtureNames = ["rest", "brush", "dense"] as const;
export type FixtureName = (typeof fixtureNames)[number];

export function prepareFixture(
	scene: BrowserGpuSceneSession,
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
	}
	if (stamp.tick !== 0 || stamp.acceptedTimeS !== 0 || stamp.stateRevision !== paints) {
		throw new Error("Activity fixture must commit all paint at zero accepted time");
	}
	return stamp;
}

export const fixtureDescriptions = {
	rest: "Reset pool without additional paint",
	brush: "Reset pool plus one Water disk at (240,100), radius 4",
	dense: "Reset pool plus 50 Water disks, radius 16, x=24..456 and y=24..216 at spacing 48",
};
