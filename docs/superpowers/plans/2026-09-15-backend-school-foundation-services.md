# Backend School Foundation Services Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete checkpoint 2 by extracting the backend error taxonomy, HTTP envelopes, authorization runtime, and database test harness into four cohesive workspace crates without changing any external behavior.

**Architecture:** `school-errors` owns transport-independent failures and may use the protocol-neutral `http::StatusCode`, while `school-http` owns Axum response conversion and JSON envelopes. `school-authorization` owns actor permission evaluation, effective-permission loading, and a permission-only cache; application/auth composition separately owns the session cache and invalidates both synchronously. `school-test-db` is a root dev-dependency only and owns schema-isolated PostgreSQL test setup plus canonical migrations.

**Tech Stack:** Rust 2021, Cargo workspace resolver 2, Axum 0.8, SQLx 0.8/PostgreSQL, Tokio, DashMap, serde/utoipa, Node static architecture tests, rootless Podman.

**Spec:** `docs/superpowers/specs/2026-09-14-backend-school-crate-architecture-design.md`

## Global Constraints

- Preserve one `backend-school` executable, one container, and the existing Docker build context.
- Do not edit any file under `backend-school/migrations/` or `backend-school/migrations_legacy/`.
- Preserve permission codes, OpenAPI, generated frontend contracts, statuses, headers, public messages, cache semantics, and bounded logging.
- Never store or log plaintext national IDs, credentials, tokens, cookies, database URLs, encryption keys, object keys, signed grants, raw request bodies, or provider payloads.
- Internal crates must use workspace dependencies/lints, remain acyclic, never depend on `backend-school`, and never accept `AppState`.
- `school-test-db` must be a dev-dependency only and must not enter the runtime image dependency graph.
- When permission authority changes, invalidate permission and session identity caches synchronously before events or fallible follow-up work.
- No compatibility re-export, duplicate owner, production `#[path]`, or catch-all shared crate may remain.
- Compare representative compile medians using Rust/Cargo 1.98.1 and the checkpoint-1 target directory; block an unexplained regression above 10 percent.

---

## Task 1: Extract `school-errors` and `school-http`

**Files:**

- Create: `backend-school/crates/school-errors/Cargo.toml`
- Create: `backend-school/crates/school-errors/src/lib.rs`
- Create: `backend-school/crates/school-http/Cargo.toml`
- Create: `backend-school/crates/school-http/src/lib.rs`
- Modify: `backend-school/Cargo.toml`
- Modify: every root Rust consumer of `crate::error::AppError`
- Modify: every root Rust consumer of `crate::api_response`
- Modify: handler and Axum-middleware result signatures that currently expose `AppError`
- Delete: `backend-school/src/error.rs`
- Delete: `backend-school/src/api_response.rs`
- Test: package tests in both new `src/lib.rs` files
- Test: `backend-school/src/api_contract.rs`

**Interfaces:**

- Produces: `school_errors::AppError`, with the existing variants except the certificate-named variant, `status_code()`, `public_message()`, and `retry_after_seconds()`.
- Produces: `school_errors::AppError::ResourceLocked { request_id: Option<Uuid> }` as the feature-neutral structured conflict used until the certificate crate owns its domain error in checkpoint 5.
- Produces: `school_http::{ApiResponse, EmptyData, IdData, UuidIdData, ApiErrorResponse, ApiErrorResponseWithData, ApiErrorResponseWithOptionalData, HttpError}`.
- Produces: `From<school_errors::AppError> for school_http::HttpError` and `IntoResponse for HttpError`.

- [x] **Step 1: Add failing package and architecture tests for the ownership boundary**

Add tests asserting that `school_errors::AppError` retains the exact status/message behavior, that `school_http::HttpError` emits the standard envelope and bounded `Retry-After`, and that a resource-lock conflict serializes `data.code = "resource_locked"` plus the allowlisted request ID. Extend `static_architecture.rs` to require both members, reject Axum in `school-errors`, and reject the old root owner files.

- [x] **Step 2: Run the new tests and verify RED**

Run from `backend-school`:

```bash
cargo test -p school-errors
cargo test -p school-http
cargo test --test static_architecture backend_school_workspace -- --exact
```

Expected: FAIL because the packages and canonical owners do not exist yet.

- [x] **Step 3: Create manifests and move the error taxonomy**

Add both workspace members and workspace path dependencies. `school-errors` uses only `http`, `sqlx`, `thiserror`, and `uuid`; keep all existing database-code mappings, Thai/English public strings, and the retry clamp of 1 through 30 seconds. Rename `CertificateResourceLocked` to feature-neutral `ResourceLocked` and do not import certificate models.

- [x] **Step 4: Move envelopes and Axum conversion to `school-http`**

Move the response structs without changing serde or utoipa annotations. Implement `HttpError(AppError)`, `From<AppError>`, delegated inspection methods, safe logging, status mapping, the retry header, and the structured resource-lock JSON using a private serializable `{ code, request_id }` transport object.

- [x] **Step 5: Cut consumers directly to canonical imports**

Replace `crate::error::AppError` with `school_errors::AppError` in services, policies, models, tests, and non-Axum helpers. Replace root response imports with `school_http`. Change route handlers and middleware to return `HttpError`; keep local validation in `AppError`, convert explicit `Err(...)` with `.into()`, and let `?` use `From<AppError>`. Public-certificate redaction wraps `HttpError` after matching the underlying domain error. Delete both root owner files and their module declarations.

- [x] **Step 6: Verify GREEN and unchanged API artifacts**

```bash
cargo fmt --all -- --check
cargo test -p school-errors
cargo test -p school-http
cargo check --workspace --all-targets
cargo test api_contract::tests --bin backend-school
cd ../frontend-school
npm run check:api-contracts
npm run test:api-contracts
```

Expected: PASS and no diff in `contracts/openapi/school-api.json` or generated frontend API types.

- [x] **Step 7: Commit the canonical error/HTTP owners**

```bash
git add backend-school/Cargo.toml backend-school/Cargo.lock backend-school/crates/school-errors \
  backend-school/crates/school-http backend-school/src frontend-school/tests/static
git commit -m "refactor: extract school error and http contracts"
```

## Task 2: Extract Permission Authorization

**Files:**

- Create: `backend-school/crates/school-authorization/Cargo.toml`
- Create: `backend-school/crates/school-authorization/src/lib.rs`
- Create: `backend-school/crates/school-authorization/src/cache.rs`
- Create: `backend-school/crates/school-authorization/src/permissions.rs`
- Modify: `backend-school/Cargo.toml`
- Modify: all `ActorContext`, permission matcher, effective-permission loader, and `PermissionCache` consumers
- Move DB-backed academic cutover assertion to: `backend-school/src/db/authorization_tests.rs`
- Delete: `backend-school/src/db/permission_cache.rs`
- Delete: `backend-school/src/middleware/permission.rs`
- Modify: `backend-school/src/db.rs`
- Modify: `backend-school/src/middleware.rs`
- Test: package unit tests and root DB-backed authorization test

**Interfaces:**

- Produces: `school_authorization::{ActorContext, PermissionCache, PermissionCacheRevision, TenantUserKey}`.
- Produces: `permission_matches`, `module_permission_matches`, `get_cached_user_permissions`, `load_actor_context`, and `load_actor_context_for_session` with their current arguments and outcomes.
- Consumes: `school_errors::AppError`, `school_permissions::registry::codes`, `PgPool`.

- [x] **Step 1: Add failing architecture assertions and package characterization tests**

Require the workspace member and dependency direction. Copy the pure matcher, actor requirement, tenant isolation, TTL, revision, and stale-fill tests into the planned package modules. Add a root DB test that calls the public loader after migrations 40/44 and verifies active replacement permissions while removed legacy permissions remain absent.

- [x] **Step 2: Run tests and verify RED**

```bash
cargo test -p school-authorization
cargo test --test static_architecture backend_school_workspace -- --exact
```

Expected: FAIL before the package is created.

- [x] **Step 3: Move the permission-only cache**

Move cache entry/revision logic verbatim but remove `SessionCache` from `PermissionCache`. `new()` constructs only permission state. `invalidate_user` and `invalidate_tenant` advance revisions and remove permission entries only; they must not call auth code.

- [x] **Step 4: Move actor and effective-permission logic**

Move `ActorContext`, requirement helpers, matcher functions, the effective SQL query, and loaders. Preserve the active-user fail-closed clause, organization position checks, delegation expiry checks, ordering, and stale-fill rejection. Do not accept `AppState` or import auth.

- [x] **Step 5: Cut all root consumers directly to `school_authorization`**

Update policies, services, handlers, session loading, request context, WebSockets, and tests. Delete the old modules and register `authorization_tests` under root DB tests. No `crate::middleware::permission` or `crate::db::permission_cache` reference may remain.

- [x] **Step 6: Verify focused and workspace behavior**

```bash
cargo fmt --all -- --check
cargo test -p school-authorization
cargo check --workspace --all-targets
cd ..
./scripts/test_backend_school.sh effective_permissions_exclude_removed_legacy_cutover_permissions -- --exact --nocapture --test-threads=1
```

Expected: PASS with the same effective permission set and no database/schema changes.

- [x] **Step 7: Commit authorization ownership**

```bash
git add backend-school/Cargo.toml backend-school/Cargo.lock \
  backend-school/crates/school-authorization backend-school/src
git commit -m "refactor: extract school authorization"
```

## Task 3: Compose Permission and Session Cache Invalidation

**Files:**

- Modify: `backend-school/src/main.rs`
- Modify: `backend-school/src/modules/auth/runtime.rs`
- Modify: `backend-school/src/modules/auth/session_service.rs`
- Modify: every permission mutation/invalidation caller under `backend-school/src/`
- Modify: session/cache tests under `backend-school/src/modules/auth/`
- Test: `backend-school/src/modules/auth/session_cache_tests.rs`
- Test: `backend-school/src/modules/auth/session_service_tests.rs`
- Test: `backend-school/src/modules/staff/services/status_tests.rs`

**Interfaces:**

- Produces: `AuthRuntime::invalidate_permission_user(&self, tenant: &str, user_id: Uuid)` and `AuthRuntime::invalidate_permission_tenant(&self, tenant: &str)`.
- `AuthRuntime` owns `Arc<PermissionCache>` and `Arc<SessionCache>` as sibling fields.
- `SessionServiceContext` owns both sibling cache arcs and authenticated sessions retain only the session identity cache.

- [x] **Step 1: Write failing cache-composition tests**

Add tests proving permission invalidation removes both permission and session identities, tenant/user boundaries remain isolated, in-flight permission fills stay stale, and the identity invalidation occurs before an event send or any later fallible operation. Add a structural assertion that `school-authorization` source contains no `SessionCache` or auth path.

- [x] **Step 2: Run focused tests and verify RED**

```bash
cargo test --bin backend-school modules::auth::session_cache_tests
cargo test --bin backend-school modules::staff::services::status_tests
```

Expected: FAIL because the application runtime does not yet compose sibling caches.

- [x] **Step 3: Add the sibling session cache to auth composition**

Construct one `Arc<SessionCache>` in `main`, pass it to `AuthRuntime`, and pass both caches into `SessionServiceContext`. Replace every `permission_cache.session_cache` access with the explicit session-cache field.

- [x] **Step 4: Centralize ordered dual invalidation at the application boundary**

The runtime methods invalidate the session identity cache first and permission cache second, synchronously, then callers may emit permission events. `AppState` exposes equivalent delegation methods for non-auth handlers. Replace direct invalidation in tenant resolution, migrations, students, staff roles/members/delegations/permissions/user roles, and status tests.

- [x] **Step 5: Verify session, permission, and realtime semantics**

```bash
cargo test --bin backend-school modules::auth::session_cache_tests
cargo test --bin backend-school modules::auth::session_service_tests
cargo test --bin backend-school modules::auth::session_http_tests
cargo test --bin backend-school modules::academic::websockets::security_tests
cargo test --bin backend-school modules::staff::services::status_tests
cargo test -p school-authorization
```

Expected: PASS with no stale identity or stale permission cache window.

- [x] **Step 6: Commit cache composition**

```bash
git add backend-school/src backend-school/tests/static_architecture.rs
git commit -m "refactor: compose authorization and session caches"
```

## Task 4: Extract Dev-Only `school-test-db`

**Files:**

- Create: `backend-school/crates/school-test-db/Cargo.toml`
- Create: `backend-school/crates/school-test-db/src/lib.rs`
- Modify: `backend-school/Cargo.toml`
- Modify: all test-only consumers of `crate::test_helpers`
- Delete: `backend-school/src/test_helpers.rs`
- Modify: `backend-school/src/main.rs`
- Test: package unit tests and database-backed root tests

**Interfaces:**

- Produces: `school_test_db::{create_test_pool, create_named_test_pool, create_named_test_pool_with_max_connections, run_test_migrations, create_test_user}`.
- Consumes: `school_migrations::run_tenant_migrations` and `TEST_DATABASE_URL` only in test execution.
- The root manifest lists `school-test-db` only under `[dev-dependencies]`; no runtime crate depends on it.

- [x] **Step 1: Add failing ownership and dev-dependency tests**

Extend architecture tests to require the member, require root dev-only use, reject it from all normal dependencies, and reject the old root helper. Copy deterministic schema-name, direct-Neon-authority, lazy-pool, and zero-connection tests into the package.

- [x] **Step 2: Run tests and verify RED**

```bash
cargo test -p school-test-db
cargo test --test static_architecture backend_school_workspace -- --exact
```

Expected: FAIL before extraction.

- [x] **Step 3: Move the database harness**

Move schema locks, name normalization/digesting, direct URL handling, schema reset, search-path pool creation, migration serialization, and test-user creation. Preserve current panic messages and connection counts. Depend only on `bcrypt`, `dotenvy`, `school-migrations`, `sha2`, `sqlx`, `tokio`, and `uuid`.

- [x] **Step 4: Cut every test directly to the dev crate**

Replace `crate::test_helpers` imports and paths with `school_test_db`; remove the root module declaration and owner file. Keep feature fixtures and business assertions with their existing feature modules.

- [x] **Step 5: Verify package and database-backed usage**

```bash
cargo fmt --all -- --check
cargo test -p school-test-db
cargo check --workspace --all-targets
cd ..
./scripts/test_backend_school.sh test_helpers::tests -- --nocapture --test-threads=1
```

The final filtered root command is expected to report no old root helper tests; package tests must own them, and at least one root DB-backed feature test must pass using the dev crate.

- [x] **Step 6: Commit the test harness owner**

```bash
git add backend-school/Cargo.toml backend-school/Cargo.lock \
  backend-school/crates/school-test-db backend-school/src
git commit -m "refactor: extract school test database harness"
```

## Task 5: Guard, Measure, Document, and Verify Checkpoint 2

**Files:**

- Modify: `backend-school/tests/static_architecture.rs`
- Modify: `backend-school/README.md`
- Modify: `docs/TESTING.md`
- Modify: `docs/superpowers/specs/2026-09-14-backend-school-crate-architecture-design.md`
- Verify only: contract artifacts and migrations

**Interfaces:**

- Consumes: all four canonical checkpoint-2 crates.
- Produces: guarded dependency direction, package commands, compile evidence, and a releasable exact tree.

- [x] **Step 1: Add durable architecture guards**

Require exact members and allowed internal edges: `school-http -> school-errors`, `school-authorization -> school-errors + school-permissions`, and dev-only `school-test-db -> school-migrations`. Reject root owner imports/files, Axum from `school-errors`, auth/session references from authorization, runtime `school-test-db`, root package dependencies, full `AppState`, and production `#[path]`.

- [x] **Step 2: Measure representative invalidation three times**

Using `/home/ake/Dev/.schoolorbit-backend-school-workspace-target-20260915`, add and remove numbered comment probes with `apply_patch`, then run `cargo check --workspace --all-targets -vv` for an error-only edit and an authorization-only edit. Record medians outside the repository and confirm Cargo marks the changed package plus actual dependents dirty while unrelated sibling packages remain fresh. Compare the local-package median with checkpoint 1's 59.445-second baseline and block unexplained regression above 10 percent.

- [x] **Step 3: Update canonical documentation**

Document the four package owners, focused commands, dev-only test database rule, and sibling cache invalidation ordering. Mark checkpoint 2 implemented only after the full matrix passes.

- [x] **Step 4: Run the full checkpoint matrix**

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test -p school-errors
cargo test -p school-http
cargo test -p school-authorization
cargo test -p school-test-db
cargo test --test static_architecture
cd ..
./scripts/test_backend_school.sh
cd frontend-school
npm run check:permissions
npm run test:permissions
npm run check:api-contracts
npm run test:api-contracts
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
npm run check:docs
cd ..
podman build -f backend-school/Dockerfile backend-school
podman run --rm -v "$PWD:/repo" -w /repo docker.io/rhysd/actionlint:1.7.7
```

- [x] **Step 5: Verify immutable artifacts and exact-tree cleanliness**

```bash
git diff origin/main -- contracts/permissions.json contracts/permissions.lock.json \
  contracts/openapi/school-api.json frontend-school/src/lib/permissions/registry.generated.ts \
  backend-school/migrations backend-school/migrations_legacy
rg -n 'crate::error|crate::api_response|crate::middleware::permission|crate::db::permission_cache|crate::test_helpers' backend-school --glob '*.rs'
git diff --check
git status --short
```

Expected: artifact/migration diff and old-owner search are empty, all gates pass, and the working tree is clean after status documentation is committed.

## Completion Gate

Checkpoint 2 is complete only when all four owners are singular, `school-errors` has no Axum response implementation, permission cache has no session/auth dependency, dual invalidation is synchronous and tested, `school-test-db` is dev-only, all contract/runtime behavior is unchanged, compile evidence meets the gate, and the full verification matrix is green. Then create the focused checkpoint 3 plan for `school-tenancy` before moving its source owners.
