# Academic Delivery Lifecycle Guards Implementation Plan

> Execute inline with `superpowers:executing-plans`; run tests/builds serially.

**Goal:** Make the approved term/year closure boundary authoritative for Delivery before any lifecycle transition endpoint is exposed.

**Spec:** `docs/superpowers/specs/2026-09-10-academic-term-lifecycle-design.md` and the Release 4 closed-year invariant.

**Prerequisite:** Result-source guards in `2026-09-11-academic-result-lifecycle-guards.md`.

## Locking and ownership

Core owns the academic transition shared/exclusive advisory lock and academic state validation. Delivery retains its offering/group/change-set authorization, optimistic versions, immutable snapshots, and audit ownership. Closed/cancelled terms and closed/archived years reject ordinary writes; `closing` allows finishing unresolved operational changes. No permission bypasses academic closure.

Acquire lifecycle coordination before any term, offering, group, change-set, timetable, or handoff entity lock. Resolve immutable context identifiers without row locks first. Callers needing a term `FOR UPDATE` must acquire that mode initially, not upgrade two concurrent `FOR SHARE` holders. Keep the year lock before the term lock. A preview requiring no mutation must not make a closed context editable.

Same-intent completed idempotent retries may return their retained original response without new writes. A new request, changed intent, or unfinished operation must revalidate closure in its own transaction. No compatibility layer or direct live data repair.

## Tasks

- [x] Extend Core lifecycle guard with an explicit exclusive-term row-lock entry point while retaining shared tenant transition coordination. Test a source versus exclusive-term writer and two exclusive-term writers; ensure no shared-to-exclusive lock upgrade cycle.
- [x] Route Delivery `require_writable_term` through Core, resolve year first, then fetch term details using the already-held locks. Remove the duplicate status rule that currently rejects `closing`; preserve immutable snapshot/group rules.
- [x] Audit and fix lock ordering for offering create/update/publish and curriculum apply. In apply, acquire the required term lock before building the authoritative source preview, not after a shared-lock preview. Retain exact idempotency and curriculum mappings.
- [x] Fix group create/update/homeroom replacement ordering: resolve context, acquire lifecycle/term guard, then offering and group. Keep teacher replacement, roster application/publication and dated membership operations in the same order. Read-only roster previews must not write or require editable history.
- [x] Verify activity enrollment/unenrollment and registration context use the same guard without weakening student scope, capacity, date-window or activity eligibility checks.
- [x] Audit every change-set create/update/cancel/item upsert/delete/publish path and teacher-handoff apply. Handoff must acquire lifecycle coordination before its idempotency and version locks. All mutation helpers called during publication must reuse the caller's transaction and compatible lock order.
- [x] Align change-set readiness and its timetable draft-cloning dependency with the same `closing`/closed-year rule. In `timetable_version_service.rs`, resolve version context and take the initial exclusive term lock before locking the source version; remove the older duplicate `closing` rejection. Keep published source/version/date checks and historical reads. Cover both standalone cloning and change-set preparation in disposable tests.
- [x] Add disposable PostgreSQL regressions for ordinary writes in closed term/year, allowed operations in `closing`, continued historical reads, rejected foreign contexts, concurrent closure/source writes, and retained idempotent responses.
- [x] Run Delivery and affected Core/timetable regressions sequentially, plus formatting, architecture, compile and API artifact gates. Review the complete diff and commit on the feature branch.

## Verification and remaining boundary

Use `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh <filter> -- --test-threads=1`. Do not run another build/test job concurrently. Follow `.rules` for contracts and any newly affected UI.

This change does not expose closure. Timetable block/version/template guards are covered by `2026-09-11-academic-timetable-lifecycle-guards.md`. Core year-owned student/placement/bell operations, exam rounds/days/sessions/rooms/invigilation/publication, and supervision cycle/observation/evaluation/review mutations still require their own guard integration before Release 3 transitions can be enabled. Future-term preparation reads closed source configuration but writes only explicitly mapped target drafts; it must not reuse a same-term writer that forbids reading a closed source.
