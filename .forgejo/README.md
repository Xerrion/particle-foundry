# Forgejo CI and merge policy

The primary repository is
[xerrion/particle-foundry](https://git.xerrion.io/xerrion/particle-foundry).
[CI](workflows/ci.yml) uses the same pinned tools and mise tasks as local development.
The former [GitHub settings](../.github/README.md) are a dated record.
They do not establish the Forgejo branch settings.

## Runner and tool image

CI uses the existing self-hosted Docker runners through the `ubuntu-latest` label.
Every job selects a prepared Debian Bookworm image from this Forgejo instance.
The workflow pins the published image by digest. It requires amd64 Docker support
and access to Forgejo, the npm registry and crates.io. The automatic job token
authenticates image pulls. Jobs run as root inside
isolated containers. No extra runner is required.

The [Dockerfile](ci/Dockerfile) contains digest-pinned Docker Hub tool images,
the pinned Rust toolchain, compiler tools, wasm-bindgen and a checksum-pinned Chrome.
Mise links the existing Bun, Node and Python installations. Its offline setting
prevents tool downloads during CI. Cargo uses the sparse crates.io protocol.
GitHub-hosted actions, their Forgejo mirrors and GitHub container images are absent
from the workflow. All checkout, cache and artifact operations use local steps.

To update the tool image:

1. Keep tool versions aligned with `mise.toml` and `engine/rust-toolchain.toml`.
2. Build from `.forgejo/ci/` with `docker build --platform linux/amd64 -t git.xerrion.io/xerrion/particle-foundry-ci:<new-tag> .forgejo/ci`.
3. Check the tool versions and browser execution inside the image. Push it with an existing Forgejo registry credential.
4. Set every workflow container image to the new immutable digest. The cache key also includes the image source and mise configuration.

No credentials belong in the build context, Dockerfile or image layers.
The image build obtains tools from Docker Hub, Rust components from the official
Rust distribution, packages from Debian and Chrome from Google.
Some upstream tool projects develop their software on GitHub. CI does not contact
GitHub to obtain their tools or execute their actions.

## Checkout and native helpers

Checkout fetches the exact pull request head or push commit from Forgejo.
The temporary HTTP authentication header exists only during the fetch.
It is removed before repository scripts run and is never written to Git configuration.
The scope job fetches the commit history to find the common base.
Other jobs use a shallow checkout.

The Python helpers use Forgejo's runtime APIs directly:

- [Cache](ci/cache.py) uses Forgejo Runner's lookup, reserve, chunk upload and finalize endpoints.
- [Artifact upload](ci/artifact.py) uses Forgejo's artifact v3 API with a checksum for each chunk and one-day retention.
- [HTTP client](ci/runtime.py) rejects redirects and unexpected origins. Errors omit opaque runtime URLs and tokens.

The docs job runs [helper contract tests](ci/test_helpers.py). These cover cache
misses, exact hits, previous-build restores, path restrictions and artifact chunks.
It also runs [deployment contract tests](ci/test_deploy.py) against a local HTTP fixture.
See the [Forgejo environment reference](https://forgejo.org/docs/v15.0/user/actions/reference/),
[runner cache implementation](https://code.forgejo.org/forgejo/runner/src/tag/v13.0.0/act/artifactcache/handler.go)
and [artifact implementation](https://codeberg.org/forgejo/forgejo/src/tag/v15.0.9/routers/api/actions/artifacts.go).

## Build cache and task grouping

Each web, Rust and browser job has a separate native cache. All three cache Bun
package downloads and `web/node_modules/`. Rust and browser also cache Cargo
registry/git downloads and `engine/target/`. Installed toolchains and Chrome live
in the image, so cache archives do not transfer them on every run.
Cargo credentials and global configuration are outside the selected paths.

Keys include the job, image source, mise configuration, Rust toolchain, lockfiles
and engine source. A changed engine source creates a new immutable cache entry.
Restore prefixes can reuse earlier outputs for the same tools. Cargo checks which
outputs need to be rebuilt. Cache failures issue a warning and allow a normal build.
Every applicable check still runs. Successful jobs save new entries. Exact hits
avoid recompressing and uploading the same cache.

The cache service must be reachable from job containers. Each existing runner
stores its cache in `/data/cache` on its persistent bind mount. The runners keep
separate entries, so each runner can need an initial build. Pull request writes
are isolated to that pull request. The first run on `main` can therefore be cold.
See [Forgejo cache support](https://forgejo.org/docs/v15.0/user/actions/advanced-features/#cache).

`ci:web` shares one dependency installation. `ci:rust` shares one toolchain check.
`ci:browser` shares WASM generation across production build, types and browser smoke.
The WASM and catalogue freshness checks run after those tasks complete.

## Checks and merge requirements

CI runs for pull request updates and pushes to `main`.
The documentation check always runs.
Changes confined to `docs/` skip the web, Rust and browser jobs for pull requests
and when deployment is disabled. Every other change runs all three jobs.
When deployment is enabled, every push to `main` runs all three jobs, including
documentation changes. This validates the latest source revision for deployment.
The final `verify` job requires every applicable job to succeed.
`mise run ci` remains the complete local check.

Use a feature branch and a conventional PR title.
Review the complete change before delivery.
Require a successful `verify` result and resolve review findings before merging.
Passing checks do not authorize a merge or deployment.
The optional deployment automation below requires separate enablement.
The Forgejo branch protection settings need separate verification.
Do not infer their configuration from this policy or the old GitHub ruleset.

## Optional Coolify deployment

The `deploy` job runs after successful `verify` for pushes to `refs/heads/main`.
Pull requests do not deploy to production.
The job remains disabled until the repository variable `COOLIFY_DEPLOY_ENABLED`
equals `true`. Configure these values in Forgejo repository Actions settings:

| Kind | Name | Value |
| --- | --- | --- |
| Variable | `COOLIFY_DEPLOY_ENABLED` | `true` only after deployment authorization and setup |
| Variable | `COOLIFY_URL` | Coolify base URL without `/api/v1` |
| Variable | `COOLIFY_APP_UUID` | The selected Coolify application UUID |
| Secret | `COOLIFY_TOKEN` | An approved Coolify API token with Read and Write permissions |

Enabling deployment makes every push to `main` run the complete CI checks.
This costs a full validation run for documentation changes.
It prevents a newer documentation commit from blocking deployment of an earlier
application change. Older runs skip after the source check, and the latest main
revision can deploy after its own checks pass.

Coolify v4.3.23 API tokens apply to the creating user's entire team.
Read and Write permissions cover all applications in that team.
Coolify cannot restrict this token to one application.
The user must approve that scope before creating the token.
The UI defaults to a 30-day expiry. Refresh the Forgejo secret before the token
expires, or deployment will fail. Keep tokens out of source, logs and image layers.
The Forgejo job token supplies repository read access for the source check.
Runner containers must reach both API endpoints.

[The deployment helper](ci/deploy.py) checks the current Forgejo `main` commit
immediately before changing Coolify. If it differs from the verified `CI_SHA`,
the helper reports a skipped deployment. This prevents an old workflow rerun
from intentionally restoring an older commit.
One `PATCH /api/v1/applications/{uuid}` sets the full `git_commit_sha` and
`instant_deploy: true`. Coolify saves and queues from the same application object.
The helper does not send a separate branch-based deploy request.

The helper reads `/api/v1/deployments/applications/{uuid}?skip=0&take=10` before
the request and until it finds a new deployment UUID with the verified commit
and no pull request. It then monitors `/api/v1/deployments/{deployment_uuid}`
directly. Newer previews or manual deployments cannot hide the tracked deployment
by displacing it from recent history. A prior finished deployment cannot satisfy
the queue check. HTTP 200 alone does not prove that Coolify accepted the deployment.
The helper fails if no matching deployment appears within 60 seconds, if it
fails or is cancelled, or if it does not finish within 20 minutes.
API responses, build logs, tokens and raw HTTP errors are absent from CI output.
Authenticated requests reject redirects.

Use HTTPS for public API endpoints. The helper permits explicitly configured
HTTP endpoints under `.ts.net` for a private Tailscale connection.
The workflow groups runs by `forgejo.ref` with `cancel-in-progress: false`.
[Forgejo 15 concurrency](https://forgejo.org/docs/v15.0/user/actions/reference/#concurrency)
provides best-effort ordering. Exact commit selection does not depend on that ordering.

The API contract comes from the Coolify v4.3.23
[application update controller](https://github.com/coollabsio/coolify/blob/v4.3.23/app/Http/Controllers/Api/ApplicationsController.php),
[queue helper](https://github.com/coollabsio/coolify/blob/v4.3.23/bootstrap/helpers/applications.php),
[deployment controller](https://github.com/coollabsio/coolify/blob/v4.3.23/app/Http/Controllers/Api/DeployController.php)
and [status enum](https://github.com/coollabsio/coolify/blob/v4.3.23/app/Enums/ApplicationDeploymentStatus.php).
These contracts and local fixture tests do not establish a real deployment.
After enabling the job, check the deployed application in a browser and record
the public URL and deployed commit separately. See the
[Coolify application setup](../docs/deployment.md) for the Dockerfile and host settings.

## Optional PR previews

The separate `preview.yml` workflow handles PR previews with trusted control code.
The [preview helper](ci/preview.py) owns CI gating and the application lifecycle.
It uses `pull_request_target` for opened, updated, reopened, edited and closed PRs targeting `main`.
Manual execution can reconcile an existing PR by number.
The workflow executes from `main` and never checks out PR code with the Coolify secret.

Enable previews only after the dedicated Coolify environment and domain are configured:

| Variable | Value |
| --- | --- |
| `COOLIFY_PREVIEW_ENABLED` | `true` to enable previews |
| `COOLIFY_PREVIEW_PROJECT_UUID` | The Coolify project UUID |
| `COOLIFY_PREVIEW_SERVER_UUID` | The preview deployment server UUID |
| `COOLIFY_PREVIEW_ENV_UUID` | The dedicated preview environment UUID |
| `COOLIFY_PREVIEW_DOMAIN_TEMPLATE` | `https://sand-pr-{pr}.xerrion.io` |
| `COOLIFY_PREVIEW_KEY_NAME` | The unique stored repository read key name |

Previews reuse `COOLIFY_URL`, `COOLIFY_APP_UUID` and the approved `COOLIFY_TOKEN` secret.
The production application UUID identifies the source configuration only.
The helper does not update or delete that application.
Read and Write permissions suffice. No Sensitive or additional Deploy ability is required.
Key discovery reads only key metadata and does not read private key material.

Only same-repository PRs targeting `main` are eligible.
Eligible PRs receive full CI, including documentation changes, when previews are enabled.
The controller requires changes, docs, web, Rust, browser and verify tasks to succeed
for the current PR head in the latest matching CI attempt.
It rejects fork PRs and rechecks the live PR before changing Coolify.
Repository writers can change the CI workflow and test logic in eligible PRs.
Successful task results establish completion of those checked-in CI definitions.

Coolify creates one separate application per PR in the preview environment.
Each deployment pins the verified full head SHA and monitors a new deployment UUID.
The job fails on a failed, cancelled, mismatched or timed-out deployment.
Closing or merging a PR targeting `main` removes only its managed preview application.
Cleanup waits for active deployment jobs before deletion and guards against renewed eligibility.
It requests no volume, connected-network or server-wide Docker cleanup.
After retargeting a PR away from `main`, run manual reconciliation to remove its preview.
Forgejo selects a target workflow from the PR base branch, which can lack the new workflow.
Automatic target events stay restricted to `main` to preserve the trusted workflow source.

Workflow concurrency uses the PR number, because target events share the `main` ref.
Forgejo concurrency provides best-effort ordering.
Live PR checks and exact commit selection provide additional stale-run protection.
The helper cannot make Forgejo state and Coolify changes one atomic transaction.
A later event reconciles newer PR state.

See [PR preview setup](../docs/deployment.md#pr-preview-setup) for live configuration
and browser verification. Local fixture tests do not prove a real preview deployment.

The implementation targets the Coolify v4.3.23
[application API](https://github.com/coollabsio/coolify/blob/v4.3.23/app/Http/Controllers/Api/ApplicationsController.php)
and [deletion job](https://github.com/coollabsio/coolify/blob/v4.3.23/app/Jobs/DeleteResourceJob.php).
Forgejo v15 supplies the
[run and task API](https://codeberg.org/forgejo/forgejo/src/tag/v15.0.9/routers/api/v1/repo/action.go)
and [trusted target workflow selection](https://codeberg.org/forgejo/forgejo/src/tag/v15.0.9/services/actions/notifier_helper.go).

## Browser evidence

The image selects [a Chrome wrapper](ci/chrome-ci) through `CHROME_BIN`.
It disables the sandbox only when Chrome runs as root in the isolated job container.
Local browser execution keeps its existing settings.

The default browser smoke can pass with `gpu.status: unavailable`.
That result covers the executed browser and WASM checks.
It does not prove GPU simulation or rendering.
Run `mise run test:browser artifacts/validation/browser/<new-run> --require-gpu`
on a machine with a real adapter for that gate.
Record hardware evidence separately. Native artifact upload preserves nonempty
browser evidence files and excludes symbolic links.
