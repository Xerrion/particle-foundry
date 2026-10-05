#!/usr/bin/env bash
set -euo pipefail
umask 077

readonly runner_distribution=/opt/actions-runner
readonly runner_state=${RUNNER_STATE_DIR:-/runner-state}
readonly runner_work=${RUNNER_WORK_DIR:-/home/runner/_work}
readonly runner_url=${RUNNER_URL:-https://github.com/Xerrion/particle-foundry}
readonly runner_labels=${RUNNER_LABELS:-particle-foundry-ci}
: "${RUNNER_NAME:?Set RUNNER_NAME to the registered slot name}"

if [[ $(id -u) != 1001 ]]; then
    printf 'The runner must use UID 1001.\n' >&2
    exit 1
fi
mkdir -p "$runner_state" "$runner_work" "$CARGO_HOME" "$CARGO_TARGET_DIR" \
    "$MISE_DATA_DIR/bin" "$MISE_CACHE_DIR" "$BUN_INSTALL_CACHE_DIR" "$XDG_CACHE_HOME"

# Tools remain in the image. Only downloads and build products use the cache volume.
install -m 0755 /usr/local/bin/mise "$MISE_DATA_DIR/bin/mise"
mise link bun@1.3.12 /opt/bun >/dev/null
mise link node@24.15.0 /opt/node >/dev/null
mise link python@3.14.7 /usr/local >/dev/null

# The persistent directory is the actual runner root. Its delete-and-recreate
# credential writes then stay on the volume without relying on symbolic links.
# Refresh only distribution files from the immutable image on every start.
rm -rf -- "${runner_state:?}/bin" "${runner_state:?}/externals"
cp -R --preserve=mode,timestamps -- \
    "$runner_distribution/bin" "$runner_distribution/externals" "$runner_state/"
for file in "$runner_distribution"/*; do
    if [[ -f "$file" ]]; then
        cp --preserve=mode,timestamps --remove-destination -- "$file" "$runner_state/"
    fi
done
cd "$runner_state"

if [[ -f "$runner_state/.runner" ]]; then
    if [[ ! -s "$runner_state/.credentials" || ! -s "$runner_state/.credentials_rsaparams" ]]; then
        printf 'Registration state is incomplete. Follow the state recovery guide.\n' >&2
        exit 1
    fi
    if ! jq --exit-status --arg name "$RUNNER_NAME" --arg work "$runner_work" \
        --arg url "$runner_url" \
        '.agentName == $name and .workFolder == $work and .gitHubUrl == $url' \
        "$runner_state/.runner" >/dev/null; then
        printf 'Saved registration does not match this slot. Follow the state recovery guide.\n' >&2
        exit 1
    fi
else
    if [[ -z ${ACTIONS_RUNNER_INPUT_TOKEN:-} ]]; then
        readonly bootstrap_file=$runner_state/bootstrap-token
        printf 'Waiting for the protected registration token file.\n'
        while true; do
            if [[ -f "$bootstrap_file" && ! -L "$bootstrap_file" ]]; then
                if [[ $(stat --format='%u:%a' "$bootstrap_file") != 1001:600 ]]; then
                    printf 'The registration token file must belong to UID 1001 with mode 0600.\n' >&2
                    exit 1
                fi
                # A complete newline-terminated token avoids racing its writer.
                if IFS= read -r ACTIONS_RUNNER_INPUT_TOKEN < "$bootstrap_file" \
                    && [[ -n "$ACTIONS_RUNNER_INPUT_TOKEN" ]]; then
                    export ACTIONS_RUNNER_INPUT_TOKEN
                    rm "$bootstrap_file"
                    break
                fi
            fi
            sleep 1
        done
    fi
    # config.sh reads and masks the token from its environment. Keep it out of argv.
    ./config.sh --unattended --url "$runner_url" --name "$RUNNER_NAME" \
        --labels "$runner_labels" --work "$runner_work" --disableupdate
fi
unset ACTIONS_RUNNER_INPUT_TOKEN
rm -f "$runner_state/bootstrap-token"

printf 'Starting runner %s.\n' "$RUNNER_NAME"
exec ./run.sh
