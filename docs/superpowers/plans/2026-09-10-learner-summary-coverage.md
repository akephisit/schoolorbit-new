# Learner Summary Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline. Steps use checkbox syntax for tracking.

**Goal:** Prevent an empty or prematurely closed teaching group from falsely satisfying learner-evaluation coverage before term aggregation consumes it.

**Architecture:** Learner Evaluation remains the owner of its summary. Its existing pure summary and repeatable-read loader must preserve expected course membership independently of group operational status; effective locked evaluation rows remain authoritative. No new workflow, permission, migration, or wire shape is introduced.

**Tech Stack:** Rust, SQLx/PostgreSQL, existing disposable database fixtures.

**Spec:** `docs/superpowers/specs/2026-09-10-academic-term-lifecycle-design.md`, aggregate coverage and readiness requirements.

## Global Constraints

- Work inline at repository root on `feature/academic-lifecycle-release-3-4`.
- Keep zero/exceptional/missing data distinct. Keep domain locks independent.
- Preserve existing correction-aware exact averaging and authorization.
- Run one test/build command at a time; database tests use `scripts/test_backend_school.sh`.
- No production data changes and no claim that this finishes Release 3 or 4.

## Task 1: Reproduce and fix empty summary completeness

**Files:** `backend-school/src/modules/academic/learner_evaluation/services_tests.rs`, `backend-school/src/modules/academic/learner_evaluation/services/summary.rs`.

**Interface:** Existing `summarize_domains(rows, expected, locked, bands) -> Result<Vec<DomainEvaluationSummary>, AppError>`.

- [x] Add a regression test with no expected subjects and no rows, asserting both domains remain incomplete and have no average/quality level:

```rust
let summaries = summarize_domains(&[], &[], &[], &bands()).unwrap();
assert!(summaries.iter().all(|domain| !domain.complete));
assert!(summaries.iter().all(|domain| domain.average.is_none()));
```

- [x] Run `CARGO_BUILD_JOBS=1 cargo test --bin backend-school learner_summary_empty_coverage -- --test-threads=1` in backend-school and confirm the completeness assertion fails.
- [x] Replace `complete: missing_subjects.is_empty()` with `complete: !expected.is_empty() && missing_subjects.is_empty()`; do not fabricate a missing subject ID or numeric level.
- [x] Rerun the focused test and existing pure summary tests.

## Task 2: Preserve missing evaluations after group closure

**Files:** Same two files. Use the existing real `fixture` and `summarize_student_term` functions.

**Interface:** Existing `summarize_student_term(pool, ctx, student) -> Result<StudentEvaluationSummary, AppError>`.

- [x] Add a database test selecting one active member from the fixture. Load the summary before locks, assert the chosen subject is missing in both domains, close the local fixture group, and assert the same subject is still missing:

```rust
assert!(domain.missing_subjects.iter().any(|row| row.subject_id == subject));
assert!(!domain.complete);
```

- [x] Run the disposable database runner with filter `learner_summary_` from root and observe the missing-subject assertion fail after closure while both pure summary tests pass.
- [x] Remove only the `g.status<>'closed'` exclusion from the expected-membership query. Keep student/year/term identity, active membership filtering, retained locked-result union, and deterministic ordering unchanged.
- [x] Rerun the regression and full Learner Evaluation suite, including independent domain locks and append-only correction recalculation: 26 passed.

## Task 3: Verify and checkpoint

- [x] Run backend formatting, architecture tests, and `cargo check` sequentially: formatting/check pass, 163 architecture tests pass. No API artifact regeneration is required for this fix because public types/routes are unchanged.
- [x] Review diff, run `git diff --check`, and commit this coverage fix independently.

## Remaining scope

The course preview and learner summary remain read-only. Combined activity/evaluation snapshots, immutable aggregate policies, aggregate locking/holds, term lifecycle transitions/preparation, annual results and promotion remain work under the approved Release 3/4 specs.
