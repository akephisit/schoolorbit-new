# Student Enrollment Lifecycle Boundary Implementation Plan

> Execute inline with `superpowers:executing-plans`; one build/test job at a time.

**Goal:** Coordinate admission enrollment and student deactivation with academic year closure without rewriting closed-year history.

**Approved references:** `2026-09-10-academic-term-lifecycle-design.md` and `2026-09-10-academic-year-promotion-design.md` in the sibling specs directory.

**Ownership:** Academic Core owns year guards and withdrawal of student-year/placement records. Admission keeps application/enrollment receipts and account provisioning. Students keeps account deactivation and the existing permission-cache invalidation. No new permission grants, automatic promotion, migration changes, or live-data cleanup.

## Enrollment

- [x] Extend the existing disposable admission enrollment fixture through the current migrations. Add closed/archived-year denials, unchanged application/account/placement counts, retained completed-enrollment replay after closure, and a concurrency probe proving year coordination precedes application/assignment locks.
- [x] Start shared tenant transition coordination before resolving the application and round year. Resolve the exact year without locking the application, then acquire Core's year-exclusive guard before application/assignment locks for a new enrollment. Recheck the round/context after locking; reject a changed context rather than acquiring another year after entity locks.
- [x] Keep an already-completed receipt read-only and replayable after closure. A receipt that changes back to a writable enrollment path while waiting must conflict, not bypass the earlier guard. Preserve existing planning/ready versus active placement status behavior and the current atomic provisioning transaction.
- [x] Document the existing complete-enrollment endpoint with typed request/response DTOs and 409 lifecycle/context conflicts. Preserve the existing camel-case wire shape and flexible enrollment form payload; consume the generated DTO in its frontend wrapper where applicable.
- [x] Keep student-code allocation, credentials, uploads and family-data handling outside this focused change; do not log application payloads or national IDs.

## Student deactivation

- [x] Add disposable tests with current, future-planned and closed-year records for one synthetic student. Prove deactivation preserves closed/archived academic records, withdraws only mutable-year records, retains all identities/history, and handles a planned placement whose start is in the future without violating its date constraint. Preserve missing-student and permission invalidation behavior.
- [x] Introduce a Core-owned transactional withdrawal command called before the Students account UPDATE. Acquire the exclusive tenant transition lock before resolving the student's mutable-year set, then sorted year locks before student/placement entities. This rare cross-year operation must not discover and lock an extra year after taking a student/entity lock.
- [x] Scope bulk placement and student-year updates to the guarded mutable-year IDs; increment row versions. End dates must remain at or after placement start. Closed-year rows remain untouched even if an old migration left their status active/current. Keep account deactivation possible without rewriting academic history.
- [x] Use the existing Core audit mechanism with the authenticated actor and changed academic record IDs, in the same transaction. Preserve the Students handler's existing cache invalidation and signal after successful commit. Do not introduce a second public withdrawal workflow.

## Verification

- [x] Run focused regression tests from red to green, then full admission enrollment and student mutation tests plus the combined lifecycle suite using `scripts/test_backend_school.sh` and `--test-threads=1`.
- [x] Run backend fmt/architecture/check/API tests; generate/check/test API artifacts and frontend lint/check/static tests serially for affected generated/frontend files.
- [x] Review lock order, completed receipt replay, closed-history preservation, transactional rollback and unchanged authentication behavior. Commit on the existing feature branch; do not deploy the unfinished Releases 3–4.

The remaining lifecycle transitions/workspace, preparation, annual aggregation and reviewed promotion stay under their approved specs; this boundary does not complete either release.
