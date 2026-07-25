# Backend Admin

## Purpose

The control-plane API manages school records, tenant database provisioning metadata, and deployment coordination. Backend-school uses its authenticated internal endpoints to resolve tenant database information.

## Stack

- Rust
- Axum and Tokio
- SQLx with PostgreSQL
- Serde and tracing

## Local Setup

```bash
cd backend-admin
cp .env.example .env
cargo build
```

Set a local admin `DATABASE_URL` and replace every example secret before using a shared environment.

## Run

```bash
cargo run
```

The service binds to `0.0.0.0:8080`.

## Check and Test

```bash
cargo fmt --all -- --check
cargo test
cargo check
```

Run focused tests first for changed handlers, clients, or services.

## Environment

The main groups are:

- `DATABASE_URL`, `JWT_SECRET`, and `INTERNAL_API_SECRET`;
- `BACKEND_SCHOOL_URL` for internal coordination;
- Neon credentials for tenant database provisioning;
- Cloudflare and GitHub credentials for DNS/deployment operations;
- `RUST_LOG` for structured log filtering.

See `.env.example` for names and [Operations](../docs/OPERATIONS.md) for secret-handling and rotation rules.

## Health

- `GET /health` checks process liveness without querying PostgreSQL.
- `GET /ready` checks admin database readiness and is the deployment gate.

## Project Documentation

- [Development rules](../.rules)
- [Testing](../docs/TESTING.md)
- [Operations](../docs/OPERATIONS.md)
