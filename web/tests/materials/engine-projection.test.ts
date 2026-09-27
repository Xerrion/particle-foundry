import { expect, test } from "bun:test";
import { projectEngineCatalogue } from "../../scripts/project-engine-catalogue";
import { EMPTY, materialDefinitions, STONE, WATER } from "../../src/materials/definitions";

test("E03 projection derives the candidate carrier, liquid and wall from authored SI properties", async () => {
	const projection = JSON.parse(await projectEngineCatalogue());
	expect(projection.schemaVersion).toBe(1);
	expect(projection.sourceSha256).toMatch(/^[a-f0-9]{64}$/);
	expect(
		projection.materials.map((entry: { legacyMaterialId: number }) => entry.legacyMaterialId),
	).toEqual([EMPTY, WATER, STONE]);
	for (const entry of projection.materials) {
		const authored =
			materialDefinitions[entry.legacyMaterialId as keyof typeof materialDefinitions];
		expect(entry.densityKgPerM3).toBe(authored.densityKgPerM3);
		expect(entry.viscosityPas).toBe(authored.viscosityPas);
	}
});
