# Academic Timetable Lifecycle Guards Implementation Plan

> Execute inline with `superpowers:executing-plans`; run builds/tests serially.

**Goal:** Enforce the approved closure boundary for every timetable block and template mutation before enabling lifecycle transitions.

**Architecture:** Reuse Core lifecycle coordination through `timetable_version_service::require_version_term_write`. Resolve immutable version context first, take tenant/year/term locks, then version and block locks. Delivery synchronization helpers reuse their caller's already-guarded transaction; they must not upgrade its term lock after locking groups.

**Tech Stack:** Rust, SQLx/PostgreSQL, existing typed OpenAPI contracts.

**Spec:** `docs/superpowers/specs/2026-09-10-academic-term-lifecycle-design.md`.

## Global constraints

- `closing` remains writable; closed/cancelled terms and closed/archived years reject ordinary mutations, including school administrators.
- Do not change published-version immutability, optimistic versions, instructor eligibility, collision rules or individual-target deletion semantics.
- No migration, compatibility layer, live data repair or global template restriction is needed. A closed source remains readable/exportable; applying a template writes only an editable target.

## Task 1: Block mutation boundary and lock order

Files: `backend-school/src/modules/academic/services/timetable_block_service.rs` and `timetable_block_service_tests.rs`.

Consumes: `require_version_term_write(&mut Transaction<'_, Postgres>, Uuid) -> Result<(), AppError>` from `timetable_version_service.rs`.

- [x] Add a disposable test creating a real structural block before closing the year/term. Assert create, update, remove-target, deactivate and deactivate-series reject `AppError::Conflict`, and the original block/target revisions remain unchanged. Use the existing `migrated_pool` and `draft_version` fixtures and `CreateStructuralTimetableBlocksRequest` pattern, not a mocked database.
- [x] Run `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh timetable_lifecycle -- --test-threads=1` and confirm a real mutation succeeds unexpectedly before the fix.
- [x] Guard `lock_draft_version`, `ensure_draft_version_id`, and `remove_target` before entity locks:

```rust
super::timetable_version_service::require_version_term_write(transaction, version_id).await?;
```

- [x] In `lock_block` and `swap_blocks`, acquire the version boundary before block locks. Scope locked block queries to the requested version; reject foreign-version IDs without locking another term's blocks. Remove duplicated term-state predicates after routing through Core, while retaining draft-version and optimistic-revision checks.
- [x] Cover swap, synchronized-group retry/restore and structural per-target deletion through the existing real fixtures. Add a concurrent regression proving a mutation waiting on the term does not hold the block/version first. Run the complete `timetable_block_service_tests` filter.

## Task 2: Template application and clear

Files: `backend-school/src/modules/academic/services/timetable_template_service.rs` and `timetable_template_service_tests.rs`.

- [x] Extend the existing source/apply/clear regression while its draft is empty and template still valid. Close the year with the term active, then close the term with the year active; in each case assert both operations fail and the published source stays readable. Restore the fixture's original state before its separate ineligible-instructor checks.
- [x] Run the `timetable_template_service_tests` filter and confirm the old apply/clear path writes in a closed context.
- [x] In `apply_template` and `clear_timetable`, call the shared version-term boundary immediately after beginning the transaction. Lock the target version before testing draft state and before touching blocks. Keep the explicit requested-term match; never fall back to an active context.
- [x] Preserve `from_current`, template list/edit/delete as global configuration operations that do not mutate closed source timetables. Run the full template and version service suites.

## Task 3: Integrated verification

- [x] Run Delivery, Core lifecycle, timetable block/version/template and applicable conflict/synchronization tests using the disposable runner, one invocation at a time.
- [x] Run `cargo fmt --all -- --check`, `CARGO_BUILD_JOBS=1 cargo test --test static_architecture -- --test-threads=1`, and `CARGO_BUILD_JOBS=1 cargo check` from `backend-school`.
- [x] Check generated API artifacts; regenerate only if the typed wire contract changes. Review `git diff --check`, the complete diff and worktree status, then commit the coherent guarded behavior on the feature branch.

## Remaining release boundary

This does not expose closure or complete Release 3. Year-owned Core mutations, exams and supervision still need transactional guard integration; readiness/transitions/preparation UI and Release 4 annual results/promotion remain governed by their approved specifications.
