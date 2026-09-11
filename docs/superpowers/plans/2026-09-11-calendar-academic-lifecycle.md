# Calendar Academic Lifecycle Implementation Plan

> Execute inline with `superpowers:executing-plans`; run tests and builds serially.

**Goal:** Keep annual and optional term-scoped calendar events consistent with academic closure and the current optional planned-end-date schema.

**Architecture:** Calendar retains event/target/reminder ownership. Its transaction acquires Core transition coordination and sorted year locks before term and event locks. Updates guard both existing and requested contexts and recheck mutable ownership under the event lock.

**Tech Stack:** Rust, SQLx/PostgreSQL, generated OpenAPI.

**Spec:** `docs/superpowers/specs/2026-09-10-academic-term-lifecycle-design.md`.

## Constraints

- Closed/archived years and closed/cancelled terms reject ordinary mutations, including administrators. Closing remains writable.
- Year-wide events retain optional terms; do not invent a term or require a known planned end date.
- Historical reads and sent reminder receipts remain available. No applied migration edits or live data changes.
- This implements one source-write boundary, not the full lifecycle UI or Release 4.

## Task 1: Current-schema calendar regression

Files: `backend-school/src/modules/calendar/services_tests.rs`, `services/events.rs`.

- [x] Extend the disposable fixture through current migrations, not only the Phase B migration 46.
- [x] Create a term with `planned_end_date = NULL`, create an event within its year, and assert the event retains that term. Set a planned end date and assert an event beyond it is rejected. The old `academic_terms.end_date` query must fail this regression first.
- [x] Replace the obsolete date predicate with `AND (planned_end_date IS NULL OR planned_end_date >= $4)`. Retain the enclosing year date check so an unknown end does not allow events outside the year.

## Task 2: Transactional closure boundary

Files: the same Calendar service and test files.

- [x] Test create/update/delete in closed and archived years, closed and cancelled terms; assert conflicts and unchanged event/targets/reminders. Test updates cannot escape a closed source by moving to an open context, and cannot move into a closed target.
- [x] Add private helpers taking `&mut Transaction<'_, Postgres>`: read event context `(Uuid, Option<Uuid>)` without row locks; acquire shared transition coordination, sorted/deduplicated year exclusive guards, then sorted/deduplicated term guards. For update/delete, lock the event and recheck its context before writing; return conflict if another update moved it.
- [x] Create guards the requested context. Update guards old and new; delete guards old. Keep all checks and event/target/reminder writes in the same transaction.
- [x] For a valid cross-year move, remove old targets after the event lock but before changing the parent year, then replace targets with the requested year. This preserves the immediate composite FK without compatibility columns or deferred constraints; transaction rollback retains targets on failure.
- [x] Test closing-term success, year-wide events, cross-year target replacement, and contention: holding the year lock must block the mutation before the event lock. Concurrent opposing context moves acquire years in canonical order.

## Task 3: Contracts and verification

Files: Calendar `handlers.rs`, `models.rs`, `backend-school/src/api_contract.rs`, `frontend-school/src/lib/api/calendar.ts`, `frontend-school/tests/static/calendar.test.mjs`, and generated OpenAPI/TypeScript artifacts.

- [x] Document the existing create/update/delete calendar endpoints with typed request/success/error envelopes, including lifecycle 409, and register them. Use existing permission constants; no grants or scopes change.
- [x] Add API assertions for the three mutations and their error schemas. Regenerate with `npm run generate:api-contracts`, then `check:api-contracts` and `test:api-contracts`.
- [x] Replace handwritten event/target request interfaces with `Schemas['UpsertCalendarEventRequest']` and `Schemas['CalendarEventTargetInput']`. Extend the existing calendar contract guard to inspect generated schemas and their consumption, retaining audience and contextual-field checks.
- [x] Run `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh modules::calendar -- --test-threads=1`, the focused lifecycle tests, and API tests. Verify original visibility, audiences and reminder delivery tests still pass.
- [x] Run `.rules` backend matrix and generated-frontend matrix serially, review the diff and commit on the existing feature branch. No deployment of this unfinished release.

## Remaining release boundary

Before exposing lifecycle transitions, complete supervision's transactional source-write integration. The year-close audit must also cover final enrollment in `backend-school/src/modules/admission/services/application_service.rs` and deactivation in `backend-school/src/modules/students/services.rs`: both currently write student-year/placement records outside the Core year commands. Preserve historical closed-year records and do not expand this into a general admission/security rewrite. Readiness, preparation, transitions and their UI, followed by annual aggregation and reviewed promotion, remain governed by the approved Release 3/4 specs.
