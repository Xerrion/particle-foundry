import { createHash } from "node:crypto";
import { mkdir, readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { EMPTY, materialDefinitions, STONE, WATER } from "../src/materials/definitions";

const source = resolve(import.meta.dir, "../src/materials/definitions.ts");
const output = resolve(import.meta.dir, "../generated/catalogue/low-mach-candidate-v1.json");
const entries = [
	{ id: EMPTY, role: "carrier" },
	{ id: WATER, role: "liquid" },
	{ id: STONE, role: "fixed-wall" },
] as const;

/** This is source data for later M2/M3 selection, not a validated physical scene. */
export async function projectEngineCatalogue(): Promise<string> {
	const sourceSha256 = createHash("sha256")
		.update(await readFile(source))
		.digest("hex");
	const materials = entries.map(({ id, role }) => {
		const authored = materialDefinitions[id];
		if (!authored || authored.id !== id) throw new Error(`Missing authored material ${id}`);
		for (const [field, value] of [
			["densityKgPerM3", authored.densityKgPerM3],
			["viscosityPas", authored.viscosityPas],
		] as const) {
			if (!Number.isFinite(value) || value < 0 || (field === "densityKgPerM3" && value === 0)) {
				throw new Error(`Invalid ${field} for material ${id}`);
			}
			if (value > 0 && Math.abs(Math.fround(value) - value) / value > 1e-6) {
				throw new Error(`Material ${id} ${field} loses too much f32 precision`);
			}
		}
		return {
			legacyMaterialId: id,
			role,
			densityKgPerM3: authored.densityKgPerM3,
			viscosityPas: authored.viscosityPas,
		};
	});
	return `${JSON.stringify({ schemaVersion: 1, sourceSha256, materials }, null, 2)}\n`;
}

if (import.meta.main) {
	const check = process.argv[2] === "--check";
	if (process.argv.length > (check ? 3 : 2)) {
		throw new Error("Usage: project-engine-catalogue.ts [--check]");
	}
	const expected = await projectEngineCatalogue();
	if (check) {
		if ((await readFile(output, "utf8")) !== expected) {
			throw new Error("Catalogue projection is stale; run mise run build:catalogue");
		}
		console.log("Catalogue projection is current");
	} else {
		await mkdir(dirname(output), { recursive: true });
		await Bun.write(output, expected);
	}
}
