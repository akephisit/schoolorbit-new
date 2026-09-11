# Academic Core Year Lifecycle Guards Implementation Plan

> Execute inline with `superpowers:executing-plans`; keep tests and builds serial.

**Goal:** Coordinate year-owned academic configuration, homerooms and placements with lifecycle transitions without changing their existing planning-only eligibility.

**Architecture:** Core owns the tenant transition lock and initial year lock. Resolve immutable entity-to-year identity without row locks; lock the year before homerooms, student-years, placements, bell schedules or terms. New mutations fail for closed/archived years; a completed transfer retry returns its retained receipt without new writes.

**Tech Stack:** Rust, SQLx/PostgreSQL, existing typed Core APIs.

**Spec:** `docs/superpowers/specs/2026-09-10-academic-term-lifecycle-design.md` and `docs/superpowers/specs/2026-09-10-academic-year-promotion-design.md`.

## Global constraints

- No admin bypass for ordinary closed-year writes. Do not weaken current planning-only rules for homeroom/student-year/bell configuration or draft-term edits.
- Preserve row versions, placement date containment, student/year/grade/program matching, audit events and transfer receipts.
- Future-year preparation must not mutate the current year's student record or placements.
- No applied migration changes, live data repairs, compatibility layer or frontend permission changes.

## Task 1: Year lock and student/homeroom source writes

Files: `backend-school/src/modules/academic/core/services/lifecycle_guard.rs`, `student_years.rs`, and `../services_tests.rs`.

Produces `require_year_write_exclusive(tx: &mut Transaction<'_, Postgres>, year: Uuid) -> Result<AcademicYearStatus, AppError>`: acquire existing `lock_transition_shared`, then year `FOR UPDATE`, reject `Closed | Archived`, return the retained status for stricter planning predicates.

- [x] Extend disposable Core fixtures to prove advisor replacement and placement create/transfer cannot write into closed/archived years. Assert historical reads and completed transfer replay remain unchanged. Include valid planning writes to prove the guard is not blanket denial.
- [x] Run `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh lifecycle_year -- --test-threads=1`; capture the old successful closed-year mutation before implementing.
- [x] Add the year boundary and reuse its shared year-state predicate in `AcademicWriteState::is_writable`:

```rust
lock_transition_shared(tx).await?;
let status: AcademicYearStatus = sqlx::query_scalar(
    "SELECT status FROM academic_years WHERE id=$1 FOR UPDATE",
).bind(year).fetch_optional(&mut **tx).await?
 .ok_or_else(|| AppError::NotFound("ไม่พบปีการศึกษา".into()))?;
if matches!(status, AcademicYearStatus::Closed | AcademicYearStatus::Archived) {
    return Err(AppError::Conflict("ปีการศึกษานี้ปิดแล้ว ดูข้อมูลเดิมได้ แต่แก้ไขผ่านงานปกติไม่ได้".into()));
}
Ok(status)
```

- [x] Guard create/update homeroom, replace advisors, create/update student-year and placement create at the start of their transaction. For IDs other than year, first read their immutable year without `FOR UPDATE`; retain the existing entity lock and row-version checks after the year lock.
- [x] Transfer acquires shared transition coordination before the receipt advisory key. After checking completed receipts, resolve the placement year, acquire the year boundary, then existing placement/student locks. Do not add new audit or business writes to replay.
- [x] Add a concurrent test: hold year `FOR UPDATE`, start advisor replacement, observe `pg_blocking_pids`, verify homeroom `FOR UPDATE NOWAIT` remains available, release the year and assert the mutation succeeds.

## Task 2: Bell and year/term configuration

Files: `bell_schedules.rs`, `years_terms.rs`, and `../services_tests.rs` in the same Core directory.

- [x] Extend existing bell replacement and future draft-term tests with lifecycle coordination checks. Preserve stale-revision failures and active-year draft preparation.
- [x] `require_planning_year` in bell services consumes the new year boundary and retains `Planning` as the only allowed status. Update/replace resolve schedule year without a child lock before invoking it, then acquire their original schedule lock.
- [x] Year create takes shared transition coordination before insert. Year update takes the initial year boundary before term-containment reads. `lock_term_planning_year` takes the initial year boundary before reading dates and preserving `Planning | Active` eligibility. Term create/update/delete continue to use that helper before child locks.
- [x] Run the Core service suite through the disposable runner. Cross-year target rejection, source-history retention, existing planning restrictions and future-term APIs must remain passing.

## Task 3: Verification and integration

- [x] Run Core/lifecycle and affected Delivery/timetable/exam regressions serially. Inspect the call graph for any shared-year lock upgraded after a domain entity lock.
- [x] Check affected Core API annotations for lifecycle 409 responses, regenerate only when declarations change, and run API checks when applicable.
- [x] Run `.rules` backend matrix: formatting check, static architecture, `cargo check`, final diff review and `git diff --check`. Commit on the existing feature branch.

## Remaining release boundary

Supervision and any remaining term-owned mutation paths still need an owner audit before exposing close/reopen transitions. Lifecycle readiness/preparation UI and annual aggregation/promotion are not completed by these guards.
