FROM oven/bun:1.3.12@sha256:8956c7667fa17beb6e3c664115e66bdacfe502da5d99603626e74c197bdef160 AS bun
FROM node:24.15.0-bookworm@sha256:f22d6a1f082c02f292e86929b5b0442ac2e5eaf438a5dea9b1566601c3e05940 AS node
FROM jdxcode/mise:2026.9.13@sha256:1e74fb7d744027d453731d948dda0748cf19a3e7b4357b09787448088d1ff2ad AS mise
FROM rust:1.98.1-bookworm@sha256:93ce27a88655056a51dbdd8f5f2d7ddc071c7b0070fb288a37b5a285fc83971e AS rust

RUN rustup component add rustfmt clippy \
    && rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version 0.2.108 --locked \
    && rm -rf /usr/local/cargo/registry /usr/local/cargo/git

FROM python:3.14.7-bookworm@sha256:bfb689a7986adc6d5f16722e06c78e755efe4062e56276715fef450cbad09436 AS build

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

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        build-essential ca-certificates git libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY --from=bun /usr/local/bin/bun /opt/bun/bin/bun
COPY --from=node /usr/local/bin/node /opt/node/bin/node
COPY --from=mise /usr/local/bin/mise /usr/local/bin/mise
COPY --from=rust /usr/local/cargo /usr/local/cargo
COPY --from=rust /usr/local/rustup /usr/local/rustup

RUN mise link bun@1.3.12 /opt/bun \
    && mise link node@24.15.0 /opt/node \
    && mise link python@3.14.7 /usr/local

WORKDIR /app
COPY . .
ARG VITE_GLITCHTIP_WEB_DSN
ARG VITE_GLITCHTIP_SIM_DSN
ARG VITE_GLITCHTIP_RELEASE
ARG VITE_GLITCHTIP_REVISION
ARG VITE_GLITCHTIP_ENVIRONMENT=production
RUN mise trust /app/mise.toml \
    && mise run build

FROM scratch AS reporting-artifacts
COPY --from=build /app/web/reporting-artifacts/ /

FROM nginx:1.28.1-alpine@sha256:52e3ada4d978443601f286cc2f9e7b95c82aa3ad5a78ce9c6b94ce00258e68cc AS runtime

RUN rm -rf /usr/share/nginx/html/*
COPY web/deployment/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/web/dist/ /usr/share/nginx/html/

EXPOSE 80
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget -q -O /dev/null http://127.0.0.1/ || exit 1
