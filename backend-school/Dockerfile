# Base stage with cargo-chef
FROM rustlang/rust:nightly-bookworm-slim AS base
# Install OpenSSL and pkg-config for all build stages
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef
WORKDIR /app

# Planner stage
FROM base AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Builder stage
FROM base AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin backend-school

# Runtime stage
FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends \
    openssl \
    ca-certificates \
    curl \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*

# Copy binary and migrations
COPY --from=builder /app/target/release/backend-school /app/backend-school
COPY migrations ./migrations

# Create non-root user
RUN useradd -m -u 1000 appuser && \
    chown -R appuser:appuser /app

USER appuser

EXPOSE 8081

CMD ["/app/backend-school"]
