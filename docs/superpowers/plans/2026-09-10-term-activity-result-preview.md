# Term Activity Result Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline. Steps use checkbox syntax for tracking.

**Goal:** Include independent activity pass/fail/missing coverage in the existing read-only per-student term preview without treating activities as GPA credits.

**Architecture:** Results loads expected activity groups and effective locked results inside the same repeatable-read transaction as course totals. A pure aggregator validates source metadata and counts outcomes by group. The checksum includes activity identities and effective versions; there is still no official aggregate lock or term transition.

**Tech Stack:** Rust, SQLx/PostgreSQL, utoipa and generated TypeScript.

**Spec:** `docs/superpowers/specs/2026-09-10-academic-term-lifecycle-design.md`.

## Global Constraints

- Inline on the existing feature branch, one test/build at a time.
- Preserve missing results, explicit activity failure and source history as different states.
- Do not invent activity grades, credits, expected groups or passing outcomes.
- No new migration, compatibility layer, production writes or deployment.
- Existing school-level Results authorization guards the preview; no new evaluation data is exposed under this permission.

## Task 1: Aggregate activity coverage

**Files:** Create `backend-school/src/modules/academic/results/services/activity_aggregation.rs`; modify `results/services.rs` and `results/models/aggregation.rs` under the same module.

**Interfaces:** `ActivityAggregateInput { learning_group_id, learning_offering_id, result_id, effective_version, outcome }`; UUID identities, optional UUID/result version, optional existing `ActivityOutcome`. `ActivityOutcomeTotals { expected_group_count, passed_group_count, failed_group_count, missing_result_count, coverage_complete, all_passed }`. Function `aggregate_activity_outcomes(&[ActivityAggregateInput]) -> Result<ActivityOutcomeTotals, AppError>`.

- [x] Add failing pure tests: one pass, one fail and one missing produce counts 1/1/1, incomplete coverage and not all-passed. Two distinct groups of one offering remain two groups. Duplicate group IDs and inconsistent result metadata are rejected. Empty input is not complete/all-passed.

```rust
assert_eq!(totals.passed_group_count, 1);
assert_eq!(totals.failed_group_count, 1);
assert_eq!(totals.missing_result_count, 1);
assert!(!totals.coverage_complete);
assert!(!totals.all_passed);
```

- [x] Run the disposable database runner with `activity_` filter; observe missing DTO/function failures for the pure tests and missing preview-field failures for the integration test before implementing either task.
- [x] Implement typed source validation and set-based group deduplication. A nonempty input with zero missing is complete even if a failure remains; all-passed additionally requires zero failures. Rerun focused tests.

## Task 2: Integrate correction-aware activity coverage

**Files:** Create `results/services/activity_preview.rs`; modify `results/services/term_preview.rs`, `results/models/aggregation.rs`, `results/services.rs`, and `results/services_tests.rs`.

**Interfaces:** Internal `load_activity_inputs(tx: &mut Transaction<'_, Postgres>, context: &ResultContext, student_year_id: Uuid) -> Result<Vec<ActivityAggregateInput>, AppError>`. Add `activities: Vec<ActivityAggregateInput>` and `activity_totals: ActivityOutcomeTotals` to `TermResultPreview`.

- [x] Add an integration test using the existing `fixture` and `prepare_activity_group`: read missing coverage, lock one activity, observe pass; correct it to fail, observe changed checksum without any GPA change; close the group and verify the result remains visible.
- [x] Observe the missing preview fields in the combined pre-implementation `activity_` run, then run the focused `results_term_activity_preview` test after implementation.
- [x] Load active memberships joined to `activity_offering_details` UNION retained `academic_activity_results`, scoped by student/year/term and not by current group status. LEFT JOIN results and latest corrections ordered by effective version. Preserve absent results, reject invalid stored outcomes, order by group ID, reject more than 2000 rows rather than truncate.
- [x] Aggregate and include activities in the existing term-preview transaction/checksum. Do not alter numeric course totals. Full Results suite: 42 passed. Strengthen the integration fixture with a numeric course result producing GPA 4.00 and rerun that test: passed; activity pass/fail/closure leaves those numeric totals unchanged.

## Task 3: Contracts and verification

**Files:** `backend-school/src/api_contract.rs`, generated `contracts/openapi/school-api.json` and `frontend-school/src/lib/api/generated/school-api.ts`. Existing wrapper consumes the expanded generated response without duplicated DTOs.

- [x] Register new schemas, generate/check API contracts and run generator tests sequentially: generated artifacts match; all four generator tests pass.
- [x] Run backend formatting/architecture/check and frontend lint/check/static tests. Backend architecture: 163 passed; frontend static: 621 passed; frontend check: zero errors/warnings. Existing backend dead-code warnings remain. Review diffs and commit this activity preview independently.

## Remaining scope

This preview reports configured activity-group coverage, not whether every curriculum requirement has been opened. Delivery readiness must supply that separate requirement check. Learner Evaluation keeps its independent summary until the aggregate snapshot coordinator is implemented. Durable aggregate policies/locks, reviewed holds, lifecycle transitions, preparation, annual results and promotion remain under the approved specs.
