# syntax=docker/dockerfile:1.10

ARG SCHOOL_CARGO_BUILD_STAGE=fresh-builder

FROM docker.io/library/rust:1.100.0-slim-bookworm@sha256:7c7d4eb4d05b7781187a327809466cfddf29753f6defdc3d19a0e291951b7a37 AS base
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
COPY scripts/sccache-startup.json /etc/sccache-school.json
ENV SCCACHE_CONF=/etc/sccache-school.json

FROM base AS compiler-tools
RUN apt-get update && apt-get install -y --no-install-recommends jq python3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=sccache-tool /usr/local/bin/sccache /usr/local/bin/rustc-cache /usr/local/bin/
COPY --from=sccache-tool /etc/sccache-school.json /etc/sccache-school.json
ENV SCCACHE_CONF=/etc/sccache-school.json

FROM compiler-tools AS dependencies
COPY --from=planner /app/recipe.json recipe.json
COPY scripts/build_release.sh scripts/release_linker.sh scripts/
ARG SCHOOL_COMPILER_CACHE=off
# A changed workspace recipe misses the whole cargo-chef layer. Compiler caching
# still reuses unchanged dependencies instead of compiling them all from scratch.
RUN --mount=type=secret,id=sccache_gha_url,target=/run/secrets/sccache_gha_url \
    --mount=type=secret,id=sccache_gha_token,target=/run/secrets/sccache_gha_token \
    --mount=type=cache,id=backend-school-sccache,target=/var/cache/sccache \
    bash scripts/build_release.sh dependencies

FROM dependencies AS fresh-builder
COPY . .
# Disable local ThinLTO only for the final application crate. Dependencies keep
# their existing release settings and cargo-chef cache; opt-level is unchanged.
RUN --mount=type=secret,id=sccache_gha_url,target=/run/secrets/sccache_gha_url \
    --mount=type=secret,id=sccache_gha_token,target=/run/secrets/sccache_gha_token \
    --mount=type=cache,id=backend-school-sccache,target=/var/cache/sccache \
    bash scripts/build_release.sh

# A cache carrier is data-only. Named image contexts are copied without inheriting
# image configuration/ONBUILD. Validation performs no Cargo or compiler-tool build.
FROM docker.io/library/debian:bookworm-slim@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587 AS snapshot-restored
RUN apt-get update && apt-get install -y --no-install-recommends python3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=cargo-snapshot-data /snapshot.tar /cache/snapshot.tar
COPY --from=cargo-snapshot-data /snapshot.json /cache/snapshot.json
COPY . /incoming/
ARG SCHOOL_CARGO_CONFIG_HASH
ARG SCHOOL_CARGO_SOURCE_MANIFEST_HASH
RUN python3 /incoming/scripts/cargo_snapshot.py preflight /app --incoming /incoming \
    --archive /cache/snapshot.tar --metadata /cache/snapshot.json \
    --configuration-hash "$SCHOOL_CARGO_CONFIG_HASH" \
    --source-manifest-hash "$SCHOOL_CARGO_SOURCE_MANIFEST_HASH"

# Building compiler-tools is strict. A public identity mismatch is returned as
# data before application Cargo runs; a compiler-tool build failure fails the job.
FROM compiler-tools AS snapshot-native-check
COPY scripts/cargo_snapshot.py /usr/local/lib/cargo_snapshot.py
COPY --from=snapshot-restored /prevalidated.json /prevalidated.json
RUN python3 /usr/local/lib/cargo_snapshot.py native-gate /app \
    --metadata /prevalidated.json --destination /native-gate.json
FROM scratch AS cargo-native-gate
COPY --from=snapshot-native-check /native-gate.json /native-gate.json

FROM compiler-tools AS snapshot-materialized
COPY scripts/cargo_snapshot.py /usr/local/lib/cargo_snapshot.py
COPY --from=snapshot-restored /app/ /app/
COPY --from=snapshot-restored /prevalidated.json /prevalidated.json
ARG SCHOOL_CARGO_CONFIG_HASH
RUN python3 /usr/local/lib/cargo_snapshot.py materialize-gate /app --metadata /prevalidated.json \
    --configuration-hash "$SCHOOL_CARGO_CONFIG_HASH" --destination /materialization-gate.json
FROM scratch AS cargo-materialization-gate
COPY --from=snapshot-materialized /materialization-gate.json /materialization-gate.json

FROM snapshot-materialized AS snapshot-builder
RUN python3 -c 'import json; assert json.load(open("/materialization-gate.json"))["compatible"] is True'
ARG SCHOOL_COMPILER_CACHE=off
RUN --mount=type=secret,id=sccache_gha_url,target=/run/secrets/sccache_gha_url \
    --mount=type=secret,id=sccache_gha_token,target=/run/secrets/sccache_gha_token \
    --mount=type=cache,id=backend-school-sccache,target=/var/cache/sccache \
    bash scripts/build_release.sh

FROM ${SCHOOL_CARGO_BUILD_STAGE} AS builder

# A controlled TAR writes every target alias as regular bytes. Metadata and all
# headers are validated before extraction into an empty private staging directory.
FROM builder AS cargo-snapshot-seal
ARG SCHOOL_CARGO_CONFIG_HASH
RUN python3 scripts/cargo_snapshot.py archive /app --destination /snapshot \
    --configuration-hash "$SCHOOL_CARGO_CONFIG_HASH"
FROM scratch AS cargo-snapshot
COPY --from=cargo-snapshot-seal /snapshot/snapshot.tar /snapshot.tar
COPY --from=cargo-snapshot-seal /snapshot/snapshot.json /snapshot.json
FROM scratch AS cargo-snapshot-metadata
COPY --from=cargo-snapshot-seal /snapshot/snapshot.json /snapshot.json
FROM scratch AS cargo-restore-evidence
COPY --from=snapshot-builder /app/target/cargo-timings/snapshot-restore.json /snapshot-restore.json

# Explicit diagnostic target only; the release/runtime graph never invokes it.
FROM builder AS cargo-freshness-check
ARG SCHOOL_FRESHNESS_PROBE
RUN test -n "$SCHOOL_FRESHNESS_PROBE" && bash scripts/build_release.sh
FROM scratch AS cargo-freshness-evidence
COPY --from=cargo-freshness-check /app/target/cargo-timings/cargo-artifacts.jsonl /cargo-artifacts.jsonl

FROM scratch AS build-timings
COPY --from=builder /app/target/cargo-timings/cargo-timing.html /cargo-timing.html
COPY --from=builder /app/target/cargo-timings/sccache-stats.json /sccache-stats.json
COPY --from=builder /app/target/cargo-timings/build-metrics.json /build-metrics.json
COPY --from=builder /app/target/cargo-timings/cargo-artifacts.jsonl /cargo-artifacts.jsonl
COPY --from=builder /app/target/cargo-timings/snapshot-restore.json /snapshot-restore.json
COPY --from=builder /app/target/cargo-timings/link-nanoseconds.txt /link-nanoseconds.txt
COPY --from=builder /app/target/dependency-timings/build-metrics.json /dependency-build-metrics.json
COPY --from=builder /app/target/dependency-timings/sccache-stats.json /dependency-sccache-stats.json

FROM docker.io/library/debian:bookworm-slim@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587 AS runtime
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
