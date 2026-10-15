# Backend School

## Purpose

The tenant data-plane API serves school workflows. It resolves the school from each request, obtains tenant database information from backend-admin, and applies the active SQLx migrations to tenant databases.

## Stack

- Rust
- Axum and Tokio
- SQLx with PostgreSQL
- Serde, Utoipa/OpenAPI, tracing, WebSocket, and SSE support

## Workspace

`Cargo.toml` is the application package and workspace root. Domain crates own models, services,
resource policies and concrete HTTP handlers. A typical owner has `src/models.rs`, `src/services.rs`
and `src/http/{state,routes,handlers}.rs`. Related smaller capabilities remain modules within that
owner. See the [workspace manifest](Cargo.toml) for the crate inventory.

`src/main.rs` starts the process and prepares dependencies. `src/http_state.rs` selects narrow,
cloneable HTTP state with `FromRef`; no domain accepts full `AppState` or imports the executable.
`src/app.rs` combines concrete route factories under global prefixes and applies the single shared
session boundary and system middleware. `src/system` owns internal provisioning, migration and
health operations. `src/adapters` binds cross-domain lifecycle, result-lock and Attendance/Behavior
ports; process scheduling and cleanup remain application composition responsibilities.

Existing Academic domain boundaries remain separate, with transport in `school-academic-http`.
Certificate transport stays in `school-certificates-http`. `school-auth` owns identity/session
operations and consent; `school-auth-http` owns canonical cookie/origin/tenant/session context and
Auth/Consent transport. `school-file-platform` owns storage/lifecycle and provider ports;
`school-files-http` composes consumer-specific file authorization without a dependency back from
storage to its consumers.

Parents lives within Students, Achievement within Staff, Work within Workflow, and menu, feature
toggles and route registration within Navigation. `school-administration` keeps settings, facility,
supply catalog and lookup as separate modules. Each owner returns its existing typed responses;
`src/api_contract.rs` composes the offline OpenAPI document and forwards the optional `api-docs`
feature without adding metadata dependencies to production.

Unit tests stay beside implementations. Deliberate application integration tests live in the
`#[cfg(test)]` namespace `src/integration_tests`; shared historical migration fixtures belong to
`school-test-db`, which is development-only. See [Testing](../docs/TESTING.md) for the owned
verification commands, database fixtures and compile measurement procedure.

## Environment

Required groups include:

- `BACKEND_ADMIN_URL` and `INTERNAL_API_SECRET`;
- `SESSION_HMAC_KEY`, `BASE_DOMAIN`, `TRUSTED_PROXY_CIDRS`, `ENCRYPTION_KEY`, `BLIND_INDEX_KEY`, and `DEPLOY_KEY`;
- optional `SCHOOL_ALLOWED_DEV_ORIGINS` for exact local origins only; production leaves it empty;
- optional backend-admin timeout/retry tuning;
- R2-compatible storage credentials and upload limits;
- Web Push VAPID values;
- `HOST`, `PORT`, and `RUST_LOG`.

See `.env.example` for names. Keep the session HMAC, encryption, and blind-index keys stable after data exists. School uses only opaque sessions and has no JWT runtime key or pre-session rollback image mapping. Backend-admin keeps its separate `JWT_SECRET`. See [Operations](../docs/OPERATIONS.md) for the full cutover and rollback procedure.

## Health

- `GET /health` checks process liveness without tenant/database resolution.
- `GET /ready` checks the backend-admin control plane and is the deployment gate.

## Project Documentation

- [Development rules](../.rules) — feature, permission, API, migration, and security workflow
- [Testing](../docs/TESTING.md)
- [Operations](../docs/OPERATIONS.md)
