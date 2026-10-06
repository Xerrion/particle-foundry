/// <reference types="vite/client" />

declare const __PF_VERIFY_LIVE__: boolean;

interface ImportMetaEnv {
	readonly VITE_SENTRY_WEB_DSN?: string;
	readonly VITE_SENTRY_SIM_DSN?: string;
	readonly VITE_SENTRY_RELEASE?: string;
	readonly VITE_SENTRY_REVISION?: string;
	readonly VITE_SENTRY_ENVIRONMENT?: string;
}
