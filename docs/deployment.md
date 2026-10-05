# Coolify deployment

Deploy the static browser application from the Forgejo `main` branch.
Optional PR previews use separate applications in a dedicated Coolify environment.
The [production Dockerfile](../Dockerfile) builds the existing TypeScript sandbox
and the experimental GPU page. This setup does not promote the GPU engine to the default sandbox.
The Dockerfile defines the container. Live Coolify settings and a successful deployment require separate checks.

## Build and runtime

The builder uses the same public tool images and digests as the
[Forgejo tool image](../.forgejo/ci/Dockerfile). It runs `mise run build` from the
repository root. That task installs dependencies from `web/bun.lock`, generates
the Rust/WASM bindings and material projection, and builds both Vite entrypoints.
[mise.toml](../mise.toml) remains the task owner.

Only `web/dist/` enters the final Nginx image. The
[server configuration](../web/deployment/nginx.conf) serves `/` and `/gpu.html`
on port `80`. WASM responses use `application/wasm`. The Docker healthcheck sends
`GET /` every 30 seconds and requires a successful response. Missing paths return `404`.

The application needs no environment variables, database or persistent volume.
The [.dockerignore](../.dockerignore) limits the context to the authored build
inputs. It excludes credentials, `.env` files, artifacts, caches and build outputs.
The builder uses public images and does not require Forgejo registry credentials.

## Coolify settings

Use these settings for the production application:

Create the resource with **Private Repository (with deploy key)**. Select the
stored `particle-foundry-readonly` key. A Public Git application cannot attach
its first deploy key through the Coolify v4.3.23 UI.

| Setting | Value |
| --- | --- |
| Repository | `git@charles.grayling-bigeye.ts.net:2222/xerrion/particle-foundry.git` |
| Branch | `main` |
| Build pack | Dockerfile |
| Base directory | `/` |
| Dockerfile location | `/Dockerfile` |
| Exposed port | `80` |
| Healthcheck | HTTP `GET /` on port `80`, expected status `200` |
| Healthcheck timing | Interval `30` seconds, timeout `3` seconds, retries `3`, start period `5` seconds |
| Domain | `https://sand.xerrion.io` |
| Auto deploy | Manual deployments only; Forgejo CI sends the approved API request |
| Application environment | Empty |

Grant Coolify read access to the private Forgejo repository through a dedicated
deploy key. Store its private key in Coolify. Do not include it in Git or the build context.
Coolify v4.3.23 accepts the extended SCP-style URL above and extracts SSH port
`2222`. Its creation form rejects the equivalent `ssh://` clone URL shown by Forgejo.

Coolify terminates HTTPS and forwards requests to container port `80`.
HTTPS is required for the browser GPU preview. A browser still needs a supported
WebGPU adapter. The existing TypeScript sandbox remains the default page.

## Deployment trigger

Automatic production deployment must follow a successful `verify` job for the
same `main` revision. Keep Coolify's push-based auto-deployment disabled.
Connect the Forgejo deployment trigger after `verify` succeeds.
Store its Coolify API credential in a Forgejo Actions secret.
See the [CI and merge policy](../.forgejo/README.md) for the required checks.

A container build alone does not run all repository checks. A healthy HTTP
response also does not establish browser or GPU correctness.

## PR preview setup

Create an empty `pr-previews` environment in the same Coolify project.
Select the deployment server and the existing repository read key for preview builds.
Keep this environment separate from production. Do not copy production environment
variables, persistent storage, host port mappings or custom Docker options.

Configure the Forgejo preview variables listed in the
[CI policy](../.forgejo/README.md#optional-pr-previews).
Use `https://sand-pr-{pr}.xerrion.io` as the domain template.
The helper replaces `{pr}` with the PR number, such as `sand-pr-16.xerrion.io`.
DNS and HTTPS must cover those addresses before preview deployment is enabled.

Coolify v4.3.23 cannot independently pin an API-triggered Git preview to its PR commit.
The helper therefore creates one separate Dockerfile application per PR.
It uses the same port and healthcheck as production, with a full PR commit SHA.
Keep Coolify's native preview and push-based auto-deployment settings disabled.
Forgejo owns preview creation, updates and removal.

PRs from this repository targeting `main` can receive previews after full CI succeeds.
PRs from forks do not receive previews on the deployment server.
Repository writers control eligible branches and their build and test definitions.
The secret-bearing preview workflow executes control code from `main`.
It reads PR metadata and CI results as data.

New commits update the same preview application after their checks pass.
Closing or merging a PR that targets `main` removes its managed preview application.
Cleanup waits for active deployments to finish and rechecks whether removal still applies.
It preserves volumes, connected networks and server-wide Docker resources.
If cleanup fails, use the preview workflow's manual PR-number input to retry.
Also run that manual reconciliation after retargeting a PR away from `main`.
Forgejo selects target workflows from the PR's base branch, so the new base may have no cleanup workflow.
The manual controller removes only the existing managed preview and stops if the PR targets `main` again.

Check the preview address and its deployed SHA separately from the CI result.
Also check that production still serves its original revision after a preview update.
The existing experimental GPU limitations apply to previews too.

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
[browser evidence requirements](../.forgejo/README.md#browser-evidence) when an
acceptance check requires a real GPU adapter.
