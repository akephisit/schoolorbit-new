# Auto-Scheduler Removal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the complete auto-scheduler and scheduling-configuration feature while preserving every existing manual or generated timetable entry.

**Architecture:** Remove the scheduler at its public and persistence boundaries: router/modules and Rust OpenAPI source first, generated/frontend consumers second, then active documentation. A new forward-only migration drops scheduler metadata and configuration but only drops the provenance column from `academic_timetable_entries`; it never deletes timetable rows.

**Tech Stack:** Rust 1.96, Axum, sqlx/PostgreSQL, utoipa/OpenAPI, SvelteKit 5, TypeScript, Node test runner, Docker/PostgreSQL sandbox.

## Global Constraints

- Do not edit `backend-school/migrations/001_baseline.sql` or any existing migration; add `028_remove_auto_scheduler.sql`.
- Migration 028 must not `DELETE`, `TRUNCATE`, recreate, or replace `academic_timetable_entries`.
- Preserve every existing timetable entry, including entries whose `scheduler_job_id` is non-null.
- Preserve manual timetable, realtime timetable, timetable-template, course-planning, exam-scheduling, period, room, subject, user, semester, and academic-year behavior.
- Remove old scheduler frontend URLs and backend routes without redirects or compatibility shims; they must return 404.
- Retain `ACADEMIC_COURSE_PLAN_READ_ALL` and `ACADEMIC_COURSE_PLAN_MANAGE_ALL` because non-scheduler features still use them.
- OpenAPI must contain exactly 177 unique operation IDs after removing the seven Scheduling Configuration operations.
- Use `apply_patch` for source-file edits and deletions; formatting and generated-output commands may write mechanically.
- Run Svelte autofixer on every surviving `.svelte` file modified by this plan.
- Never print database credentials or commit them.

---

### Task 1: Remove backend engine and scheduler persistence safely

**Files:**
- Create: `backend-school/migrations/028_remove_auto_scheduler.sql`
- Modify: `backend-school/tests/static_architecture.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/src/modules/academic/handlers.rs`
- Modify: `backend-school/src/modules/academic/models.rs`
- Modify: `backend-school/src/modules/academic/services.rs`
- Modify: `backend-school/src/api_contract.rs`
- Delete: `backend-school/src/modules/academic/handlers/scheduling.rs`
- Delete: `backend-school/src/modules/academic/handlers/scheduling_config.rs`
- Delete: `backend-school/src/modules/academic/models/scheduling.rs`
- Delete: `backend-school/src/modules/academic/models/scheduling_config.rs`
- Delete: `backend-school/src/modules/academic/services/scheduler.rs`
- Delete: `backend-school/src/modules/academic/services/scheduler/backtracking.rs`
- Delete: `backend-school/src/modules/academic/services/scheduler/quality.rs`
- Delete: `backend-school/src/modules/academic/services/scheduler/types.rs`
- Delete: `backend-school/src/modules/academic/services/scheduler/validator.rs`
- Delete: `backend-school/src/modules/academic/services/scheduler_data.rs`
- Delete: `backend-school/src/modules/academic/services/scheduling_service.rs`
- Delete: `backend-school/src/modules/academic/services/scheduling_config_service.rs`
- Delete: `backend-school/src/modules/academic/services/scheduling_config_service_tests.rs`

**Interfaces:**
- Consumes: router/module layout, migration sequence 001-027, `SchoolApiDoc` utoipa source.
- Produces: migration 028, backend with no scheduler modules/routes, Rust OpenAPI source with 177 operations.

- [ ] **Step 1: Replace the positive scheduler architecture test with a failing removal and migration-safety guard**

Replace `scheduling_configuration_routes_permissions_and_boundaries_are_explicit` in `backend-school/tests/static_architecture.rs` with:

```rust
#[test]
fn auto_scheduler_backend_and_schema_are_removed_without_deleting_timetable_entries() {
    let router = read_source(manifest_dir().join("src/modules/academic.rs"));
    for removed in [
        "/scheduling/auto-schedule",
        "/scheduling/jobs",
        "/scheduling/configuration",
        "/scheduling/instructors",
        "/scheduling/subjects",
        "/scheduling/settings",
        "/scheduling/classroom-courses",
        "/scheduling/rooms",
        "/instructor-preferences",
        "/instructor-rooms",
        "/timetable/locked-slots",
    ] {
        assert!(!router.contains(removed), "removed route remains: {removed}");
    }

    for removed in [
        "src/modules/academic/handlers/scheduling.rs",
        "src/modules/academic/handlers/scheduling_config.rs",
        "src/modules/academic/models/scheduling.rs",
        "src/modules/academic/models/scheduling_config.rs",
        "src/modules/academic/services/scheduler.rs",
        "src/modules/academic/services/scheduler",
        "src/modules/academic/services/scheduler_data.rs",
        "src/modules/academic/services/scheduling_service.rs",
        "src/modules/academic/services/scheduling_config_service.rs",
        "src/modules/academic/services/scheduling_config_service_tests.rs",
    ] {
        assert!(!manifest_dir().join(removed).exists(), "removed module remains: {removed}");
    }

    let migration = read_source(
        manifest_dir()
            .join("migrations")
            .join("028_remove_auto_scheduler.sql"),
    );
    let normalized = migration.to_ascii_lowercase();
    assert!(normalized.contains("drop column scheduler_job_id"));
    assert!(normalized.contains("drop table timetable_scheduling_jobs"));
    assert!(!normalized.contains("delete from academic_timetable_entries"));
    assert!(!normalized.contains("truncate academic_timetable_entries"));
    assert!(!normalized.contains("drop table academic_timetable_entries"));
}
```

- [ ] **Step 2: Run the guard and verify the red state**

Run:

```bash
cd backend-school
cargo test --test static_architecture auto_scheduler_backend_and_schema_are_removed_without_deleting_timetable_entries -- --exact
```

Expected: FAIL because scheduler routes/files and migration 028 are still present/missing respectively.

- [ ] **Step 3: Add the forward-only removal migration**

Create `backend-school/migrations/028_remove_auto_scheduler.sql` with this order so dependencies are removed before tables and enum types:

```sql
-- Remove the retired auto-scheduler while preserving every timetable entry.
ALTER TABLE academic_timetable_entries
    DROP COLUMN scheduler_job_id;

DROP TABLE timetable_scheduling_jobs;
DROP TABLE timetable_locked_slots;
DROP TABLE scheduler_settings;
DROP TABLE classroom_course_preferred_rooms;
DROP TABLE instructor_preferences;
DROP TABLE instructor_room_assignments;

ALTER TABLE classroom_courses
    DROP COLUMN consecutive_pattern,
    DROP COLUMN same_day_unique,
    DROP COLUMN hard_unavailable_slots;

ALTER TABLE subjects
    DROP COLUMN min_consecutive_periods,
    DROP COLUMN max_consecutive_periods,
    DROP COLUMN allow_single_period,
    DROP COLUMN allowed_period_ids,
    DROP COLUMN allowed_days;

DROP FUNCTION validate_allowed_days(jsonb);
DROP TYPE scheduling_algorithm;
DROP TYPE scheduling_status;
```

Do not add `CASCADE`; unexpected dependencies must fail loudly during migration verification.

- [ ] **Step 4: Delete scheduler routes, declarations, modules, services, and tests**

Use `apply_patch` to remove the complete Auto-Scheduling, Instructor Preferences, Instructor Room Assignments, Locked Slots, and Scheduling Constraints router blocks from `academic.rs`. Remove only these module declarations/re-exports:

```rust
// handlers.rs
pub mod scheduling;
pub mod scheduling_config;

// models.rs
pub mod scheduling;
pub mod scheduling_config;

// services.rs
pub mod scheduler;
pub mod scheduler_data;
pub mod scheduling_config_service;
pub mod scheduling_service;
#[cfg(test)]
mod scheduling_config_service_tests;
pub use scheduler::{types::SchedulingAlgorithm, SchedulerBuilder};
```

Delete every file listed in this task. Preserve `timetable*.rs`, `course_planning_service.rs`, and `exam_schedule_service.rs`.

- [ ] **Step 5: Remove scheduler operations and schemas from the Rust OpenAPI source**

In `backend-school/src/api_contract.rs`:

```rust
// Remove imports from models::scheduling and models::scheduling_config.
// Remove all seven handlers::scheduling_config path registrations.
// Remove scheduler-only component schemas and response wrappers.
// Delete academic_scheduling_configuration_contracts().
// Change every global operation-count assertion from 184 to 177.
```

Keep all non-scheduler path registrations and schemas byte-for-byte equivalent after formatting.

- [ ] **Step 6: Run backend formatting, focused architecture, API contract, and migration tests**

Run:

```bash
cd backend-school
cargo fmt --all
cargo test --test static_architecture auto_scheduler_backend_and_schema_are_removed_without_deleting_timetable_entries -- --exact
cargo test --bin backend-school api_contract::tests
cargo test --test static_architecture active_migrations_are_clean_sequential_timeline -- --exact
cargo check --all-targets --all-features
```

Expected: all commands PASS; the API tests report 177 operations and migration versions 001-028 remain contiguous.

- [ ] **Step 7: Commit the backend and migration removal**

```bash
git add backend-school
git commit -m "refactor(academic): remove auto-scheduler backend"
```

---

### Task 2: Remove generated contracts and frontend scheduler surfaces

**Files:**
- Modify: `contracts/openapi/school-api.json` (generated)
- Modify: `frontend-school/src/lib/api/generated/school-api.ts` (generated)
- Modify: `frontend-school/src/lib/api/scheduling.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`
- Delete: `frontend-school/src/routes/(app)/staff/academic/timetable/scheduling-config/+page.svelte`
- Delete: `frontend-school/src/routes/(app)/staff/academic/timetable/scheduling-config/+page.ts`
- Delete: `frontend-school/src/routes/(app)/staff/academic/timetable/scheduling/jobs/+page.svelte`
- Delete: `frontend-school/src/routes/(app)/staff/academic/timetable/scheduling/jobs/[jobId]/+page.svelte`
- Create: `frontend-school/tests/static/auto-scheduler-removal.test.mjs`
- Modify: `frontend-school/tests/static/api-response-contract.test.mjs`
- Modify: `frontend-school/tests/static/academic-activity-template-contract.test.mjs`
- Modify: `frontend-school/tests/static/academic-activity-workspace-contract.test.mjs`
- Modify: `frontend-school/tests/static/academic-curriculum-core-contract.test.mjs`
- Modify: `frontend-school/tests/static/academic-structure-mutation-contract.test.mjs`
- Modify: `frontend-school/tests/static/frontend-layout-components.test.mjs`
- Modify: `frontend-school/tests/static/frontend-state-components.test.mjs`
- Modify: `frontend-school/tests/static/mobile-drag-drop-loading.test.mjs`

**Interfaces:**
- Consumes: the 177-operation Rust OpenAPI source from Task 1.
- Produces: generated contract artifacts and frontend with no scheduler/config/job routes or calls; timetable templates remain available.

- [ ] **Step 1: Add a failing static feature-removal test**

Create `frontend-school/tests/static/auto-scheduler-removal.test.mjs`:

```javascript
import assert from 'node:assert/strict';
import { access, readFile } from 'node:fs/promises';
import { test } from 'node:test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const readRepo = (file) => readFile(path.join(repoRoot, file), 'utf8');
const exists = async (file) => access(path.join(repoRoot, file)).then(() => true, () => false);

test('auto scheduler frontend and generated contract are absent', async () => {
    for (const removed of [
        'frontend-school/src/routes/(app)/staff/academic/timetable/scheduling-config/+page.svelte',
        'frontend-school/src/routes/(app)/staff/academic/timetable/scheduling-config/+page.ts',
        'frontend-school/src/routes/(app)/staff/academic/timetable/scheduling/jobs/+page.svelte',
        'frontend-school/src/routes/(app)/staff/academic/timetable/scheduling/jobs/[jobId]/+page.svelte'
    ]) assert.equal(await exists(removed), false, removed);

    const [api, timetable, generated, contractText] = await Promise.all([
        readRepo('frontend-school/src/lib/api/scheduling.ts'),
        readRepo('frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte'),
        readRepo('frontend-school/src/lib/api/generated/school-api.ts'),
        readRepo('contracts/openapi/school-api.json')
    ]);
    const combined = `${api}\n${timetable}\n${generated}`;
    for (const removed of [
        'autoScheduleTimetable', 'SchedulingJobResponse', 'undoSchedulingJob',
        '/scheduling/jobs', '/scheduling/configuration', '/scheduling-config',
        'InstructorConstraintView', 'SaveSchedulingConfigurationRequest'
    ]) assert.doesNotMatch(combined, new RegExp(removed.replaceAll('/', '\\/')));

    const contract = JSON.parse(contractText);
    const operationIds = Object.values(contract.paths).flatMap((item) =>
        Object.values(item).flatMap((operation) => operation.operationId ?? [])
    );
    assert.equal(operationIds.length, 177);
    assert.equal(new Set(operationIds).size, 177);
    assert.equal(Object.keys(contract.paths).some((route) => route.includes('/scheduling/')), false);
});
```

- [ ] **Step 2: Run the new test and verify it fails**

```bash
cd frontend-school
node --test tests/static/auto-scheduler-removal.test.mjs
```

Expected: FAIL because frontend pages, wrappers, generated schemas, and seven contract paths still exist.

- [ ] **Step 3: Regenerate the OpenAPI and TypeScript artifacts from the reduced Rust source**

```bash
cd frontend-school
npm run generate:api-contracts
npm run check:api-contracts
```

Expected: generation succeeds and check mode reports no diff. Confirm the JSON contains 177 unique operation IDs.

- [ ] **Step 4: Delete scheduler pages and remove the manual-timetable entry point**

Delete the four route files listed above using `apply_patch`. In the surviving manual timetable page, remove the button whose handler is:

```svelte
onclick={() => goto(resolve('/staff/academic/timetable/scheduling-config'))}
```

Remove imports that become unused, but preserve every manual timetable drag/drop, save, clear, template, conflict, realtime, and export action.

- [ ] **Step 5: Reduce `scheduling.ts` to periods and timetable-template APIs**

Remove every declaration from the file beginning with `SchedulingAlgorithm`, `SchedulingStatus`, or `LockedSlotScope` through `SaveSchedulingConfigurationRequest`/`SchedulingConfigurationSaveResult`, while retaining `Period` and `DAY_OPTIONS`. Remove the block beginning at `// Constraints API` and ending immediately before `// Phase F: Timetable Templates`. Remove everything beginning at `// ==================== Legacy / Other API Functions` through end of file.

The surviving exported symbols must be exactly the existing `Period`, `DAY_OPTIONS`, `TimetableTemplateView`, `TimetableTemplateEntry`, `listTimetableTemplates`, `getTimetableTemplate`, `updateTimetableTemplate`, `deleteTimetableTemplate`, `createTemplateFromCurrent`, `applyTimetableTemplate`, `clearTimetable`, and `listPeriods`. Keep their existing implementations unchanged. Remove the generated `components` import and `Schemas` alias because no surviving type uses them. Do not rename the module in this removal change.

- [ ] **Step 6: Update frontend static inventories and contract counts**

Remove the deleted pages from layout/state/mobile test arrays. Replace the `scheduling configuration uses generated DTOs and one atomic save` test with a template-only assertion that does not read a deleted page:

```javascript
test('timetable template API keeps typed empty responses after scheduler removal', async () => {
    const schedulingApi = await readRepoFile('frontend-school/src/lib/api/scheduling.ts');
    assert.match(schedulingApi, /updateTimetableTemplate[\s\S]*apiClient\.put<Record<string, never>>/);
    assert.match(schedulingApi, /deleteTimetableTemplate[\s\S]*apiClient\.delete<Record<string, never>>/);
    assert.doesNotMatch(schedulingApi, /autoScheduleTimetable|SchedulingJobResponse|saveSchedulingConfiguration/);
});
```

Change global contract inventory assertions in the four focused contract tests from `184` to `177`. Leave the documentation-checkpoint test temporarily at 184 until Task 3 updates the documentation and its assertion together.

- [ ] **Step 7: Run Svelte analysis and frontend verification**

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/timetable/+page.svelte' --svelte-version 5
node --test tests/static/auto-scheduler-removal.test.mjs
npm run test:api-contracts
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:8080 PUBLIC_VAPID_KEY=test-sandbox-key npm run check
```

Expected: autofixer reports no unresolved issue; 4 API generator tests, all static tests, and Svelte check pass with 0 errors and 0 warnings.

- [ ] **Step 8: Commit frontend and generated contract removal**

```bash
git add contracts/openapi/school-api.json frontend-school
git commit -m "refactor(frontend): remove auto-scheduler surfaces"
```

---

### Task 3: Remove stale scheduler documentation and record the new checkpoint

**Files:**
- Modify: `.rules`
- Modify: `IMPROVEMENT_PLAN.md`
- Modify: `docs/TESTING.md`
- Modify: `docs/backend-school/API_DEVELOPMENT.md`
- Modify: `frontend-school/tests/static/api-response-contract.test.mjs`
- Delete: `docs/plans/AUTO_SCHEDULER_REDESIGN.md`
- Delete: `docs/superpowers/specs/2026-07-22-scheduling-configuration-contracts-design.md`
- Delete: `docs/superpowers/plans/2026-07-22-scheduling-configuration-contracts.md`

**Interfaces:**
- Consumes: removed code/schema and 177-operation contract from Tasks 1-2.
- Produces: active documentation that describes manual timetable editing as the sole timetable-construction workflow.

- [ ] **Step 1: Make the documentation checkpoint test expect the removal decision**

Replace the 184-operation scheduling-configuration checkpoint test with assertions equivalent to:

```javascript
test('project docs record the 177-operation manual timetable checkpoint', async () => {
    for (const file of ['.rules', 'IMPROVEMENT_PLAN.md', 'docs/TESTING.md', 'docs/backend-school/API_DEVELOPMENT.md']) {
        const source = await readRepoFile(file);
        assert.match(source, /177 unique operations/i);
        assert.match(source, /manual timetable/i);
        assert.doesNotMatch(source, /scheduling jobs\/undo.*next|auto[- ]scheduler.*supported/i);
    }
});
```

- [ ] **Step 2: Run the focused documentation test and verify it fails**

```bash
cd frontend-school
node --test --test-name-pattern="177-operation manual timetable checkpoint" tests/static/api-response-contract.test.mjs
```

Expected: FAIL because active documentation still records 184 operations and the scheduling-configuration rollout.

- [ ] **Step 3: Update active documentation and delete superseded plans**

Document these exact decisions in each active reference:

```text
The generated school API contract contains 177 unique operations.
Auto Schedule, Scheduling Configuration, scheduling jobs, and undo were intentionally removed.
Manual timetable editing is the only supported timetable-construction workflow.
Migration 028 preserves all academic_timetable_entries rows while removing scheduler-only metadata and schema.
```

Update M-7 in `IMPROVEMENT_PLAN.md` so it no longer proposes scheduling jobs/undo as the next contract batch. Delete the three superseded design/plan documents listed above using `apply_patch`; keep the new removal design and this implementation plan.

- [ ] **Step 4: Run documentation and full frontend static tests**

```bash
cd frontend-school
node --test --test-name-pattern="177-operation manual timetable checkpoint" tests/static/api-response-contract.test.mjs
npm run test:static
```

Expected: PASS; the full static inventory contains no current recommendation to restore the removed scheduler.

- [ ] **Step 5: Commit documentation cleanup**

```bash
git add .rules IMPROVEMENT_PLAN.md docs frontend-school/tests/static/api-response-contract.test.mjs
git commit -m "docs(academic): retire auto-scheduler guidance"
```

---

### Task 4: Prove migration preservation and complete verification

**Files:**
- No expected source changes; fix only defects exposed by verification and commit each focused fix separately.

**Interfaces:**
- Consumes: Tasks 1-3.
- Produces: reviewed, clean, merge-ready branch with evidence that timetable rows survive migration 028.

- [ ] **Step 1: Verify a 027-to-028 migration against a disposable PostgreSQL database**

From the repository root, start a disposable database, apply migrations 001-027, seed one manual and one scheduler-tagged timetable entry, then apply migration 028. Keep the password only in a shell variable:

```bash
removal_db_password=$(openssl rand -hex 24)
docker run --rm -d --name schoolorbit-auto-scheduler-removal-test \
  -e POSTGRES_USER=removal_test \
  -e POSTGRES_PASSWORD="$removal_db_password" \
  -e POSTGRES_DB=removal_test -P postgres:17-alpine
removal_db_port=$(docker port schoolorbit-auto-scheduler-removal-test 5432/tcp | sed -n 's/.*://p')
until PGPASSWORD="$removal_db_password" psql -h 127.0.0.1 -p "$removal_db_port" \
  -U removal_test -d removal_test -c 'SELECT 1' >/dev/null 2>&1; do sleep 1; done
PGPASSWORD="$removal_db_password" psql -h 127.0.0.1 -p "$removal_db_port" \
  -U removal_test -d removal_test \
  -c 'CREATE EXTENSION IF NOT EXISTS "uuid-ossp"; CREATE EXTENSION IF NOT EXISTS pg_trgm;'
for migration in backend-school/migrations/*.sql; do
  migration_version=$(basename "$migration" | cut -d_ -f1)
  if [ "$migration_version" -le 27 ]; then
    PGPASSWORD="$removal_db_password" psql -v ON_ERROR_STOP=1 \
      -h 127.0.0.1 -p "$removal_db_port" -U removal_test -d removal_test -f "$migration"
  fi
done
PGPASSWORD="$removal_db_password" psql -v ON_ERROR_STOP=1 \
  -h 127.0.0.1 -p "$removal_db_port" -U removal_test -d removal_test <<'SQL'
WITH inserted_year AS (
  INSERT INTO academic_years (year, name, start_date, end_date, is_active)
  VALUES (9901, 'Removal Test', '9901-01-01', '9901-12-31', true)
  RETURNING id
), inserted_semester AS (
  INSERT INTO academic_semesters (academic_year_id, term, name, start_date, end_date)
  SELECT id, '1', 'Removal Semester', '9901-01-01', '9901-06-30' FROM inserted_year
  RETURNING id, academic_year_id
), inserted_period AS (
  INSERT INTO academic_periods (academic_year_id, name, start_time, end_time, order_index)
  SELECT academic_year_id, 'Removal Period', '08:00', '08:50', 1 FROM inserted_semester
  RETURNING id
), inserted_job AS (
  INSERT INTO timetable_scheduling_jobs (academic_semester_id, classroom_ids)
  SELECT id, '[]'::jsonb FROM inserted_semester
  RETURNING id, academic_semester_id
), manual_entry AS (
  INSERT INTO academic_timetable_entries
    (day_of_week, period_id, academic_semester_id, entry_type, title)
  SELECT 'MON', p.id, s.id, 'BREAK', 'Manual entry'
  FROM inserted_period p CROSS JOIN inserted_semester s
  RETURNING id
)
INSERT INTO academic_timetable_entries
  (day_of_week, period_id, academic_semester_id, entry_type, title, scheduler_job_id)
SELECT 'TUE', p.id, j.academic_semester_id, 'BREAK', 'Former generated entry', j.id
FROM inserted_period p CROSS JOIN inserted_job j;

DO $$
BEGIN
  IF (SELECT COUNT(*) FROM academic_timetable_entries WHERE title IN ('Manual entry', 'Former generated entry')) <> 2 THEN
    RAISE EXCEPTION 'pre-migration timetable fixture is incomplete';
  END IF;
END $$;
SQL
PGPASSWORD="$removal_db_password" psql -v ON_ERROR_STOP=1 \
  -h 127.0.0.1 -p "$removal_db_port" -U removal_test -d removal_test \
  -f backend-school/migrations/028_remove_auto_scheduler.sql
```

Then assert:

```sql
SELECT COUNT(*) = 2 AS timetable_rows_preserved FROM academic_timetable_entries;
SELECT NOT EXISTS (
  SELECT 1 FROM information_schema.columns
  WHERE table_name = 'academic_timetable_entries' AND column_name = 'scheduler_job_id'
) AS provenance_column_removed;
SELECT to_regclass('public.timetable_scheduling_jobs') IS NULL AS jobs_table_removed;
```

Run those assertions through the same `psql` connection. Expected: all three values are `t`. Keep this named disposable container running for Step 2.

- [ ] **Step 2: Run backend verification with a migrated test database**

With `TEST_DATABASE_URL` pointing to the disposable sandbox database, run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
TEST_DATABASE_URL="postgresql://removal_test:${removal_db_password}@127.0.0.1:${removal_db_port}/removal_test" \
  cargo test --all-targets --all-features -- --test-threads=1
```

Expected: all backend and architecture tests pass with no warnings.

After the tests finish, stop only the disposable container:

```bash
docker stop schoolorbit-auto-scheduler-removal-test
```

- [ ] **Step 3: Run final contract and frontend verification**

```bash
cd frontend-school
npm run check:api-contracts
npm run test:api-contracts
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:8080 PUBLIC_VAPID_KEY=test-sandbox-key npm run check
```

Expected: generated files are current, 4 generator tests pass, all static tests pass, and Svelte reports 0 errors and 0 warnings.

- [ ] **Step 4: Run repository invariants**

```bash
git diff --check
test -z "$(git diff --name-only HEAD~3 -- backend-school/migrations | grep -v '^backend-school/migrations/028_remove_auto_scheduler.sql$')"
rg -n "autoScheduleTimetable|SchedulingJobResponse|timetable_scheduling_jobs|scheduling_config_service|/scheduling/configuration" backend-school/src frontend-school/src contracts/openapi .rules IMPROVEMENT_PLAN.md docs/TESTING.md docs/backend-school/API_DEVELOPMENT.md
git status --short --branch
```

Expected: diff check passes; only migration 028 changed in active migrations; `rg` returns no matches; worktree contains no uncommitted files.

- [ ] **Step 5: Review against the design**

Confirm every acceptance criterion in `docs/superpowers/specs/2026-07-23-auto-scheduler-removal-design.md`, with special attention to preservation of manual timetable rows, absence of old routes, 177 unique operations, and no stale frontend URLs. Address any Critical or Important issue before integration.

- [ ] **Step 6: Integrate only after fresh verification**

If working in an isolated worktree, fast-forward the verified feature branch into `main`. Do not force-push. Push `main` only when the project owner has authorized publication, then confirm local `HEAD` equals `origin/main` before deleting the worktree and feature branch.
