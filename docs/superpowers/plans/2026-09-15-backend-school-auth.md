# Backend School Authentication Extraction Plan

> **Execution:** Implement task-by-task with the approved master architecture and the security
> rules in `.rules` as the governing specification.

**Goal:** Complete checkpoint 6 by making `school-auth` the singular owner of session identity,
credentials, cache, repository, throttling, user-profile domain operations, and permission-change
events while preserving every cookie, CSRF, cache-invalidation, realtime, audit, API, database, and
deployment contract.

**Architecture:** `school-auth` depends downward on tenancy, authorization, crypto, errors, and the
database/runtime primitives it needs. It owns `AuthRuntime` but never receives `AppState`; the root
implements the Axum `FromRef<AppState>` adapter and retains cookie/header parsing, request-origin
resolution, routes, OpenAPI composition, File Platform cleanup orchestration, and cross-domain HTTP
tests. Permission-change events move beside the caches they invalidate so SSE, WebSocket, staff,
student, migration, and tenant adapters consume one typed event. The application preserves the
ordered invariant: invalidate identity, invalidate permissions, then publish realtime change.

**Constraints:** Keep one executable and image. Do not edit migrations or generated contracts,
change session token/cookie/CSRF formats or lifetimes, cache TTLs/capacity/striping, password policy,
throttle behavior, audit fields, public errors, OpenAPI shapes, user-profile file ordering, tenant
resolution, or permission semantics. Raw credentials, prior-token acceptance, secrets, database
URLs, cookies, and national IDs must never enter logs or new public/debug output. Test database and
environment-lock support remain dev-only.

## Task 1: Prove and Guard the Boundary

- [x] Add static architecture assertions that require `school-auth`, prohibit root-package/source
  dependencies and full `AppState` in it, keep HTTP/session adapters in root, and make auth runtime,
  cache, policy, repository, service, and permission-event ownership singular; record the expected
  RED before the crate exists.
- [x] Inventory root auth consumers and classify each as a direct domain API consumer, an Axum
  adapter, or application orchestration. Preserve the exact provider direction instead of adding a
  compatibility module or `#[path]` production sharing.

## Task 2: Extract Auth Domain and Runtime

- [x] Add `school-auth` to the workspace with centralized dependencies/lints and dev-only
  `school-test-db`, tenancy test support, and crypto test support where tests require them.
- [x] Move auth audit, config, domain events, models, user-profile services, session cache/crypto/
  policy/repository/service, throttle repository, and their focused tests into the crate.
- [x] Move `PermissionChangeEvent` from notification into `school-auth`; keep tenant notification
  and work events with their current application owner, and cut SSE/WebSocket/application consumers
  to the new canonical type.
- [x] Move the root-independent `AuthRuntime` into the crate and keep only the application-owned
  `FromRef<AppState>` implementation at root. Preserve shared cache instances, tenant service
  context construction, and synchronous identity-before-permission invalidation.

## Task 3: Preserve Application Adapters and Side Effects

- [x] Keep session/profile handlers, cookie and CSRF HTTP helpers, client-address/origin resolution,
  router registration, OpenAPI composition, and HTTP/middleware integration tests in root; point
  them directly at `school-auth` without compatibility re-exports.
- [x] Keep profile-image deletion as root File Platform orchestration driven by the typed auth
  result. Preserve ready/owner/purpose validation, retention promotion, user-row locking, commit,
  replacement detection, and deletion-request ordering without introducing an auth-to-file crate
  dependency.
- [x] Keep the deliberate staff-soft-delete/session invalidation scenario as a root cross-domain
  integration test; all auth-owned repository, service, crypto, cache, policy, and profile tests run
  inside `school-auth` through its dev-only fixtures.
- [x] Update every handler, policy, request-context, notification, timetable WebSocket, startup,
  and test consumer to use the crate types directly. Remove the old production auth owner only
  after no root source path remains.

## Task 4: Verify Session, Cache, and Realtime Security

- [x] Run all `school-auth`, authorization, tenancy, crypto, root auth HTTP/middleware, notification
  SSE, timetable WebSocket security, profile/File Platform, and static architecture tests.
- [x] Run the disposable PostgreSQL root suite so login, rotation, previous-token grace, touch-only
  realtime, password change, revocation, throttling, profile update, tenant isolation, and composed
  cache invalidation execute against the unchanged canonical migrations.
- [x] Run frontend session/account/auth-state static tests and Playwright discovery with dedicated
  disposable session credential names. Execute destructive browser tests or deployed-proxy smoke
  only when their required runtime credentials/services are available, and report unavailable gates
  explicitly rather than claiming them.
- [x] Confirm tracked OpenAPI/generated API, permissions, and both migration directories are byte
  unchanged; confirm the normal release graph contains no `school-test-db` or test-support feature.

## Task 5: Measure, Document, and Close Checkpoint 6

- [x] Measure three comment-only incremental workspace checks for `school-auth`; record dirty/fresh
  package evidence and compare the median with the 59.445 second checkpoint-1 local-package
  baseline.
- [x] Update the approved master graph/status, backend README, testing documentation, and this plan
  only after the complete behavior gate passes.
- [x] Run the complete `.rules` matrix: fmt, warnings, workspace package/static/full DB, frontend
  permission/API/lint/type/menu/static/docs, release image inspection, actionlint, exact artifact and
  migration diff, `git diff --check`, and final clean-tree review.

Verification on 2026-09-15 passed all runnable gates. Playwright discovery found four tests in the
two session suites. Destructive browser execution and deployed-proxy smoke were not run because the
environment had neither dedicated disposable session credentials nor an isolated deployed target;
they were not replaced with weaker checks or reported as passing.

## Completion Gate

Checkpoint 6 is complete only when auth has one acyclic crate owner; root retains only HTTP,
composition, File Platform orchestration, and deliberate cross-domain integration; session/cookie/
CSRF/cache/throttle/audit/profile behavior and identity-before-permission-before-event ordering are
unchanged; all runnable focused/full verification passes; environment-dependent omissions are
named; and a representative auth edit meets the compile regression gate.
