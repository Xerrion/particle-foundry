FROM oven/bun:1.4.4@sha256:1d22548f177f4ff38e0c15baa3c2c46c6c2060ba2d1bc573bb4720b407548f1d AS bun
FROM node:26.13.0-bookworm@sha256:a546d3f8acd2af4382668a7e6acd3bc4418e6a27478373e3246f8162f5181509 AS node
FROM jdxcode/mise:2026.11.2@sha256:39cd8aad519a3dbc860eceb64dfd8cb5efe4cdbb8ad741a6867b544b2787b79c AS mise
FROM rust:1.99.1-bookworm@sha256:e6f2cff35d0f309acf8433b4f05878ed77092602c5065cc81801177da0bb3a9c AS rust

RUN rustup component add rustfmt clippy \
    && rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version 0.2.130 --locked \
    && rm -rf /usr/local/cargo/registry /usr/local/cargo/git

FROM python:3.15.2-bookworm@sha256:6f060cab421d9509c6c6a6a7762c651baf03c3f1d0145befe16b688762b7fe81 AS build

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

RUN mise link bun@1.4.4 /opt/bun \
    && mise link node@26.13.0 /opt/node \
    && mise link python@3.15.2 /usr/local

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

FROM nginx:1.31.8-alpine@sha256:b2f80f8f88146b0772fdcc7d92cea621aa8c2889fa8d07ed61d4884fcea53e09 AS runtime

RUN rm -rf /usr/share/nginx/html/* \
    && sed -i -e '/^user[[:space:]]/d' \
        -e 's|/run/nginx.pid|/tmp/nginx.pid|' /etc/nginx/nginx.conf \
    && chown -R 101:101 /var/cache/nginx
COPY web/deployment/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/web/dist/ /usr/share/nginx/html/

USER 101:101
EXPOSE 80
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["wget", "-q", "-O", "/dev/null", "http://127.0.0.1/"]
