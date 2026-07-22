# Exam Schedule Service Modularization Implementation Plan

> **For agentic workers:** Execute this plan one task at a time. Keep every extraction behavior-preserving, run the listed verification before each commit, and stop if SQL, bind order, transaction boundaries, advisory-lock order, errors, or public API behavior would change.

**Goal:** Split the 4,876-line exam-schedule service into cohesive private modules while preserving its existing public Rust API, HTTP/OpenAPI contracts, database behavior, and concurrency guarantees.

**Architecture:** Keep `exam_schedule_service.rs` as a compatibility facade. Move implementation into eight private child modules under `services/exam_schedule_service/`, expose only narrow `pub(super)` sibling interfaces, and re-export the 23 currently consumed public functions from the facade. Add PostgreSQL characterization tests before moving high-risk workflows and retain focused unit/static tests beside their owning behavior.

**Tech stack:** Rust 2021, Axum 0.8, sqlx 0.8/PostgreSQL, Tokio tests, Utoipa/OpenAPI, Cargo formatting/lint/test tooling.

**Approved design:** `docs/superpowers/specs/2026-07-23-exam-schedule-service-modularization-design.md`

## Global constraints

- Do not change routes, DTOs, permission checks, HTTP statuses, error messages, WebSocket behavior, or generated API contracts.
- Do not create or modify a database migration.
- During extraction, preserve SQL text, bind order, transaction scope, row locks, advisory-lock keys, and lock-before-read ordering exactly.
- Do not add repository traits, mocks, dynamic dispatch, caching, query rewrites, or unrelated cleanup.
- External callers must continue importing `exam_schedule_service::function_name`; child modules remain private.
- Keep `backend-school/src/modules/academic/models/exam_schedule.rs` as the model owner.
- Use `TEST_DATABASE_URL` only for database-backed tests. Never embed credentials.
- Each task ends with a focused commit. If a listed test fails before the task begins, diagnose it before editing.

## Target file map

**Create:**

- `backend-school/src/modules/academic/services/exam_schedule_service_tests.rs` — PostgreSQL characterization tests for cross-module workflows.
- `backend-school/src/modules/academic/services/exam_schedule_service/shared.rs` — pure shared time/conflict/lock primitives.
- `backend-school/src/modules/academic/services/exam_schedule_service/rounds_and_days.rs` — round/day lifecycle and draft transition.
- `backend-school/src/modules/academic/services/exam_schedule_service/workspace.rs` — admin workspace, import, cleanup, readiness.
- `backend-school/src/modules/academic/services/exam_schedule_service/room_assignments.rs` — room assignments, capacity, seats, room views.
- `backend-school/src/modules/academic/services/exam_schedule_service/invigilation.rs` — invigilator assignment, workload, locking, conflicts.
- `backend-school/src/modules/academic/services/exam_schedule_service/sessions_and_conflicts.rs` — placement/deletion and scheduling conflicts.
- `backend-school/src/modules/academic/services/exam_schedule_service/publishing.rs` — publish transaction and readiness gate.
- `backend-school/src/modules/academic/services/exam_schedule_service/published_views.rs` — student/staff/parent published views.

**Modify:**

- `backend-school/src/modules/academic/services.rs` — register the characterization test module.
- `backend-school/src/modules/academic/services/exam_schedule_service.rs` — progressively reduce to the compatibility facade.
- `backend-school/tests/static_architecture.rs` — enforce the facade/child-module boundary without checking implementation wording.

## Public compatibility surface

The final facade must re-export exactly these externally consumed functions:

```rust
mod invigilation;
mod published_views;
mod publishing;
mod room_assignments;
mod rounds_and_days;
mod sessions_and_conflicts;
mod shared;
mod workspace;

pub use invigilation::{
    assign_invigilator_to_assignment, get_invigilator_workspace,
    list_invigilator_staff_options, remove_invigilator_from_assignment,
    update_assignment_invigilators,
};
pub use published_views::{
    list_child_published_exam_schedule, list_my_published_exam_schedule,
    list_staff_published_exam_schedule,
};
pub use publishing::publish_round;
pub use room_assignments::{
    generate_seats_for_assignment, list_day_room_assignments,
    upsert_day_room_assignment,
};
pub use rounds_and_days::{
    create_round, delete_exam_day, list_rounds, update_exam_day, update_round, upsert_exam_day,
};
pub use sessions_and_conflicts::{delete_exam_session, place_exam_session};
pub use workspace::{clear_mismatched_exam_items, get_workspace, import_exam_items};
```

`shared.rs` types and helpers are internal implementation details and must not be re-exported unless direct repository search proves an external caller exists.

---

### Task 1: Add the PostgreSQL characterization fixture

**Files:**

- Create: `backend-school/src/modules/academic/services/exam_schedule_service_tests.rs`
- Modify: `backend-school/src/modules/academic/services.rs`

**Step 1: Register the test module**

Add this beside the existing academic service test modules:

```rust
#[cfg(test)]
mod exam_schedule_service_tests;
```

Run:

```bash
cd backend-school
cargo test exam_schedule_service_tests --no-run
```

Expected: compilation fails because the new file does not exist. This confirms the test is registered.

**Step 2: Build a deterministic fixture in the new test file**

Import `create_test_pool`, `run_test_migrations`, the exam-schedule models needed by the public calls, `AppError`, `Uuid`, `NaiveDate`, and `NaiveTime`. Define:

```rust
struct ExamScheduleFixture {
    academic_year_id: Uuid,
    semester_id: Uuid,
    grade_level_id: Uuid,
    classroom_id: Uuid,
    second_classroom_id: Uuid,
    subject_id: Uuid,
    second_subject_id: Uuid,
    course_id: Uuid,
    second_course_id: Uuid,
    third_course_id: Uuid,
    assessment_plan_id: Uuid,
    second_assessment_plan_id: Uuid,
    assessment_category_id: Uuid,
    second_assessment_category_id: Uuid,
    room_id: Uuid,
    second_room_id: Uuid,
    student_user_id: Uuid,
    second_student_user_id: Uuid,
    staff_user_id: Uuid,
    parent_user_id: Uuid,
}
```

Provide these helpers with explicit responsibilities:

- `migrated_pool()` creates the isolated schema pool and runs all migrations.
- `insert_active_user(pool, user_type, label)` inserts an active test user with a unique UUID and no real personal data.
- `insert_fixture(pool)` inserts one future academic year and semester, one existing baseline grade level, one study plan/version, two active classrooms, two subjects, three classroom courses, two subject-level assessment plans, two `midterm`/`in_timetable` categories with a 60-minute duration, one active student enrollment per classroom, one active staff user, one parent user linked to the first student, and two active facility rooms. Course layout is `(classroom 1, subject 1)`, `(classroom 1, subject 2)`, and `(classroom 2, subject 1)` so one import produces two same-classroom items for concurrency tests and items across two classrooms for invigilator tests. Use UUID-derived codes to avoid collisions.
- `create_round_with_day(pool, fixture)` calls the current public `create_round` and `upsert_exam_day`, then returns `(round_id, exam_day_id)` from the service responses.
- `import_items(pool, round_id, fixture)` calls `import_exam_items`, asserts three items were imported, and returns item IDs keyed by `(classroom_id, subject_id)` after querying `academic_exam_schedule_items` for the round.
- `assign_room(pool, exam_day_id, fixture)` calls `upsert_day_room_assignment` and returns the resulting assignment ID.

Use only columns confirmed from the current migrations. Keep setup SQL inside this test module; do not change production code to make fixture insertion easier.

Add `fixture_builds_all_prerequisites`, which calls `insert_fixture`, then asserts the three courses join the expected classrooms/subjects/semester, each classroom has one active enrollment, both assessment categories join their expected subject-level plans and expose `midterm` plus a 60-minute duration, the parent-child row links the fixture IDs, and both facility rooms are active. This catches an incomplete fixture before workflow assertions obscure the setup failure.

**Step 3: Prove the fixture compiles and runs**

Run:

```bash
cd backend-school
cargo fmt --check
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test exam_schedule_service_tests -- --test-threads=1
```

Expected: the registered module compiles and its fixture smoke test passes. If `TEST_DATABASE_URL` is absent, record the DB verification as pending; do not silently replace PostgreSQL with mocks.

**Step 4: Commit**

```bash
git add backend-school/src/modules/academic/services.rs \
  backend-school/src/modules/academic/services/exam_schedule_service_tests.rs
git commit -m "test(exams): add schedule service fixture"
```

---

### Task 2: Characterize critical workflows before extraction

**Files:**

- Modify: `backend-school/src/modules/academic/services/exam_schedule_service_tests.rs`

**Step 1: Add round/day lifecycle coverage**

Add `round_and_day_lifecycle_preserves_identity_and_draft_rules`:

1. Create a round and day through public service calls.
2. Update the same day to a different valid time window.
3. Assert the day UUID is unchanged and `list_rounds` returns the updated values.
4. Update the round name/kind and assert the same round UUID remains.
5. Delete the day and assert it is absent from the round detail.

**Step 2: Add room/seat coverage**

Add `room_assignment_and_seat_generation_preserve_capacity_rules`:

1. Create a round/day, import the single course item, and create a room assignment.
2. Generate seats and assert the active student receives seat `01`.
3. Reduce effective capacity below the student count and assert the existing `AppError::BadRequest` message.
4. Assert the failed capacity update leaves the prior assignment and seat unchanged.

**Step 3: Add invigilator conflict coverage**

Add `invigilator_assignment_rejects_overlapping_live_sessions`:

1. Create two room assignments on one day.
2. Assign the same staff member to the first room.
3. Place an exam session in each room with overlapping live ranges.
4. Attempt to assign that staff member to the second room.
5. Assert the existing conflict error variant/message and assert the first assignment remains intact.

**Step 4: Add placement lock/concurrency coverage**

Add `concurrent_overlapping_placements_allow_only_one_commit`:

1. Prepare two imported items targeting the same classroom/day with overlapping times.
2. Synchronize two Tokio tasks with `tokio::sync::Barrier` and call `place_exam_session` concurrently through cloned pools.
3. Assert exactly one call succeeds and one returns the existing conflict error.
4. Query `exam_sessions` and assert exactly one row exists for the two items.

Do not weaken the assertion to “at least one succeeds”; the purpose is to freeze advisory-lock behavior.

**Step 5: Add publish and published-view coverage**

Add `publish_exposes_the_same_session_to_student_staff_and_linked_parent`:

1. Build a ready round: day, imported item, room assignment, seat, invigilator, and valid session.
2. Call `publish_round` and assert the round status is `published`.
3. Call `list_my_published_exam_schedule` for the student user, `list_staff_published_exam_schedule` for the staff user, and `list_child_published_exam_schedule` for the linked parent/student pair.
4. Assert all three views contain the same round/session IDs and the role-appropriate room, seat, subject, and invigilation data.
5. Call the parent view with an unrelated parent UUID and assert the existing forbidden/not-found behavior exactly.

**Step 6: Run the characterization suite**

```bash
cd backend-school
cargo fmt --check
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test exam_schedule_service_tests -- --test-threads=1
cargo test exam_schedule_service::tests
```

Expected: all new DB tests and the existing focused tests pass against the monolithic implementation.

**Step 7: Commit**

```bash
git add backend-school/src/modules/academic/services/exam_schedule_service_tests.rs
git commit -m "test(exams): characterize schedule workflows"
```

---

### Task 3: Establish the first private child-module boundary and extract shared primitives

**Files:**

- Create: `backend-school/src/modules/academic/services/exam_schedule_service/shared.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Step 1: Add a failing shared-module boundary guard**

In `static_architecture.rs`, add a test named `exam_schedule_shared_module_is_private`. It must:

- Read `src/modules/academic/services/exam_schedule_service.rs`.
- Assert `mod shared;` exists.
- Assert `pub mod shared;` does not exist.
- Assert `src/modules/academic/services/exam_schedule_service/shared.rs` exists.

Run:

```bash
cd backend-school
cargo test --test static_architecture exam_schedule_shared_module_is_private
```

Expected: FAIL because `shared.rs` and its private declaration do not exist yet.

**Step 2: Extract only pure shared primitives**

Move these definitions and their focused tests into `shared.rs`:

- `SessionValidationError`
- `add_minutes`
- `time_ranges_overlap`
- `is_exam_session_start_on_slot`
- `CandidateSession`
- `InvigilatorSessionWindow`
- `invigilator_workload_minutes`
- `minutes_between_times`
- `has_invigilator_time_conflict`
- `has_same_classroom_conflict`
- `has_same_room_conflict`
- `exam_session_conflict_lock_key`
- `exam_session_conflict_lock_keys`
- `exam_invigilator_staff_lock_keys`
- `validate_session_window`
- `validation_error_to_app_error`
- `unique_uuids`

Use `pub(super)` only for items referenced by sibling modules. Leave SQL lock-acquisition functions with the use case that owns the transaction until Tasks 7–8. Preserve formulas, constants, ordering, deduplication, and error strings unchanged.

Move the corresponding time overlap, duration, lock-key, placement-window, grade-independent conflict, and workload unit tests into `shared.rs` under `#[cfg(test)]`.

**Step 3: Make the boundary guard pass**

Run:

```bash
cd backend-school
cargo fmt --check
cargo test exam_schedule_service::shared::tests
cargo test --test static_architecture exam_schedule_shared_module_is_private
```

Expected: both commands PASS. Do not commit a failing structural test while the remaining modules are extracted incrementally.

**Step 4: Commit**

```bash
git add backend-school/src/modules/academic/services/exam_schedule_service.rs \
  backend-school/src/modules/academic/services/exam_schedule_service/shared.rs \
  backend-school/tests/static_architecture.rs
git commit -m "refactor(exams): extract shared scheduling primitives"
```

---

### Task 4: Extract published read views

**Files:**

- Create: `backend-school/src/modules/academic/services/exam_schedule_service/published_views.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service.rs`

**Step 1: Establish the pre-extraction baseline**

```bash
cd backend-school
cargo test exam_schedule_service_tests::publish_exposes_the_same_session_to_student_staff_and_linked_parent -- --test-threads=1
```

Expected: PASS.

**Step 2: Move the complete published-view slice**

Move, without rewriting SQL, these public functions and their private dependency closure:

- `list_my_published_exam_schedule`
- `list_staff_published_exam_schedule`
- `list_child_published_exam_schedule`
- `ensure_active_student_user`
- `ensure_active_staff_user_for_exam_schedule`
- `ensure_parent_user_for_exam_schedule`
- `ensure_parent_student_link_for_exam_schedule`
- `list_published_exam_schedule_for_student`
- `list_published_exam_schedule_for_staff`
- `group_personal_exam_schedule_rows`
- the private published-row structs and their `into_*` conversions used only by these functions

Declare the three caller-facing functions `pub(super)` and re-export them from the facade. Keep validation order and all query/bind order unchanged.

**Step 3: Verify**

```bash
cd backend-school
cargo fmt --check
cargo test exam_schedule_service_tests::publish_exposes_the_same_session_to_student_staff_and_linked_parent -- --test-threads=1
cargo test exam_schedule_service::published_views::tests
cargo check --all-targets
```

Expected: PASS. The last command proves existing handlers and parent services still compile through the facade.

**Step 4: Commit**

```bash
git add backend-school/src/modules/academic/services/exam_schedule_service.rs \
  backend-school/src/modules/academic/services/exam_schedule_service/published_views.rs
git commit -m "refactor(exams): extract published schedule views"
```

---

### Task 5: Extract round and day lifecycle

**Files:**

- Create: `backend-school/src/modules/academic/services/exam_schedule_service/rounds_and_days.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service.rs`

**Step 1: Run the lifecycle characterization test**

```bash
cd backend-school
cargo test exam_schedule_service_tests::round_and_day_lifecycle_preserves_identity_and_draft_rules -- --test-threads=1
```

Expected: PASS.

**Step 2: Move the complete lifecycle slice**

Move these functions and their private row/context structs:

- `list_rounds`, `create_round`, `update_round`
- `upsert_exam_day`, `update_exam_day`, `replace_exam_day_configuration`, `delete_exam_day`
- `mark_round_draft_after_mutation`
- `ensure_exam_round_is_mutable`
- `fetch_exam_day_context_for_update`
- `map_exam_day_write_error`
- `validate_exam_day_window`
- `normalize_exam_kind`
- `normalize_update_round_request`
- `normalize_blocked_windows`
- `fetch_round`
- `fetch_exam_day_detail`, `fetch_exam_day_details_for_round`, `hydrate_exam_day_details`

Expose `mark_round_draft_after_mutation`, `ensure_exam_round_is_mutable`, and the minimum fetch function needed by publishing as `pub(super)`. Re-export only the six public lifecycle calls listed in the public compatibility surface.

Move the round-kind, blocked-window, update-request, published-round guard, day identity, occupied-date mapping, and relevant route/source-shape tests into this module. Source-shape tests must read `rounds_and_days.rs`, not the facade.

**Step 3: Verify**

```bash
cd backend-school
cargo fmt --check
cargo test exam_schedule_service::rounds_and_days::tests
cargo test exam_schedule_service_tests::round_and_day_lifecycle_preserves_identity_and_draft_rules -- --test-threads=1
cargo check --all-targets
```

Expected: PASS.

**Step 4: Commit**

```bash
git add backend-school/src/modules/academic/services/exam_schedule_service.rs \
  backend-school/src/modules/academic/services/exam_schedule_service/rounds_and_days.rs
git commit -m "refactor(exams): extract round and day lifecycle"
```

---

### Task 6: Extract workspace, import, cleanup, and readiness

**Files:**

- Create: `backend-school/src/modules/academic/services/exam_schedule_service/workspace.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service.rs`

**Step 1: Run the current workspace/readiness tests**

```bash
cd backend-school
cargo test exam_schedule_service::tests::import_exam_items_filters_source_categories_by_round_kind
cargo test exam_schedule_service::tests::readiness_requires_days_items_rooms_and_sessions
```

Expected: PASS before moving code.

**Step 2: Move the workspace slice**

Move these functions, associated SQL constants, and workspace-only row structs:

- `WorkspaceCounts`
- `build_readiness`
- `get_workspace`
- `import_exam_items`
- `clear_mismatched_exam_items`
- `fetch_workspace_counts_in_tx`
- `workspace_counts_from_row`
- `fetch_unscheduled_items`
- `fetch_scheduled_sessions`
- `fetch_workspace_counts`

Also move any hydration helper used only by `get_workspace`. When hydration needs room/invigilator data, call a narrow `pub(super)` sibling helper after Tasks 7–8 rather than duplicating its SQL. Until then, move the exact current dependency closure with the workspace slice and relocate it later in the owning extraction task.

Expose `fetch_workspace_counts_in_tx` and `build_readiness` as `pub(super)` for publishing. Re-export only `get_workspace`, `import_exam_items`, and `clear_mismatched_exam_items`.

Move all import-filter, mismatched-item, readiness-SQL, and readiness-count tests into `workspace.rs`. Update `include_str!` or source-path assertions to target this child file.

**Step 3: Verify**

```bash
cd backend-school
cargo fmt --check
cargo test exam_schedule_service::workspace::tests
cargo test exam_schedule_service_tests -- --test-threads=1
cargo check --all-targets
```

Expected: PASS.

**Step 4: Commit**

```bash
git add backend-school/src/modules/academic/services/exam_schedule_service.rs \
  backend-school/src/modules/academic/services/exam_schedule_service/workspace.rs
git commit -m "refactor(exams): extract workspace workflows"
```

---

### Task 7: Extract room assignments and seats

**Files:**

- Create: `backend-school/src/modules/academic/services/exam_schedule_service/room_assignments.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service/workspace.rs`

**Step 1: Run the room/seat characterization test**

```bash
cd backend-school
cargo test exam_schedule_service_tests::room_assignment_and_seat_generation_preserve_capacity_rules -- --test-threads=1
```

Expected: PASS.

**Step 2: Move the complete room slice**

Move these functions, private context/row structs, and their exact queries:

- `SeatStudent`, `SeatAssignmentDraft`, `build_default_seat_assignments`
- `validate_seat_generation_capacity`
- `list_day_room_assignments`
- `upsert_day_room_assignment`
- `generate_seats_for_assignment`
- `fetch_classroom_assignment_context`
- `fetch_room_assignment_context`
- `count_active_classroom_students`
- `fetch_day_room_assignment_views_for_day`
- `fetch_day_room_assignment_view`
- `hydrate_day_room_assignment_views`
- `fetch_invigilator_views_by_assignment_ids` only if room response hydration owns it; otherwise call the invigilation helper introduced in Task 8
- `fetch_seat_assignment_context`
- `fetch_seat_assignments_for_assignment`
- `fetch_ordered_seat_students`
- `map_day_room_assignment_write_error`
- `validate_capacity_override`

Expose only the minimum placement contexts and room-view hydration functions as `pub(super)`. Re-export the three public room calls. `upsert_day_room_assignment` may call narrow `pub(super)` invigilation operations once Task 8 owns them.

Do not move staff-level assignment/move/remove orchestration into this file; that belongs to `invigilation.rs`.

Move payload compatibility, capacity, seat-order, day-room lock-order, and seat-generation unit/static tests into this module.

**Step 3: Verify**

```bash
cd backend-school
cargo fmt --check
cargo test exam_schedule_service::room_assignments::tests
cargo test exam_schedule_service_tests::room_assignment_and_seat_generation_preserve_capacity_rules -- --test-threads=1
cargo test exam_schedule_service_tests -- --test-threads=1
cargo check --all-targets
```

Expected: PASS.

**Step 4: Commit**

```bash
git add backend-school/src/modules/academic/services/exam_schedule_service.rs \
  backend-school/src/modules/academic/services/exam_schedule_service/room_assignments.rs \
  backend-school/src/modules/academic/services/exam_schedule_service/workspace.rs
git commit -m "refactor(exams): extract room and seat workflows"
```

---

### Task 8: Extract invigilation workflows

**Files:**

- Create: `backend-school/src/modules/academic/services/exam_schedule_service/invigilation.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service/room_assignments.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service/workspace.rs`

**Step 1: Run invigilation baselines**

```bash
cd backend-school
cargo test exam_schedule_service_tests::invigilator_assignment_rejects_overlapping_live_sessions -- --test-threads=1
cargo test exam_schedule_service::tests::exam_invigilator_staff_lock_keys_are_sorted_deduped_and_stable
```

Expected: PASS.

**Step 2: Move the invigilation slice**

Move these functions and their dependency closure:

- `get_invigilator_workspace`
- `list_invigilator_staff_options`
- `update_assignment_invigilators`
- `assign_invigilator_to_assignment`
- `remove_invigilator_from_assignment`
- `replace_assignment_invigilators_in_tx`
- `delete_staff_invigilator_from_other_day_assignments_in_tx`
- `insert_staff_invigilator_if_missing_in_tx`
- `delete_staff_invigilator_from_assignment_in_tx`
- `lock_exam_invigilator_staff_conflict_scope`
- `validate_active_staff_users`
- `fetch_assignment_session_windows`
- `fetch_existing_invigilator_session_windows`
- `validate_invigilator_time_conflicts`
- `build_invigilator_candidate_session_windows`
- `fetch_invigilator_staff_ids_for_assignment`
- `validate_invigilator_candidate_session_conflicts`
- `fetch_invigilator_assignment_summaries`
- `fetch_invigilator_staff_workloads`
- `build_invigilator_staff_workloads`
- `fetch_invigilator_views_by_assignment_ids`
- `fetch_invigilator_assignment_mutation_context_for_update`
- `fetch_invigilators_by_assignment_ids`
- `invigilators_for_assignment`
- `validate_unique_invigilator_staff_ids`
- `invigilator_staff_option_limit`
- `invigilator_staff_option_search_pattern`

Keep lock-key calculation in `shared.rs`; keep actual SQL advisory-lock acquisition here. Expose narrow `pub(super)` conflict, lock, workload, and hydration functions needed by room/session/workspace modules. Re-export the five public invigilation calls.

Move all invigilator workload, conflict, staff-move semantics, remove semantics, lock-order, staff-option normalization, workspace query-order, handler-route, and candidate-window tests into this module. Update source-path assertions to inspect `invigilation.rs`.

**Step 3: Verify**

```bash
cd backend-school
cargo fmt --check
cargo test exam_schedule_service::invigilation::tests
cargo test exam_schedule_service_tests::invigilator_assignment_rejects_overlapping_live_sessions -- --test-threads=1
cargo test exam_schedule_service_tests -- --test-threads=1
cargo check --all-targets
```

Expected: PASS.

**Step 4: Commit**

```bash
git add backend-school/src/modules/academic/services/exam_schedule_service.rs \
  backend-school/src/modules/academic/services/exam_schedule_service/invigilation.rs \
  backend-school/src/modules/academic/services/exam_schedule_service/room_assignments.rs \
  backend-school/src/modules/academic/services/exam_schedule_service/workspace.rs
git commit -m "refactor(exams): extract invigilation workflows"
```

---

### Task 9: Extract session placement, deletion, and conflicts

**Files:**

- Create: `backend-school/src/modules/academic/services/exam_schedule_service/sessions_and_conflicts.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service.rs`

**Step 1: Run concurrency and placement baselines**

```bash
cd backend-school
cargo test exam_schedule_service_tests::concurrent_overlapping_placements_allow_only_one_commit -- --test-threads=1
cargo test exam_schedule_service::tests::placement_locks_conflict_scope_before_conflict_queries
```

Expected: PASS.

**Step 2: Move the complete session slice**

Move these functions, placement context structs, and queries:

- `place_exam_session`
- `delete_exam_session`
- `lock_exam_session_conflict_scope`
- `fetch_schedule_item_placement_context`
- `fetch_exam_day_placement_context`
- `fetch_blocked_windows_for_day_for_placement`
- `fetch_day_room_assignment_placement_context`
- `fetch_existing_session_id_for_item`
- `fetch_candidate_sessions_for_day`
- `fetch_candidate_room_sessions_for_day`
- `fetch_exam_session_view`
- `validate_day_allows_grade_level`
- `grade_level_allowed_by_day_scope`

Import conflict primitives from `shared`, room contexts from `room_assignments`, invigilator conflict validation from `invigilation`, and `mark_round_draft_after_mutation` from `rounds_and_days`.

Preserve this transaction order exactly: begin transaction → lock deterministic conflict scope → fetch/validate current state → perform write → mark round draft → commit. Do not move validation before the locks where the current implementation performs it after locking.

Move placement conflict, lock-order, five-minute-slot, day-window, blocked-window, grade-scope, and shared-assignment-invigilator tests into this module. Update source inspection to this child file.

**Step 3: Verify**

```bash
cd backend-school
cargo fmt --check
cargo test exam_schedule_service::sessions_and_conflicts::tests
cargo test exam_schedule_service_tests::concurrent_overlapping_placements_allow_only_one_commit -- --test-threads=1
cargo test exam_schedule_service_tests -- --test-threads=1
cargo check --all-targets
```

Expected: PASS, including exactly one successful concurrent placement.

**Step 4: Commit**

```bash
git add backend-school/src/modules/academic/services/exam_schedule_service.rs \
  backend-school/src/modules/academic/services/exam_schedule_service/sessions_and_conflicts.rs
git commit -m "refactor(exams): extract session conflict workflows"
```

---

### Task 10: Extract publishing and finish the compatibility facade

**Files:**

- Create: `backend-school/src/modules/academic/services/exam_schedule_service/publishing.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service.rs`

**Step 1: Run publish baseline**

```bash
cd backend-school
cargo test exam_schedule_service_tests::publish_exposes_the_same_session_to_student_staff_and_linked_parent -- --test-threads=1
cargo test exam_schedule_service::tests::publish_round_locks_round_before_readiness_check
```

Expected: PASS.

**Step 2: Move publishing without changing lock/readiness order**

Move `publish_round` and any publish-only row type/query into `publishing.rs`. Use the narrow `workspace::fetch_workspace_counts_in_tx` and `workspace::build_readiness` interfaces. Import the minimum round fetch/mutability interface from `rounds_and_days`.

Preserve this order exactly: begin transaction → lock round row → re-read/check readiness in the same transaction → update status → commit. Keep the current error variants and messages.

Move `publish_round_locks_round_before_readiness_check` into `publishing.rs` and point source inspection at the new file.

**Step 3: Replace the root file with the final facade**

Before changing the facade, add `exam_schedule_service_uses_a_thin_private_module_facade` to `static_architecture.rs`. It must:

- Assert all eight `mod name;` declarations exist and none is `pub mod`.
- Assert every expected child file exists.
- Assert the facade contains no `sqlx::query`, `pool.begin()`, `SELECT `, or `#[derive(sqlx::FromRow)]` implementation text.
- Assert the facade is at most 90 non-blank lines.

Run the new guard once and confirm it fails against the still-partially-monolithic facade. Then perform the facade reduction below and rerun it in Step 4; do not commit the failing state.

Reduce `exam_schedule_service.rs` to the eight private `mod` declarations and the public re-exports shown in “Public compatibility surface.” Do not leave model aliases, SQL, transaction helpers, or business rules in the facade.

Run a direct surface audit:

```bash
rg -o 'exam_schedule_service::[A-Za-z0-9_]+' backend-school/src \
  --glob '!modules/academic/services/exam_schedule_service.rs' \
  | sed 's/.*exam_schedule_service:://' | sort -u
```

Expected: every result is present in the facade re-export list, with no caller changed to a child-module path.

**Step 4: Verify the final structure**

```bash
cd backend-school
cargo fmt --check
cargo test exam_schedule_service::publishing::tests
cargo test exam_schedule_service_tests -- --test-threads=1
cargo test --test static_architecture exam_schedule_service_uses_a_thin_private_module_facade
cargo check --all-targets
```

Expected: all PASS, including the structural guard that intentionally failed since Task 3.

**Step 5: Commit**

```bash
git add backend-school/src/modules/academic/services/exam_schedule_service.rs \
  backend-school/src/modules/academic/services/exam_schedule_service/publishing.rs \
  backend-school/tests/static_architecture.rs
git commit -m "refactor(exams): finish modular service facade"
```

---

### Task 11: Full regression, contract, and diff verification

**Files:**

- Modify only if verification finds a refactor-caused issue; keep any correction in its owning child module.

**Step 1: Run formatting and compile checks**

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: all commands exit 0 with no warnings.

**Step 2: Run backend tests**

```bash
cd backend-school
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test --bin backend-school -- --test-threads=1
cargo test --test static_architecture
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test exam_schedule_service_tests -- --test-threads=1
```

Expected: all tests pass. Database tests must run against PostgreSQL when credentials are available; absence of `TEST_DATABASE_URL` must be reported explicitly.

**Step 3: Check API and frontend contract generation**

```bash
cd frontend-school
npm run check:api-contracts
npm run check
```

Expected: both commands exit 0 and no generated contract file changes.

**Step 4: Audit scope and unsafe drift**

From the repository root:

```bash
git diff --check 629e2423..HEAD
git diff --name-only 629e2423..HEAD
git diff -- backend-school/migrations frontend-school/src
rg -n 'pub mod (shared|rounds_and_days|workspace|room_assignments|invigilation|sessions_and_conflicts|publishing|published_views)' \
  backend-school/src/modules/academic/services/exam_schedule_service.rs
```

Expected:

- `git diff --check` is clean.
- Changed implementation paths are limited to the exam-schedule service, its tests, `services.rs`, static architecture tests, and this documentation series.
- Migration and frontend diffs are empty.
- The `pub mod` search returns no matches because all child modules are private.

Review the full diff and compare every moved SQL block, `.bind(...)` sequence, `begin/commit`, and advisory-lock call with commit `629e2423`. Any semantic change must be reverted or split into a separately approved follow-up.

**Step 5: Request review**

Ask the reviewer to focus on:

- exact facade compatibility for all 23 callers;
- accidental visibility expansion;
- circular or overly broad sibling dependencies;
- SQL/bind/transaction drift;
- advisory-lock ordering in session and invigilator workflows;
- readiness checked while holding the publish lock;
- characterization-test strength rather than implementation wording.

Address only verified findings, rerun the affected focused test, then rerun Steps 1–4.

**Step 6: Final commit if review produced corrections**

If and only if review required changes:

```bash
git add backend-school/src/modules/academic/services \
  backend-school/tests/static_architecture.rs
git commit -m "fix(exams): address modularization review"
```

Do not merge or push in this task unless the user separately authorizes integration.
