import {
	isMatterId,
	type MaterialDefinition,
	type MaterialId,
	materialDefinitions,
	physicalProperties,
	pickerIds,
	type ToolId,
	WATER,
} from "../../src/materials/definitions";

// Compile with strict + noUncheckedIndexedAccess; intentionally not a runtime test.
const water: MaterialId = WATER;
const waterDefinition: MaterialDefinition = materialDefinitions[water];
const tool: ToolId = 254;
// @ts-expect-error Tool IDs are not matter IDs.
const notMatter: MaterialId = 254;
// @ts-expect-error Arbitrary numbers are not catalogue IDs.
const unknownMatter: MaterialId = 99;
// @ts-expect-error Matter IDs are not tool IDs.
const notTool: ToolId = WATER;
// @ts-expect-error Unknown registry keys are not accepted.
const missing = materialDefinitions[99];

function checkedLookup(input: number): MaterialDefinition {
	if (isMatterId(input)) {
		const narrowed: MaterialId = input;
		return materialDefinitions[narrowed];
	}
	return physicalProperties(input); // Checked arbitrary-number boundary remains supported.
}

const selection: MaterialId | ToolId | undefined = pickerIds.unrecognised;
void [waterDefinition, tool, notMatter, unknownMatter, notTool, missing, checkedLookup, selection];
