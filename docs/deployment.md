# Coolify deployment

Deploy the static browser application from GitHub `main` through Coolify's
GitHub App integration. The intended setup uses native PR previews at
`sand-pr-<PR>.xerrion.io` and production at `sand.xerrion.io`.
The [production Dockerfile](../Dockerfile) builds the existing TypeScript sandbox
and the experimental GPU page. This setup does not promote the GPU engine to the default sandbox.
The Dockerfile defines the container. The GitHub return, native preview setup
and live deployment checks are pending until recorded with deployment evidence.

## Build and runtime

The builder uses digest-pinned public tool images. It runs `mise run build`
from the repository root. That task installs dependencies from `web/bun.lock`, generates
the Rust/WASM bindings and material projection, and builds both Vite entrypoints.
[mise.toml](../mise.toml) remains the task owner.

Only `web/dist/` enters the final Nginx image. The
[server configuration](../web/deployment/nginx.conf) serves `/` and `/gpu.html`
on port `80`. WASM responses use `application/wasm`. The Docker healthcheck sends
`GET /` every 30 seconds and requires a successful response. Missing paths return `404`.

The application needs no environment variables, database or persistent volume.
The [.dockerignore](../.dockerignore) limits the context to the authored build
inputs. It excludes credentials, `.env` files, artifacts, caches and build outputs.
The builder needs no private container registry credential.

## Coolify settings

Use a Coolify application connected to the GitHub App source. Select the public
`Xerrion/particle-foundry` repository and the existing deployment server.
Check that the selected application supports native PR previews before enabling them.
An existing resource that uses a Forgejo deploy key can require a new resource
with the GitHub App source.

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
| Auto deploy | Enabled for pushes to protected GitHub `main` after setup verification |
| Application environment | Empty |

Use the GitHub App connection for repository access and webhook events.
Keep its private key and webhook secret in Coolify.
Do not copy the former Forgejo deployment token or deploy key into GitHub Actions,
source files or the build context. The static application needs no runtime secrets.

Coolify terminates HTTPS and forwards requests to container port `80`.
HTTPS is required for the browser GPU preview. A browser still needs a supported
WebGPU adapter. The existing TypeScript sandbox remains the default page.

## Deployment trigger

GitHub protects `main` with a pull request requirement and the `verify` status check.
Resolve review conversations before squash merging. Coolify's native auto-deploy
receives the resulting push to `main`. It does not wait for the separate CI run
after that push. See the [CI and merge policy](../.github/README.md) for the requirements.

Enable the native trigger only after the GitHub App source, build and public
production URL pass their setup checks. Keep the former Forgejo deployment
workflow inactive. No mirrored repository or custom API trigger is required.

A container build alone does not run all repository checks. A healthy HTTP
response also does not establish browser or GPU correctness.

## PR preview setup

Keep native preview deployments disabled until the baseline deployment works.
Set the custom preview domain to `sand-pr-{{pr_id}}.xerrion.io` in Coolify v4.3.23.
Coolify replaces `{{pr_id}}` with the GitHub pull request number.
DNS and HTTPS must cover the preview addresses before previews are enabled.

Preview builds use the root Dockerfile, port `80` and HTTP `GET /` healthcheck.
Keep preview environment variables empty. Do not copy production secrets,
persistent storage, host port mappings or custom Docker options into previews.

Use a same-repository PR targeting `main` to check the native lifecycle:

1. Open the PR and check its preview address and deployed head SHA.
2. Push a new commit and check that the preview updates to that SHA.
3. Check that production still serves its expected revision after the preview update.
4. Close the PR and check that Coolify removes the preview deployment.
5. Record HTTPS, browser behavior and cleanup results with the tested revisions.

Native preview events and GitHub CI results are separate checks. Check `verify`
before merging. The preview lifecycle still needs live verification.
The experimental GPU limitations apply to previews too.

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
