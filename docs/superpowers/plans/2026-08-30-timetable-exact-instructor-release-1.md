# Timetable Exact Instructor Release 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cut every timetable consumer to exact per-entry instructors, establish effective teacher-assignment episodes, migrate existing tenant data deterministically, and keep the current form editor usable with an instructor multi-select.

**Architecture:** `learning_group_teachers` owns dated teacher eligibility while `timetable_entry_instructors` becomes the sole owner of teachers who actually teach one timetable period. Migration 054 backfills current undated assignments and reconciles every group entry to the instructor set the current runtime already derives; Rust services then remove the group-teacher fallback and protect exact child rows under timetable-version immutability. The existing timetable page receives a minimal exact-instructor picker before drag-and-drop work begins.

**Tech Stack:** PostgreSQL tenant migrations, Rust/Axum/SQLx, Utoipa OpenAPI, generated TypeScript contracts, SvelteKit 5 runes, Tailwind CSS, local shadcn-svelte primitives, Node static tests, Playwright.

**Spec:** `docs/superpowers/specs/2026-08-30-timetable-drag-drop-and-effective-teacher-assignment-design.md`

## Global Constraints

- Add migration `054`; never edit applied migrations 001–053.
- Do not add teacher add/adjust/stop change-item actions in Release 1; Release 4 owns that workflow.
- `timetable_entry_instructors` is the only post-cutover runtime source for actual period teachers.
- Existing group entries backfill every currently assigned group teacher, matching current runtime semantics; never choose one teacher arbitrarily.
- One drag/placement remains one bell period; Release 1 does not add drag-and-drop.
- Draft course/activity entries may have zero exact instructors, but publication readiness blocks them.
- Published timetable versions and their instructor child rows are immutable.
- Conflict guards cover learning group, full covered-homeroom set, physical room, and exact instructor.
- Use Rust DTOs and Utoipa as contract authority; regenerate OpenAPI and TypeScript artifacts instead of editing them.
- Do not add permissions, automatic scheduling, linked double periods, payroll weighting, or legacy compatibility branches.
- Run all commands sequentially. Use `CARGO_BUILD_JOBS=1` and `--test-threads=1` for focused Rust/database verification.
- Preserve the existing SchoolOrbit shell, Kanit typography, semantic tokens, shadcn-svelte controls, dark mode, and Thai labels.

---

### Task 1: Forward-only effective-assignment and exact-instructor migration

**Files:**
- Create: `backend-school/migrations/054_timetable_exact_instructors.sql`
- Modify: `backend-school/src/modules/academic/core/schema_tests.rs`

**Interfaces:**
- Consumes: migration-042 `learning_group_teachers`, migration-052 `academic_timetable_versions`, `academic_timetable_entries`, `timetable_entry_instructors`, and published-version immutability functions.
- Produces: effective teacher episode columns on `learning_group_teachers`; exact group-entry instructor rows; named slot-conflict guards; and published-version protection for `timetable_entry_instructors`.

- [ ] **Step 1: Write the failing migration reconciliation test**

Add `migration_054_reconciles_exact_instructors_and_teacher_episodes`. Capture the pre-migration entry and expected relationship counts, apply 054, and assert the literal invariants:

```rust
#[tokio::test]
async fn migration_054_reconciles_exact_instructors_and_teacher_episodes() {
    let pool = phase_a_fixture("academic_core_054_exact_instructors").await;
    record_passing_phase_a_reconciliation_marker(&pool).await.unwrap();
    apply_migrations_through(&pool, 53).await.unwrap();

    let expected_group_entry_instructors: i64 = sqlx::query_scalar(
        r#"SELECT count(*)
           FROM academic_timetable_entries entry
           JOIN learning_group_teachers teacher
             ON teacher.learning_group_id = entry.learning_group_id
           WHERE entry.learning_group_id IS NOT NULL"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    apply_migrations_through(&pool, 54).await.unwrap();

    let actual_group_entry_instructors: i64 = sqlx::query_scalar(
        r#"SELECT count(*)
           FROM timetable_entry_instructors instructor
           JOIN academic_timetable_entries entry ON entry.id = instructor.entry_id
           WHERE entry.learning_group_id IS NOT NULL"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(actual_group_entry_instructors, expected_group_entry_instructors);

    let invalid_intervals: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM learning_group_teachers WHERE starts_on IS NULL OR ends_on < starts_on",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(invalid_intervals, 0);
    assert!(column_exists(&pool, "learning_group_teachers", "row_version").await);
    assert!(column_exists(&pool, "learning_group_teachers", "started_by_change_set_id").await);
    assert!(column_exists(&pool, "learning_group_teachers", "ended_by_change_set_id").await);
}
```

Also assert:

- every migrated `starts_on` equals the owning offering's `starts_on`;
- standalone structural-entry instructor rows are unchanged;
- every group entry's instructor IDs equal its complete teacher-assignment IDs as sets;
- exactly one child is marked `primary` per non-empty entry and all others are `secondary`;
- migration provenance contains `exactInstructorCutover.migration = 54`; and
- all new conflict/immutability triggers report `tgenabled = 'O'`.

- [ ] **Step 2: Write failing rollback and database-guard tests**

Add `migration_054_rejects_unmappable_group_entries_atomically`. Before applying 054, create one active group entry under a group with no teacher, then assert:

```rust
let error = apply_migrations_through(&pool, 54)
    .await
    .expect_err("an active group entry without a deterministic instructor set must block cutover");
assert!(error.to_string().contains("ACADEMIC_054_ENTRY_INSTRUCTORS_UNMAPPABLE"));
let applied: i64 = sqlx::query_scalar("SELECT coalesce(max(version), 0) FROM _sqlx_migrations")
    .fetch_one(&pool)
    .await
    .unwrap();
assert_eq!(applied, 53);
assert!(!column_exists(&pool, "learning_group_teachers", "starts_on").await);
```

Add `migration_054_database_guards_reject_direct_conflicts_and_published_child_mutation`. After 054, attempt direct SQL that creates each conflict class and assert the named errors:

```text
ACADEMIC_TIMETABLE_GROUP_CONFLICT
ACADEMIC_TIMETABLE_HOMEROOM_CONFLICT
ACADEMIC_TIMETABLE_ROOM_CONFLICT
ACADEMIC_TIMETABLE_INSTRUCTOR_DOUBLE_BOOKED
ACADEMIC_PUBLISHED_TIMETABLE_VERSION_CHILD_IMMUTABLE
```

- [ ] **Step 3: Run migration tests and verify RED**

Run each test separately:

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_054_reconciles_exact_instructors_and_teacher_episodes \
  -- --exact --nocapture --test-threads=1
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_054_rejects_unmappable_group_entries_atomically \
  -- --exact --nocapture --test-threads=1
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_054_database_guards_reject_direct_conflicts_and_published_child_mutation \
  -- --exact --nocapture --test-threads=1
```

Expected: FAIL because migration 054 does not exist.

- [ ] **Step 4: Implement migration 054 preflight and teacher episode columns**

Begin with bounded `DO` preflight blocks. Reject active group entries with no teacher, invalid teacher users, cross-context relationships, or current-runtime teacher double-booking. Add these columns:

```sql
ALTER TABLE learning_group_teachers
    ADD COLUMN starts_on DATE,
    ADD COLUMN ends_on DATE,
    ADD COLUMN started_by_change_set_id UUID,
    ADD COLUMN ended_by_change_set_id UUID,
    ADD COLUMN row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    ADD COLUMN created_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    ADD COLUMN updated_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ADD CONSTRAINT learning_group_teachers_interval_check
        CHECK (ends_on IS NULL OR ends_on >= starts_on),
    ADD CONSTRAINT learning_group_teachers_started_change_set_fkey
        FOREIGN KEY (started_by_change_set_id, academic_term_id, academic_year_id)
        REFERENCES academic_term_change_sets(id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT,
    ADD CONSTRAINT learning_group_teachers_ended_change_set_fkey
        FOREIGN KEY (ended_by_change_set_id, academic_term_id, academic_year_id)
        REFERENCES academic_term_change_sets(id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT;

UPDATE learning_group_teachers teacher
SET starts_on = offering.starts_on,
    migration_provenance = teacher.migration_provenance ||
        jsonb_build_object('exactInstructorCutover', jsonb_build_object('migration', 54))
FROM learning_groups learning_group
JOIN learning_offerings offering ON offering.id = learning_group.learning_offering_id
WHERE learning_group.id = teacher.learning_group_id;

ALTER TABLE learning_group_teachers ALTER COLUMN starts_on SET NOT NULL;
ALTER TABLE learning_group_teachers DROP CONSTRAINT learning_group_teachers_unique_key;
ALTER TABLE learning_group_teachers
    ADD CONSTRAINT learning_group_teachers_episode_key
    UNIQUE (learning_group_id, teacher_id, starts_on);
```

Install a named trigger that rejects overlapping inclusive intervals for the same group/teacher. Release 1 retains the published-group direct mutation lock; Release 4 will replace it with change-set-provenance mutation rules.

- [ ] **Step 5: Reconcile group-entry instructor children deterministically**

For active and historical group entries, treat `learning_group_teachers` as the current runtime source. Delete only child rows whose parent has `learning_group_id IS NOT NULL`, then insert every group teacher. Assign child roles deterministically:

```sql
WITH ordered AS (
    SELECT entry.id AS entry_id,
           teacher.teacher_id,
           row_number() OVER (
               PARTITION BY entry.id
               ORDER BY CASE teacher.role
                   WHEN 'primary' THEN 1 WHEN 'secondary' THEN 2 ELSE 3 END,
                   teacher.starts_on, teacher.id
           ) AS teacher_order
    FROM academic_timetable_entries entry
    JOIN learning_group_teachers teacher
      ON teacher.learning_group_id = entry.learning_group_id
    WHERE entry.learning_group_id IS NOT NULL
)
INSERT INTO timetable_entry_instructors (id, entry_id, instructor_id, role)
SELECT uuid_generate_v4(), entry_id, teacher_id,
       CASE WHEN teacher_order = 1 THEN 'primary' ELSE 'secondary' END
FROM ordered;
```

Add `exactInstructorCutover` provenance to parent timetable entries without changing their identity, slot, version, active state, or row version.

- [ ] **Step 6: Install final database conflict and immutability guards**

Replace slot-conflict functions with version-aware guards that acquire one transaction advisory lock derived from `timetable_version_id`, `day_of_week`, and `bell_schedule_period_id`, then check:

```sql
-- same learning group
candidate.learning_group_id = other.learning_group_id

-- any shared covered homeroom, including direct structural homeroom entries
EXISTS (
    SELECT 1 FROM candidate_homerooms c
    JOIN other_homerooms o ON o.homeroom_id = c.homeroom_id
)

-- same non-null physical room
candidate.room_id IS NOT NULL AND candidate.room_id = other.room_id
```

Keep exact instructor double-booking on `timetable_entry_instructors`, but remove any exception that permits two distinct entries merely because their subject/activity matches. Add a specific instructor-child immutability trigger that resolves the parent entry's timetable version and rejects insert/update/delete when it is published.

- [ ] **Step 7: Run migration tests and verify GREEN**

Run the three exact tests one at a time with `CARGO_BUILD_JOBS=1` and `--test-threads=1`. Expected: PASS with exact counts and named database failures.

- [ ] **Step 8: Commit the schema task**

```bash
git add backend-school/migrations/054_timetable_exact_instructors.sql \
  backend-school/src/modules/academic/core/schema_tests.rs
git commit -m "feat(timetable): migrate exact period instructors"
```

### Task 2: Effective teacher read model and exact instructor mutations

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/groups.rs`
- Modify: `backend-school/src/modules/academic/delivery/services.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Modify: `backend-school/src/modules/academic/models/timetable.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service_tests.rs`

**Interfaces:**
- Consumes: migration-054 teacher episodes and exact instructor children.
- Produces: `LearningGroupTeacherAssignment`, exact `CreateTimetableEntryRequest.instructor_ids`, optional replacement `UpdateTimetableEntryRequest.instructor_ids`, atomic timetable audit payloads, and service helpers that never fall back to all group teachers.

- [ ] **Step 1: Write failing group-episode and exact-instructor service tests**

Add these focused tests:

```rust
#[tokio::test]
async fn group_read_model_returns_effective_teacher_episodes() {
    // Assert id, teacher_id, display_name, role, starts_on, ends_on and row_version.
}

#[tokio::test]
async fn timetable_entries_split_and_coteach_with_exact_instructors() {
    // Create Monday with A, Wednesday with B, Friday with A+B.
    // Assert each returned instructor set exactly matches its request.
}

#[tokio::test]
async fn timetable_update_replaces_the_complete_instructor_set_atomically() {
    // Update A to A+B, assert parent row_version increments once and child set is exact.
}

#[tokio::test]
async fn timetable_rejects_instructor_outside_group_effective_date() {
    // Teacher episode begins after version.effective_from; expect ValidationError.
}

#[tokio::test]
async fn timetable_teacher_set_change_audits_exact_before_and_after_sets() {
    // Update A to A+B and assert one audit payload contains sorted before/after IDs,
    // entry/version IDs, actor, and old/new parent row versions.
}
```

The split/co-teach request literals must be:

```rust
CreateTimetableEntryRequest {
    timetable_version_id: draft.id,
    academic_term_id: term_id,
    learning_group_id: Some(group_id),
    homeroom_id: None,
    day_of_week: "MON".to_string(),
    bell_schedule_period_id: period_id,
    room_id: None,
    note: None,
    entry_type: "COURSE".to_string(),
    title: None,
    instructor_ids: vec![teacher_a],
}
```

- [ ] **Step 2: Run the focused tests and verify RED**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_service_tests::timetable_entries_split_and_coteach_with_exact_instructors \
  -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_service_tests::timetable_update_replaces_the_complete_instructor_set_atomically \
  -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_service_tests::timetable_rejects_instructor_outside_group_effective_date \
  -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_service_tests::timetable_teacher_set_change_audits_exact_before_and_after_sets \
  -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::group_read_model_returns_effective_teacher_episodes \
  -- --exact --test-threads=1
```

Expected: compile failures because the read model and update instructor field do not exist, followed by behavior failures while group entries still derive all teachers.

- [ ] **Step 3: Add the effective assignment read type**

Replace `LearningGroup.teacher_assignments: Vec<TeacherAssignmentInput>` with a read-specific type while retaining `TeacherAssignmentInput` for draft write requests:

```rust
#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LearningGroupTeacherAssignment {
    pub id: Uuid,
    pub teacher_id: Uuid,
    pub display_name: String,
    pub role: LearningTeacherRole,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
    pub row_version: i64,
}
```

Update group hydration to load all episodes in stable role/start/ID order. Draft `replace_teachers` creates one episode per selected teacher beginning at the offering's `starts_on`, preserves the existing published-group service conflict, and returns the new read model.

- [ ] **Step 4: Extend exact timetable mutation DTOs**

Keep create's existing exact vector and add one optional replacement field to update:

```rust
pub struct UpdateTimetableEntryRequest {
    pub timetable_version_id: Uuid,
    pub row_version: i64,
    pub day_of_week: Option<String>,
    pub bell_schedule_period_id: Option<Uuid>,
    pub room_id: Option<Uuid>,
    pub clear_room: Option<bool>,
    pub note: Option<String>,
    pub clear_note: Option<bool>,
    pub title: Option<String>,
    pub instructor_ids: Option<Vec<Uuid>>,
}
```

`None` preserves the exact set; `Some(vec![])` intentionally clears it in a draft; `Some(ids)` replaces it after canonical deduplication.

- [ ] **Step 5: Replace group fallback with exact child hydration**

Change `RelationshipIndexes::instructors` to read only `instructors_by_entry`. Remove `instructors_by_group` and its `learning_group_teachers` query from timetable list/occupancy/conflict paths. Exact hydration continues to join display names and subject-group labels from child instructor IDs.

Add a source-level architecture assertion that the timetable service's relationship-index block does not contain:

```text
FROM learning_group_teachers
instructors_by_group
```

- [ ] **Step 6: Implement exact eligibility and transactional child replacement**

For a group entry, load the draft version effective date and require every proposed ID to satisfy:

```sql
SELECT teacher.teacher_id
FROM learning_group_teachers teacher
WHERE teacher.learning_group_id = $1
  AND teacher.starts_on <= $2
  AND (teacher.ends_on IS NULL OR teacher.ends_on >= $2)
  AND teacher.teacher_id = ANY($3)
ORDER BY CASE teacher.role WHEN 'primary' THEN 1 WHEN 'secondary' THEN 2 ELSE 3 END,
         teacher.starts_on, teacher.id
```

Create and update write child rows in the same transaction as the parent. Update locks the parent, validates `row_version`, checks the proposed set against the target slot, replaces child rows, and increments the parent once. The first ordered selected teacher receives child role `primary`; remaining teachers receive `secondary`.

Structural entries continue to validate direct instructor IDs as active staff without requiring a learning-group episode.

Append `academic_audit_events` inside the same SQL transaction for create/update/deactivate mutations. The payload records timetable/entry/context IDs, day/period/room, sorted exact instructor IDs and roles before/after, actor, and old/new row versions. An audit insert failure rolls back the timetable mutation.

- [ ] **Step 7: Run focused tests and verify GREEN**

Repeat the five Step 2 commands, then run the bounded timetable service module once:

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_service_tests -- --test-threads=1
```

Expected: PASS with no fallback behavior, including existing create, update, batch, swap, occupancy, and validation coverage.

- [ ] **Step 8: Commit the service task**

```bash
git add backend-school/src/modules/academic/delivery/models.rs \
  backend-school/src/modules/academic/delivery/services/groups.rs \
  backend-school/src/modules/academic/delivery/services.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs \
  backend-school/src/modules/academic/models/timetable.rs \
  backend-school/src/modules/academic/services/timetable_service.rs \
  backend-school/src/modules/academic/services/timetable_service_tests.rs
git commit -m "feat(timetable): use exact entry instructors"
```

### Task 3: Clone, readiness, personal timetable, and downstream cutover

**Files:**
- Modify: `backend-school/src/modules/academic/services/timetable_version_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_version_service_tests.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/change_sets.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Modify: `backend-school/src/modules/academic/services/daily_teaching_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service_tests.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_template_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_template_service_tests.rs`
- Modify: `backend-school/src/modules/supervision/services/observations.rs`
- Modify: `backend-school/src/modules/parents/services.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: exact `TimetableEntry.instructors` from Task 2.
- Produces: exact clone/readiness/personal/daily/export/supervision behavior and `MissingEntryInstructor` / `IneligibleEntryInstructor` readiness codes.

- [ ] **Step 1: Write failing downstream behavior tests**

Add focused coverage that proves:

```rust
#[tokio::test]
async fn cloned_timetable_version_preserves_exact_instructor_sets() {
    // Source entry A+B; clone; assert target child IDs equal {A, B} exactly.
}

#[tokio::test]
async fn readiness_blocks_course_entry_without_exact_instructor() {
    // Clear draft child set; preview contains MissingEntryInstructor/Blocking.
}

#[tokio::test]
async fn personal_timetable_returns_only_periods_the_staff_member_teaches() {
    // A and B are group teachers, but only B is on the entry; A gets no row, B gets one.
}
```

Extend template apply coverage so applied group entries copy the template-selected exact teacher set only when those teachers are eligible; otherwise the draft entry is created without instructors and readiness blocks publication.

- [ ] **Step 2: Run focused tests and verify RED**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_version_service_tests::cloned_timetable_version_preserves_exact_instructor_sets \
  -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::readiness_blocks_course_entry_without_exact_instructor \
  -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_service_tests::personal_timetable_returns_only_periods_the_staff_member_teaches \
  -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_template_service_tests -- --test-threads=1
```

Expected: at least the personal test fails while list filters still derive all group teachers.

- [ ] **Step 3: Cut clone and readiness to exact children**

Keep version clone's existing child copy, then assert exact cardinality in the service. Replace teacher-conflict readiness with `timetable_entry_instructors` only and add findings:

```rust
MissingEntryInstructor,
IneligibleEntryInstructor,
```

`MissingEntryInstructor` applies to active COURSE/ACTIVITY entries with zero children. `IneligibleEntryInstructor` applies when a child teacher has no episode active on target `effective_from`.

- [ ] **Step 4: Cut every reader to exact instructors**

Inventory and update personal timetable, daily teaching, parent/student timetable, supervision timetable options, template application, teacher load export inputs, and any direct SQL instructor filter. Staff filtering must be an `EXISTS` against `timetable_entry_instructors`; no reader may join `learning_group_teachers` to decide who teaches a period.

Add a static architecture test that scans these modules and rejects a timetable-entry query containing `JOIN learning_group_teachers` in an instructor filter.

- [ ] **Step 5: Run focused and architecture tests and verify GREEN**

Run every new behavior test, then:

```bash
CARGO_BUILD_JOBS=1 cargo test --manifest-path backend-school/Cargo.toml \
  --test static_architecture timetable_exact_instructor_consumers_do_not_fallback_to_group_teachers \
  -- --exact --test-threads=1
```

Expected: PASS.

- [ ] **Step 6: Commit the downstream cutover**

```bash
git add backend-school/src/modules/academic/services/timetable_version_service.rs \
  backend-school/src/modules/academic/services/timetable_version_service_tests.rs \
  backend-school/src/modules/academic/delivery/services/change_sets.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs \
  backend-school/src/modules/academic/services/daily_teaching_service.rs \
  backend-school/src/modules/academic/services/timetable_service_tests.rs \
  backend-school/src/modules/academic/services/timetable_template_service.rs \
  backend-school/src/modules/academic/services/timetable_template_service_tests.rs \
  backend-school/src/modules/supervision/services/observations.rs \
  backend-school/src/modules/parents/services.rs \
  backend-school/tests/static_architecture.rs
git commit -m "refactor(timetable): cut consumers to exact instructors"
```

### Task 4: Generated contract and current-editor instructor picker

**Files:**
- Modify: `backend-school/src/api_contract.rs`
- Modify (generated): `contracts/openapi/school-api.json`
- Modify (generated): `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/timetable.ts`
- Create: `frontend-school/src/lib/components/academic/timetable/TimetableInstructorPicker.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`
- Modify: `frontend-school/tests/static/timetable-version-contract.test.mjs`
- Create: `frontend-school/tests/static/timetable-exact-instructors.test.mjs`
- Modify: `frontend-school/tests/e2e/timetable-version-workspace.spec.ts`

**Interfaces:**
- Consumes: Task 2 Rust DTOs and effective teacher assignment labels.
- Produces: generated exact instructor request/response types and a reusable `TimetableInstructorPicker` used by Release 2/3 boards.

- [ ] **Step 1: Write failing generated-contract and UI tests**

Require these contract facts:

```js
assert.ok(openapi.components.schemas.UpdateTimetableEntryRequest.properties.instructorIds);
assert.ok(openapi.components.schemas.LearningGroupTeacherAssignment.required.includes('startsOn'));
assert.ok(openapi.components.schemas.LearningGroupTeacherAssignment.required.includes('displayName'));
```

In `timetable-exact-instructors.test.mjs`, require the picker import, `formInstructorIds`, exact create/update payloads, solo/multi teacher copy, and reject the old literal:

```js
assert.doesNotMatch(page, /instructorIds:\s*\[\]/);
assert.match(page, /instructorIds:\s*formInstructorIds/);
assert.match(page, /ครูผู้สอนของคาบนี้/);
assert.match(picker, /type="button"/);
assert.match(picker, /aria-pressed=/);
```

- [ ] **Step 2: Run focused frontend tests and verify RED**

```bash
cd frontend-school
node --test tests/static/timetable-version-contract.test.mjs \
  tests/static/timetable-exact-instructors.test.mjs --test-concurrency=1
```

Expected: FAIL because the generated update request and picker do not exist.

- [ ] **Step 3: Register and regenerate the API contract**

Register `LearningGroupTeacherAssignment` and changed timetable requests in `api_contract.rs`, then run:

```bash
cd frontend-school
CARGO_BUILD_JOBS=1 npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
```

Do not hand-edit either generated artifact.

- [ ] **Step 4: Implement the reusable picker**

The component contract is:

```ts
type InstructorOption = {
    id: string;
    displayName: string;
    role: 'primary' | 'secondary' | 'assistant';
};

let {
    options,
    value = $bindable<string[]>([]),
    disabled = false,
    label = 'ครูผู้สอนของคาบนี้'
}: {
    options: InstructorOption[];
    value?: string[];
    disabled?: boolean;
    label?: string;
} = $props();
```

Render shadcn Button chips with `aria-pressed`, visible role copy, stable keyed iteration by teacher ID, and a distinct empty state. When exactly one eligible teacher exists and `value` is empty, the parent explicitly selects that teacher while initializing the form; the picker itself emits no hidden side effect.

- [ ] **Step 5: Wire exact instructors into the current form editor**

Add `formInstructorIds`. `resetForm` selects the group's only effective teacher, or leaves a multi-teacher group visibly unselected. `editEntry` copies `entry.instructors.map(item => item.userId)`. Create and update send the exact vector:

```ts
await updateTimetableEntry(selectedEntry.id, {
    timetableVersionId: selectedVersion.id,
    rowVersion: selectedEntry.rowVersion,
    dayOfWeek: formDay,
    bellSchedulePeriodId: formPeriodId,
    roomId: formRoomId || null,
    clearRoom: !formRoomId,
    note: formNote.trim() || null,
    clearNote: !formNote.trim(),
    title: formTitle.trim() || null,
    instructorIds: formInstructorIds
});
```

Show the picker only for learning-group COURSE/ACTIVITY entries. Published versions render exact instructor badges read-only.

- [ ] **Step 6: Extend the Playwright mock workflow**

In `timetable-version-workspace.spec.ts`, make the draft group expose teachers A and B. Create one entry with A+B, update it to B, and assert intercepted request bodies contain the exact arrays in stable order. Also assert published entry details render the instructors without an editable picker.

- [ ] **Step 7: Run Svelte tooling and focused frontend tests**

Run sequentially:

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic/timetable/TimetableInstructorPicker.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/timetable/+page.svelte' --svelte-version 5
node --test tests/static/timetable-version-contract.test.mjs \
  tests/static/timetable-exact-instructors.test.mjs --test-concurrency=1
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/timetable-version-workspace.spec.ts --workers=1
```

Expected: no Svelte issues and all focused tests pass.

- [ ] **Step 8: Commit the contract and minimal UI task**

```bash
git add backend-school/src/api_contract.rs contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  frontend-school/src/lib/api/timetable.ts \
  frontend-school/src/lib/components/academic/timetable/TimetableInstructorPicker.svelte \
  'frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte' \
  frontend-school/tests/static/timetable-version-contract.test.mjs \
  frontend-school/tests/static/timetable-exact-instructors.test.mjs \
  frontend-school/tests/e2e/timetable-version-workspace.spec.ts
git commit -m "feat(timetable): select exact period instructors"
```

### Task 5: Release 1 verification and sandbox checkpoint

**Files:**
- Modify only if a durable procedure changed: `docs/TESTING.md`
- Modify only if rollout/recovery changed: `docs/OPERATIONS.md`

**Interfaces:**
- Consumes: Tasks 1–4.
- Produces: a verified Release 1 commit suitable for push and the existing automated tenant-migration deployment.

- [ ] **Step 1: Run the complete backend gates sequentially**

```bash
cargo fmt --manifest-path backend-school/Cargo.toml --all -- --check
CARGO_BUILD_JOBS=1 cargo test --manifest-path backend-school/Cargo.toml \
  --test static_architecture -- --test-threads=1
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo check --manifest-path backend-school/Cargo.toml
```

Expected: PASS; pre-existing warnings may be reported but no new warning is accepted without review.

- [ ] **Step 2: Run API and frontend gates sequentially**

From `frontend-school`:

```bash
npm run check:api-contracts
npm run test:api-contracts
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

Expected: PASS with Svelte check at 0 errors and 0 warnings.

- [ ] **Step 3: Run focused browser discovery and execution**

```bash
cd frontend-school
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test --list tests/e2e/timetable-version-workspace.spec.ts --workers=1
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/timetable-version-workspace.spec.ts --workers=1
```

Expected: discovery and execution pass without credentials because this spec uses a local mock harness.

- [ ] **Step 4: Run repository hygiene checks**

```bash
git diff --check
git status --short
git log --oneline --decorate -8
```

Expected: only intentional Release 1 changes remain and no generated artifact is stale.

- [ ] **Step 5: Push and monitor the normal deployment only after explicit user approval**

Use non-interactive `git push origin main`, then inspect the backend, frontend, API-contract, and permission workflows serially. The backend workflow must apply migration 054 through the existing maintenance gate; do not run live SQL. Keep the protected Neon snapshot until authenticated read-only checks prove:

```text
existing timetable entry count unchanged
group entry exact instructor count reconciled
teacher personal timetable shows only exact periods
published timetable remains read-only
draft create/update accepts solo and co-teachers
```

- [ ] **Step 6: Record the Release 1 checkpoint commit if documentation changed**

```bash
git add docs/TESTING.md docs/OPERATIONS.md
git commit -m "docs(timetable): record exact instructor rollout"
```

Skip this commit when neither durable document changed; report the executed commands and deployment run links in the handoff instead.
