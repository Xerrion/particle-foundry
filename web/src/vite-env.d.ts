/// <reference types="vite/client" />

declare module "*.svelte" {
  import type { Component } from "svelte";

  const component: Component;
  export default component;
}

declare const __PF_VERIFY_LIVE__: boolean;

interface ImportMetaEnv {
  readonly VITE_SENTRY_WEB_DSN?: string;
  readonly VITE_SENTRY_SIM_DSN?: string;
  readonly VITE_SENTRY_RELEASE?: string;
  readonly VITE_SENTRY_REVISION?: string;
  readonly VITE_SENTRY_ENVIRONMENT?: string;
}
