FROM rustlang/rust:nightly-slim

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy Cargo files
COPY backend-school/Cargo.toml ./

# Build dependencies (cache layer)
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source code and migrations
COPY backend-school/src ./src
COPY backend-school/migrations ./migrations

# Build the application
RUN cargo build --release --bin backend-school


# Create non-root user
RUN useradd -m -u 1000 appuser && \
    chown -R appuser:appuser /app

USER appuser

EXPOSE 8081

CMD ["/app/target/release/backend-school"]
