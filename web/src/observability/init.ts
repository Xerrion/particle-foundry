import { initializeErrorReporting } from "./reporting";

// Vite replaces these public values at build time. Bun tests have no Vite environment.
const env = typeof import.meta.env?.DEV === "boolean" ? import.meta.env : undefined;
export const errorReporting = initializeErrorReporting({
	webDsn: env?.VITE_SENTRY_WEB_DSN,
	simDsn: env?.VITE_SENTRY_SIM_DSN,
	release: env?.VITE_SENTRY_RELEASE,
	environment: env?.VITE_SENTRY_ENVIRONMENT ?? env?.MODE,
	revision: env?.VITE_SENTRY_REVISION,
});
