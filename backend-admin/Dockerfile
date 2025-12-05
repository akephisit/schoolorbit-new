FROM rust:1.83-slim-bookworm

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY backend-admin ./backend-admin
COPY shared ./shared

# Build the application
RUN cargo build --release --bin backend-admin

# Create non-root user
RUN useradd -m -u 1000 appuser && \
    chown -R appuser:appuser /app

USER appuser

EXPOSE 8080

CMD ["/app/target/release/backend-admin"]
