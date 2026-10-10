# Backend School Academic Assessment and Results Extraction Plan

> **Execution:** Implement task-by-task with the approved crate-architecture specification and
> `.rules` as the governing standard.

**Goal:** Complete checkpoint 9 by making `school-academic-assessment` and
`school-academic-results` the singular owners of assessment configuration, gradebook, learner
evaluation, result aggregation, locking, correction, and publication logic while preserving all
routes, DTOs, transactions, authorization decisions, generated contracts, and migrations.

**Architecture:** Keep Axum handlers, router/OpenAPI composition, tenant/session resolution,
realtime invalidation, and deliberate cross-domain integration tests in the application. Assessment
owns assessment plans and phase controls, gradebook score entry and confirmations, learner
evaluation configuration/entry/locking, and their policies. Results depends on Core and Assessment
and owns result policies, readiness, aggregation, annual revisions, locks, corrections, and
promotion-facing read operations. Assessment never imports Results: result-lock checks required by
assessment and gradebook writes are expressed through an Assessment-owned async port implemented
by the application using the Results crate, preserving the existing transaction and lock order.

**Dependency direction:** `core -> assessment -> results`; Delivery and Timetable remain sibling
branches and neither Assessment nor Results may depend on Lifecycle or the root package.

**Constraints:** Preserve one executable/image, schema and operation names, serialized fields,
error text, permission codes, advisory/row lock order, transaction boundaries, audit behavior,
idempotency, migrations, and generated output. Do not introduce an applied migration edit,
production `#[path]`, root-package dependency, full `AppState`, global service locator, duplicate
SQL owner, or permanent compatibility implementation.

## Task 1: Prove and Prepare the Acyclic Boundary

- [x] Inventory Assessment/Gradebook/Learner Evaluation-to-Results calls, Results-to-Assessment
  types and operations, policies, lifecycle consumers, system migration consumers, handlers, and
  tests.
- [x] Add RED static guards for both workspace members, exact dependency edges, singular source
  ownership, no root/application dependency, no Assessment-to-Results import, and no production
  root copy of moved implementation.
- [x] Add an Assessment-owned async `ResultLockPort` for same-transaction course-result lock
  validation. Implement it in the application using the Results owner without changing lock order
  or transaction scope.
- [x] Move assessment, gradebook, and learner-evaluation policies below the Assessment owner; move
  result and combined aggregate policies below Results.

## Task 2: Extract `school-academic-assessment`

- [x] Add the workspace crate with centralized dependencies/lints, Core dependency, and dev-only
  database test support.
- [x] Move assessment models/services, gradebook models/services, learner-evaluation models/services,
  validation, policy logic, and focused pure tests.
- [x] Retain only handlers/routes, the result-lock adapter, realtime publication, and deliberate
  application integration tests in the root modules. Any temporary test-only facade must contain no
  SQL or business rule and be removed or explicitly justified before the final checkpoint.
- [x] Update API composition and every production consumer to the crate public API.

## Task 3: Extract `school-academic-results`

- [x] Add the workspace crate depending only on Core, Assessment, and approved foundation crates.
  Move result models, policies, services, aggregation/revision logic, and focused pure tests.
- [x] Publish the narrow lock-check operation used by the application adapter and the narrow
  lifecycle/promotion read operations required by checkpoint 10. Do not expose private row types
  solely to retain root visibility.
- [x] Cut handlers, lifecycle, system migration, API composition, and all other production consumers
  directly to Results; remove the old model/service/policy owners.
- [x] Keep migration/cutover and genuinely multi-owner tests in the application, while crate-owned
  unit and canonical-migration package tests live with their owners.

## Task 4: Preserve Contracts and Behavior

- [x] Compile both crates and the application tests, then run focused Assessment, Gradebook,
  Learner Evaluation, Results, Lifecycle, Core, and static architecture suites.
- [x] Add canonical-migration package persistence tests for representative assessment/gradebook or
  learner-evaluation and result aggregation/lock paths, with `school-test-db` dev-only.
- [x] Regenerate API and permission artifacts and prove tracked OpenAPI, TypeScript, permission,
  and migration outputs are byte unchanged.
- [x] Update backend and frontend source-path guards to follow the new singular owners without
  weakening their behavioral assertions.

## Task 5: Verify Compile Scope and Checkpoint Candidate

- [x] After a warm-up, measure three comment-only workspace checks for Assessment and Results.
  A Results edit must leave Timetable fresh and a Timetable edit must leave Results fresh.
- [x] Compare medians with the 59.445-second checkpoint-1 local-package baseline and block an
  unexplained regression above the 10-percent gate.
- [x] Run frontend contract/static/docs/lint/type/build checks, backend formatting, workspace
  all-target checks, warning-denied binary check, actionlint, and runtime image build/inspection.
- [x] Update the master status and checkpoint checklist only after the exact candidate passes every
  runnable gate. The complete disposable root database suite may be consolidated with checkpoint
  12, but it must pass before the program is declared complete.

## Completion Gate

Checkpoint 9 is complete only when Assessment and Results each have one crate owner and narrow
public APIs; Cargo dependencies enforce `core -> assessment -> results`; Assessment-to-Results
same-transaction checks remain atomic through the application adapter; root retains only HTTP,
composition, and deliberate integration tests; API, permission, migration, lock, audit, and runtime
behavior are unchanged; and representative edits prove Results and Timetable compile independently.
