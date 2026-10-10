# Backend School Fonts and Certificates Extraction Plan

> **Execution:** Implement task-by-task with the approved master architecture as the governing
> specification.

**Goal:** Complete checkpoint 5 by extracting reusable field encryption, the school font library,
and the certificate domain into acyclic workspace crates while preserving every ciphertext,
blind-index, database, authorization, file-lifecycle, HTTP, OpenAPI, and public verification
contract.

**Architecture:** `school-crypto` becomes the singular environment-backed AES-GCM and keyed blind
index owner used by current and future domain crates. Exact organization-unit permission queries
move into `school-authorization`. `school-fonts` owns font models, validation, persistence, typed
staging relations, deletion conflicts, and focused tests while depending on File Platform values.
`school-certificates` owns certificate models, policies, validation, repositories/services,
rendering, proofs, verification, purge, rate limiting, and focused tests; it depends on fonts, File
Platform, crypto, authorization, errors, and generated permission constants. Root retains Axum
handlers, AppState/tenant resolution, File Platform deletion orchestration, OpenAPI composition,
and migration tests whose fixture deliberately crosses the academic cutover timeline.

**Constraints:** Keep one executable and runtime image. Do not edit migrations or generated
contracts, change encryption formats/keys/domains, expose proof plaintext, weaken exact-unit
authorization, alter certificate number/layout/render semantics, or change file/font lifecycle and
purge ordering. Every test-only database or environment-lock edge stays dev-only.

## Task 1: Guard and Extract Shared Prerequisites

- [x] Add static architecture guards for singular crypto, font, and certificate owners and the
  approved dependency direction; record the expected RED before packages exist.
- [x] Extract `utils::field_encryption` unchanged into `school-crypto`, with a dev-only
  `test-support` feature for the serialized environment lock, and cut all consumers over directly.
- [x] Move exact-unit permission resolution into `school-authorization`; keep root resource policy
  as a consumer and preserve every query and active membership/role/grant/delegation condition.

## Task 2: Extract the School Font Library

- [x] Add `school-fonts` and move font models, validation, repository/service operations, typed
  staging relationships, and core tests into it; keep the Axum handlers in the application.
- [x] Move central and certificate font-upload relationship recording to their owning font and
  certificate public operations so root File Platform adapters no longer own feature SQL.
- [x] Preserve atomic campaign locks, file promotion, staging cleanup, audit writes, duplicate
  classification, reference-conflict outcomes, and the root cross-domain file-policy integration
  test.

## Task 3: Extract the Certificate Domain

- [x] Add `school-certificates` and move models, domain policies, limiter, validation, services,
  proof/receipt crypto calls, rendering, purge, and focused service tests into it.
- [x] Replace root-only lookup output with a typed certificate owner option mapped by the HTTP
  adapter, and replace root File Platform error mapping with a domain-safe local conversion.
- [x] Keep certificate HTTP handlers and their security-header/error mapping test in root; keep
  academic-cutover migration integration tests in root while all certificate-owned runtime and
  focused tests move to the crate.
- [x] Cut File Platform policy, startup state, cleaner, API composition, and every remaining
  consumer to direct crate APIs without compatibility modules, `#[path]` production sharing, or
  cyclic dependencies.

## Task 4: Verify Security and Cross-Domain Semantics

- [x] Run crypto, authorization, font, certificate, File Platform, and all workspace package tests;
  run focused proof/receipt, limiter, layout, file-policy, font lifecycle, rendering, purge, and
  handler tests.
- [x] Run the disposable PostgreSQL root suite so cross-domain migration, relationship, lock,
  issuance, verification, and deletion behavior executes against the unchanged canonical timeline.
- [x] Confirm tracked OpenAPI/generated API, permissions, and both migration directories are byte
  unchanged; confirm normal release trees contain neither `school-test-db` nor test-support.

## Task 5: Measure, Document, and Close Checkpoint 5

- [x] Measure three comment-only incremental workspace checks for both `school-fonts` and
  `school-certificates`; record dirty/fresh evidence and compare medians with the 59.445 second
  checkpoint-1 local-package baseline.
- [x] Update the approved master graph/status, backend README, testing documentation, and this plan
  only after the complete gate passes.
- [x] Run the complete `.rules` matrix: fmt, warnings, workspace packages/static/full DB, frontend
  permissions/API/lint/type/menu/static/docs, release image inspection, actionlint, exact artifact
  and migration diff, `git diff --check`, and final clean-tree review.

## Completion Gate

Checkpoint 5 is complete only when crypto, font, and certificate ownership is singular and acyclic;
root contains only HTTP/composition and deliberate cross-domain migration integration; ciphertext,
proof, exact-scope authorization, file/font lifecycle, rendering, purge, and public response
behavior remain unchanged; all focused/full verification passes; and representative edits meet the
compile regression gate.
