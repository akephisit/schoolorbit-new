# syntax=docker/dockerfile:1.10

FROM rust:1.98.1-slim-bookworm AS base
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --version 0.1.78 --locked
ADD --checksum=sha256:67c4a96dd237c1f518f6b36083f270f9976d516f1e57fce891755ea782e50006 \
    https://github.com/mozilla/sccache/releases/download/v0.17.0/sccache-v0.17.0-x86_64-unknown-linux-musl.tar.gz \
    /tmp/sccache.tar.gz
RUN tar -xzf /tmp/sccache.tar.gz -C /tmp \
    && install -m 0755 \
        /tmp/sccache-v0.17.0-x86_64-unknown-linux-musl/sccache \
        /usr/local/bin/sccache \
    && rm -rf /tmp/sccache.tar.gz /tmp/sccache-v0.17.0-x86_64-unknown-linux-musl
WORKDIR /app

FROM base AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM base AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
ARG SCHOOL_COMPILER_CACHE=off
RUN --mount=type=secret,id=sccache_gha_url,target=/run/secrets/sccache_gha_url \
    --mount=type=secret,id=sccache_gha_token,target=/run/secrets/sccache_gha_token \
    set -eu; \
    compile_started="$(date +%s%N)"; cache_enabled=false; \
    if [ "$SCHOOL_COMPILER_CACHE" = gha ] && [ -s /run/secrets/sccache_gha_url ] && [ -s /run/secrets/sccache_gha_token ]; then \
        cache_enabled=true; \
        ACTIONS_RESULTS_URL="$(cat /run/secrets/sccache_gha_url)" \
        ACTIONS_RUNTIME_TOKEN="$(cat /run/secrets/sccache_gha_token)" \
        SCCACHE_GHA_ENABLED=on \
        ACTIONS_CACHE_SERVICE_V2=true \
        SCCACHE_GHA_VERSION=schoolorbit-backend-admin \
        SCCACHE_IGNORE_SERVER_IO_ERROR=1 \
        RUSTC_WRAPPER=/usr/local/bin/sccache \
        cargo build --release --locked --bin backend-admin --bin create_admin --timings; \
        /usr/local/bin/sccache --show-stats || true; \
    else \
        cargo build --release --locked --bin backend-admin --bin create_admin --timings; \
    fi; \
    compile_nanoseconds=$(($(date +%s%N) - compile_started)); \
    mkdir -p target/cargo-timings; \
    printf '{"schemaVersion":1,"compileNanoseconds":%s,"cacheEnabled":%s}\n' "$compile_nanoseconds" "$cache_enabled" >target/cargo-timings/build-metrics.json; \
    printf '{}\n' >target/cargo-timings/sccache-stats.json; \
    if [ "$cache_enabled" = true ]; then \
        /usr/local/bin/sccache --show-stats --stats-format=json >target/cargo-timings/sccache-stats.json || printf '{}\n' >target/cargo-timings/sccache-stats.json; \
    fi

FROM scratch AS build-timings
COPY --from=builder /app/target/cargo-timings/cargo-timing.html /cargo-timing.html
COPY --from=builder /app/target/cargo-timings/sccache-stats.json /sccache-stats.json
COPY --from=builder /app/target/cargo-timings/build-metrics.json /build-metrics.json

FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends \
    openssl \
    ca-certificates \
    curl \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 1000 appuser \
    && install -d -o 1000 -g 1000 /app

COPY --chown=1000:1000 --from=builder /app/target/release/backend-admin /app/backend-admin
COPY --chown=1000:1000 --from=builder /app/target/release/create_admin /app/create_admin
COPY --chown=1000:1000 migrations ./migrations

USER appuser

EXPOSE 8080

CMD ["/app/backend-admin"]
