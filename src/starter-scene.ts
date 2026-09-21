import { METAL, OIL, PLANT, SAND, STONE, WATER, WOOD } from "./materials";
import type { World } from "./world";

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

	fillRect(12, 102, 3, 29, STONE);
	fillRect(52, 111, 3, 20, STONE);
	fillRect(12, 127, 43, 4, STONE);
	fillRect(15, 113, 37, 14, WATER);

	fillRect(77, 104, 4, 27, WOOD);
	fillRect(101, 104, 4, 27, WOOD);
	fillRect(77, 102, 28, 4, WOOD);
	fillRect(86, 89, 10, 13, SAND);
	for (let row = 0; row < 12; row += 1) {
		fillRect(126 - row, 127 - row, 2 + row * 2, 1, SAND);
	}

	fillRect(162, 108, 3, 23, STONE);
	fillRect(205, 108, 3, 23, STONE);
	fillRect(162, 127, 46, 4, STONE);
	fillRect(165, 116, 40, 11, OIL);

	fillRect(222, 100, 3, 31, METAL);
	fillRect(225, 100, 8, 3, METAL);

	fillRect(61, 119, 2, 8, PLANT);
	fillRect(59, 117, 2, 2, PLANT);
	fillRect(63, 115, 2, 4, PLANT);
}
