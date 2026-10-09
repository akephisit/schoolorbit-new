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

FROM base AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
ADD --checksum=sha256:67c4a96dd237c1f518f6b36083f270f9976d516f1e57fce891755ea782e50006 \
    https://github.com/mozilla/sccache/releases/download/v0.17.0/sccache-v0.17.0-x86_64-unknown-linux-musl.tar.gz \
    /tmp/sccache.tar.gz
RUN tar -xzf /tmp/sccache.tar.gz -C /tmp \
    && install -m 0755 \
        /tmp/sccache-v0.17.0-x86_64-unknown-linux-musl/sccache \
        /usr/local/bin/sccache \
    && rm -rf /tmp/sccache.tar.gz /tmp/sccache-v0.17.0-x86_64-unknown-linux-musl
COPY . .
# Disable local ThinLTO only for the final application crate. Dependencies keep
# their existing release settings and cargo-chef cache; opt-level is unchanged.
ARG SCHOOL_COMPILER_CACHE=off
RUN --mount=type=secret,id=sccache_gha_url,target=/run/secrets/sccache_gha_url \
    --mount=type=secret,id=sccache_gha_token,target=/run/secrets/sccache_gha_token \
    --mount=type=cache,id=backend-school-sccache,target=/var/cache/sccache \
    bash scripts/build_release.sh

FROM scratch AS build-timings
COPY --from=builder /app/target/cargo-timings/cargo-timing.html /cargo-timing.html
COPY --from=builder /app/target/cargo-timings/sccache-stats.json /sccache-stats.json

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
