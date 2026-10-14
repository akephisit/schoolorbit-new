# syntax=docker/dockerfile:1.10

FROM public.ecr.aws/docker/library/rust:1.98.1-slim-bookworm@sha256:ff521445a372125ed4f76e1453a1f8098f2d05332d1601d30db1c1f62757e730 AS base
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --version 0.1.78 --locked
WORKDIR /app

FROM base AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Keep the upstream cache-key format; retry only transient GHA storage failures.
FROM base AS sccache-tool
RUN apt-get update && apt-get install -y --no-install-recommends patch \
    && rm -rf /var/lib/apt/lists/*
ADD --checksum=sha256:6d445dc788b9f51d131d193d9ab466e76425b39f7df079f0dfddcac3222f7eb5 \
    https://github.com/mozilla/sccache/archive/c037e117c7625a2668633574028a6addf2a96a6e.tar.gz \
    /tmp/sccache-source.tar.gz
COPY scripts/sccache-gha-retry.patch /tmp/sccache-gha-retry.patch
RUN --mount=type=cache,id=school-sccache-tool-build,target=/tmp/sccache-target \
    mkdir /tmp/sccache-source \
    && tar -xzf /tmp/sccache-source.tar.gz -C /tmp/sccache-source --strip-components=1 \
    && cd /tmp/sccache-source \
    && patch --batch --fuzz=0 -p1 < /tmp/sccache-gha-retry.patch \
    && CARGO_TARGET_DIR=/tmp/sccache-target cargo build --release --locked --no-default-features --features=gha --bin sccache \
    && install -m 0755 /tmp/sccache-target/release/sccache /usr/local/bin/sccache \
    && printf '%s\n' '#!/bin/sh' 'exec /usr/local/bin/sccache "$@"' > /usr/local/bin/rustc-cache \
    && chmod 0755 /usr/local/bin/rustc-cache \
    && rm -rf /tmp/sccache-source /tmp/sccache-source.tar.gz /tmp/sccache-gha-retry.patch

FROM base AS builder
COPY --from=sccache-tool /usr/local/bin/sccache /usr/local/bin/rustc-cache /usr/local/bin/
COPY --from=planner /app/recipe.json recipe.json
COPY scripts/build_release.sh scripts/release_linker.sh scripts/
ARG SCHOOL_COMPILER_CACHE=off
# A changed workspace recipe misses the whole cargo-chef layer. Compiler caching
# still reuses unchanged dependencies instead of compiling them all from scratch.
RUN --mount=type=secret,id=sccache_gha_url,target=/run/secrets/sccache_gha_url \
    --mount=type=secret,id=sccache_gha_token,target=/run/secrets/sccache_gha_token \
    --mount=type=cache,id=backend-school-sccache,target=/var/cache/sccache \
    bash scripts/build_release.sh dependencies
COPY . .
# Disable local ThinLTO only for the final application crate. Dependencies keep
# their existing release settings and cargo-chef cache; opt-level is unchanged.
RUN --mount=type=secret,id=sccache_gha_url,target=/run/secrets/sccache_gha_url \
    --mount=type=secret,id=sccache_gha_token,target=/run/secrets/sccache_gha_token \
    --mount=type=cache,id=backend-school-sccache,target=/var/cache/sccache \
    bash scripts/build_release.sh

FROM scratch AS build-timings
COPY --from=builder /app/target/cargo-timings/cargo-timing.html /cargo-timing.html
COPY --from=builder /app/target/cargo-timings/sccache-stats.json /sccache-stats.json
COPY --from=builder /app/target/cargo-timings/build-metrics.json /build-metrics.json
COPY --from=builder /app/target/cargo-timings/link-nanoseconds.txt /link-nanoseconds.txt
COPY --from=builder /app/target/dependency-timings/build-metrics.json /dependency-build-metrics.json
COPY --from=builder /app/target/dependency-timings/sccache-stats.json /dependency-sccache-stats.json

FROM public.ecr.aws/docker/library/debian:bookworm-slim@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587 AS runtime
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

COPY --chown=1000:1000 --from=builder /app/target/release/backend-school /app/backend-school
COPY --chown=1000:1000 migrations ./migrations

USER appuser

EXPOSE 8081

CMD ["/app/backend-school"]
