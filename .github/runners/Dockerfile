FROM oven/bun:1.3.12@sha256:8956c7667fa17beb6e3c664115e66bdacfe502da5d99603626e74c197bdef160 AS bun
FROM node:24.15.0-bookworm@sha256:f22d6a1f082c02f292e86929b5b0442ac2e5eaf438a5dea9b1566601c3e05940 AS node
FROM jdxcode/mise:2026.9.13@sha256:1e74fb7d744027d453731d948dda0748cf19a3e7b4357b09787448088d1ff2ad AS mise
FROM python:3.14.7-bookworm@sha256:bfb689a7986adc6d5f16722e06c78e755efe4062e56276715fef450cbad09436 AS python
FROM rust:1.98.1-bookworm@sha256:93ce27a88655056a51dbdd8f5f2d7ddc071c7b0070fb288a37b5a285fc83971e AS rust

RUN rustup component add rustfmt clippy \
    && rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version 0.2.108 --locked

FROM ghcr.io/actions/actions-runner:2.337.0@sha256:e5496277be5d09bc968b3d64911b74e219ac4a3f2edce956a3ecf9271bea1ef4

USER root

# The official runner image uses Ubuntu 24.04. Package installation also resolves
# Chrome's runtime libraries; the additional libraries support the copied Python.
RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        build-essential ca-certificates curl git jq libssl-dev pkg-config \
        libbz2-1.0 libexpat1 libffi8 libgdbm6t64 libgdbm-compat4t64 liblzma5 \
        libncursesw6 libreadline8t64 libsqlite3-0 libtcl8.6 libtk8.6 libuuid1 \
        tar xz-utils zlib1g zstd \
    && curl --fail --location --retry 3 \
        --proto '=https' --proto-redir '=https' --tlsv1.2 \
        https://dl.google.com/linux/chrome/deb/pool/main/g/google-chrome-stable/google-chrome-stable_154.0.8037.97-1_amd64.deb \
        --output /tmp/google-chrome.deb \
    && printf '%s  %s\n' \
        a4edbe95e9b01db6c9b97d7a1323121eda18362b5620df06abac1b59bee80053 \
        /tmp/google-chrome.deb | sha256sum --check --strict \
    && apt-get install --yes --no-install-recommends /tmp/google-chrome.deb \
    && rm /tmp/google-chrome.deb \
    && rm -rf /var/lib/apt/lists/*

COPY --from=bun /usr/local/bin/bun /opt/bun/bin/bun
COPY --from=node /usr/local/bin/node /opt/node/bin/node
COPY --from=mise /usr/local/bin/mise /usr/local/bin/mise
COPY --from=python /usr/local/ /usr/local/
COPY --from=rust --chown=1001:1001 /usr/local/cargo/bin/ /opt/cargo/bin/
COPY --from=rust --chown=1001:1001 /usr/local/rustup/ /opt/rustup/

ENV CARGO_HOME=/home/runner/cache/cargo \
    CARGO_TARGET_DIR=/home/runner/cache/target \
    RUSTUP_HOME=/opt/rustup \
    MISE_DATA_DIR=/home/runner/cache/mise \
    MISE_CACHE_DIR=/home/runner/cache/mise-downloads \
    MISE_YES=1 \
    MISE_DISABLE_UPDATE_WARNING=1 \
    BUN_INSTALL_CACHE_DIR=/home/runner/cache/bun \
    XDG_CACHE_HOME=/home/runner/cache/xdg \
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
    CHROME_BIN=/usr/bin/google-chrome \
    PATH=/opt/bun/bin:/opt/node/bin:/opt/cargo/bin:/usr/local/bin:/usr/local/sbin:/usr/sbin:/usr/bin:/sbin:/bin

RUN ldconfig \
    && usermod --groups runner runner \
    && chmod 0750 /home/runner \
    && install --directory --owner=0 --group=0 --mode=0755 /opt/actions-runner \
    && cp -a /home/runner/. /opt/actions-runner/ \
    && chown -R root:root /opt/actions-runner \
    && chmod -R a+rX,go-w /opt/actions-runner \
    && chmod 0755 /opt/actions-runner \
    && test "$(stat --format='%u:%g:%a' /opt/actions-runner)" = 0:0:755 \
    && test -z "$(find /opt/actions-runner -type d ! -perm -005 -print -quit)" \
    && test -z "$(find /opt/actions-runner -type d -perm /0022 -print -quit)" \
    && test -z "$(find /opt/actions-runner -type f -perm /0022 -print -quit)" \
    && install --directory --owner=1001 --group=1001 --mode=0700 \
        /runner-state /home/runner/_work /home/runner/cache \
    && python --version \
    && bun --version \
    && node --version \
    && mise --version \
    && wasm-bindgen --version \
    && google-chrome --version \
    && chown -R 1001:1001 /home/runner/cache \
    && test -z "$(find /home/runner/cache ! -uid 1001 -print -quit)" \
    && test -z "$(find /home/runner/cache ! -gid 1001 -print -quit)"

COPY --chmod=0755 .github/runners/entrypoint.sh /usr/local/bin/particle-foundry-runner

USER 1001:1001
RUN test "$(id -u)" = 1001 \
    && mkdir -p "$CARGO_HOME" "$CARGO_TARGET_DIR" "$MISE_DATA_DIR/bin" \
        "$MISE_CACHE_DIR" "$BUN_INSTALL_CACHE_DIR" "$XDG_CACHE_HOME" \
    && touch "$MISE_DATA_DIR/bin/.runner-cache-write-check" \
    && rm "$MISE_DATA_DIR/bin/.runner-cache-write-check"
WORKDIR /runner-state
ENTRYPOINT ["/usr/local/bin/particle-foundry-runner"]
