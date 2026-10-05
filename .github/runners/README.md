# Particle Foundry runners on Charles

This image supports the existing Linux CI tasks with two persistent slots:
`particle-foundry-charles-1` and `particle-foundry-charles-2`. Both register only
with `Xerrion/particle-foundry` and use the label `particle-foundry-ci`.
Workflow routing belongs in [ci.yml](../workflows/ci.yml).

Each slot has a two-CPU limit, a 4 GiB memory limit, 1 GiB shared memory, an init
process and its own configuration, work and cache volumes. The combined limits
leave capacity for the other services on Charles. Actual CI duration and peak
memory still need validation on this host.

The containers run as UID/GID `1001:1001`, drop all capabilities and enable
`no-new-privileges`. They have no host Docker socket, host network, published
ports or GPU device. The image includes Docker tooling inherited from the
official runner, but that tooling has no Docker daemon connection.

## Image and tool pins

The base is the official `ghcr.io/actions/actions-runner:2.337.0` image, pinned
to digest `sha256:e5496277be5d09bc968b3d64911b74e219ac4a3f2edce956a3ecf9271bea1ef4`.
Its Linux distribution is Ubuntu 24.04. The tool image versions and digests
match the [production Dockerfile](../../Dockerfile): Bun `1.3.12`, Node
`24.15.0`, Python `3.14.7`, mise `2026.9.13` and Rust `1.98.1`.
Rustfmt, Clippy, `wasm32-unknown-unknown` and wasm-bindgen CLI `0.2.108` are
installed during the image build.

Chrome is `154.0.8037.97-1`, downloaded from Google's official Debian package
URL. The build verifies SHA256
`a4edbe95e9b01db6c9b97d7a1323121eda18362b5620df06abac1b59bee80053`
before installation. Chrome runs with its sandbox enabled. CI uses headless
Chrome and localhost ports `4174` and `4175`; it needs no Xvfb or public port.

[seccomp-profile.json](seccomp-profile.json) comes from
[Playwright commit ce480a952553175eae75342aad2c5e86cdf2cbba](https://github.com/microsoft/playwright/blob/ce480a952553175eae75342aad2c5e86cdf2cbba/utils/docker/seccomp_profile.json),
the `v1.58.2` revision. It extends Docker's default policy to allow `clone`,
`setns` and `unshare` for Chrome's user namespace sandbox. There are two local
adjustments: `clone3` returns `ENOSYS` so modern glibc can fall back to `clone`,
and `chroot` is allowed for Chrome's filesystem confinement inside its user
namespace. The initial Charles Chrome probe reached that namespace but failed
at `sys_chroot` with all capabilities dropped. The allowance retains the kernel's
capability checks and does not grant a host capability. See
[Playwright's container guidance](https://playwright.dev/docs/docker).
The host must permit unprivileged user namespaces. Host AppArmor policy can
still block them even when the seccomp profile permits them. Validate Chrome
inside this exact container policy before enabling workflow routing.

Normal CI does not require a GPU. Native GPU tests remain ignored, and the
browser smoke can report GPU unavailability. These runners do not replace the
separate native and browser hardware qualification.

## Build and initial registration

Build on Charles from the repository root. No registration credential is
needed to build the image:

```sh
docker build --platform linux/amd64 \
  --file .github/runners/Dockerfile \
  --tag particle-foundry-github-runner:2026-10-05 .
```

For the existing Coolify deployment, start the registered service through
Coolify or use its generated Compose directory below. Do not start another
fleet from the repository directory. The checked-in Compose file can also
create a standalone installation, but that is a separate deployment.

Keep this directory on the host, including the seccomp file. If Coolify runs
the Compose file from another directory, set `RUNNER_SECCOMP_PROFILE` to the
absolute host path of this same profile. Use the already built image;
the Compose file does not start a build.

The active Coolify service is `1rtp5fpxk7xrvx1tgxmgfajc`. Its generated Compose
directory is `/data/coolify/services/1rtp5fpxk7xrvx1tgxmgfajc`; place the profile
at `seccomp-profile.json` in that directory. Coolify changes the Docker container
names to `runner-1-1rtp5fpxk7xrvx1tgxmgfajc` and
`runner-2-1rtp5fpxk7xrvx1tgxmgfajc`, and can prefix volume names. These Docker
names differ from the GitHub runner identities. Resolve containers through the
active Compose project instead of assuming the standalone names.

Each unconfigured slot waits for `/runner-state/bootstrap-token`. The file
must belong to UID `1001`, have mode `0600` and contain one complete token
line. Supply a fresh one-hour repository registration token through SSH
standard input. On the operator's authenticated GitHub machine, with shell
tracing disabled and the `charles` SSH host configured:

```sh
RUNNER_COMPOSE_DIR=/data/coolify/services/1rtp5fpxk7xrvx1tgxmgfajc
RUNNER_1_CONTAINER=$(ssh charles "cd '$RUNNER_COMPOSE_DIR' && docker compose ps --all --quiet runner-1")
RUNNER_2_CONTAINER=$(ssh charles "cd '$RUNNER_COMPOSE_DIR' && docker compose ps --all --quiet runner-2")
test -n "$RUNNER_1_CONTAINER" && test -n "$RUNNER_2_CONTAINER"
gh api --method POST repos/Xerrion/particle-foundry/actions/runners/registration-token \
  --jq .token | ssh charles \
  "docker exec --interactive --user 1001:1001 '$RUNNER_1_CONTAINER' sh -c 'umask 077; cat > /runner-state/bootstrap-token'"
gh api --method POST repos/Xerrion/particle-foundry/actions/runners/registration-token \
  --jq .token | ssh charles \
  "docker exec --interactive --user 1001:1001 '$RUNNER_2_CONTAINER' sh -c 'umask 077; cat > /runner-state/bootstrap-token'"
```

For a standalone deployment, set `RUNNER_COMPOSE_DIR` to its Compose directory.
Run the remaining host commands from the active Compose directory on Charles.

Wait until both runners are online in GitHub and the logs show
`Listening for Jobs`. The entrypoint reads the file into
`ACTIONS_RUNNER_INPUT_TOKEN` for `config.sh`, removes the file, then unsets
the environment input before `run.sh`. The token is absent from Compose,
Coolify environment settings, Docker metadata and process arguments. Verify
the consumed files on Charles without printing their contents:

```sh
docker compose exec -T runner-1 test ! -e /runner-state/bootstrap-token
docker compose exec -T runner-2 test ! -e /runner-state/bootstrap-token
```

Do not print or archive the bootstrap file. Do not enable shell tracing.
No personal access token belongs in these services. The script also accepts
`ACTIONS_RUNNER_INPUT_TOKEN` for initial configuration, but Compose deliberately
does not define it; use the file path for deployment. Saved runner credentials
handle later starts without any new registration token.

## Persistent state and image updates

Each configuration volume is the actual runner root at `/runner-state`, holding
settings, credentials and diagnostics. The runner can delete and recreate its
configuration files directly on that volume. Persistence does not depend on
symbolic links or copying credentials during shutdown.

The image keeps a root-owned distribution at `/opt/actions-runner`, with root
directory mode `0755`, readable files and traversable directories. Group and
other write permissions are removed. Build checks verify these permissions.
On every start, the entrypoint replaces only `bin`, `externals` and the distribution's
startup files in `/runner-state` with this image copy. The refreshed files belong
to UID `1001` and retain executable permissions. Image updates therefore refresh
the actual runner binaries instead of leaving older volume copies active.
Settings, credentials, diagnostics, work files and caches are retained.
The entrypoint rejects incomplete state or a saved name, repository URL or work
directory that differs from the configured slot. It never replaces an existing
GitHub registration automatically.

Each work volume holds checkouts and job files. Each cache volume holds mise,
Bun and Cargo downloads plus the slot's Cargo build output. Tool binaries and
the Rust toolchain stay in the image. No two slots share a Cargo target directory.

Automatic runner updates are disabled during registration. Build and deploy a
new pinned image when updating the runner. GitHub requires an update within
30 days of a runner release, or sooner for a required security update. See the
[runner update policy](https://docs.github.com/en/actions/reference/runners/self-hosted-runners#runner-software-updates-on-self-hosted-runners).
Recreate one slot only after GitHub shows that slot is idle. Its configuration
volume retains its identity. Reusing the image tag requires an explicit
recreation. These commands affect only the named slot:

```sh
docker compose up --detach --no-build --no-deps --force-recreate runner-1
```

Wait for runner 1 to return online. Run the following command only after
GitHub shows runner 2 is idle:

```sh
docker compose up --detach --no-build --no-deps --force-recreate runner-2
```

## State recovery

Keep credential backups outside the checkout and restrict them to the operator.
Stop an idle affected slot before copying its configuration volume. For slot 1:

```sh
RUNNER_1_CONTAINER=$(docker compose ps --all --quiet runner-1)
RUNNER_1_CONFIG_VOLUME=$(docker inspect --format '{{range .Mounts}}{{if eq .Destination "/runner-state"}}{{.Name}}{{end}}{{end}}' "$RUNNER_1_CONTAINER")
test -n "$RUNNER_1_CONFIG_VOLUME"
docker compose stop runner-1
umask 077
docker run --rm --user 1001:1001 \
  --mount "type=volume,src=$RUNNER_1_CONFIG_VOLUME,dst=/state,readonly" \
  --entrypoint tar particle-foundry-github-runner:2026-10-05 \
  -C /state --exclude=./bin --exclude=./externals -czf - . \
  > /secure-backups/particle-foundry-charles-1-state.tar.gz
```

The operator must create `/secure-backups` with restricted permissions first.
Treat the archive as a credential, and never upload it as a CI artifact.
A valid backup must be restored with UID/GID `1001:1001` into the same slot's
configuration volume. Preserve the existing GitHub runner record when restoring
valid credentials, then start that slot without a registration token.

If the credentials are lost, corrupt, or GitHub has deleted the registration,
remove the old runner record in GitHub before registering that slot again.
After preserving available state, recreate only its configuration volume:

```sh
docker compose rm --force runner-1
docker volume rm "$RUNNER_1_CONFIG_VOLUME"
```

Repeat initial registration for `runner-1` only, then recreate that container
if needed to apply a new image. Keep its work and cache volumes. Never use
`docker compose down --volumes` for a normal restart or image update; that
would delete both identities and their saved data.
