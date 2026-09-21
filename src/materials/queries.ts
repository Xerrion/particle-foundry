import { materialsById } from "./definitions";

// Derived lookup tables, compiled from the catalogue once. No material constants
// are duplicated here and the transport hot path avoids repeated object lookups.
const gasMaterials = new Uint8Array(256);
const liquidMaterials = new Uint8Array(256);
const density = new Float64Array(256);
for (const material of Object.values(materialsById)) {
	gasMaterials[material.id] = Number(material.state === "ambient" || material.state === "gas");
	liquidMaterials[material.id] = Number(material.state === "liquid");
	density[material.id] = material.displacementDensity;
}

/** Motion policy in fixed 60 Hz ticks, not SI viscosity. Larger intervals resist flow more. */
export const liquidMotion: Readonly<
	Record<number, { readonly interval: number; readonly spread: number } | undefined>
> = Object.freeze(
	Object.fromEntries(Object.values(materialsById).map((material) => [material.id, material.flow])),
);

/** Identifies matter represented by the liquid transport model. */
export function isLiquid(material: number): boolean {
	return liquidMaterials[material] === 1;
}

/** Identifies cells that can be displaced by falling matter, including empty air. */
export function isGas(material: number): boolean {
	return gasMaterials[material] === 1;
}

/** A denser parcel can settle into gas or lighter liquid, without merging materials. */
export function canDisplaceFluid(material: number, target: number): boolean {
	return isGas(target) || (isLiquid(target) && density[material] > density[target]);
}
