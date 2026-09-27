/**
 * Browser-facing entrypoint for the active simulation backend and its view metadata.
 * Currently delegates to the synchronous legacy backend. Rust/WASM initialization,
 * backend selection and asynchronous observations belong here when implemented.
 */
export { PRESSURE_SCALE, VELOCITY_SCALE } from "../legacy/rendering/field-map";
export type { PointerState, ViewMode } from "../legacy/rendering/renderer";
export { TEMPERATURE_SCALE } from "../legacy/rendering/temperature-map";
export type { PhysicalTotals } from "../legacy/simulation/diagnostics";
export { type CellReading, createSandbox, type Sandbox } from "../legacy/simulation/sandbox";
