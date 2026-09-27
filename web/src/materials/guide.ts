import { materialsById, toolDefinitions } from ".";

/** Selection-specific instructions; element interaction claims live with their models. */
export function materialGuide(id: number): {
	description: string;
	interactions: readonly string[];
} {
	const tool = toolDefinitions.find((entry) => entry.id === id);
	if (tool)
		return {
			description: {
				heat: "Hold over matter to add heat.",
				cool: "Hold over matter to remove heat.",
				blast: "Click or hold to apply an outward impulse.",
				eraser: "Paint over matter to replace it with air.",
			}[tool.key],
			interactions:
				tool.key === "heat" || tool.key === "cool"
					? [
							"Use the temperature map (T) to follow heating, cooling and phase changes.",
							"Stored latent heat must be supplied or removed before a phase change completes.",
						]
					: ["Brush radius controls the affected area. Pause with Space for precise edits."],
		};
	const material = materialsById[id];
	if (material.description)
		return { description: material.description, interactions: material.interactions ?? [] };
	const descriptions: Record<string, string> = {
		sand: "Loose grains settle into piles and sink through water.",
		water: "Pour a pool, fill a container or cool hot material.",
		wood: "Paint fixed structures or fuel for a fire.",
		plant: "Grows beside water and burns when ignited.",
		oil: "Floats on water and flows while burning.",
		fire: "An ignition brush: light fuel or create a brief flame in air.",
		gunpowder: "Loose explosive powder. Ignites from heat or direct flame contact.",
		stone: "Fixed terrain for containers and barriers.",
		metal: "Generic metal preset; use Iron or Copper for individually modeled elements.",
	};
	const interactions: string[] = [];
	if (material.neutralization)
		interactions.push(
			material.neutralization.role === "acid"
				? "Touch Sodium hydroxide to release neutralization heat and form neutralized solution."
				: "Touch Hydrochloric acid or Sulfuric acid to neutralize and release heat.",
		);
	if (material.electrical)
		interactions.push(
			"Complete a path from Battery through Wire, Metal, Iron or Copper and a Lamp to Ground. Current heats conductors and drains the battery.",
		);
	if (material.phaseFamily)
		for (let i = 0; i < material.phaseFamily.transitions.length; i++) {
			const family = material.phaseFamily;
			interactions.push(
				`${materialsById[family.phases[i]].name} ↔ ${materialsById[family.phases[i + 1]].name} at ${family.transitions[i].temperature} °C at normal pressure; requires latent heat.`,
			);
		}
	if (Number.isFinite(material.ignitionTemperatureC) && !material.atomicNumber)
		interactions.push(
			`Ignites at ${material.ignitionTemperatureC} °C with oxygen. Fire or Heat can ignite it; water quenches it.`,
		);
	if (material.explosive)
		interactions.push(
			"Fire or sufficient heat triggers a finite, fuel-funded explosion. Walls block the blast front.",
		);
	if (material.key === "fire")
		interactions.push(
			"Ignites wood, oil, plants and gunpowder. Ignite hydrogen in open air or on either side of an H₂/O₂ contact. Ignite carbon or sulfur against O₂.",
		);
	if (!interactions.length)
		interactions.push(
			"Exchanges heat with touching matter. Other chemical reactions are not modeled.",
		);
	return {
		description:
			descriptions[material.key] ??
			`${material.name} · ${material.state}. Hold on the canvas to paint.`,
		interactions,
	};
}
