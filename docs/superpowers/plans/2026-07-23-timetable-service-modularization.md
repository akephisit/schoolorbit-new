# Timetable Service Modularization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split `timetable_service.rs` into cohesive private modules without changing timetable APIs, conflicts, transactions, realtime behavior, or shared admin/self/parent data semantics.

**Architecture:** Preserve `academic::services::timetable_service` as the public facade. Add database characterization tests first, extract read/validation/mutation slices behind private child modules, and keep all caller-facing functions and outcome types re-exported from the facade.

**Tech Stack:** Rust 2021, Axum 0.8, sqlx 0.8/PostgreSQL, Tokio tests, WebSocket/static architecture guards.

**Approved design:** `docs/superpowers/specs/2026-07-23-timetable-service-modularization-design.md`

## Global Constraints

- Preserve every route, DTO, permission, HTTP status, conflict message, sequence value, transaction, and lock.
- Preserve manual editing, team teaching, activity visibility, optimistic response, and WebSocket behavior.
- Do not add migrations, endpoints, query rewrites, caches, scheduler behavior, or frontend changes.
- Keep `/api/me/timetable`, `/api/academic/timetable`, and the parent timetable path on one `list_entries` service source.
- External callers continue using `timetable_service::...`; child modules are private.
- Use `TEST_DATABASE_URL` only for database tests.
- Each extraction ends with focused tests and a commit.

## Target File Map

**Create:**

- `backend-school/src/modules/academic/services/timetable_service_tests.rs`
- `backend-school/src/modules/academic/services/timetable_service/shared.rs`
- `backend-school/src/modules/academic/services/timetable_service/entries.rs`
- `backend-school/src/modules/academic/services/timetable_service/validation.rs`
- `backend-school/src/modules/academic/services/timetable_service/instructors.rs`
- `backend-school/src/modules/academic/services/timetable_service/moves_and_swaps.rs`
- `backend-school/src/modules/academic/services/timetable_service/occupancy.rs`
- `backend-school/src/modules/academic/services/timetable_service/batch_mutations.rs`

**Modify:**

- `backend-school/src/modules/academic/services.rs`
- `backend-school/src/modules/academic/services/timetable_service.rs`
- `backend-school/tests/static_architecture.rs`

## Public Compatibility Surface

```rust
mod batch_mutations;
mod entries;
mod instructors;
mod moves_and_swaps;
mod occupancy;
mod shared;
mod validation;

pub use batch_mutations::{
    create_batch_entries, delete_batch_group, delete_entries_by_slot, BatchBlockedCell,
    BatchCreateOutcome, BatchDeletedEntry, BatchExcludedInstructor, BatchInstructorConflict,
    BatchSkippedCell,
};
pub use entries::{
    create_entry, delete_entry, fetch_entry_by_id, list_entries,
    resolve_classroom_course_semester_id, update_entry, CreateEntryOutcome,
    TimetableFilter, UpdateEntryOutcome,
};
pub use instructors::{
    add_entry_instructor, get_my_activity_for_entry, hide_instructor_from_slot,
    hide_instructor_from_slot_period, remove_entry_instructor, restore_instructor_to_slot,
    AddInstructorResult, MyActivityForEntry, MyActivityInstructor, RemoveInstructorResult,
};
pub use moves_and_swaps::{
    swap_entries, validate_moves, SwapConflictInfo, SwapOutcome,
};
pub use occupancy::{get_occupancy, OccupancyRow};
pub use validation::validate_entry;
```

---

### Task 1: Add Timetable Database Characterization Tests

**Files:**

- Create: `backend-school/src/modules/academic/services/timetable_service_tests.rs`
- Modify: `backend-school/src/modules/academic/services.rs`

**Interfaces:**

- Consumes: current public timetable service and existing isolated-schema test helpers.
- Produces: fixed behavioral checks for all later extraction tasks.

- [ ] **Step 1: Register the test module**

Add:

```rust
#[cfg(test)]
mod timetable_service_tests;
```

Run `cargo test timetable_service_tests --no-run` and confirm failure because the file is absent.

- [ ] **Step 2: Add the fixture**

Create:

```rust
struct TimetableFixture {
    semester_id: Uuid,
    classroom_id: Uuid,
    second_classroom_id: Uuid,
    course_id: Uuid,
    second_course_id: Uuid,
    period_id: Uuid,
    second_period_id: Uuid,
    room_id: Uuid,
    second_room_id: Uuid,
    instructor_id: Uuid,
    second_instructor_id: Uuid,
    student_user_id: Uuid,
    parent_user_id: Uuid,
}
```

Use `create_test_pool`, `run_test_migrations`, and synthetic UUID-derived codes. Insert two
classrooms, two courses in one semester, two periods, two rooms, two active instructors, one
student enrollment, and one parent-child link.

- [ ] **Step 3: Add entry and shared-view tests**

Add `create_update_delete_and_filtered_lists_preserve_joined_entry_shape` using public create,
update, list, and delete calls. Add
`admin_self_and_parent_filters_resolve_the_same_persisted_entry` using the exact filters passed by
the three handler paths. Assert subject/classroom/period/room/instructor fields, sequence value
where returned, and the same entry UUID across the three view filters.

- [ ] **Step 4: Add conflict and instructor tests**

Add `room_classroom_and_instructor_conflicts_preserve_existing_outcomes_and_rollback` with one
conflicting create for each resource. Add
`add_remove_hide_and_restore_instructors_preserve_team_and_entry_state` using the public
instructor functions. Assert the existing outcome enum/error for each conflict and verify failed
writes leave rows unchanged.

- [ ] **Step 5: Add move, occupancy, and batch tests**

Add `swap_and_validate_moves_preserve_atomic_conflict_behavior` with two entries and both a valid
and conflicting target. Add `occupancy_and_batch_mutations_report_exact_persisted_rows` with a
mixed batch containing inserted, skipped, and blocked cells. Assert occupancy contains both
instructor IDs, swap changes both cells atomically, batch summary counts equal persisted rows, and
batch-group deletion removes only the target group.

- [ ] **Step 6: Run and commit**

```bash
cd backend-school
cargo fmt --all -- --check
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test timetable_service_tests -- --test-threads=1
cargo test timetable_service::tests
```

```bash
git add backend-school/src/modules/academic/services.rs backend-school/src/modules/academic/services/timetable_service_tests.rs
git commit -m "test(timetable): characterize service workflows"
```

---

### Task 2: Extract Shared Types and Occupancy

**Files:**

- Create: `backend-school/src/modules/academic/services/timetable_service/shared.rs`
- Create: `backend-school/src/modules/academic/services/timetable_service/occupancy.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**

- Produces: shared entry/conflict primitives and read-only occupancy.

- [ ] **Step 1: Add the failing private-module guard**

Add a static test that requires `mod shared;` and `mod occupancy;`, rejects public child modules,
and requires both child files. Run it and confirm failure.

- [ ] **Step 2: Move the shared closure**

Move shared outcome/input types and pure helpers used by at least two children:

```text
BatchCreateOutcome, BatchSkippedCell, BatchBlockedCell, BatchDeletedEntry,
BatchInstructorConflict, BatchExcludedInstructor, SwapConflictInfo, SwapOutcome,
CreateEntryOutcome, UpdateEntryOutcome, TimetableFilter,
SwapEntryRow, MoveSourceRow, MoveEntryRow, MoveCellKey, MoveEntryRefs
```

Move pure mapping/deduplication/conflict helpers and their unit tests as revealed by compiler usage.
Use `pub(super)` internally and retain public facade exports for public types.

- [ ] **Step 3: Extract occupancy**

Move `OccupancyRow`, `get_occupancy`, and its private row mapping/query closure unchanged into
`occupancy.rs`.

- [ ] **Step 4: Verify and commit**

```bash
cd backend-school
cargo fmt --all -- --check
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test occupancy_and_batch_mutations_report_exact_persisted_rows -- --test-threads=1
cargo test timetable_service::occupancy::tests
cargo test --test static_architecture timetable_service_uses_private_child_modules
cargo check --all-targets
```

```bash
git add backend-school/src/modules/academic/services/timetable_service.rs backend-school/src/modules/academic/services/timetable_service/shared.rs backend-school/src/modules/academic/services/timetable_service/occupancy.rs backend-school/tests/static_architecture.rs
git commit -m "refactor(timetable): extract shared types and occupancy"
```

---

### Task 3: Extract Entry Reads and Individual Mutations

**Files:**

- Create: `backend-school/src/modules/academic/services/timetable_service/entries.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service.rs`

**Interfaces:**

- Consumes: shared filters/outcomes and validation interface.
- Produces: list/fetch/resolve/create/update/delete entry operations.

- [ ] **Step 1: Run entry characterization**

Run both entry/shared-view tests against the monolith and confirm pass.

- [ ] **Step 2: Move the complete entry closure**

Move:

```text
ENTRY_SELECT_WITH_JOINS, list_entries, fetch_entry_by_id,
resolve_classroom_course_semester_id, create_entry, update_entry, delete_entry
```

Move their owned row structs, query builders, mapping functions, and sequence lookup. Keep
validation calls narrow and preserve the current transaction boundaries and conflict outcome
mapping.

- [ ] **Step 3: Verify and commit**

```bash
cd backend-school
cargo fmt --all -- --check
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test timetable_service_tests -- --test-threads=1
cargo check --all-targets
```

```bash
git add backend-school/src/modules/academic/services/timetable_service.rs backend-school/src/modules/academic/services/timetable_service/entries.rs
git commit -m "refactor(timetable): extract entry workflows"
```

---

### Task 4: Extract Instructor Workflows

**Files:**

- Create: `backend-school/src/modules/academic/services/timetable_service/instructors.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service.rs`

**Interfaces:**

- Consumes: entry hydration and validation helpers.
- Produces: all public instructor/team/activity operations and result types.

- [ ] **Step 1: Run instructor characterization**

Run `add_remove_hide_and_restore_instructors_preserve_team_and_entry_state`.

- [ ] **Step 2: Move the exact instructor closure**

Move:

```text
AddInstructorResult, RemoveInstructorResult,
MyActivityInstructor, MyActivityForEntry,
add_entry_instructor, remove_entry_instructor,
restore_instructor_to_slot, hide_instructor_from_slot,
hide_instructor_from_slot_period, get_my_activity_for_entry
```

Move only private SQL/helpers exclusively used by these functions. Keep primary/team instructor
synchronization, activity visibility, and returned IDs unchanged.

- [ ] **Step 3: Verify and commit**

```bash
cd backend-school
cargo fmt --all -- --check
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test add_remove_hide_and_restore_instructors_preserve_team_and_entry_state -- --test-threads=1
cargo check --all-targets
```

```bash
git add backend-school/src/modules/academic/services/timetable_service.rs backend-school/src/modules/academic/services/timetable_service/instructors.rs
git commit -m "refactor(timetable): extract instructor workflows"
```

---

### Task 5: Extract Validation, Moves, and Swaps

**Files:**

- Create: `backend-school/src/modules/academic/services/timetable_service/validation.rs`
- Create: `backend-school/src/modules/academic/services/timetable_service/moves_and_swaps.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service.rs`

**Interfaces:**

- Produces: public `validate_entry`, `swap_entries`, and `validate_moves`; narrow validation helpers for entries/batches.

- [ ] **Step 1: Run conflict and move characterization**

Run the conflict and swap tests and confirm pass.

- [ ] **Step 2: Extract validation**

Move `validate_entry` plus classroom, room, instructor, day, period, semester, and activity
conflict helpers. Keep SQL/bind order and all error strings unchanged.

- [ ] **Step 3: Extract moves and swaps**

Move `swap_entries`, `validate_moves`, their row aliases/context builders, lock acquisition, and
conflict detail mapping. Preserve lock order and two-entry atomicity.

- [ ] **Step 4: Verify and commit**

```bash
cd backend-school
cargo fmt --all -- --check
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test timetable_service_tests -- --test-threads=1
cargo test timetable_service::validation::tests
cargo test timetable_service::moves_and_swaps::tests
```

```bash
git add backend-school/src/modules/academic/services/timetable_service.rs backend-school/src/modules/academic/services/timetable_service/validation.rs backend-school/src/modules/academic/services/timetable_service/moves_and_swaps.rs
git commit -m "refactor(timetable): extract validation and move workflows"
```

---

### Task 6: Extract Batch Mutations and Seal the Facade

**Files:**

- Create: `backend-school/src/modules/academic/services/timetable_service/batch_mutations.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**

- Consumes: shared batch types and validation.
- Produces: batch create/delete operations and final facade.

- [ ] **Step 1: Move the batch closure**

Move:

```text
delete_entries_by_slot, delete_batch_group, create_batch_entries
```

Move exact private validation, grouping, instructor conflict, skipped/blocked/deleted summary, and
bulk SQL helpers. Preserve counts and transaction boundaries.

- [ ] **Step 2: Add a failing facade guard**

Require all child declarations and public re-exports in this plan. Reject SQL/query/transaction
tokens in the facade and cap nonblank lines at 85. Confirm it fails before reducing the facade.
Update existing timetable architecture guards that inspect service implementation text to read
the owning child file or an explicit concatenation of all child files.

- [ ] **Step 3: Replace the facade and verify**

Use the exact compatibility surface above, then run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test timetable_service_tests -- --test-threads=1
cargo test --test static_architecture timetable -- --test-threads=1
```

- [ ] **Step 4: Commit**

```bash
git add backend-school/src/modules/academic/services/timetable_service.rs backend-school/src/modules/academic/services/timetable_service/batch_mutations.rs backend-school/tests/static_architecture.rs
git commit -m "refactor(timetable): finish modular service facade"
```

---

### Task 7: Full Phase Verification

**Files:**

- Modify only for proven regressions.

- [ ] **Step 1: Run complete backend verification**

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test --bin backend-school -- --test-threads=1
cargo test --test static_architecture -- --test-threads=1
```

- [ ] **Step 2: Audit source of truth and scope**

```bash
rg -n 'list_entries' backend-school/src/modules/{academic,parents} backend-school/src/main.rs
rg -n 'sqlx::|\.fetch_|\.execute\(|\.begin\(' backend-school/src/modules/academic/services/timetable_service.rs
git diff --check
git status --short
```

Expected: all three timetable view paths still delegate to `list_entries`, the facade contains no
database work, no frontend/migration/generated files changed, and the worktree is clean.
