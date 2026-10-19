FROM oven/bun:1.4.3@sha256:ec06c3b6cea04192ae6770c434f668ca41d343ad19fa6472216c7b48be39c598 AS bun
FROM node:26.12.0-bookworm@sha256:a856ae5e25d498bbe5ec85fde88f00f5ed99908fda803ba39d7735ab8574f58c AS node
FROM jdxcode/mise:2026.10.14@sha256:7cb6019fd7e60dfc3622b1d667d3e57628549fbc9e36b4a5768265388342f754 AS mise
FROM rust:1.99.1-bookworm@sha256:e12566d5abc17a0da741628782cdb504b68b8961f5bfcf677936bf3e8b32de29 AS rust

RUN rustup component add rustfmt clippy \
    && rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version 0.2.129 --locked \
    && rm -rf /usr/local/cargo/registry /usr/local/cargo/git

FROM python:3.15.0-bookworm@sha256:589def1896e0bb0f4d73278a020391b4f91c16cccc3736631f6ab53a3d6c5acc AS build

ENV CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    MISE_DATA_DIR=/opt/mise \
    MISE_CACHE_DIR=/tmp/mise-cache \
    MISE_YES=1 \
    MISE_OFFLINE=1 \
    MISE_NOT_FOUND_AUTO_INSTALL=0 \
    MISE_DISABLE_UPDATE_WARNING=1 \
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
    PATH=/opt/bun/bin:/opt/node/bin:/usr/local/cargo/bin:/usr/local/bin:/usr/local/sbin:/usr/sbin:/usr/bin:/sbin:/bin

# Install current security updates from the pinned Debian release.
# hadolint ignore=DL3008
RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        build-essential ca-certificates git libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY --from=bun /usr/local/bin/bun /opt/bun/bin/bun
COPY --from=node /usr/local/bin/node /opt/node/bin/node
COPY --from=mise /usr/local/bin/mise /usr/local/bin/mise
COPY --from=rust /usr/local/cargo /usr/local/cargo
COPY --from=rust /usr/local/rustup /usr/local/rustup

RUN mise link bun@1.4.3 /opt/bun \
    && mise link node@26.12.0 /opt/node \
    && mise link python@3.15.0 /usr/local

WORKDIR /app
COPY . .
ARG VITE_SENTRY_WEB_DSN
ARG VITE_SENTRY_SIM_DSN
ARG VITE_SENTRY_RELEASE
ARG VITE_SENTRY_REVISION
ARG VITE_SENTRY_ENVIRONMENT=production
RUN mise trust /app/mise.toml \
    && mise run build

FROM scratch AS reporting-artifacts
COPY --from=build /app/web/reporting-artifacts/ /

FROM nginx:1.31.7-alpine@sha256:892bb2152464f0c5d8972593a5f5afaf8054552580b699d95f8ff4af08c2d33d AS runtime

RUN rm -rf /usr/share/nginx/html/*
COPY web/deployment/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/web/dist/ /usr/share/nginx/html/

EXPOSE 80
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["wget", "-q", "-O", "/dev/null", "http://127.0.0.1/"]
