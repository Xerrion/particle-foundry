FROM oven/bun:1.3.12@sha256:8956c7667fa17beb6e3c664115e66bdacfe502da5d99603626e74c197bdef160 AS bun
FROM node:24.15.0-bookworm@sha256:f22d6a1f082c02f292e86929b5b0442ac2e5eaf438a5dea9b1566601c3e05940 AS node
FROM jdxcode/mise:2026.9.13@sha256:1e74fb7d744027d453731d948dda0748cf19a3e7b4357b09787448088d1ff2ad AS mise
FROM rust:1.98.1-bookworm@sha256:93ce27a88655056a51dbdd8f5f2d7ddc071c7b0070fb288a37b5a285fc83971e AS rust

RUN rustup component add rustfmt clippy \
    && rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version 0.2.108 --locked \
    && rm -rf /usr/local/cargo/registry /usr/local/cargo/git

FROM python:3.14.7-bookworm@sha256:bfb689a7986adc6d5f16722e06c78e755efe4062e56276715fef450cbad09436

ENV CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    MISE_DATA_DIR=/opt/mise \
    MISE_CACHE_DIR=/tmp/mise-cache \
    MISE_YES=1 \
    MISE_NOT_FOUND_AUTO_INSTALL=0 \
    CHROME_BIN=/usr/local/bin/chrome-ci \
    PATH=/opt/bun/bin:/opt/node/bin:/usr/local/cargo/bin:/usr/local/bin:/usr/local/sbin:/usr/sbin:/usr/bin:/sbin:/bin

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        build-essential ca-certificates curl git libssl-dev pkg-config zstd \
    && curl --fail --silent --show-error --location \
        --output /tmp/chrome.deb \
        https://dl.google.com/linux/chrome/deb/pool/main/g/google-chrome-stable/google-chrome-stable_154.0.8037.97-1_amd64.deb \
    && echo 'a4edbe95e9b01db6c9b97d7a1323121eda18362b5620df06abac1b59bee80053  /tmp/chrome.deb' | sha256sum --check \
    && apt-get install --yes --no-install-recommends /tmp/chrome.deb \
    && rm -rf /tmp/chrome.deb /var/lib/apt/lists/*

ENV MISE_OFFLINE=1 \
    MISE_DISABLE_UPDATE_WARNING=1 \
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

COPY --from=bun /usr/local/bin/bun /opt/bun/bin/bun
COPY --from=node /usr/local/bin/node /opt/node/bin/node
COPY --from=mise /usr/local/bin/mise /usr/local/bin/mise
COPY --from=rust /usr/local/cargo /usr/local/cargo
COPY --from=rust /usr/local/rustup /usr/local/rustup
COPY chrome-ci /usr/local/bin/chrome-ci

# Link the installed versions so repository tasks never fetch tool releases.
RUN chmod 755 /usr/local/bin/chrome-ci \
    && mise link bun@1.3.12 /opt/bun \
    && mise link node@24.15.0 /opt/node \
    && mise link python@3.14.7 /usr/local \
    && bun --version \
    && node --version \
    && python --version \
    && mise --version \
    && rustc --version \
    && wasm-bindgen --version \
    && google-chrome --version

CMD ["bash"]
