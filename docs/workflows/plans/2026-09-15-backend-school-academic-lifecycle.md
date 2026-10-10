# Backend School Academic Lifecycle Extraction Plan

> **Execution:** Implement task-by-task with the approved crate-architecture specification and
> `.rules` as the governing standard.

**Goal:** Complete checkpoint 10 by making `school-academic-lifecycle` the singular owner of
academic year/term preparation, activation, opening, closing, reopening, readiness, promotion,
impact review, and transition orchestration while preserving every HTTP/API, database, lock,
idempotency, audit, and permission behavior.

**Architecture:** The application keeps Axum handlers/routes, OpenAPI composition, tenant/session
resolution, realtime publication, and deliberate cross-domain integration tests. Lifecycle depends
only on approved foundation crates plus Core, Delivery, Timetable, Assessment, and Results. Exam
schedule and Supervision remain application-owned during this checkpoint, so Lifecycle owns a
narrow async provider port for their pending-work and same-transaction preparation operations; the
application implements that port without changing transaction or lock scope. Assessment-owned
preparation writes move behind an Assessment public operation instead of retaining Lifecycle SQL.

**Dependency direction:** `core + delivery + timetable + assessment + results -> lifecycle`.
No lower academic crate may depend on Lifecycle, and Lifecycle may not depend on the application
package or import application source paths.

**Constraints:** Preserve one executable/image, route and operation names, DTO serialization,
Thai error text, permission codes, advisory/row lock order, transaction boundaries, readiness
checksums, audit receipts, retry/idempotency behavior, migrations, and generated artifacts. Do not
add a production `#[path]`, duplicate SQL owner, full `AppState`, global service locator, reverse
dependency, or compatibility implementation.

## Task 1: Prove the Boundary Before Moving It

- [x] Inventory production dependencies, public consumers, root-only Exam/Supervision calls,
  promotion/result operations, preparation writes, models, policies, handlers, and tests.
- [x] Add a RED static guard for the workspace member, exact lower-domain edges, no root imports,
  no lower-domain reverse edge, singular model/service ownership, and thin application adapters.
- [x] Add focused port contract tests proving Exam and Supervision failures propagate and that
  pending-work evidence participates in the unchanged readiness checksum.

## Task 2: Establish Explicit Provider and Preparation Contracts

- [x] Add a Lifecycle-owned async provider trait accepting the existing PostgreSQL transaction for
  Exam/Supervision pending work and term-preparation apply operations.
- [x] Implement the trait in the application by delegating to the existing Exam Schedule and
  Supervision owners with the same transaction.
- [x] Add the Assessment-owned term-preparation operation and call lower-domain public operations
  for Delivery, Timetable, Assessment, and Results instead of reverse imports.

## Task 3: Extract `school-academic-lifecycle`

- [x] Add the workspace crate with centralized dependencies/lints and only the approved academic
  dependency edges.
- [x] Move lifecycle models, validation, readiness, opening/activation/transition, preparation,
  promotion/review/execution/reopening logic, and focused pure tests.
- [x] Retain only handlers/routes, provider injection, realtime publication, and genuinely
  cross-domain/cutover integration tests in the root application.
- [x] Update all production consumers and API composition to the crate public API; any root facade
  must be a narrow application adapter with no domain SQL or business rule.

## Task 4: Preserve Behavior and Contracts

- [x] Compile and run Lifecycle package tests plus focused root Lifecycle, Results, Assessment,
  Delivery, Timetable, Core, and static architecture suites.
- [x] Add a canonical-migration Lifecycle package persistence test for a representative transition
  or policy path using dev-only `school-test-db`.
- [x] Regenerate API and permission artifacts and prove tracked OpenAPI, TypeScript, permission,
  and migration outputs are byte unchanged.
- [x] Update backend/frontend source-path guards to follow the Lifecycle crate without weakening
  behavioral assertions.

## Task 5: Verify Compile Scope and Checkpoint Candidate

- [x] After a warm-up, measure three comment-only Lifecycle workspace checks, prove lower academic
  crates remain fresh, and compare the median with the 59.445-second checkpoint-1 baseline.
- [x] Run the full runnable `.rules` matrix and update the master status/checklist only after the
  exact candidate passes. The complete disposable root database suite may be consolidated with
  checkpoint 12, but it must pass before the program is declared complete.

## Completion Gate

Checkpoint 10 is complete only when Lifecycle has one crate owner and a narrow public API; Cargo
enforces the one-way lower-domain graph; Exam/Supervision integration crosses a typed
same-transaction port; root retains only HTTP/composition/adapters and deliberate integration
tests; API, permission, migration, lock, checksum, audit, and runtime behavior are unchanged; and a
representative Lifecycle edit rebuilds only Lifecycle and its actual application consumers.
