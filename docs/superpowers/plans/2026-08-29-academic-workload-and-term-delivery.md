# Academic Workload and Term Delivery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repair legacy curriculum workload values, prevent incomplete catalog publication, and give each course offering an explicit term-specific weekly period target that defaults from the catalog.

**Architecture:** Published catalog versions own official credit, standard periods per week, and official total hours. `course_offering_details` owns a positive `weekly_period_target`; every manual and curriculum-generated offering copies the catalog standard on the server, while draft offering updates may override the term target without mutating the catalog. Delivery read models expose both values, and the existing timetable remains unchanged.

**Tech Stack:** PostgreSQL tenant migrations, Rust/Axum/SQLx, Utoipa OpenAPI, generated TypeScript contracts, SvelteKit 5, Tailwind CSS, local shadcn-svelte primitives.

**Spec:** `docs/superpowers/specs/2026-08-29-academic-workload-and-term-delivery-design.md`

## Global Constraints

- Add migration `051`; never edit applied migrations 041–050.
- Use exactly 20 instructional weeks only for the provenance-scoped legacy repair.
- Existing non-null workload values are never overwritten.
- Course target overrides apply once per offering and never per learning group.
- New offerings always default on the server from `subject_versions.periods_per_week`; prior-term overrides are never copied.
- Do not change timetable completion logic, drag-and-drop, scheduling patterns, activity-to-period conversion, permissions, or realtime payloads.
- Rust DTOs and Utoipa own the wire contract; regenerate OpenAPI and TypeScript artifacts rather than editing them.
- Run commands sequentially. Use `CARGO_BUILD_JOBS=1` and `--test-threads=1` for focused Rust/database verification.
- UI direction: preserve the SchoolOrbit visual system and use a single paired workload signature, `ตามหลักสูตร 1 → จัดจริง 2 คาบ/สัปดาห์`, with Orbit blue `#0B65B1`, pale blue `#E9F3FC`, ink `#172033`, slate `#64748B`, amber `#D97706`, and card white `#FFFFFF`; inherit existing application typography and spacing.

---

### Task 1: Forward-only workload repair and offering target schema

**Files:**
- Create: `backend-school/migrations/051_academic_workload_and_term_delivery.sql`
- Modify: `backend-school/src/modules/academic/core/schema_tests.rs`

**Interfaces:**
- Consumes: migration-041 `migration_provenance`, `subject_versions.periods_per_week`, `subject_versions.hours_per_semester`, migration-048 `activity_versions.hours_per_term`, and migration-042 `course_offering_details`.
- Produces: `course_offering_details.weekly_period_target INTEGER NOT NULL CHECK (weekly_period_target > 0)` and migration provenance under `workloadRepair` with `migration: 51` and `instructionalWeeks: 20`.

- [ ] **Step 1: Write failing migration tests**

Add two database tests. The passing test reaches migration 050, intentionally creates missing legacy metrics while the existing immutable triggers are temporarily disabled, then asserts the literal outcomes:

```rust
#[tokio::test]
async fn migration_051_repairs_legacy_workload_and_backfills_offering_targets() {
    let pool = phase_a_fixture("academic_core_051_workload_repair").await;
    record_passing_phase_a_reconciliation_marker(&pool).await.unwrap();
    apply_migrations_through(&pool, 50).await.unwrap();

    sqlx::raw_sql(
        r#"
        ALTER TABLE subject_versions DISABLE TRIGGER subject_versions_published_immutable;
        UPDATE subject_versions
        SET hours_per_semester = 40, periods_per_week = NULL
        WHERE id = (SELECT subject_version_id FROM course_offering_details ORDER BY learning_offering_id LIMIT 1);
        ALTER TABLE subject_versions ENABLE TRIGGER subject_versions_published_immutable;

        ALTER TABLE activity_versions DISABLE TRIGGER activity_versions_published_immutable;
        UPDATE activity_versions SET hours_per_term = NULL;
        ALTER TABLE activity_versions ENABLE TRIGGER activity_versions_published_immutable;
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    apply_migrations_through(&pool, 51).await.unwrap();

    let periods: i32 = sqlx::query_scalar(
        "SELECT periods_per_week FROM subject_versions WHERE hours_per_semester = 40 ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(periods, 2);

    let activity_total: String = sqlx::query_scalar(
        "SELECT hours_per_term::text FROM activity_versions WHERE hours_per_week = 1 ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(activity_total, "20.00");

    let target: i32 = sqlx::query_scalar(
        "SELECT weekly_period_target FROM course_offering_details ORDER BY learning_offering_id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(target, 2);
}
```

Before applying migration 051, set one other activity's non-null `hours_per_term` to `22.00` and
leave one other subject's existing `periods_per_week = 3`; after the migration assert both literal
values are unchanged. Also assert the repaired rows preserve `migration_provenance.migration = 41`,
add `workloadRepair.migration = 51`, the new column is non-nullable, and update attempts against all
three published resources still raise their existing immutability errors.

The rollback test sets a curriculum/offering-referenced legacy subject to 45 hours with no periods, expects `ACADEMIC_CORE_051_SUBJECT_HOURS_NOT_DIVISIBLE`, then verifies the migration version remains 50 and the new column does not exist.

- [ ] **Step 2: Run the focused tests and verify RED**

Run:

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_051_repairs_legacy_workload_and_backfills_offering_targets \
  -- --exact --nocapture --test-threads=1
```

Expected: FAIL because migration 051 does not exist.

- [ ] **Step 3: Implement migration 051**

The migration must use this ordering:

```sql
ALTER TABLE course_offering_details
    ADD COLUMN weekly_period_target INTEGER,
    ADD CONSTRAINT course_offering_details_weekly_period_target_check
        CHECK (weekly_period_target IS NULL OR weekly_period_target > 0);

ALTER TABLE subject_versions DISABLE TRIGGER subject_versions_published_immutable;
ALTER TABLE activity_versions DISABLE TRIGGER activity_versions_published_immutable;
ALTER TABLE course_offering_details DISABLE TRIGGER course_offering_details_published_immutable;

UPDATE subject_versions
SET periods_per_week = hours_per_semester / 20,
    migration_provenance = migration_provenance ||
        jsonb_build_object('workloadRepair', jsonb_build_object(
            'migration', 51, 'instructionalWeeks', 20
        )),
    row_version = row_version + 1,
    updated_at = now()
WHERE migration_provenance @> '{"migration":41}'::jsonb
  AND periods_per_week IS NULL
  AND hours_per_semester > 0
  AND mod(hours_per_semester, 20) = 0;

UPDATE activity_versions
SET hours_per_term = hours_per_week * 20,
    migration_provenance = migration_provenance ||
        jsonb_build_object('workloadRepair', jsonb_build_object(
            'migration', 51, 'instructionalWeeks', 20
        )),
    row_version = row_version + 1,
    updated_at = now()
WHERE migration_provenance @> '{"migration":41}'::jsonb
  AND hours_per_term IS NULL
  AND hours_per_week > 0;
```

Add `DO` preflight blocks that raise bounded `ACADEMIC_CORE_051_*` errors for a referenced non-divisible subject, incomplete curriculum subject/activity metrics, or an offering whose subject cannot supply a positive target. Backfill every course offering from its referenced subject and merge the same `workloadRepair` provenance into `course_offering_details.migration_provenance`. Then set the column `NOT NULL`, re-enable all three triggers, and query `pg_trigger.tgenabled = 'O'` for all three names before completing.

- [ ] **Step 4: Run both migration tests and verify GREEN**

Run each focused test separately with the disposable database runner and `--test-threads=1`. Expected: PASS, including rollback and trigger restoration.

- [ ] **Step 5: Commit the schema task**

```bash
git add backend-school/migrations/051_academic_workload_and_term_delivery.sql \
  backend-school/src/modules/academic/core/schema_tests.rs
git commit -m "feat(academic): repair catalog workload metrics"
```

### Task 2: Catalog publication guard and official workload form

**Files:**
- Modify: `backend-school/src/modules/academic/core/services_tests.rs`
- Modify: `backend-school/src/modules/academic/core/services/catalog.rs`
- Modify: `frontend-school/src/lib/components/academic-core/CatalogVersionHistory.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/catalog/subjects/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/catalog/activities/+page.svelte`
- Modify: `frontend-school/tests/static/academic-catalog-ui.test.mjs`

**Interfaces:**
- Consumes: existing `CreateSubjectVersionRequest.hoursPerSemester` and `.periodsPerWeek` fields.
- Produces: server-authoritative subject publish validation and a subject form that sends positive official workload values instead of `null`.

- [ ] **Step 1: Write failing catalog tests**

Add a database-backed service test that creates a draft with `hours_per_semester: Some(40)` and `periods_per_week: None`, then asserts `publish_subject_version` returns a Thai validation error mentioning `คาบมาตรฐานต่อสัปดาห์`. Add a second draft with `hours_per_semester: None` and `periods_per_week: Some(2)` and assert the error mentions `ชั่วโมงรวมต่อภาคเรียน`.

Extend the existing catalog UI static test to require the visible labels `คาบมาตรฐานต่อสัปดาห์`, `ชั่วโมงรวมต่อภาคเรียน`, and `ภาระการเรียนตามหลักสูตร`, and to reject the old literal assignments `hoursPerSemester: null` and `periodsPerWeek: null`.

- [ ] **Step 2: Run focused tests and verify RED**

Run the Rust service test through the disposable database runner, then run:

```bash
cd frontend-school
node --test --test-name-pattern="subject catalog" tests/static/academic-catalog-ui.test.mjs
```

Expected: both fail because subject publication and the form still allow missing workload values.

- [ ] **Step 3: Implement the catalog publish guard**

Before calling generic `publish_version`, load the subject version and reject missing/non-positive credit, `periods_per_week`, or `hours_per_semester` with field-specific validation text. Do not require a fixed 20-week formula for newly authored versions.

- [ ] **Step 4: Implement the official workload form and presentation**

Extend `CatalogVersionDraft` with `standardPeriodsPerWeek: string`. For subject creation send:

```ts
hoursPerSemester: Number.parseInt(draft.totalValue, 10),
periodsPerWeek: Number.parseInt(draft.standardPeriodsPerWeek, 10)
```

Render the three subject values together in a quiet bordered workload group. Inputs use the local shadcn `Input`, with `type="number"`, `min="1"`, and integer steps for periods/hours. Keep the activity form at weekly hours plus total term hours. Add the same values to version-history summaries, the desktop subject table, and the mobile subject card.

- [ ] **Step 5: Run focused Rust, static, Svelte, and type checks**

Run the focused database service test, the catalog static test, Svelte autofixer for all three touched `.svelte` files, and the frontend `check` command. Expected: PASS with no Svelte issues.

- [ ] **Step 6: Commit the catalog task**

```bash
git add backend-school/src/modules/academic/core/services_tests.rs \
  backend-school/src/modules/academic/core/services/catalog.rs \
  frontend-school/src/lib/components/academic-core/CatalogVersionHistory.svelte \
  'frontend-school/src/routes/(app)/staff/academic/catalog/subjects/+page.svelte' \
  'frontend-school/src/routes/(app)/staff/academic/catalog/activities/+page.svelte' \
  frontend-school/tests/static/academic-catalog-ui.test.mjs
git commit -m "feat(academic): require official subject workload"
```

### Task 3: Course offering weekly target domain and read models

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/offerings.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/workspaces.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`

**Interfaces:**
- Consumes: migration-051 `course_offering_details.weekly_period_target` and catalog `subject_versions.periods_per_week`.
- Produces: `CourseOfferingSnapshot.standard_periods_per_week: i32`, `CourseOfferingSnapshot.weekly_period_target: i32`, optional course workload fields on `HomeroomDeliveryItem`, and `UpdateLearningOfferingRequest.weekly_period_target: Option<i32>`.

- [ ] **Step 1: Write the failing offering behavior test**

Add `course_offering_weekly_target_defaults_and_resets_per_term`. It must:

1. create a course offering and assert snapshot standard and target both equal the catalog literal `3`;
2. call `offerings::update` with the same owner/targets and `weekly_period_target: Some(2)`;
3. assert the returned target is `2`, the standard remains `3`, and the database subject value remains `3`;
4. create the same subject in another planning term in the same year and assert its target starts at `3`, not `2`;
5. assert a course update with `weekly_period_target: None` returns `AppError::ValidationError`.

- [ ] **Step 2: Run the focused delivery test and verify RED**

Run:

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::course_offering_weekly_target_defaults_and_resets_per_term \
  -- --exact --nocapture --test-threads=1
```

Expected: compile failure because the DTO and snapshot fields do not exist.

- [ ] **Step 3: Add typed DTO and read-model fields**

Add:

```rust
pub struct UpdateLearningOfferingRequest {
    pub row_version: i64,
    pub owning_organization_unit_id: Uuid,
    pub targets: Vec<OfferingTargetInput>,
    pub weekly_period_target: Option<i32>,
}

pub struct CourseOfferingSnapshot {
    // existing fields
    pub standard_periods_per_week: i32,
    pub weekly_period_target: i32,
}
```

Add `standard_periods_per_week: Option<i32>` and `weekly_period_target: Option<i32>` to `HomeroomDeliveryItem`, because activity rows have no period conversion in this release.

- [ ] **Step 4: Default and update the target in every service path**

Extend `CourseVersionSource` with `standard_periods_per_week: Option<i32>`. Manual and curriculum-generated inserts validate it is positive and write it to `weekly_period_target`. Hydration joins `subject_versions` and returns both values. Draft update requires `Some(value > 0)` for courses, rejects a supplied value for activities, updates `course_offering_details`, and increments the parent offering row version in the same transaction. Course publish validates that the stored target remains positive.

Extend the homeroom workspace query so course rows return both values while activity rows return null, then map them directly to `HomeroomDeliveryItem`.

- [ ] **Step 5: Run focused delivery and migration tests and verify GREEN**

Run the new delivery behavior test, migration-051 passing test, and existing mixed batch hydration test one at a time. Expected: PASS and no N+1 query additions.

- [ ] **Step 6: Commit the delivery domain task**

```bash
git add backend-school/src/modules/academic/delivery/models.rs \
  backend-school/src/modules/academic/delivery/services/offerings.rs \
  backend-school/src/modules/academic/delivery/services/workspaces.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs
git commit -m "feat(academic): add term weekly period targets"
```

### Task 4: Generated API contract and delivery UI

**Files:**
- Modify (generated): `contracts/openapi/school-api.json`
- Modify (generated): `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/tests/static/academic-core-cutover-contract.test.mjs`
- Modify: `frontend-school/tests/static/learning-delivery-workspace.test.mjs`
- Modify: `frontend-school/src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte`
- Modify: `frontend-school/src/lib/components/learning-delivery/HomeroomDeliveryWorkspace.svelte`
- Modify: `frontend-school/src/lib/components/learning-delivery/OfferingOverviewTable.svelte`

**Interfaces:**
- Consumes: the Rust DTO fields from Task 3 and existing typed `updateLearningOffering` wrapper.
- Produces: generated camelCase `standardPeriodsPerWeek` and `weeklyPeriodTarget` contracts plus clear, editable term-allocation UI for draft course offerings.

- [ ] **Step 1: Write failing contract and UI assertions**

In the OpenAPI contract test, assert `CourseOfferingSnapshot.required` includes both new fields, `UpdateLearningOfferingRequest.properties.weeklyPeriodTarget` is nullable/optional int32, and `HomeroomDeliveryItem` exposes both optional fields. In the delivery workspace test, require the user-facing labels `ตามหลักสูตร`, `จัดจริงภาคเรียนนี้`, and the `updateLearningOffering` call on the detail page.

- [ ] **Step 2: Run focused frontend tests and verify RED**

Run the two Node test files separately with `--test-name-pattern`. Expected: FAIL because generated contracts and UI do not contain the new boundary.

- [ ] **Step 3: Regenerate API contracts**

From `frontend-school`, run:

```bash
CARGO_BUILD_JOBS=1 npm run generate:api-contracts
```

Do not manually edit either generated artifact.

- [ ] **Step 4: Implement the delivery UI**

On the detail page, narrow `offering.snapshot.kind === 'course'`. Display a paired workload strip with official standard on the left and the term target on the right. For an authorized draft, render a positive integer shadcn `Input` and `บันทึกจำนวนคาบ` button. Reject a missing `owningOrganizationUnitId` with an actionable page error before saving, then save with:

```ts
const ownerId = offering.owningOrganizationUnitId;
if (!ownerId) {
    actionError = 'รายการเปิดสอนยังไม่มีหน่วยงานเจ้าของ จึงบันทึกจำนวนคาบไม่ได้';
    return;
}
await updateLearningOffering(offering.id, {
    rowVersion: offering.rowVersion,
    owningOrganizationUnitId: ownerId,
    targets: offering.targets.map((target) => ({
        targetKind: target.targetKind,
        homeroomId: target.homeroomId ?? null,
        gradeLevelId: target.gradeLevelId,
        studyProgramId: target.studyProgramId
    })),
    weeklyPeriodTarget: parsedTarget
});
```

The homeroom workspace and offering overview table show the paired text only for courses. When values match, say `ตามหลักสูตรและจัดจริง 3 คาบ/สัปดาห์`; when different, say `ตามหลักสูตร 1 · จัดจริงภาคเรียนนี้ 2 คาบ/สัปดาห์`. Activities remain unchanged.

- [ ] **Step 5: Validate Svelte and generated contracts**

Run Svelte autofixer on all three touched components, then `npm run check:api-contracts`, `npm run test:api-contracts`, the two focused static test files, and `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check`. Expected: PASS.

- [ ] **Step 6: Commit the contract and UI task**

```bash
git add contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  frontend-school/tests/static/academic-core-cutover-contract.test.mjs \
  frontend-school/tests/static/learning-delivery-workspace.test.mjs \
  'frontend-school/src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte' \
  frontend-school/src/lib/components/learning-delivery/HomeroomDeliveryWorkspace.svelte \
  frontend-school/src/lib/components/learning-delivery/OfferingOverviewTable.svelte
git commit -m "feat(academic): clarify term delivery workload"
```

### Task 5: Full verification and implementation review

**Files:**
- Review only: all files changed in Tasks 1–4

**Interfaces:**
- Consumes: the complete implementation and `.rules` verification matrix.
- Produces: a clean main-branch implementation ready for deployment review; no push occurs unless the user authorizes it.

- [ ] **Step 1: Run focused database tests serially**

Run both migration-051 tests, the catalog publish test, the course offering target test, and mixed offering hydration one at a time through `scripts/test_backend_school.sh`, always with `CARGO_BUILD_JOBS=1` and `--test-threads=1`.

- [ ] **Step 2: Run backend-school verification serially**

```bash
cd backend-school
cargo fmt --all -- --check
CARGO_BUILD_JOBS=1 cargo test --test static_architecture -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo check
```

- [ ] **Step 3: Run frontend and contract verification serially**

```bash
cd frontend-school
npm run check:api-contracts
npm run test:api-contracts
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

- [ ] **Step 4: Review migration safety and final diff**

Confirm migration 051 contains no tenant identifiers or secrets, all three triggers are restored, the timetable service has no diff, and no permission/realtime contract changed. Then run:

```bash
git diff --check
git status --short
git log -5 --oneline
```

- [ ] **Step 5: Commit any verification-only correction**

If verification required a source correction, commit only the corrected files with a message naming the actual behavior fixed. If no correction was needed, do not create an empty commit.
