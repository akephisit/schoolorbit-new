# Academic Exam Lifecycle Guards Implementation Plan

> Execute inline with `superpowers:executing-plans`, as requested. Run builds and tests serially.

**Goal:** Enforce the approved year/term closure boundary for exam scheduling before lifecycle transitions become available.

**Architecture:** Resolve immutable academic context before exam entity locks. Reuse Core's tenant → year → exclusive-term coordination in each public mutation transaction. Internal placement, invigilator and sync helpers reuse that already-guarded transaction.

**Tech Stack:** Rust, SQLx/PostgreSQL, generated OpenAPI contracts.

**Spec:** `docs/superpowers/specs/2026-09-10-academic-term-lifecycle-design.md`.

## Global constraints

- `closing` remains writable; closed/cancelled terms and closed/archived years reject ordinary mutations, including administrators.
- Preserve historical reads and source previews, explicit source synchronization, published-round rules and optimistic source checksums.
- Missing invigilators do not become a publication blocker.
- Do not edit migrations or live data. No compatibility layer or frontend policy bypass.

## Task 1: Round/day closure and canonical lock order

Files: `backend-school/src/modules/academic/services/exam_schedule_service/shared.rs`, `rounds_and_days.rs`, and `../exam_schedule_service_tests.rs`.

Consumes Core `require_term_write_exclusive(&mut Transaction<'_, Postgres>, Uuid, Uuid) -> Result<(), AppError>`.

Produces private `ExamWriteTarget::{Term, Round, Day, Assignment, Session}(Uuid)` and `require_exam_write(tx, target) -> Result<(Uuid, Uuid), AppError>`, returning `(academic_year_id, academic_term_id)`.

- [x] Extend canonical round-creation status regression to include `closing`. Add closed-year/open-term and open-year/closed-term real mutation regressions, preserving original round/day snapshots.
- [x] Run `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh exam_lifecycle -- --test-threads=1` to witness an existing closed-context write before implementing.
- [x] Resolve context with a static query per typed target, without row locks; sessions join their parent round because sessions have no year/term columns. Then call Core before any exam lock:

```rust
let (year_id, term_id): (Uuid, Uuid) = sqlx::query_as(context_query)
    .bind(target_id).fetch_optional(&mut **tx).await?
    .ok_or_else(|| AppError::NotFound("ไม่พบข้อมูลจัดสอบ".into()))?;
lifecycle_guard::require_term_write_exclusive(tx, year_id, term_id).await?;
Ok((year_id, term_id))
```

- [x] At transaction entry, guard round create/update/delete and day upsert/update/delete. Create uses the helper's returned year. Remove only the old duplicate `closing` rejection; retain validation and permission checks.
- [x] Verify `closing` operations succeed and the closed-state regressions fail with `AppError::Conflict` before any mutation. Add a concurrent test holding the term lock and proving a waiting day update has not locked the day/round first.

## Task 2: Room/session/invigilation/source mutations

Files under the same service directory: `room_assignments.rs`, `sessions_and_conflicts.rs`, `invigilation.rs`, `publishing.rs`, `workspace.rs`; database tests in `../exam_schedule_service_tests.rs`.

- [x] Extend the closed-context fixture to exercise valid room upsert, seat generation, session placement/removal, invigilator replacement/add/remove, source sync and publication. Capture unchanged workspace snapshots after rejected operations.
- [x] Run the focused regressions and confirm the unguarded paths write before adding their transaction-entry boundaries.
- [x] Use `Day` for room upsert and session placement, `Assignment` for seat/invigilator changes, `Session` for session removal, and `Round` for publication/source sync. Call the helper before existing entity or staff-conflict locks.
- [x] Before placement locks any item/day, resolve both immutable parent round IDs and reject mismatches. Keep the authoritative post-lock comparison as well; do not lock foreign-term items while holding the selected term.
- [x] Audit `mark_round_draft_after_mutation`, `replace_assignment_invigilators_in_tx`, and `revalidate_session_duration_change_in_tx` callers: every public writer acquires the boundary first; internal helpers do not upgrade row locks later.
- [x] Run all exam service regressions; existing published-without-invigilators, retained source snapshots, canonical identity, deletion, room/teacher conflict and placement-duration tests must stay passing.

## Task 3: Contracts and verification

Files: `backend-school/src/modules/academic/handlers/exam_schedule.rs`, generated `contracts/openapi/school-api.json` and frontend generated API artifacts only if error contract declarations change.

- [x] Add 409 responses to changed exam mutation annotations where missing, using existing `ApiErrorResponse`. Do not alter DTO shapes or permissions.
- [x] Sequentially run disposable `exam_` tests, backend API contract tests, `cargo fmt --all -- --check`, architecture tests and `cargo check`.
- [x] From `frontend-school`, regenerate API contracts if annotations change, then run `npm run check:api-contracts` and `npm run test:api-contracts`. Inspect generated diff for only intended error responses.
- [x] Review the complete diff and `git diff --check`, then commit on the existing feature branch. Do not deploy shared tenants as a local test step.

## Remaining release boundary

This is a closure prerequisite, not all of Release 3. Core year-owned operations and supervision still need integration; lifecycle readiness/transitions/preparation UI and Release 4 annual aggregation/promotion remain governed by their approved specifications.
