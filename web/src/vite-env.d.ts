/// <reference types="vite/client" />

declare const __PF_BUILD_REVISION__: string;
declare const __PF_VERIFY_LIVE__: boolean;

interface ImportMetaEnv {
	readonly VITE_GLITCHTIP_WEB_DSN?: string;
	readonly VITE_GLITCHTIP_SIM_DSN?: string;
	readonly VITE_GLITCHTIP_RELEASE?: string;
	readonly VITE_GLITCHTIP_ENVIRONMENT?: string;
}
