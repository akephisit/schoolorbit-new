# Backend School Tenancy Extraction Plan

> **Execution:** Implement task-by-task with the approved master architecture as the governing specification.

**Goal:** Complete checkpoint 3 by extracting backend-admin tenant discovery, immutable tenant
metadata, tenant pool caching, and lazy migration coordination into `school-tenancy` without moving
request header/origin policy or changing migration, retry, connection, error, or logging behavior.

**Architecture:** `school-tenancy` is a low-level runtime crate that depends on
`school-migrations`. It owns `AdminClient`, `PoolManager`, `SchoolDatabaseInfo`, `ActiveSchool`,
`TenantContext`, and the school mapping API. Axum header/origin parsing and permission/session cache
invalidation remain root application adapters. Test-only injection is feature-gated so it is absent
from release builds.

**Constraints:** Keep one executable and image; do not edit applied migrations or contracts; never
log database URLs or secrets; preserve retry bounds, single-flight pool creation, 30-minute TTL,
connection settings, statement-cache behavior, lazy migration semantics, permission-change signal,
and all public error messages.

## Task 1: Characterize and Guard the Boundary

- [x] Extend static architecture tests to require `school-tenancy -> school-migrations`, forbid a
  root-package/AppState dependency, reject the old root owners, and keep request origin parsing in
  `utils::tenant`/`utils::subdomain`.
- [x] Add package characterization tests for retry bounds, retryable statuses, response validation,
  migration-status non-retry, pool TTL/touch, single-flight creation, tenant isolation, failure
  retry, statement-cache capacity, and permission-sync coordination.
- [x] Run the focused architecture test before creating the package and record the expected RED.

## Task 2: Extract the Canonical Tenancy Owner

- [x] Add the workspace member, workspace dependency, root normal dependency, and a root dev edge
  enabling only the `test-support` feature.
- [x] Move `admin_client.rs`, `pool_manager.rs`, and `school_mapping.rs` into the crate without
  changing production logic.
- [x] Move `TenantContext` into the crate so later auth/domain crates can depend on a narrow tenant
  value rather than a root utility module.
- [x] Expose test-only config/pool injection behind `cfg(any(test, feature = "test-support"))` and
  confirm the feature is absent from a normal release dependency tree.

## Task 3: Cut Root Composition and Request Adapters Over

- [x] Import tenancy types directly in startup, `AppState`, auth runtime/session services, calendar
  reminders, migration handlers, request context, WebSocket/SSE tests, and all fixtures.
- [x] Keep tenant header/origin resolution in root; it calls `school_tenancy` discovery/pool APIs,
  maps bounded application errors, then synchronously invalidates identity and permission caches
  before publishing permission events.
- [x] Delete root `db::admin_client`, `db::pool_manager`, and `db::school_mapping` declarations and
  files; leave only root integration tests that genuinely exercise cross-crate composition.

## Task 4: Verify Runtime and Migration Semantics

- [x] Run package tests, workspace all-target checks, the complete static architecture suite,
  tenant resolver tests, pool permission-batch tests, migration-status tests, auth protected-router
  tests, calendar reminder tests, and system migration tests.
- [x] Run at least one disposable-PostgreSQL test through `scripts/test_backend_school.sh` that
  exercises the extracted pool/migration path.
- [x] Confirm permission/OpenAPI/generated artifacts and both migration directories are unchanged.

## Task 5: Measure, Document, and Close Checkpoint 3

- [x] Measure three comment-only `school-tenancy` incremental workspace checks with the checkpoint
  target directory; record dirty/fresh package evidence and compare the median with the 59.445
  second checkpoint-1 local-package baseline.
- [x] Update backend README, testing documentation, master status/evidence, and mark this plan only
  after all gates pass.
- [x] Run the complete `.rules` matrix: backend packages/static/full DB, frontend
  permissions/API/lint/type/static/docs, Docker image, actionlint, exact artifact/migration diff,
  `git diff --check`, and final tree review.

## Completion Gate

Checkpoint 3 is complete only when tenancy has one canonical owner, request-origin policy stays in
the application, test support is absent from release composition, lazy migrations and permission
invalidation preserve their ordering, focused and full verification pass, and representative edits
meet the compile regression gate.
