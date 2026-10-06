import { initializeErrorReporting } from "./reporting";

// Vite replaces these public values at build time. Bun tests have no Vite environment.
const env = typeof import.meta.env?.DEV === "boolean" ? import.meta.env : undefined;
export const errorReporting = initializeErrorReporting({
	webDsn: env?.VITE_GLITCHTIP_WEB_DSN,
	simDsn: env?.VITE_GLITCHTIP_SIM_DSN,
	release: env?.VITE_GLITCHTIP_RELEASE,
	environment: env?.VITE_GLITCHTIP_ENVIRONMENT ?? env?.MODE,
	baseUrl: env?.BASE_URL,
	revision: typeof __PF_BUILD_REVISION__ === "string" ? __PF_BUILD_REVISION__ : undefined,
});
