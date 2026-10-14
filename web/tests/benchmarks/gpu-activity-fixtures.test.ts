import { describe, expect, test } from "bun:test";
import { type FixtureName, prepareFixture } from "../../benchmarks/gpu-activity/fixtures";

function paintedCells(fixture: FixtureName) {
	const cells = new Set<string>();
	let revision = 0;
	let resets = 0;
	const status = () =>
		JSON.stringify({ epoch: 7, tick: 0, acceptedTimeS: 0, stateRevision: revision });
	const stamp = prepareFixture(
		{
			reset() {
				resets += 1;
				return status();
			},
			paint(centerX, centerY, radius, material) {
				expect(material).toBe("water");
				for (let y = centerY - radius; y <= centerY + radius; y += 1) {
					for (let x = centerX - radius; x <= centerX + radius; x += 1) {
						if ((x - centerX) ** 2 + (y - centerY) ** 2 <= radius ** 2) {
							cells.add(`${x},${y}`);
						}
					}
				}
				revision += 1;
				return status();
			},
		},
		fixture,
	);
	expect(resets).toBe(1);
	expect(stamp).toEqual({ epoch: 7, tick: 0, acceptedTimeS: 0, stateRevision: revision });
	return cells;
}

describe("GPU activity water-line geometry", () => {
	test("the stamped line is connected but has scalloped edges", () => {
		const cells = paintedCells("water-line");
		expect(cells.size).toBe(1757);
		const heights = new Set<number>();
		for (let x = 114; x <= 358; x += 1) {
			expect(cells.has(`${x},89`)).toBe(true);
			let height = 0;
			for (let y = 85; y <= 93; y += 1) {
				if (cells.has(`${x},${y}`)) height += 1;
			}
			heights.add(height);
		}
		expect(heights.size).toBeGreaterThan(1);
		expect(cells.has("118,85")).toBe(true);
		expect(cells.has("121,85")).toBe(false);
		expect(cells.has("354,93")).toBe(true);
	});

	for (const [fixture, firstRow, lastRow] of [
		["flat-line", 85, 93],
		["flat-line-thin", 86, 92],
	] as const) {
		test(`${fixture} covers exactly one rectangle with no holes`, () => {
			const cells = paintedCells(fixture);
			expect(cells.size).toBe(245 * (lastRow - firstRow + 1));
			for (let y = firstRow; y <= lastRow; y += 1) {
				for (let x = 114; x <= 358; x += 1) {
					expect(cells.has(`${x},${y}`)).toBe(true);
				}
			}
		});
	}

	for (const fixture of ["flat-line", "water-line"] as const) {
		test(`a one-cell translation preserves every painted ${fixture} cell`, () => {
			const original = paintedCells(fixture);
			const shifted = paintedCells(`${fixture}-shift`);
			expect(shifted.size).toBe(original.size);
			for (const cell of original) {
				const [x, y] = cell.split(",").map(Number);
				expect(shifted.has(`${x + 1},${y}`)).toBe(true);
			}
		});
	}
});
