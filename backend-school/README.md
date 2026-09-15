# Backend School

## Purpose

The tenant data-plane API serves school workflows. It resolves the school from each request, obtains tenant database information from backend-admin, and applies the active SQLx migrations to tenant databases.

## Stack

- Rust
- Axum and Tokio
- SQLx with PostgreSQL
- Serde, Utoipa/OpenAPI, tracing, WebSocket, and SSE support

## Workspace

`Cargo.toml` is both the application package and the non-virtual workspace root. The root
`backend-school` package owns startup, routing, full application state, OpenAPI composition,
schedulers, and cross-domain adapters. Internal crates under `crates/` expose narrow public APIs,
do not depend on the root package, and do not receive the complete `AppState`.

The current internal crates are:

- `school-permissions`, the generated backend permission registry owner;
- `school-migrations`, the canonical tenant migration and permission-reconciliation runner;
- `school-errors`, the transport-independent application error taxonomy;
- `school-http`, the Axum error conversion and standard API response envelopes;
- `school-authorization`, actor permission evaluation, effective-permission loading, and the
  permission-only cache plus exact organization-unit authorization queries;
- `school-auth`, session credentials, policy, persistence, throttling, audit events, identity
  cache, user-profile domain operations, and the root-independent authentication runtime;
- `school-academic-core`, academic-year, term, curriculum, catalog, homeroom, student-year,
  promotion, and foundational academic persistence;
- `school-academic-delivery`, teaching offerings, enrollments, assignments, workload, and delivery
  workspaces;
- `school-academic-timetable`, timetable models, policies, templates, versions, conflicts, and
  term-preparation operations;
- `school-academic-assessment`, assessment, gradebook, learner-evaluation, and result-lock ports;
- `school-academic-results`, activity/course/annual result models, policies, revisions, locking,
  correction, and reporting operations;
- `school-academic-lifecycle`, academic readiness and lifecycle orchestration through narrow
  external-provider ports;
- `school-workflow`, workflow-window and work-item models, policy, persistence, and lifecycle;
- `school-question-bank`, question models, rich content, resource access policy, persistence, and
  file relationships;
- `school-admission`, admission rounds, applications, examinations, scoring, selection, and
  enrollment outcomes;
- `school-supervision`, supervision templates, cycles, observations, evaluations, reviews, and
  reports;
- `school-students`, student/parent models, encrypted identity persistence, and student queries;
- `school-staff`, staff identity, roles, permissions, organization units, memberships,
  delegations, and audit writes;
- `school-calendar`, calendar models, validation, categories, tags, event persistence, and
  visibility;
- `school-tenancy`, backend-admin tenant discovery, immutable tenant metadata, lazy migration
  coordination, and the tenant pool cache;
- `school-file-platform`, provider-neutral file inspection, purpose policy, persistence, storage,
  malware scanning, lifecycle operations, and reconciliation;
- `school-crypto`, the canonical AES-256-GCM and keyed blind-index implementation;
- `school-fonts`, school-font models, validation, persistence, typed upload relationships, and
  reference-aware deletion;
- `school-certificates`, certificate models, policies, layouts, issuance, rendering, verification,
  rate limiting, and purge lifecycle; and
- `school-test-db`, the schema-isolated PostgreSQL test harness, available to the root package only
  as a development dependency.

The root package retains Axum handlers, cookie/CSRF and tenant/request adapters, OpenAPI
composition, File Platform deletion orchestration, Calendar notification delivery and all-tenant
reminder scheduling, academic external-provider and consequence adapters, cache/realtime side
effects, and deliberate cross-domain integration tests. Parents remains here because it composes
Student, Calendar, and Exam views; the smaller school, facility, consent, achievement, lookup,
menu, notification, and system modules also remain application-owned until they meet the crate
admission rule. Root code maps application concerns to narrow crate APIs and does not duplicate
their domain SQL or business rules.

`school-auth` owns the session identity cache and composes it with the authorization-owned
permission cache. Permission-authority changes invalidate session identity first and permission
data second, then the application may publish realtime invalidation events.

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

Plain `cargo run` continues to select the root application. By default the service binds to
`0.0.0.0:8081`; `HOST` and `PORT` configure it.

## Check and Test

```bash
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check --workspace --all-targets
cargo test -p school-permissions
cargo test -p school-migrations
cargo test -p school-errors
cargo test -p school-http
cargo test -p school-authorization
cargo test -p school-auth -- --test-threads=8
cargo test -p school-academic-core -- --test-threads=8
cargo test -p school-academic-delivery -- --test-threads=8
cargo test -p school-academic-timetable -- --test-threads=8
cargo test -p school-academic-assessment -- --test-threads=8
cargo test -p school-academic-results -- --test-threads=8
cargo test -p school-academic-lifecycle -- --test-threads=8
cargo test -p school-workflow
cargo test -p school-question-bank -- --test-threads=8
cargo test -p school-admission -- --test-threads=8
cargo test -p school-supervision -- --test-threads=8
cargo test -p school-students -- --test-threads=8
cargo test -p school-staff -- --test-threads=8
cargo test -p school-calendar -- --test-threads=8
cargo test -p school-tenancy
cargo test -p school-file-platform
cargo test -p school-crypto
cargo test -p school-fonts -- --test-threads=8
cargo test -p school-certificates -- --test-threads=8
cargo test -p school-test-db
```

Run focused module/service tests for changed behavior. From the repository root, run database-backed binary tests with:

```bash
./scripts/test_backend_school.sh
```

See [Testing](../docs/TESTING.md) for focused filters and the explicit Neon compatibility gate.

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
