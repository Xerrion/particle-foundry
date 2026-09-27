import { describe, expect, test } from "bun:test";
import {
	chemicalElements,
	findChemicalElement,
	searchChemicalElements,
} from "../../src/materials/element-reference";

describe("chemical element reference", () => {
	test("covers atomic numbers 1 through 118 with unique names and symbols", () => {
		expect(chemicalElements).toHaveLength(118);
		expect(new Set(chemicalElements.map((element) => element.symbol)).size).toBe(118);
		expect(new Set(chemicalElements.map((element) => element.name)).size).toBe(118);
		for (let atomicNumber = 1; atomicNumber <= 118; atomicNumber += 1) {
			expect(chemicalElements[atomicNumber - 1].atomicNumber).toBe(atomicNumber);
			expect(findChemicalElement(atomicNumber)).toBe(chemicalElements[atomicNumber - 1]);
		}
	});

	test("retains independently published names at the beginning and end of the table", () => {
		expect(findChemicalElement(1)).toEqual({ atomicNumber: 1, symbol: "H", name: "Hydrogen" });
		expect(findChemicalElement(113)).toEqual({ atomicNumber: 113, symbol: "Nh", name: "Nihonium" });
		expect(findChemicalElement(115)).toEqual({
			atomicNumber: 115,
			symbol: "Mc",
			name: "Moscovium",
		});
		expect(findChemicalElement(117)).toEqual({
			atomicNumber: 117,
			symbol: "Ts",
			name: "Tennessine",
		});
		expect(findChemicalElement(118)).toEqual({
			atomicNumber: 118,
			symbol: "Og",
			name: "Oganesson",
		});
		expect(findChemicalElement(0)).toBeUndefined();
		expect(findChemicalElement(119)).toBeUndefined();
	});

	test("finds symbols, names and exact atomic numbers without changing the reference", () => {
		expect(searchChemicalElements(" oganesson ")).toEqual([chemicalElements[117]]);
		expect(searchChemicalElements("118")).toEqual([chemicalElements[117]]);
		expect(searchChemicalElements("zzzz")).toEqual([]);
		expect(searchChemicalElements("")).toBe(chemicalElements);
	});
});
