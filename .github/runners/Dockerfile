FROM oven/bun:1.4.3@sha256:ec06c3b6cea04192ae6770c434f668ca41d343ad19fa6472216c7b48be39c598 AS bun
FROM node:26.12.0-bookworm@sha256:a856ae5e25d498bbe5ec85fde88f00f5ed99908fda803ba39d7735ab8574f58c AS node
FROM jdxcode/mise:2026.10.14@sha256:7cb6019fd7e60dfc3622b1d667d3e57628549fbc9e36b4a5768265388342f754 AS mise
FROM python:3.15.0-bookworm@sha256:589def1896e0bb0f4d73278a020391b4f91c16cccc3736631f6ab53a3d6c5acc AS python
FROM rust:1.99.1-bookworm@sha256:e12566d5abc17a0da741628782cdb504b68b8961f5bfcf677936bf3e8b32de29 AS rust

RUN rustup component add rustfmt clippy \
    && rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version 0.2.129 --locked

FROM ghcr.io/actions/actions-runner:2.338.0@sha256:4ffadc0002b2581327e06101fc8c06cd189232baf79fe561fac9caeb76f5e807

USER 0
SHELL ["/bin/bash", "-o", "pipefail", "-c"]

# The official runner image uses Ubuntu 24.04. Package installation also resolves
# Chrome's runtime libraries; the additional libraries support the copied Python.
# Install current security updates from the pinned Ubuntu release.
# hadolint ignore=DL3008
RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        build-essential ca-certificates curl git jq libssl-dev pkg-config \
        libbz2-1.0 libexpat1 libffi8 libgdbm6t64 libgdbm-compat4t64 liblzma5 \
        libncursesw6 libreadline8t64 libsqlite3-0 libtcl8.6 libtk8.6 libuuid1 \
        tar xz-utils zlib1g zstd \
    && curl --fail --location --retry 3 \
        --proto '=https' --proto-redir '=https' --tlsv1.2 \
        https://dl.google.com/linux/chrome/deb/pool/main/g/google-chrome-stable/google-chrome-stable_155.0.8059.84-1_amd64.deb \
        --output /tmp/google-chrome.deb \
    && printf '%s  %s\n' \
        8fbd2ae69cdf7ee98b7513c3396a7b5e06f974f8625fdd590df3091dd0c3e9de \
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
