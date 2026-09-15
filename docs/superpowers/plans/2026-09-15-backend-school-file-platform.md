# Backend School File Platform Extraction Plan

> **Execution:** Implement task-by-task with the approved master architecture as the governing
> specification.

**Goal:** Complete checkpoint 4 by extracting the provider-neutral File Platform runtime,
inspection, persistence, object-key registry, storage/scanner implementations, reconciliation, and
focused tests into `school-file-platform` without changing HTTP, storage, database, security, or
file-lifecycle behavior.

**Architecture:** `school-file-platform` is a cohesive runtime crate that accepts tenant pools and
explicit provider/scanner/repository interfaces. It owns platform types, file inspection and image
derivation, hashing, purpose policy data, the SQL repository, lifecycle service, R2 implementation,
clamd implementation, runtime configuration, and reconciliation. Root application adapters retain
Axum multipart/download handlers, tenant and actor resolution, cross-domain relationship writes,
cross-domain authorization policy, error-to-wire mapping, event/orchestration behavior, and
scheduled all-tenant iteration.

**Constraints:** Keep one executable and image; do not edit migrations, generated contracts, object
key formats, bucket selection, scan framing, lifecycle transitions, retry/lease bounds, public
errors, or log redaction. `school-test-db` is dev-only. The crate must not depend on the root package,
`AppState`, Axum, auth, certificates, fonts, admission, question bank, or another feature domain.

## Task 1: Characterize and Guard the Boundary

- [x] Add a static architecture test requiring the workspace member and dependency, rejecting the
  old root runtime owners and file-only utility owners, and preserving handlers/models,
  `consumer_service`, OpenAPI/cross-domain adapters, and `file_access_policy` in the application.
- [x] Extend the approved graph guard so the new crate uses workspace lints/dependencies, has no
  root package/AppState/Axum/feature-domain edge, and keeps `school-test-db` dev-only.
- [x] Run the focused architecture test before creating the package and record the expected RED.

## Task 2: Extract the Canonical Platform Runtime

- [x] Add `school-file-platform` as a workspace member/path dependency and root normal dependency.
- [x] Move platform types, purpose registry, inspection, scanner, storage provider/R2, repository,
  lifecycle service, runtime config, and reconciler into the crate.
- [x] Move `FileHasher` and `ImageProcessor` into private crate modules and delete their root utility
  declarations/files so implementation ownership is singular.
- [x] Move File Platform schema and repository tests to the crate, point test-only migration
  fixtures at the unchanged canonical timeline, and use `school-test-db` only as a dev dependency.

## Task 3: Cut Application and Domain Consumers Over

- [x] Import File Platform public APIs directly in startup/AppState, cleaner scheduling, file HTTP
  adapters/models, OpenAPI composition, cross-domain consumer service, and file access policy.
- [x] Cut admission, question bank, school fonts, certificate purge/tests, and other domain
  consumers to the public crate without compatibility modules or `#[path]` sharing.
- [x] Reduce root `modules::files` to HTTP/cross-domain adapters only and preserve exact multipart,
  policy, relationship-write, cleanup-after-failure, redirect/grant, audit, and public-error behavior.

## Task 4: Verify File and Cross-Domain Semantics

- [x] Run package tests and workspace checks plus focused inspection, purpose/object-key, clamd, R2,
  repository, lifecycle, reconciliation, HTTP handler, cross-domain file policy, font, question-bank,
  admission-document, certificate purge, readiness, and scheduler tests.
- [x] Run the disposable PostgreSQL suite so repository/schema/lifecycle and every cross-domain
  relationship/deletion path execute against canonical migrations.
- [x] Run API contract and frontend File Platform static checks; confirm OpenAPI/generated API,
  permissions, and both migration directories are unchanged.

## Task 5: Measure, Document, and Close Checkpoint 4

- [x] Measure three comment-only `school-file-platform` incremental workspace checks with the
  checkpoint target directory; record dirty/fresh evidence and compare the median with the 59.445
  second checkpoint-1 local-package baseline.
- [x] Update backend README, testing documentation, master status/evidence, and mark this plan only
  after every gate passes.
- [x] Run the complete `.rules` matrix: backend packages/static/full DB, frontend
  permissions/API/lint/type/static/docs, Docker image, actionlint, exact artifact/migration diff,
  `git diff --check`, and final tree review.

## Completion Gate

Checkpoint 4 is complete only when File Platform runtime ownership is singular, request and
cross-domain policy remain application adapters, `school-test-db` is absent from release
composition, object keys/provider details and scan results stay private, all lifecycle and
relationship behavior is unchanged, focused/full verification passes, and representative edits
meet the compile regression gate.
