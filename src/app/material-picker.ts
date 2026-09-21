import {
	type MaterialId,
	materialDefinitions,
	materialNames,
	phaseFamilies,
	pickerIds,
	SAND,
	selectableMaterials,
	type ToolId,
	toolDefinitions,
} from "../materials";
import { chemicalElements, searchChemicalElements } from "../materials/element-reference";
import { materialGuide } from "../materials/guide";
import { requiredElement } from "./dom";

/** Catalogue UI owns filtering and selection; the simulation supplies only a selection command. */
export function bindMaterialPicker(onSelect: (material: MaterialId | ToolId) => void): {
	select(key: string): void;
	selectShortcut(code: string): void;
} {
	const sidebar = requiredElement<HTMLElement>(".tool-panel");
	const liveRegion = requiredElement<HTMLElement>("#liveRegion");
	const selectedMaterialLabel = requiredElement<HTMLElement>("#selectedMaterialLabel");
	const materialGrid = requiredElement<HTMLElement>("#materialGrid");
	const materialSearch = requiredElement<HTMLInputElement>("#materialSearch");
	const materialCategory = requiredElement<HTMLSelectElement>("#materialCategory");
	const materialResultCount = requiredElement<HTMLElement>("#materialResultCount");
	const elementGrid = requiredElement<HTMLElement>("#elementGrid");
	const elementSearch = requiredElement<HTMLInputElement>("#elementSearch");
	const elementResultCount = requiredElement<HTMLElement>("#elementResultCount");
	const referenceToggle = requiredElement<HTMLInputElement>("#showReferenceElements");
	const selectionDescription = requiredElement<HTMLElement>("#selectionDescription");
	const selectionInteractions = requiredElement<HTMLElement>("#selectionInteractions");
	const quickTools = requiredElement<HTMLElement>("#quickTools");
	quickTools.replaceChildren();
	for (const tool of toolDefinitions) {
		const button = document.createElement("button");
		button.type = "button";
		button.className = "quick-tool";
		button.dataset.material = tool.key;
		button.textContent = tool.name;
		button.title = `${tool.name} (${tool.shortcut})`;
		button.setAttribute("aria-pressed", "false");
		quickTools.append(button);
	}
	const phaseNotes = requiredElement<HTMLElement>("#phaseNotes");
	phaseNotes.replaceChildren();
	for (const family of Object.values(phaseFamilies)) {
		for (let i = 0; i < family.transitions.length; i += 1) {
			const transition = family.transitions[i];
			const note = document.createElement("p");
			note.textContent = `${materialNames[family.phases[i]]} ↔ ${materialNames[family.phases[i + 1]]} at ${transition.temperature} °C.`;
			phaseNotes.append(note);
		}
	}
	for (const material of Object.values(materialDefinitions)) {
		if (!material.transforms) continue;
		const note = document.createElement("p");
		note.textContent = `${material.name} → ${materialNames[material.transforms.target]} at ${material.transforms.temperature} °C.`;
		phaseNotes.append(note);
	}

	// The picker, names, shortcuts and swatches come from the same registry
	// as the simulation. A new selectable material needs no HTML/CSS entry.
	materialGrid.replaceChildren();
	const pickerEntries = [...selectableMaterials, ...toolDefinitions];
	const materialButtons: Array<{
		entry: (typeof pickerEntries)[number];
		button: HTMLButtonElement;
		category: string;
	}> = [];
	for (const entry of pickerEntries) {
		const button = document.createElement("button");
		button.type = "button";
		button.className = entry.id === SAND ? "material active" : "material";
		button.dataset.material = entry.key;
		button.setAttribute("aria-pressed", String(entry.id === SAND));
		button.title = entry.name;
		const swatch = document.createElement("span");
		swatch.className = "swatch";
		const colors = entry.palette.map((color) => `rgb(${color.join(",")})`);
		swatch.style.background = `linear-gradient(135deg, ${colors[0]} 0 50%, ${colors[1] ?? colors[0]} 50%)`;
		const label = document.createElement("span");
		label.textContent = entry.name;
		button.append(swatch, label);
		if (entry.shortcut) {
			const shortcut = document.createElement("kbd");
			shortcut.textContent = entry.shortcut;
			button.setAttribute("aria-keyshortcuts", entry.shortcut);
			button.append(shortcut);
		}
		const category =
			"state" in entry
				? entry.neutralization
					? "chemicals"
					: entry.electrical
						? "electrical"
						: entry.key === "fire"
							? "energy"
							: entry.state === "granular"
								? "powders"
								: entry.state === "ambient" || entry.state === "gas"
									? "gases"
									: entry.state === "liquid"
										? "liquids"
										: "solids"
				: "tools";
		button.dataset.category = category;
		materialButtons.push({ entry, button, category });
		materialGrid.append(button);
	}
	function filterMaterials(): void {
		const term = materialSearch.value.trim().toLocaleLowerCase();
		let visible = 0;
		for (const { entry, button, category } of materialButtons) {
			const matches =
				(materialCategory.value === "all" || category === materialCategory.value) &&
				`${entry.name} ${entry.key}`.toLocaleLowerCase().includes(term);
			button.hidden = !matches;
			if (matches) visible += 1;
		}
		materialResultCount.textContent = `${visible} of ${materialButtons.length} materials and tools`;
		requiredElement<HTMLElement>("#materialEmptyState").hidden = visible > 0;
	}
	materialSearch.addEventListener("input", filterMaterials);
	materialCategory.addEventListener("change", filterMaterials);
	requiredElement<HTMLButtonElement>("#clearFilters").addEventListener("click", () => {
		materialSearch.value = "";
		materialCategory.value = "all";
		filterMaterials();
		materialSearch.focus();
	});
	filterMaterials();

	elementGrid.replaceChildren();
	const elementTiles = chemicalElements.map((element) => {
		const tile = document.createElement("button");
		tile.type = "button";
		const model = selectableMaterials.find(
			(material) => material.atomicNumber === element.atomicNumber,
		);
		tile.disabled = !model;
		if (model) {
			tile.dataset.material = model.key;
			tile.setAttribute("aria-pressed", "false");
		}
		tile.className = "element-tile";
		tile.dataset.atomicNumber = String(element.atomicNumber);
		tile.title = `${element.name} (${element.symbol}), atomic number ${element.atomicNumber} · ${model ? "select to paint" : "not modeled"}`;
		tile.setAttribute("aria-label", tile.title);
		const number = document.createElement("span");
		number.textContent = String(element.atomicNumber);
		const symbol = document.createElement("strong");
		symbol.textContent = element.symbol;
		const name = document.createElement("small");
		name.textContent = element.name;
		tile.append(number, symbol, name);
		const status = document.createElement("small");
		status.className = "element-status";
		status.textContent = model ? "Paintable" : "Not modeled";
		tile.append(status);
		elementGrid.append(tile);
		return tile;
	});
	function filterElements(): void {
		const matches = new Set(
			searchChemicalElements(elementSearch.value).map((element) => element.atomicNumber),
		);
		let visible = 0;
		for (const tile of elementTiles) {
			tile.hidden =
				!matches.has(Number(tile.dataset.atomicNumber)) ||
				(tile.disabled && !referenceToggle.checked);
			if (!tile.hidden) visible++;
		}
		elementResultCount.textContent = referenceToggle.checked
			? `${visible} of ${chemicalElements.length} elements · ${elementTiles.filter((tile) => !tile.disabled).length} modeled`
			: `${visible} modeled elements · ${elementTiles.filter((tile) => tile.disabled).length} not yet modeled`;
		requiredElement<HTMLElement>("#elementEmptyState").hidden = visible > 0;
	}
	elementSearch.addEventListener("input", filterElements);
	referenceToggle.addEventListener("change", filterElements);
	filterElements();
	function selectMaterial(key: string): void {
		if (!Object.hasOwn(pickerIds, key)) return;
		const material = pickerIds[key];
		if (material === undefined) return;
		if (typeof material !== "number") return;
		onSelect(material);
		sidebar.querySelectorAll<HTMLButtonElement>("[data-material]").forEach((button) => {
			const isActive = button.dataset.material === key;
			button.classList.toggle("active", isActive);
			button.setAttribute("aria-pressed", String(isActive));
		});
		const name = materialNames[material];
		selectedMaterialLabel.textContent = `${name} selected`;
		liveRegion.textContent = `${name} selected`;
		const guide = materialGuide(material);
		selectionDescription.textContent = guide.description;
		selectionInteractions.replaceChildren(
			...guide.interactions.map((text) => {
				const item = document.createElement("li");
				item.textContent = text;
				return item;
			}),
		);
	}

	sidebar.addEventListener("click", (event) => {
		if (!(event.target instanceof Element)) return;
		const button = event.target.closest("[data-material]");
		if (!(button instanceof HTMLButtonElement) || button.disabled || !sidebar.contains(button))
			return;
		const material = button.dataset.material;
		if (!material) return;
		selectMaterial(material);
	});

	return {
		select: selectMaterial,
		selectShortcut: (code) => {
			const entry = pickerEntries.find(
				(entry) =>
					entry.shortcut &&
					code === (/^\d$/.test(entry.shortcut) ? "Digit" : "Key") + entry.shortcut,
			);
			if (entry) selectMaterial(entry.key);
		},
	};
}
