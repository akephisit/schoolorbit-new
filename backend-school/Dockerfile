# syntax=docker/dockerfile:1.10

FROM rust:1.98.0-slim-bookworm AS base
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
COPY . .
# This binary-only build cannot reuse sccache outputs (crate-type); dependency
# reuse remains owned by the cargo-chef layer above.
# Disable local ThinLTO only for the final application crate. Dependencies keep
# their existing release settings and cargo-chef cache; opt-level is unchanged.
RUN cargo rustc --release --locked --bin backend-school --timings -- -C lto=off

FROM scratch AS build-timings
COPY --from=builder /app/target/cargo-timings/cargo-timing.html /cargo-timing.html

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

COPY --chown=1000:1000 --from=builder /app/target/release/backend-school /app/backend-school
COPY --chown=1000:1000 migrations ./migrations

USER appuser

EXPOSE 8081

CMD ["/app/backend-school"]
