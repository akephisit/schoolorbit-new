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
health operations. `src/adapters` binds cross-domain lifecycle, result-lock, Staff directory and
Attendance/Behavior ports; process scheduling and cleanup remain application composition responsibilities.

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

Timetable requests current teacher subject-group affiliations through its `StaffDirectoryPort`.
The application adapter calls Staff's authoritative bounded query using the request's tenant pool;
Academic domain crates do not import Staff. Administration's lookup module owns
`OrganizationUnitLookupItem`, also used by Certificate transport. Files retains its direct Staff
dependency for authoritative profile and achievement access policies. These boundaries avoid
rebuilding unrelated academic services when personnel implementation changes.

Application composition supplies the same `PlacementRosterPort` implementation to Academic,
Students and Admission. Withdrawal and enrollment invoke it inside their existing transaction;
provider failures roll back all associated changes. Behavior requests post-commit runtime refresh
through its narrow application-bound port. The application adapter retains Attendance's existing
best-effort refresh behavior without a dependency between the two domains.

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
- optional Web Push VAPID values, required to enable Push; inbox/SSE remain available without them;
- `HOST`, `PORT`, and `RUST_LOG`.

See `.env.example` for names. Keep the session HMAC, encryption, and blind-index keys stable after data exists. School uses only opaque sessions and has no JWT runtime key or pre-session rollback image mapping. Backend-admin keeps its separate `JWT_SECRET`. See [Operations](../docs/OPERATIONS.md) for the full cutover and rollback procedure.

## Health

- `GET /health` checks process liveness without tenant/database resolution.
- `GET /ready` checks the backend-admin control plane and is the deployment gate.

## Project Documentation

- [Development rules](../.rules) — feature, permission, API, migration, and security workflow
- [Testing](../docs/TESTING.md)
- [Operations](../docs/OPERATIONS.md)

Staff leave and planned school-exit requests live in `school-staff::leave`, with policy profiles resolved from effective Staff career classification. The domain uses Workflow provider operations for assignments, notifications and source-bound approvals; private evidence and signer images use dedicated File Platform purposes. School-configured limits are not statutory entitlements. Runtime configuration is available through the leave settings API; no legal quotas or approval recipients are seeded.

Policy profiles are immutable and resolve by personnel category, effective date and version; disabling a later profile ends new eligibility while retaining submitted history. Calendar or working-day counting uses the profile's explicit weekdays and excluded dates. Half-day granularity counts a full day as two units and a single morning/afternoon as one; minute requests use planned same-day times. Unit and quota-year basis cannot change after a type's first policy, and requests crossing a quota year must be split. Pending reservations and approved usage share the annual allowance across policy versions. Usage reports aggregate requests starting in the selected interval; their recorded policy limit is not a current statutory or remaining-entitlement calculation.

Approved cancellation uses a separate audited approval process. Its route comes from the current school profile for the original frozen personnel category, including disabled profiles and archived types; a future-only profile falls back to the original route. The original approval, policy and usage remain until cancellation is approved. Missing approvers require a school administrator to repair the current route. This capability does not track actual departure/return times or send late alerts.


`school-notifications` accepts authoritative inboxes through typed transactional bulk creation.
Producers own recipient snapshots, content, idempotency and schedules; application composition
provides source guards. A small pending presentation reference is consumed before independent
SSE/Push attempts and is never retried after an uncertain result. Canonical hashed receipts retain
business/source identities and terminal outcomes; read state remains inbox-owned. Authenticated
Push devices use runtime VAPID configuration, session ownership, expiry and exact-version cleanup.
See [Operations](../docs/OPERATIONS.md#notification-inbox-first-cutover) for migrations 139→140 and
forward recovery, and [Testing](../docs/TESTING.md) for owner and preservation fixtures.

Automatic device refresh requires an existing exact binding version and cannot recreate a remotely
revoked device. After legacy cutover, users explicitly enable/sync Push once; inbox and SSE remain
available immediately. New enrollment and any required session/key reactivation use that explicit flow.
