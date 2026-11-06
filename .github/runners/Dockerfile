FROM oven/bun:1.4.4@sha256:1d22548f177f4ff38e0c15baa3c2c46c6c2060ba2d1bc573bb4720b407548f1d AS bun
FROM node:26.13.0-bookworm@sha256:a546d3f8acd2af4382668a7e6acd3bc4418e6a27478373e3246f8162f5181509 AS node
FROM jdxcode/mise:2026.11.2@sha256:39cd8aad519a3dbc860eceb64dfd8cb5efe4cdbb8ad741a6867b544b2787b79c AS mise
FROM python:3.15.2-bookworm@sha256:6f060cab421d9509c6c6a6a7762c651baf03c3f1d0145befe16b688762b7fe81 AS python
FROM rust:1.99.1-bookworm@sha256:e6f2cff35d0f309acf8433b4f05878ed77092602c5065cc81801177da0bb3a9c AS rust

RUN rustup component add rustfmt clippy \
  && rustup target add wasm32-unknown-unknown \
  && cargo install wasm-bindgen-cli --version 0.2.130 --locked

FROM ghcr.io/actions/actions-runner:2.338.2@sha256:759a1d94a15b05941aba5196974ff43fe15b4cbba29b694b35cbc32a1166b232

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
  https://dl.google.com/linux/chrome/deb/pool/main/g/google-chrome-stable/google-chrome-stable_157.0.8103.38-1_amd64.deb \
  --output /tmp/google-chrome.deb \
  && printf '%s  %s\n' \
  a62b277dc03da2cff50cc04589deb66f6875be339c066d46adb49b49eedb629d \
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
