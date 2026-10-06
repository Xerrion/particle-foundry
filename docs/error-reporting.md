# Sentry error reporting

Browser errors and Rust panics use separate Sentry projects. Reporting is
optional. Empty DSNs leave the sandbox running without a reporting client.
The default TypeScript sandbox and experimental GPU preview both initialize
reporting before their startup code.

## Configure the browser

1. Copy [web/.env.example](../web/.env.example) to `web/.env.local`.
2. Set `VITE_SENTRY_WEB_DSN` to the `particle-foundry-web` ingestion DSN.
3. Set `VITE_SENTRY_SIM_DSN` to the `particle-foundry-sim` ingestion DSN.
4. Set release, environment, and `VITE_SENTRY_REVISION` when required.
5. Run `mise run dev` or build a new deployment.

Vite embeds these public values in the browser bundle at build time. A browser
DSN contains a public ingestion key. Never use an administrative API token here.
Keep local configuration files out of Git. Missing DSNs disable only their project.
An invalid browser DSN produces a warning without printing its value.

[Deployment](deployment.md#build-and-runtime) owns Docker and Coolify settings.
The Dockerfile accepts the same five values as build arguments. Use a source
revision or release identifier consistently across both projects.

## Debuggable source frames

Vite emits JavaScript source maps with the original TypeScript content.
[`prepare-reporting.ts`](../web/scripts/prepare-reporting.ts) uses the pinned
`sentry@0.45.0` build package to inject matching debug IDs into JavaScript and maps.
It archives those exact files under ignored `web/reporting-artifacts/`, outside
`web/dist/`, then removes maps from the public build.

The browser sends generated stack frames and their debug IDs. After a private
artifact upload, Sentry resolves matching frames to original TypeScript files,
lines, functions and code context. The browser does not fetch or decode maps.
This follows the [Sentry source-map workflow](https://docs.sentry.io/platforms/javascript/sourcemaps/).

Rust panics carry the structured `PanicHookInfo` file, line, and column. The
[`origin annotator`](../web/src/observability/rust-origin.ts) adds this location as
an application frame. It identifies the panic origin without code context.
The build publishes no Rust source JSON assets. Uploaded JavaScript maps do not
reconstruct the complete Rust or WASM call stack.

Set `VITE_SENTRY_REVISION` to the full 40-character Git commit SHA to add an
immutable `source_link` to the Rust origin frame. Use it only when the build's
source matches that commit. An omitted or invalid revision retains the Rust file,
line and column, but omits the link. Docker does not include Git metadata.

## Upload private source maps

A DSN permits event ingestion. Source-map uploads require a separate API token
with upload access to the configured projects. Keep the token in a secret manager
or protected process environment. Never put it in browser variables, Docker build
arguments, Coolify settings or committed files.

| Variable | Purpose |
| --- | --- |
| `SENTRY_AUTH_TOKEN` | Private upload token |
| `SENTRY_URL` | Sentry instance URL, such as `https://sentry.io` |
| `SENTRY_ORG` | Organization slug, such as `xerrion` |
| `SENTRY_PROJECT` | Web project slug, such as `particle-foundry-web` |
| `SENTRY_SIM_PROJECT` | Optional sim project slug, such as `particle-foundry-sim` |

Run these steps from the repository root in a trusted local or CI environment:

1. Set the public browser configuration, including `VITE_SENTRY_RELEASE`.
2. Run `mise run build` to prepare the public build and private artifact archive.
3. Supply the private upload variables, then run `mise run sentry:upload`.
4. Deploy the matching `web/dist/` build through the authorized deployment process.
5. Trigger a fresh error and check its original source frame in Sentry.

The upload task reads the release recorded in the archive. It reuses exact
injected artifacts without rebuilding or rewriting public JavaScript. A retry
uses the same archive and debug IDs. Preserve the archive until upload and live
verification finish. Upload maps to both projects when both receive browser frames.

The token policy follows the xerrion-io reference. Its Coolify 4.3.23 review found
encoded build environment values retained in deployment logs. The relevant
[deployment code](https://github.com/coollabsio/coolify/blob/v4.3.23/app/Jobs/ApplicationDeploymentJob.php)
and [command logging code](https://github.com/coollabsio/coolify/blob/v4.3.23/app/Traits/ExecuteRemoteCommand.php)
support this limitation for the reviewed version.

Existing stored events retain their old frames. Validate fresh events after
uploading maps; do not expect an upload to repair earlier events retroactively.

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
parameters, and stack variables. Reports retain error messages, stack frames and
debug IDs. Sentry can add source context from the private uploaded maps.
The final error filter
removes user, request, breadcrumb, and extra fields.

Error messages and stack traces remain part of the report. Do not put credentials
or personal information in application errors. Sentry can still observe the
network source address of an ingestion request.

## Native Rust diagnostics

The native
[`device-smoke`](../engine/crates/sim-gpu/examples/device-smoke.rs) example
initializes `sentry` before selecting a GPU device. Its guard remains alive until
the example exits. The portable simulation crates do not initialize reporting.

Set `SENTRY_SIM_DSN`, `SENTRY_RELEASE`, and `SENTRY_ENVIRONMENT` in the
process environment. Missing configuration disables reporting in `device-smoke`.
Invalid configuration produces a generic message without the DSN.

To send a controlled native verification error from the repository root:

```sh
mise run sentry:smoke:native
```

This example requires a DSN and prints the captured event ID. Queue flushing
establishes that the native transport completed its queue processing. The SDK
does not expose the server's HTTP result. Check the event in Sentry before
claiming live ingestion.

## Verification

[`reporting.test.ts`](../web/tests/observability/reporting.test.ts) checks disabled
reporting, project routing, removal of identity and request data, panic deduplication,
and listener cleanup. The dedicated
[`browser smoke`](../web/scripts/test-reporting.ts) uses synthetic local projects
and a separate WASM build. Its `sentry-smoke` Cargo feature exposes a deliberate
verification panic. Normal production builds omit that function.

Run `mise run test:reporting` for the real browser error and WASM panic check.
The local receiver must accept exactly one event for each project. The check
matches generated browser frames and debug IDs against the private map's original
TypeScript throw location and context. It also checks the Rust origin, project
routing, privacy filtering and unavailable public map URLs.

The receiver rejects session, log, metric and other envelope types.
`mise run ci` includes this check. Local acceptance establishes SDK delivery and
artifact matching. It does not establish upload success or acceptance by Sentry.
After upload, inspect a fresh event from the matching compiled application.
Confirm its original file, line, function and source context in Sentry.

The [Sentry browser guide](https://docs.sentry.io/platforms/javascript/) uses
`@sentry/browser`. The
[Sentry JavaScript migration guide](https://github.com/getsentry/sentry-javascript/blob/11.4.0/MIGRATION.md)
owns the version 11 `dataCollection` options. The
[Rust SDK](https://github.com/getsentry/sentry-rust/tree/0.49.3) owns native transport
and panic integration behavior.

## Migration from GlitchTip

Replace all GLITCHTIP environment variable names with SENTRY names.
Use the Sentry project ingestion DSN for each enabled client. Old variables
no longer enable reporting. The supplied Rust project DSN belongs in
`VITE_SENTRY_SIM_DSN` for WASM and `SENTRY_SIM_DSN` for native examples.
Set `SENTRY_URL=https://sentry.io` for private source-map uploads.
The SDK remains pinned to Rust 0.49.3. Personal data collection stays disabled.
