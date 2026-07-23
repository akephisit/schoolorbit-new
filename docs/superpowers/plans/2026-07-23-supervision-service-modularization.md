# Supervision Service Modularization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the 4,200-line supervision service into cohesive private modules while preserving its Rust, HTTP, database, permission, and workflow behavior.

**Architecture:** Keep `modules/supervision/services.rs` as a compatibility facade. Extract private use-case modules under `modules/supervision/services/`, re-export the existing public surface, and add PostgreSQL characterization tests before moving transaction- and status-sensitive workflows.

**Tech Stack:** Rust 2021, Axum 0.8, sqlx 0.8/PostgreSQL, Tokio tests, Cargo formatting/lint/test tooling.

**Approved design:** `docs/superpowers/specs/2026-07-23-supervision-service-modularization-design.md`

## Global Constraints

- Preserve routes, DTOs, permissions, HTTP statuses, error messages, status transitions, audit rows, and result visibility.
- Preserve SQL text, bind order, transaction boundaries, and bulk mutation behavior during extraction.
- Do not create or modify migrations, frontend files, OpenAPI artifacts, or permission registries.
- External callers continue using `modules::supervision::services`; child modules remain private.
- Do not add mocks, repository traits, caches, or query rewrites.
- Use `TEST_DATABASE_URL` for database tests and never commit credentials.
- Each task finishes with focused tests and a commit.

## Target File Map

**Create:**

- `backend-school/src/modules/supervision/services_tests.rs` — PostgreSQL characterization tests.
- `backend-school/src/modules/supervision/services/shared.rs` — shared pure workflow rules and cross-module types.
- `backend-school/src/modules/supervision/services/cycles.rs` — cycle lifecycle and targets.
- `backend-school/src/modules/supervision/services/templates.rs` — rubric template lifecycle.
- `backend-school/src/modules/supervision/services/observations.rs` — observation reads, requests, and lifecycle.
- `backend-school/src/modules/supervision/services/evaluations.rs` — evaluator assignment and submissions.
- `backend-school/src/modules/supervision/services/reviews_and_reports.rs` — review, approval, acknowledgement, and reports.

**Modify:**

- `backend-school/src/modules/supervision.rs` — register characterization tests.
- `backend-school/src/modules/supervision/services.rs` — reduce to the compatibility facade.
- `backend-school/tests/static_architecture.rs` — protect the facade and child-module boundaries.

## Public Compatibility Surface

The final facade keeps these caller-facing exports:

```rust
mod cycles;
mod evaluations;
mod observations;
mod reviews_and_reports;
mod shared;
mod templates;

pub use cycles::{create_cycle, get_cycle, list_cycles, update_cycle};
pub use evaluations::{replace_observation_evaluators, submit_my_evaluation};
pub use observations::{
    approve_observation_request, cancel_observation, cancel_requested_observation,
    evaluator_availability, get_observation, list_observations,
    observation_timetable_options, request_observation, return_observation_request,
    update_observation, update_requested_observation,
};
pub use reviews_and_reports::{
    acknowledge_observation, approve_observation, certify_observation, cycle_progress,
    cycle_teacher_status, get_observation_review,
};
pub use shared::{
    all_required_evaluators_submitted, average_submitted_evaluator_rating,
    can_transition_observation_status, can_view_observation_results,
    evaluator_conflict_status_codes, manager_can_edit_observation,
    resolve_supervision_target_rule, teacher_can_edit_requested_observation,
    EvaluatorRatingInput, EvaluatorSubmissionState, SupervisionObservationListAccess,
    SupervisionTargetMatch, SupervisionTargetRule,
};
pub use templates::{create_template, get_template, list_templates, update_template};
```

If compilation shows a public helper is used only by child modules, keep the facade export anyway
for compatibility; this phase does not narrow the existing public API.

---

### Task 1: Add Characterization Fixture and Critical Workflow Tests

**Files:**

- Create: `backend-school/src/modules/supervision/services_tests.rs`
- Modify: `backend-school/src/modules/supervision.rs`

**Interfaces:**

- Consumes: current `supervision::services` public API and `test_helpers::{create_test_pool, run_test_migrations, create_test_user}`.
- Produces: database tests that remain unchanged throughout extraction.

- [ ] **Step 1: Register the missing test module**

Add to `supervision.rs`:

```rust
#[cfg(test)]
mod services_tests;
```

Run:

```bash
cd backend-school
cargo test modules::supervision::services_tests --no-run
```

Expected: FAIL because `services_tests.rs` does not exist.

- [ ] **Step 2: Add a deterministic migrated fixture**

Create `services_tests.rs` with:

```rust
use super::models::*;
use super::services;
use crate::test_helpers::{create_test_pool, create_test_user, run_test_migrations};
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

struct SupervisionFixture {
    actor_id: Uuid,
    teacher_id: Uuid,
    evaluator_id: Uuid,
    second_evaluator_id: Uuid,
    academic_year_id: Uuid,
    semester_id: Uuid,
    timetable_entry_id: Uuid,
}

async fn migrated_pool() -> PgPool {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;
    pool
}
```

Add `insert_fixture(&PgPool) -> SupervisionFixture` using synthetic names and UUID-derived codes.
Insert an active academic year/semester, active teacher/evaluators, one classroom course, period,
and timetable entry inside the cycle date range. Do not add production fixture hooks.

- [ ] **Step 3: Characterize cycle and template persistence**

Add:

```rust
#[tokio::test]
async fn cycle_and_template_round_trip_preserves_targets_sections_items_and_steps() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;

    let cycle = create_cycle_fixture(&pool, &fixture).await;
    let template = create_template_fixture(&pool, fixture.actor_id).await;

    assert_eq!(services::get_cycle(&pool, cycle.id).await.unwrap(), cycle);
    assert_eq!(services::get_template(&pool, template.id).await.unwrap(), template);
}
```

The helpers submit two cycle targets and a template with two sections, three items, and two steps.
Assert IDs, order, required flags, item types, and target priority.

- [ ] **Step 4: Characterize request and evaluator workflows**

Add `request_approval_and_evaluator_replacement_preserve_status_and_submitted_evaluators`. It
requests an observation, approves it, submits one evaluator response, then replaces evaluators.
Assert the submitted evaluator remains, the replacement is added once, and the observation
status/action history matches current behavior.

Add `failed_evaluator_replacement_rolls_back_assignments_and_action_rows`. Capture assignment and
action IDs before supplying an unavailable evaluator, assert the current conflict error, then
reload and assert both ID sets are unchanged.

- [ ] **Step 5: Characterize review and reporting**

Add `completed_evaluations_flow_through_certification_approval_acknowledgement_and_reports`.
Submit all required evaluations, certify, approve, and acknowledge through public service calls.
Assert unreleased results are hidden before approval, visible afterward, and cycle
progress/teacher status contain the same observation and average score.

- [ ] **Step 6: Run and commit**

```bash
cd backend-school
cargo fmt --all -- --check
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::supervision::services_tests -- --test-threads=1
cargo test modules::supervision::services::tests
```

Expected: all existing and new supervision tests pass against the monolith.

```bash
git add backend-school/src/modules/supervision.rs backend-school/src/modules/supervision/services_tests.rs
git commit -m "test(supervision): characterize service workflows"
```

---

### Task 2: Extract Shared Workflow Rules

**Files:**

- Create: `backend-school/src/modules/supervision/services/shared.rs`
- Modify: `backend-school/src/modules/supervision/services.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**

- Produces: pure rules and public helper types used by later child modules.

- [ ] **Step 1: Add a failing private-module guard**

Add `supervision_service_uses_private_child_modules` to `static_architecture.rs`:

```rust
#[test]
fn supervision_service_uses_private_child_modules() {
    let facade = read_repo_file("src/modules/supervision/services.rs");
    assert!(facade.contains("mod shared;"));
    assert!(!facade.contains("pub mod shared;"));
    assert!(manifest_dir().join("src/modules/supervision/services/shared.rs").exists());
}
```

Run the test and confirm it fails because the module does not exist.

- [ ] **Step 2: Move the exact shared closure**

Move these types/functions and their unit tests unchanged:

```text
SupervisionTargetRule, SupervisionTargetMatch, EvaluatorRatingInput,
EvaluatorSubmissionState, EvaluatorReplacementState, SupervisionObservationListAccess,
resolve_supervision_target_rule, teacher_can_edit_requested_observation,
manager_can_edit_observation, normalize_evaluator_replacement, has_required_evaluator,
average_submitted_evaluator_rating, can_view_observation_results,
all_required_evaluators_submitted, can_transition_observation_status,
target_rule_matches, target_specificity_rank, evaluator_conflict_status_codes,
parse_* status/type helpers
```

Use `pub(super)` for sibling-only definitions and `pub` for the compatibility exports shown above.

- [ ] **Step 3: Verify and commit**

```bash
cd backend-school
cargo fmt --all -- --check
cargo test modules::supervision::services::shared::tests
cargo test --test static_architecture supervision_service_uses_private_child_modules
cargo check --all-targets
```

```bash
git add backend-school/src/modules/supervision/services.rs backend-school/src/modules/supervision/services/shared.rs backend-school/tests/static_architecture.rs
git commit -m "refactor(supervision): extract shared workflow rules"
```

---

### Task 3: Extract Cycles and Templates

**Files:**

- Create: `backend-school/src/modules/supervision/services/cycles.rs`
- Create: `backend-school/src/modules/supervision/services/templates.rs`
- Modify: `backend-school/src/modules/supervision/services.rs`

**Interfaces:**

- Consumes: parsing and target rules from `shared`.
- Produces: cycle/template public functions plus narrow hydration helpers.

- [ ] **Step 1: Run the round-trip characterization test**

```bash
cd backend-school
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test cycle_and_template_round_trip_preserves_targets_sections_items_and_steps -- --test-threads=1
```

- [ ] **Step 2: Extract cycles**

Move the exact dependency closure for:

```text
list_cycles, get_cycle, create_cycle, update_cycle,
validate_cycle_schedule, validate_cycle_targets, insert_cycle_targets,
load_cycle_targets, load_cycle_targets_by_cycle, cycle_target_from_row,
cycle_from_row, cycle_from_row_with_targets, SupervisionCycleRow,
SupervisionCycleTargetRow
```

Keep target bulk insertion and transaction boundaries unchanged.

- [ ] **Step 3: Extract templates**

Move the exact dependency closure for:

```text
list_templates, get_template, create_template, update_template,
validate_template_input, insert_template_sections,
build_template_section_bulk_rows, bulk_insert_template_sections,
bulk_insert_template_items, insert_template_steps, template_from_rows,
template_item_from_row, template_step_from_row,
TemplateSectionBulkRow, TemplateItemBulkRow,
SupervisionTemplateRow, SupervisionTemplateSectionRow,
SupervisionTemplateItemRow, SupervisionTemplateStepRow
```

Keep the current bulk SQL and ordering unchanged.

- [ ] **Step 4: Verify and commit**

```bash
cd backend-school
cargo fmt --all -- --check
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test cycle_and_template_round_trip_preserves_targets_sections_items_and_steps -- --test-threads=1
cargo test modules::supervision::services::cycles::tests
cargo test modules::supervision::services::templates::tests
cargo check --all-targets
```

```bash
git add backend-school/src/modules/supervision/services.rs backend-school/src/modules/supervision/services/cycles.rs backend-school/src/modules/supervision/services/templates.rs
git commit -m "refactor(supervision): extract cycles and templates"
```

---

### Task 4: Extract Observation Lifecycle

**Files:**

- Create: `backend-school/src/modules/supervision/services/observations.rs`
- Modify: `backend-school/src/modules/supervision/services.rs`

**Interfaces:**

- Consumes: shared transition rules and cycle lookup.
- Produces: observation reads, request lifecycle, lesson resolution, and action insertion helpers.

- [ ] **Step 1: Run request characterization**

Run both request/evaluator characterization tests and confirm they pass before moving code.

- [ ] **Step 2: Extract the observation dependency closure**

Move:

```text
list_observations, get_observation, evaluator_availability,
observation_timetable_options, request_observation, update_requested_observation,
cancel_requested_observation, update_observation, cancel_observation,
approve_observation_request, return_observation_request,
list_observation_rows, load_observation_row, observation_select_sql,
observation_from_row, manual_lesson_from_row, load_observation_evaluators,
load_observation_actions, action_from_row, load_cycle_for_request,
ensure_cycle_target_allows_teacher, load_supervision_target_match,
validate_cycle_accepts_requests, resolve_lesson_input,
validate_observed_at_in_cycle, day_of_week_matches_observed_at,
validate_manual_lesson, load_timetable_entry_day_for_teacher,
load_timetable_lesson_snapshot, set_observation_status,
insert_observation_action and their owned row structs
```

Expose only the smallest `pub(super)` observation hydration/status/action functions needed by
evaluations and reviews.

- [ ] **Step 3: Verify and commit**

```bash
cd backend-school
cargo fmt --all -- --check
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::supervision::services_tests -- --test-threads=1
cargo test modules::supervision::services::observations::tests
cargo check --all-targets
```

```bash
git add backend-school/src/modules/supervision/services.rs backend-school/src/modules/supervision/services/observations.rs
git commit -m "refactor(supervision): extract observation lifecycle"
```

---

### Task 5: Extract Evaluations

**Files:**

- Create: `backend-school/src/modules/supervision/services/evaluations.rs`
- Modify: `backend-school/src/modules/supervision/services.rs`

**Interfaces:**

- Consumes: observation hydration/status/action helpers and shared evaluator rules.
- Produces: evaluator replacement, submission, response hydration, and score helpers.

- [ ] **Step 1: Run evaluation rollback characterization**

Confirm both evaluator replacement tests fail if their public function is temporarily unavailable,
then restore the source before extraction.

- [ ] **Step 2: Extract the evaluation dependency closure**

Move:

```text
replace_observation_evaluators, insert_supervision_evaluators,
save_my_evaluation, submit_my_evaluation, load_evaluator_for_user,
dedupe_evaluation_responses, load_evaluation_item_specs,
build_evaluation_response_bulk_rows, bulk_upsert_evaluation_responses,
load_evaluator_submission_states, validate_evaluator_availability_for_observation,
evaluator_availability_from_row, conflict_lesson_title,
EvaluationItemSpec, EvaluationResponseBulkRow, EvaluatorForUserRow,
EvaluatorAvailabilityRow, EvaluatorConflictRow
```

Move the deduplication, rating-range, required-item, and availability tests with the code.

- [ ] **Step 3: Verify and commit**

```bash
cd backend-school
cargo fmt --all -- --check
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::supervision::services_tests -- --test-threads=1
cargo test modules::supervision::services::evaluations::tests
cargo test --test static_architecture teaching_supervision_services_use_bulk_mutations_for_multi_row_writes
```

Update the static bulk guard to read `services/evaluations.rs` and `services/templates.rs`.

```bash
git add backend-school/src/modules/supervision/services.rs backend-school/src/modules/supervision/services/evaluations.rs backend-school/tests/static_architecture.rs
git commit -m "refactor(supervision): extract evaluation workflows"
```

---

### Task 6: Extract Reviews and Reports, Then Seal the Facade

**Files:**

- Create: `backend-school/src/modules/supervision/services/reviews_and_reports.rs`
- Modify: `backend-school/src/modules/supervision/services.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**

- Consumes: observation/evaluation hydration and shared result rules.
- Produces: final thin facade and protected public surface.

- [ ] **Step 1: Extract the remaining workflow closure**

Move:

```text
get_observation_review, certify_observation, approve_observation,
acknowledge_observation, cycle_progress, cycle_teacher_status,
load_observation_review_responses, build_review_evaluator_results,
build_review_item_summaries, average_rating_from_scores,
fetch_observation_average_rating, load_evaluator_submission_states,
teacher_status_from_row, teacher_status_next_step_label,
SupervisionReviewResponseRow, TeacherStatusOverviewRow
```

If `load_evaluator_submission_states` remains owned by evaluations, expose it narrowly as
`pub(super)` rather than duplicating it.

- [ ] **Step 2: Add a failing facade-surface guard**

The guard reads `services.rs`, asserts every child declaration and public re-export listed in this
plan, rejects `sqlx::`, `.fetch_`, `.execute(`, and `.begin(`, and caps nonblank facade lines at 80.
Run it once before reducing the facade and confirm it fails.

Update every existing supervision architecture test that reads implementation strings from
`services.rs` to read the owning child file or an explicit concatenation of all six child files.
Keep route/handler checks pointed at their current files.

- [ ] **Step 3: Reduce the facade and run verification**

Replace the facade with the exact public compatibility surface from this plan. Then run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::supervision -- --test-threads=1
cargo test --test static_architecture supervision -- --test-threads=1
```

- [ ] **Step 4: Commit**

```bash
git add backend-school/src/modules/supervision/services.rs backend-school/src/modules/supervision/services/reviews_and_reports.rs backend-school/tests/static_architecture.rs
git commit -m "refactor(supervision): finish modular service facade"
```

---

### Task 7: Full Phase Verification

**Files:**

- Modify only if verification exposes a real regression.

- [ ] **Step 1: Run the complete backend gate**

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test --bin backend-school -- --test-threads=1
cargo test --test static_architecture -- --test-threads=1
```

- [ ] **Step 2: Audit scope**

```bash
git diff --check
git diff --name-only HEAD~6..HEAD
rg -n 'sqlx::|\.fetch_|\.execute\(|\.begin\(' backend-school/src/modules/supervision/services.rs
```

Expected: no frontend, migration, permission, or generated contract changes; the final `rg`
produces no matches.

- [ ] **Step 3: Record checkpoint**

```bash
git status --short
git log --oneline -7
```

Expected: clean worktree with the supervision phase represented by focused commits.
