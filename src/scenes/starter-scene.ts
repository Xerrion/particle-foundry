import { METAL, OIL, PLANT, SAND, STONE, WATER, WOOD } from "../materials";
import type { World } from "../simulation/world";

/** Resets the world and paints the starter scene at spawn temperatures, clipped to its bounds. */
export function seedStarterScene(world: World): void {
	function fillRect(x: number, y: number, width: number, height: number, material: number): void {
		for (let py = y; py < y + height; py += 1) {
			for (let px = x; px < x + width; px += 1) {
				if (world.inBounds(px, py)) world.setCell(px + py * world.width, material);
			}
		}
	}

	world.clear();
	fillRect(0, world.height - 4, world.width, 4, STONE);
	// Keep the original compact experiments together near the floor when the
	// world grows, leaving room to build separate scenes around them.
	const offsetX = Math.max(0, Math.floor((world.width - 240) / 2));
	const offsetY = Math.max(0, world.height - 135);
	function sceneRect(x: number, y: number, width: number, height: number, material: number): void {
		fillRect(x + offsetX, y + offsetY, width, height, material);
	}

	sceneRect(12, 102, 3, 29, STONE);
	sceneRect(52, 111, 3, 20, STONE);
	sceneRect(12, 127, 43, 4, STONE);
	sceneRect(15, 113, 37, 14, WATER);

	sceneRect(77, 104, 4, 27, WOOD);
	sceneRect(101, 104, 4, 27, WOOD);
	sceneRect(77, 102, 28, 4, WOOD);
	sceneRect(86, 89, 10, 13, SAND);
	for (let row = 0; row < 12; row += 1) {
		sceneRect(126 - row, 127 - row, 2 + row * 2, 1, SAND);
	}

	sceneRect(162, 108, 3, 23, STONE);
	sceneRect(205, 108, 3, 23, STONE);
	sceneRect(162, 127, 46, 4, STONE);
	sceneRect(165, 116, 40, 11, OIL);

	sceneRect(222, 100, 3, 31, METAL);
	sceneRect(225, 100, 8, 3, METAL);

	sceneRect(61, 119, 2, 8, PLANT);
	sceneRect(59, 117, 2, 2, PLANT);
	sceneRect(63, 115, 2, 4, PLANT);
}
