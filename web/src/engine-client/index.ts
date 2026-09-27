/**
 * Browser-facing entrypoint for the active simulation backend and its view metadata.
 * The active UI delegates to the synchronous legacy backend. The separate E03
 * session contract is experimental until a physical Rust scene is supported.
 */
export { PRESSURE_SCALE, VELOCITY_SCALE } from "../legacy/rendering/field-map";
export type { PointerState, ViewMode } from "../legacy/rendering/renderer";
export { TEMPERATURE_SCALE } from "../legacy/rendering/temperature-map";
export type { PhysicalTotals } from "../legacy/simulation/diagnostics";
export { selectCanvasContext } from "./canvas";
export type { CellReading, Sandbox } from "./legacy-adapter";
export { createSandbox } from "./legacy-adapter";
export type {
	AdvanceReceipt,
	EngineCommand,
	EngineSession,
	EpochOwner,
	QueueReceipt,
	SceneCapabilities,
	SceneRequest,
	SessionStatus,
	SmallProbe,
} from "./session";
export { createEngineSession, preflightScene } from "./session";
