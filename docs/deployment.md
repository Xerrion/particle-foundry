# Coolify deployment

Deploy the static browser application from GitHub `main` through Coolify's
GitHub App integration. The verified setup uses native PR previews at
`sand-pr-<PR>.xerrion.io` and production at `sand.xerrion.io`.
The [production Dockerfile](../Dockerfile) builds the existing TypeScript sandbox
and the experimental GPU page. This setup does not promote the GPU engine to the default sandbox.
The GitHub source, baseline deployment and native preview lifecycle passed setup
checks on 2026-10-05. The [recorded revisions and limits](#verified-setup) apply to
that test, not to later commits on `main`.

## Build and runtime

The builder uses digest-pinned public tool images. It runs `mise run build`
from the repository root. That task installs dependencies from `web/bun.lock`, generates
the Rust/WASM bindings and material projection, and builds both Vite entrypoints.
[mise.toml](../mise.toml) remains the task owner.

Only `web/dist/` enters the final Nginx image. The
[server configuration](../web/deployment/nginx.conf) serves `/` and `/gpu.html`
on port `80`. WASM responses use `application/wasm`. The Docker healthcheck sends
`GET /` every 30 seconds and requires a successful response. Missing paths return `404`.

The application needs no required environment variables, database or persistent volume.
Optional [GlitchTip reporting](error-reporting.md) uses public ingestion DSNs and
release metadata at build time. The Dockerfile accepts `VITE_GLITCHTIP_WEB_DSN`,
`VITE_GLITCHTIP_SIM_DSN`, `VITE_GLITCHTIP_RELEASE`, `VITE_GLITCHTIP_ENVIRONMENT`, and
`VITE_GLITCHTIP_REVISION`
as build arguments. Configure them as build variables in Coolify when reporting is required.
Changing a running container's environment does not change the compiled browser configuration.
Set the revision to the full commit SHA for links from error frames to exact source lines.
Docker excludes Git metadata, so an omitted revision produces source context without commit links.
The [.dockerignore](../.dockerignore) limits the context to the authored build
inputs. It excludes credentials, `.env` files, artifacts, caches and build outputs.
The builder needs no private container registry credential.

## Coolify settings

The application uses the existing GitHub App source with access to the public
`Xerrion/particle-foundry` repository. It runs on the existing Charles server.
The former Forgejo application is stopped, and its image remains available.

| Setting | Value |
| --- | --- |
| Repository | `https://github.com/Xerrion/particle-foundry` through the GitHub App source |
| Branch | `main` |
| Build pack | Dockerfile |
| Base directory | `/` |
| Dockerfile location | `/Dockerfile` |
| Exposed port | `80` |
| Healthcheck | HTTP `GET /` on port `80`, expected status `200` |
| Healthcheck timing | Interval `30` seconds, timeout `3` seconds, retries `3`, start period `5` seconds |
| Domain | `https://sand.xerrion.io` |
| Auto deploy | Enabled for pushes to protected GitHub `main` |
| Application environment | Empty |
| Persistent storage | Empty |

Use the GitHub App connection for repository access and webhook events.
Keep its private key and webhook secret in Coolify.
Do not copy the former Forgejo deployment token or deploy key into GitHub Actions,
source files or the build context. The static application needs no runtime secrets.

Coolify terminates HTTPS and forwards requests to container port `80`.
HTTPS is required for the browser GPU preview. A browser still needs a supported
WebGPU adapter. The existing TypeScript sandbox remains the default page.
Only `sand.xerrion.io` is configured for production. The automatic `www.sand.xerrion.io`
alias was removed because the existing edge wildcard does not cover that nested hostname.

## Deployment trigger

GitHub protects `main` with a pull request requirement and the `verify` status check.
Resolve review conversations before squash merging. Coolify's native auto-deploy
receives the resulting push to `main`. It does not wait for the separate CI run
after that push. See the [CI and merge policy](../.github/README.md) for the requirements.

The native push trigger is enabled. After a protected `main` merge, check that
the resulting deployment uses the merged revision and passes public checks. Forgejo's
`COOLIFY_DEPLOY_ENABLED` and `COOLIFY_PREVIEW_ENABLED` are both `false`.
No mirrored repository or custom API trigger is required.

A container build alone does not run all repository checks. A healthy HTTP
response also does not establish browser or GPU correctness.

## PR preview setup

Native preview deployments are enabled for repository members only.
The custom preview domain is `sand-pr-{{pr_id}}.xerrion.io` in Coolify v4.3.23.
Coolify replaces `{{pr_id}}` with the GitHub pull request number.
The tested preview address passed public HTTPS checks.

Preview builds use the root Dockerfile, port `80` and HTTP `GET /` healthcheck.
Leave preview reporting DSNs empty or use separate verification projects.
Do not copy production secrets,
persistent storage, host port mappings or custom Docker options into previews.

Use a same-repository PR targeting `main` to check the native lifecycle:

1. Open the PR and check its preview address and deployed head SHA.
2. Push a new commit and check that the preview updates to that SHA.
3. Check that production still serves its expected revision after the preview update.
4. Close the PR and check that Coolify removes the preview deployment.
5. Record HTTPS, browser behavior and cleanup results with the tested revisions.

Native preview events and GitHub CI results are separate checks. Check `verify`
before merging. The experimental GPU limitations apply to previews too.

## Retained Charles runner configuration

The active CI workflow uses GitHub-hosted `ubuntu-latest` for all jobs, including
same-repository pull requests. After this workflow is adopted on `main` and its
main-branch check passes, the Charles service is stopped with configuration and
state volumes retained. Application deployment continues through Coolify and is
independent of the runner service. The proposed
generic per-repository runner fleet was not deployed.

The retained Coolify service `Particle Foundry GitHub runners` on Charles owns two CPU
runner containers for `Xerrion/particle-foundry`. Its resource UUID is
`1rtp5fpxk7xrvx1tgxmgfajc`, in the same project and environment as the browser
application. The GitHub runner names are `particle-foundry-charles-1` and
`particle-foundry-charles-2`; Coolify generates its own container names.
The [runner guide](../.github/runners/README.md) owns the image, resource limits,
bootstrap, persistent state and maintenance commands. The
[CI policy](../.github/README.md) owns workflow routing and external approvals.

Runner images are built on Charles from the checked-in Dockerfile. Coolify
deploys the local image and preserves separate configuration, work and cache
volumes for each slot. The seccomp profile sits beside the generated Compose
file in `/data/coolify/services/1rtp5fpxk7xrvx1tgxmgfajc/`.
Registration tokens enter through a protected temporary file and are removed
before the listener starts. Coolify environment settings contain no GitHub
administrator or runner registration token.

The complete `Forgejo on Charles` service was stopped at the user's request
on 5 October 2026. This includes Forgejo, PostgreSQL, both former runners and
their Docker services. Coolify retains the resource and its configuration;
existing data under `/data/forgejo/` and storage volumes remain available.
The authoritative GitHub repository and its application deployments continue
independently. Stopping this service does not authorize deletion of its data.

### Runner verification

The first [runner CI run](https://github.com/Xerrion/particle-foundry/actions/runs/37379127856)
passed all six jobs at signed source `5edc9ee`. GitHub assigned `docs`, `rust`
and `verify` to `particle-foundry-charles-1`, and `changes`, `web` and `browser`
to `particle-foundry-charles-2`. The initial docs and changes jobs
started at the same time. This establishes actual two-slot execution, beyond
registration alone. Both runner IDs remained unchanged after individual
container recreation, with no new registration token. The runtime checks
confirmed UID 1001, CPU/memory/shared-memory limits, dropped capabilities,
no published ports and no Docker socket. Chrome's sandbox probe passed.

The ordinary browser artifact reported GPU unavailability and passed its
WASM and fallback checks. This CPU-runner result does not replace the
separate required-GPU evidence. The subsequent image correction restricts
Chrome download redirects to HTTPS and retains the same checked package hash.

## Verified setup

Setup checks ran on 2026-10-05. [Migration PR #9](https://github.com/Xerrion/particle-foundry/pull/9)
merged through the protected GitHub path. Its imported tree matched `ffc85b4`.
The original Forgejo commits remain on
[`codex/forgejo-main-archive`](https://github.com/Xerrion/particle-foundry/tree/codex/forgejo-main-archive).
The [PR run](https://github.com/Xerrion/particle-foundry/actions/runs/37303593682)
and [main run](https://github.com/Xerrion/particle-foundry/actions/runs/37304373719)
passed all six CI jobs with Bun `1.3.12`. Hosted browser evidence reported
`gpu.status: unavailable`.

Production checks used
[`c2dbad4`](https://github.com/Xerrion/particle-foundry/commit/c2dbad41a5233c9d59f8c61bf1e070b9aecf471f)
from `main`. Coolify's source was `main` with commit selection `HEAD`.
The root Dockerfile built a healthy container with HTTP `GET /` returning `200`.
Public [production](https://sand.xerrion.io) checks passed for `/`, `/gpu.html`,
JavaScript assets, WASM MIME type, missing-path `404` and HTTP-to-HTTPS `301`.

[Controlled draft PR #10](https://github.com/Xerrion/particle-foundry/pull/10)
tested native creation and update at
[`f74a02f`](https://github.com/Xerrion/particle-foundry/commit/f74a02fe2a9e31adf977bb9004248f29f9473a26)
and
[`4a431ea`](https://github.com/Xerrion/particle-foundry/commit/4a431ea680a0dd9b1a86c72783220f69326eb4c2).
Both automatic deployments succeeded. Both revisions passed HTTPS, page, asset,
WASM MIME, missing-path and redirect checks. The default browser sandbox passed
draw, pause and resume checks. The [first docs-only run](https://github.com/Xerrion/particle-foundry/actions/runs/37305228319)
and [second docs-only run](https://github.com/Xerrion/particle-foundry/actions/runs/37305574616)
passed with the expected job skips. Docker showed separate healthy production
and preview containers at their expected revisions.

PR #10 closed without a merge. Coolify removed its preview record, and Docker
no longer listed its container. The former preview address returned `503`, while
production still returned `200`. The temporary bot comment was removed.

The experimental GPU page selected `BrowserWebGpu` and repeated its known
pressure-projection stop at 125 ticks, or 2.083 seconds of model time.
These setup checks do not establish broader GPU correctness.

The optional Sonar check still failed on existing Dockerfile findings.
Review found no migration blocker and added no waiver.

## Check a deployment

From the repository root, build and start the container with an existing Docker installation:

```sh
docker build -t particle-foundry:local .
docker run --rm -p 127.0.0.1:8080:80 particle-foundry:local
```

1. Check that `/` and `/gpu.html` return `200`.
2. Check that the referenced WASM asset returns `application/wasm`.
3. Check that a missing asset returns `404`.
4. Open both pages in a browser and check the controls.
5. Check the deployed revision and HTTPS certificate in Coolify.

Record browser GPU checks separately. Use the
[browser evidence requirements](../.github/README.md#browser-evidence) when an
acceptance check requires a real GPU adapter.
