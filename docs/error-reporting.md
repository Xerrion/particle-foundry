# GlitchTip error reporting

Browser errors and Rust panics use separate GlitchTip projects. Reporting is
optional. Empty DSNs leave the sandbox running without a reporting client.
The default TypeScript sandbox and experimental GPU preview both initialize
reporting before their startup code.

## Configure the browser

1. Copy [web/.env.example](../web/.env.example) to `web/.env.local`.
2. Set `VITE_GLITCHTIP_WEB_DSN` to the `particle-foundry-web` ingestion DSN.
3. Set `VITE_GLITCHTIP_SIM_DSN` to the `particle-foundry-sim` ingestion DSN.
4. Set `VITE_GLITCHTIP_RELEASE` and `VITE_GLITCHTIP_ENVIRONMENT` when required.
5. Run `mise run dev` or build a new deployment.

Vite embeds these public values in the browser bundle at build time. A browser
DSN contains a public ingestion key. Never use an administrative API token here.
Keep local configuration files out of Git. Missing DSNs disable only their project.
An invalid browser DSN produces a warning without printing its value.

[Deployment](deployment.md#build-and-runtime) owns Docker and Coolify settings.
The Dockerfile accepts the same four values as build arguments. Use a source
revision or release identifier consistently across both projects.

## Browser and WASM ownership

[`initializeErrorReporting`](../web/src/observability/reporting.ts) constructs
two Sentry browser clients. The web client captures uncaught JavaScript errors
and rejected promises. The sim client receives caught GPU failures from
[`showError`](../web/src/gpu-main.ts) and Rust panics from the WASM boundary.
[`init.ts`](../web/src/observability/init.ts) owns build configuration.

The Rust
[`reporting` module](../engine/crates/wasm/src/reporting.rs) installs a panic hook
when WASM starts. It dispatches `particle-foundry:rust-panic` with a JavaScript
`RustPanic` error containing the Rust location and panic message. The sim client
captures that error. The next `unreachable` runtime trap within one second is
suppressed because the panic already identifies the failure. Duplicate callbacks
for that same trap are also suppressed. Independent traps remain reportable.

The native Rust SDK uses a transport that requires native threads. It does not
enter the WASM dependency graph. Browser transport owns delivery for WASM panics.
The panic hook preserves the previous Rust hook for local diagnostics.

## Data collection

The pinned browser SDK uses explicit error integrations. Session tracking,
replay, tracing, profiling, console capture, logs, and metrics are disabled.
The SDK excludes user information, cookies, HTTP headers and bodies, URL query
parameters, stack variables, and source context lines. The final error filter
removes user, request, breadcrumb, and extra fields.

Error messages and stack traces remain part of the report. Do not put credentials
or personal information in application errors. GlitchTip can still observe the
network source address of an ingestion request.

## Native Rust diagnostics

The native
[`device-smoke`](../engine/crates/sim-gpu/examples/device-smoke.rs) example
initializes `sentry` before selecting a GPU device. Its guard remains alive until
the example exits. The portable simulation crates do not initialize reporting.

Set `GLITCHTIP_SIM_DSN`, `GLITCHTIP_RELEASE`, and `GLITCHTIP_ENVIRONMENT` in the
process environment. Missing configuration disables reporting in `device-smoke`.
Invalid configuration produces a generic message without the DSN.

To send a controlled native verification error from the repository root:

```sh
mise run glitchtip:smoke:native
```

This example requires a DSN and prints the captured event ID. Queue flushing
establishes that the native transport completed its queue processing. The SDK
does not expose the server's HTTP result. Check the event in GlitchTip before
claiming live ingestion.

## Verification

[`reporting.test.ts`](../web/tests/observability/reporting.test.ts) checks disabled
reporting, project routing, removal of identity and request data, panic deduplication,
and listener cleanup. The dedicated browser reporting smoke uses synthetic local
projects and a separate WASM build. Its `glitchtip-smoke` Cargo feature exposes a
deliberate verification panic. Normal production builds omit that function.

Run `mise run test:reporting` for the real browser error and WASM panic check.
The local receiver must accept exactly one event for each project. It rejects
session, log, metric, and other envelope types. `mise run ci` includes this check.
Local receiver acceptance establishes SDK delivery and routing. It does not
establish acceptance by a deployed GlitchTip server.

The [GlitchTip browser guide](https://glitchtip.com/sdkdocs/javascript/) uses
`@sentry/browser`. The
[Sentry JavaScript migration guide](https://github.com/getsentry/sentry-javascript/blob/11.4.0/MIGRATION.md)
owns the version 11 `dataCollection` options. The
[Rust SDK](https://github.com/getsentry/sentry-rust/tree/0.49.3) owns native transport
and panic integration behavior.
