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
Changes confined to `docs/` skip the web, Rust and browser jobs.
Every other change runs all three jobs.
The final `verify` job requires every applicable job to succeed.
`mise run ci` remains the complete local check.

Use a feature branch and a conventional PR title.
Review the complete change before delivery.
Require a successful `verify` result and resolve review findings before merging.
Passing checks do not authorize a merge or deployment.
The Forgejo branch protection settings need separate verification.
Do not infer their configuration from this policy or the old GitHub ruleset.

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
