# Backend School

## Purpose

The tenant data-plane API serves school workflows. It resolves the school from each request, obtains tenant database information from backend-admin, and applies the active SQLx migrations to tenant databases.

## Stack

- Rust
- Axum and Tokio
- SQLx with PostgreSQL
- Serde, Utoipa/OpenAPI, tracing, WebSocket, and SSE support

## Local Setup

```bash
cd backend-school
cp .env.example .env
cargo build
```

Backend-admin must be reachable at `BACKEND_ADMIN_URL`. School database URLs are resolved through its internal API; backend-school does not use an admin database connection directly.

## Run

```bash
cargo run
```

By default the service binds to `0.0.0.0:8081`; `HOST` and `PORT` configure it.

## Check and Test

```bash
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check
```

Run focused module/service tests for changed behavior. Database-backed tests use `TEST_DATABASE_URL`, not a runtime tenant URL.

## Environment

Required groups include:

- `BACKEND_ADMIN_URL` and `INTERNAL_API_SECRET`;
- `SESSION_HMAC_KEY`, `BASE_DOMAIN`, `TRUSTED_PROXY_CIDRS`, `ENCRYPTION_KEY`, `BLIND_INDEX_KEY`, and `DEPLOY_KEY`;
- optional `SCHOOL_ALLOWED_DEV_ORIGINS` for exact local origins only; production leaves it empty;
- optional backend-admin timeout/retry tuning;
- R2-compatible storage credentials and upload limits;
- Web Push VAPID values;
- `HOST`, `PORT`, and `RUST_LOG`.

See `.env.example` for names. Keep the session HMAC, encryption, and blind-index keys stable after data exists. Root Compose owns `SCHOOL_ROLLBACK_JWT_SECRET` and maps it to the process-level `JWT_SECRET` for image compatibility; the current session runtime ignores that variable, and only a pre-session backend-school rollback image consumes it. Backend-admin keeps its separate `JWT_SECRET`. See [Operations](../docs/OPERATIONS.md) for the full cutover and rollback procedure.

## Health

- `GET /health` checks process liveness without tenant/database resolution.
- `GET /ready` checks the backend-admin control plane and is the deployment gate.

## Project Documentation

- [Development rules](../.rules) — feature, permission, API, migration, and security workflow
- [Testing](../docs/TESTING.md)
- [Operations](../docs/OPERATIONS.md)
