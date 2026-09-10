# Academic Result Lifecycle Guards Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline. Run tests and builds serially.

**Goal:** Make closing/closed term semantics authoritative for score and result sources before exposing any lifecycle close endpoint.

**Spec:** Approved Release 3 `2026-09-10-academic-term-lifecycle-design.md` and Release 4 closed-year invariants.

**Architecture:** Core owns a shared transaction guard. Ordinary source writers acquire the tenant transition shared lock, then year and term row locks, before existing offering/group/source locks. A lifecycle transition acquires the matching exclusive lock before reading readiness. Closed/cancelled terms and closed/archived years reject ordinary mutations independently of admin privileges. `closing` still allows completion. Dedicated corrections and aggregate recalculation coordinate through the transition lock without masquerading as ordinary editing.

## Constraints

- No closing endpoint is registered until Delivery, timetable, exam and supervision writers are guarded in their own subsequent owner changes as well.
- Existing authenticated permission and resource policies remain mandatory; a lifecycle state check never grants permission.
- All state checks happen inside the same write transaction, before lower-order entity locks.
- Historical reads never initialize new term configuration in a closed context. Existing snapshots and criteria remain readable; missing historical configuration is represented honestly, without fabricated revisions or default results.
- Existing capability fields (`canManage`, `canConfirm`) reflect closure on workspace reads. Do not mislabel a closed term as an ordinary entry-window toggle.
- Audited corrections remain possible after closure and continue to invalidate aggregate freshness. No original locked snapshot is updated/deleted.

## Tasks

- [x] Add Core guard tests for all year/term statuses, foreign context IDs, and both orderings of a concurrent source write versus transition. Test shared ordinary writers and exclusive transitions without timing-only assertions.
- [x] Implement the transaction lock and operational year/term guard in `core/services/lifecycle_guard.rs`, with focused pure tests and explicit lock ordering.
- [x] Integrate Gradebook `begin_scope` and control mutations; make read capabilities false for a closed term/year. Test admin denial, ordinary teacher denial, unchanged scores and continued historical reads.
- [x] Integrate Results course/activity preparation, confirmation and initial locks. Coordinate aggregate locks, corrections and policy activation with the transition lock; preserve their distinct allowed operations.
- [x] Split Learner Evaluation read versus write intent at `begin_subject`. Prevent closed-context lazy initialization; integrate configuration, responses, confirmations, locks and controls. Reflect closure in workspace capabilities without hiding existing history.
- [x] Integrate Assessment structure and phase-control mutations with the same guard before offering/phase locks. Preserve closed-term read access and immutable exam source semantics.
- [x] Test source mutation racing closure, no privilege bypass, continued audited corrections and current/stale aggregate behavior. Verify all affected module tests serially.
- [x] Run backend formatting, architecture and compile gates. If wire fields must change to represent absent historical configuration, regenerate typed APIs and verify affected frontend consumers and browser workflows; never introduce compatibility aliases.
- [x] Review complete mutation call sites and commit the coherent result-source guard change. Continue with the other operational owners before implementing lifecycle transitions.

## Verification

Use `scripts/test_backend_school.sh <focused-filter> -- --test-threads=1` with `CARGO_BUILD_JOBS=1` for disposable PostgreSQL tests. Run the `.rules` matrix for each touched change type. Keep live tenants and the shared deployment untouched while the lifecycle boundary remains incomplete.
