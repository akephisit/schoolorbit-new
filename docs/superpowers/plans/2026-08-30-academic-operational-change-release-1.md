# Academic Operational Change Release 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cut the existing academic timetable and term-date runtime over to immutable effective-from timetable versions, version-owned operational period targets, optional planned term end dates, offering availability, and database-enforced teacher locking for published groups.

**Architecture:** Migration 052 performs one forward-only hard cutover: it creates deterministic initial timetable versions and targets, assigns every current timetable entry, installs immutable publication boundaries, migrates term end semantics, and removes the two obsolete runtime owners. Focused Rust services resolve one published version by date and require an explicit draft version for mutations; typed DTOs/OpenAPI/generated TypeScript carry that identity through the current frontend without adding the later operational-change UI.

**Tech Stack:** PostgreSQL/SQLx migrations, Rust/Axum/Serde/Utoipa, SvelteKit 5/TypeScript, generated OpenAPI contracts, Node static tests, disposable PostgreSQL test runner.

**Spec:** `docs/superpowers/specs/2026-08-30-academic-operational-change-and-timetable-versioning-design.md`

## Global Constraints

- Never edit migrations 001–051; add only `backend-school/migrations/052_academic_operational_timetable_versioning.sql`.
- Run all commands serially; do not overlap Rust, Node, frontend, Docker, or database commands.
- Do not add dual-read, dual-write, legacy DTO, fallback parser, feature flag, or per-tenant compatibility code.
- `subject_versions` remains the owner of official credit, hours, and standard periods; timetable versions own only operational weekly-period targets.
- Do not derive activity period targets from clock hours. Migration must fail with a bounded code when current timetable entries cannot produce one deterministic integer target.
- Published timetable versions and published-group teacher rows are immutable in both service and database layers.
- The term start is required; planned end is optional; actual end is populated only by a later term-close workflow.
- Rust DTOs plus Utoipa own the wire contract; regenerate OpenAPI and TypeScript rather than editing generated artifacts.
- Existing Learning Offering resource policies remain the timetable authorization boundary.
- Do not store or log plaintext national IDs, secrets, database URLs, raw request bodies, or roster PII in migration/audit diagnostics.
- Run the `.rules` change-type verification matrix and report any credentialed external gate as unrun unless its exact authorization is available.

---

### Task 1: Prove and implement the migration 052 hard-cutover schema

**Files:**
- Create: `backend-school/migrations/052_academic_operational_timetable_versioning.sql`
- Modify: `backend-school/src/modules/academic/core/schema_tests.rs`
- Test: `backend-school/src/modules/academic/core/schema_tests.rs`

**Interfaces:**
- Consumes: the post-051 schema, `academic_terms`, `learning_offerings`, `learning_groups`, `learning_group_teachers`, `academic_timetable_entries`, and `course_offering_details.weekly_period_target`.
- Produces: `planned_end_date`, `closed_on`, `learning_offerings.starts_on/ends_on`, `academic_term_change_sets`, `academic_term_change_items`, `academic_timetable_versions`, `academic_timetable_version_targets`, and non-null `academic_timetable_entries.timetable_version_id`.

- [ ] **Step 1: Add a failing migration success test**

Add `migration_052_versions_timetables_and_removes_obsolete_owners` after the migration 051 tests. The test must apply through 051, capture counts and representative values, apply 052, then assert exact preservation and final ownership:

```rust
#[tokio::test]
async fn migration_052_versions_timetables_and_removes_obsolete_owners() {
    let pool = phase_a_fixture("academic_core_052_timetable_versions").await;
    record_passing_phase_a_reconciliation_marker(&pool).await.unwrap();
    apply_migrations_through(&pool, 51).await.unwrap();

    let entry_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_entries",
    ).fetch_one(&pool).await.unwrap();
    let course_target_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM course_offering_details",
    ).fetch_one(&pool).await.unwrap();

    apply_migrations_through(&pool, 52).await.unwrap();

    let versioned_entry_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_entries WHERE timetable_version_id IS NOT NULL",
    ).fetch_one(&pool).await.unwrap();
    let migrated_target_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_version_targets target JOIN learning_offerings offering ON offering.id = target.learning_offering_id WHERE offering.kind = 'course'",
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(versioned_entry_count, entry_count);
    assert_eq!(migrated_target_count, course_target_count);
    assert!(!column_exists(&pool, "academic_terms", "end_date").await);
    assert!(!column_exists(&pool, "course_offering_details", "weekly_period_target").await);
}
```

Add a local `column_exists` test helper using `information_schema.columns`; keep it private to the schema test module.

- [ ] **Step 2: Add failing atomic-rejection tests**

Add two tests:

The first test, `migration_052_rejects_ambiguous_activity_targets_atomically`, must prepare a
post-051 fixture containing one published activity offering with two published groups. Insert one
active recurring timetable entry for the first group and two entries on distinct bell periods for
the second group. Apply 052 and assert:

```rust
let error = apply_migrations_through(&pool, 52)
    .await
    .expect_err("unequal group counts must not invent one activity target");
assert!(error
    .to_string()
    .contains("ACADEMIC_052_ACTIVITY_TARGET_AMBIGUOUS"));
let applied: i64 = sqlx::query_scalar(
    "SELECT coalesce(max(version), 0) FROM _sqlx_migrations",
)
.fetch_one(&pool)
.await
.unwrap();
assert_eq!(applied, 51);
assert!(!table_exists(&pool, "academic_timetable_versions").await);
```

Add `table_exists` beside `column_exists` using `information_schema.tables`.

The second test, `migration_052_rejects_published_teacher_mutability_after_cutover`, applies 052,
selects one published group and one draft group, then executes direct insert, update, and delete
statements against each. Every published-group statement must fail and contain
`ACADEMIC_PUBLISHED_GROUP_TEACHERS_IMMUTABLE`; the draft-group insert/update/delete sequence must
succeed. Query the final teacher rows and assert the published assignments are byte-for-byte
unchanged by comparing `(id, teacher_id, role)` tuples captured before the attempts.

- [ ] **Step 3: Run the exact failing tests**

Run from the repository root:

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_052_versions_timetables_and_removes_obsolete_owners \
  -- --exact --nocapture --test-threads=1
```

Expected: FAIL because migration 052 does not exist.

Run the two rejection tests one at a time with the same exact-test pattern. Expected: FAIL for the same missing migration.

- [ ] **Step 4: Write migration 052 schema and deterministic backfill**

Implement the migration in this order inside the SQLx migration transaction:

```sql
ALTER TABLE academic_terms
    RENAME COLUMN end_date TO planned_end_date;
ALTER TABLE academic_terms
    ALTER COLUMN planned_end_date DROP NOT NULL,
    ADD COLUMN closed_on date;

UPDATE academic_terms
SET closed_on = planned_end_date
WHERE status = 'closed';

ALTER TABLE learning_offerings
    ADD COLUMN starts_on date,
    ADD COLUMN ends_on date,
    ADD COLUMN stop_reason text,
    ADD COLUMN stopped_at timestamptz,
    ADD COLUMN stopped_by uuid REFERENCES users(id) ON DELETE RESTRICT,
    ADD COLUMN stop_change_set_id uuid;

UPDATE learning_offerings offering
SET starts_on = term.start_date
FROM academic_terms term
WHERE term.id = offering.academic_term_id;

ALTER TABLE learning_offerings
    ALTER COLUMN starts_on SET NOT NULL,
    ADD CONSTRAINT learning_offerings_availability_order_check
        CHECK (ends_on IS NULL OR starts_on <= ends_on);
```

Create `academic_term_change_sets` and `academic_term_change_items` with term/year composite foreign keys, `draft/published/cancelled` checks, positive `row_version`, effective date, required trimmed reason, idempotency key, actor/timestamp columns, and action-shape checks for `add_offering`, `stop_offering`, and `adjust_weekly_period_target`. Add the circular target/source-version and stop-change-set foreign keys only after all referenced tables exist.

Create version tables with the following authoritative keys:

```sql
CREATE TABLE academic_timetable_versions (
    id uuid PRIMARY KEY,
    academic_term_id uuid NOT NULL,
    academic_year_id uuid NOT NULL,
    effective_from date NOT NULL,
    status text NOT NULL CHECK (status IN ('draft', 'published', 'cancelled')),
    source_version_id uuid REFERENCES academic_timetable_versions(id) ON DELETE RESTRICT,
    change_set_id uuid UNIQUE REFERENCES academic_term_change_sets(id) ON DELETE RESTRICT,
    bell_schedule_id uuid NOT NULL REFERENCES bell_schedules(id) ON DELETE RESTRICT,
    row_version bigint NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_by uuid REFERENCES users(id) ON DELETE RESTRICT,
    published_by uuid REFERENCES users(id) ON DELETE RESTRICT,
    published_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (academic_term_id, academic_year_id)
        REFERENCES academic_terms(id, academic_year_id) ON DELETE RESTRICT
);

CREATE UNIQUE INDEX academic_timetable_versions_live_effective_key
    ON academic_timetable_versions(academic_term_id, effective_from)
    WHERE status IN ('draft', 'published');

CREATE TABLE academic_timetable_version_targets (
    timetable_version_id uuid NOT NULL REFERENCES academic_timetable_versions(id) ON DELETE CASCADE,
    learning_offering_id uuid NOT NULL,
    academic_term_id uuid NOT NULL,
    academic_year_id uuid NOT NULL,
    weekly_period_target integer NOT NULL CHECK (weekly_period_target > 0),
    migration_provenance jsonb NOT NULL DEFAULT '{}'::jsonb,
    PRIMARY KEY (timetable_version_id, learning_offering_id),
    FOREIGN KEY (learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES learning_offerings(id, academic_term_id, academic_year_id) ON DELETE RESTRICT
);
```

Generate one deterministic UUIDv5 initial published version for every term containing an offering or timetable entry. Use a fixed migration-052 namespace and the term ID in the name. Backfill course targets from migration 051. For an activity with active timetable entries, derive the per-group weekly count and require all active groups under that offering to have the same positive count; raise `ACADEMIC_052_ACTIVITY_TARGET_AMBIGUOUS:<offering-id>` otherwise. Do not derive from activity hours.

Add nullable `timetable_version_id`, backfill by term, make it non-null, and replace current slot/conflict indexes so every uniqueness scope begins with `timetable_version_id`. Keep historical inactive entries attached to the same deterministic initial version.

Install triggers that reject update/delete of a published timetable version, insert/update/delete of
timetable entries/targets for published versions, and insert/update/delete of
`learning_group_teachers` for published/closed groups. Trigger errors must be bounded codes without
row content. Publication itself is the one allowed `draft -> published` transition; no published
row may return to draft or change context/effective date.

Before dropping obsolete columns, run `DO`-block assertions for exact timetable-entry count, course-target count, non-null version links, one initial version per populated term, valid availability dates, and enabled trigger state. Then drop `course_offering_details.weekly_period_target`. The renamed term column is already the clean final owner and is not duplicated.

- [ ] **Step 5: Run the three exact migration tests**

Run each exact test serially. Expected: PASS, and the rejection tests must show migration version remains 51 for a failing fixture.

- [ ] **Step 6: Commit the migration boundary**

```bash
git add backend-school/migrations/052_academic_operational_timetable_versioning.sql \
  backend-school/src/modules/academic/core/schema_tests.rs
git commit -m "feat(academic): add timetable versioning schema"
```

---

### Task 2: Cut academic-term DTOs and services over to optional planned end

**Files:**
- Modify: `backend-school/src/modules/academic/core/models.rs`
- Modify: `backend-school/src/modules/academic/core/services/years_terms.rs`
- Modify: `backend-school/src/modules/academic/core/services/context.rs`
- Modify: `backend-school/src/modules/academic/core/services.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/activities.rs`
- Modify: `backend-school/src/bin/seed_sandbox.rs`
- Modify: `backend-school/src/api_contract.rs`
- Test: `backend-school/src/modules/academic/core/services_tests.rs`
- Test: `backend-school/src/modules/academic/delivery/services_tests.rs`

**Interfaces:**
- Produces: `AcademicTerm.planned_end_date: Option<NaiveDate>`, `AcademicTerm.closed_on: Option<NaiveDate>`, and matching request/option fields serialized as `plannedEndDate` and `closedOn`.
- Produces: `TermContext::date_upper_bound() -> NaiveDate`, using `closed_on.or(planned_end_date).unwrap_or(academic_year_end_date)` only for a feature that requires a bounded lookup.

- [ ] **Step 1: Write failing pure validation tests**

Add tests proving:

```rust
assert!(validate_term_fields("ภาคเรียนที่ 1", start, None).is_ok());
assert!(validate_term_fields("ภาคเรียนที่ 1", start, Some(start)).is_ok());
assert!(validate_term_fields("ภาคเรียนที่ 1", start, Some(start.pred_opt().unwrap())).is_err());
```

Add a delivery test proving activity eligibility uses the academic-year end when both planned and actual term end are absent.

- [ ] **Step 2: Run focused tests and verify failure**

```bash
./scripts/test_backend_school.sh modules::academic::core::services::tests -- --nocapture --test-threads=1
```

Expected: FAIL because request/model and validator signatures still require `end_date`.

- [ ] **Step 3: Replace term wire and row fields**

Change only academic-term fields; academic-year `end_date` remains required:

```rust
pub struct CreateAcademicTermRequest {
    // existing context/type fields
    pub start_date: NaiveDate,
    pub planned_end_date: Option<NaiveDate>,
    // existing result/bell fields
}

pub struct AcademicTerm {
    // existing identity fields
    pub start_date: NaiveDate,
    pub planned_end_date: Option<NaiveDate>,
    pub closed_on: Option<NaiveDate>,
    // existing workflow fields
}
```

Update create/update/list/get/context SQL to read/write `planned_end_date` and read `closed_on`. Validation must enforce `planned_end_date >= start_date` when present and containment within the academic year when present. Updating `closed_on` is not exposed by these setup DTOs.

- [ ] **Step 4: Update bounded term consumers**

Change Delivery `TermContext` to load `year.end_date AS academic_year_end_date`, optional planned end, and optional closed date. Replace activity registration overlap/clamping with `date_upper_bound()` so a missing plan does not block self-registration or choose an unbounded date. Do not change subject/activity catalog effective dates or academic-year containment.

Update sandbox seeds to insert `planned_end_date`; keep their concrete dates as planning estimates. Update context JSON builders to emit `plannedEndDate` and `closedOn`, never the retired `endDate` term field.

- [ ] **Step 5: Update OpenAPI assertions and run focused tests**

Update API contract tests to require `plannedEndDate` as nullable and `closedOn` as nullable on term responses and to reject retired `endDate` on term schemas. Run:

```bash
./scripts/test_backend_school.sh modules::academic::core::services_tests -- --nocapture --test-threads=1
./scripts/test_backend_school.sh modules::academic::delivery::services_tests -- --nocapture --test-threads=1
```

Expected: PASS.

- [ ] **Step 6: Commit the term contract cutover**

```bash
git add backend-school/src/modules/academic/core backend-school/src/modules/academic/delivery/services.rs \
  backend-school/src/modules/academic/delivery/services/activities.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs \
  backend-school/src/bin/seed_sandbox.rs backend-school/src/api_contract.rs
git commit -m "feat(academic): make planned term end optional"
```

---

### Task 3: Add timetable-version models, resolution, and clone services

**Files:**
- Create: `backend-school/src/modules/academic/models/timetable_version.rs`
- Create: `backend-school/src/modules/academic/services/timetable_version_service.rs`
- Create: `backend-school/src/modules/academic/services/timetable_version_service_tests.rs`
- Create: `backend-school/src/modules/academic/handlers/timetable_versions.rs`
- Modify: `backend-school/src/modules/academic/models.rs`
- Modify: `backend-school/src/modules/academic/services.rs`
- Modify: `backend-school/src/modules/academic/handlers.rs`
- Modify: `backend-school/src/modules/academic.rs`

**Interfaces:**
- Produces: `TimetableVersionStatus`, `TimetableVersionDisplayState`, `TimetableVersion`, `TimetableVersionTarget`, `TimetableVersionQuery`, `ResolveTimetableVersionQuery`, and `CloneTimetableVersionRequest`.
- Produces service signatures:

```rust
pub async fn list_versions(pool: &PgPool, term_id: Uuid) -> Result<Vec<TimetableVersion>, AppError>;
pub async fn resolve_for_date(pool: &PgPool, term_id: Uuid, on_date: NaiveDate) -> Result<TimetableVersion, AppError>;
pub async fn clone_draft(pool: &PgPool, actor_id: Uuid, source_id: Uuid, request: CloneTimetableVersionRequest) -> Result<TimetableVersion, AppError>;
```

- [ ] **Step 1: Write failing service tests**

Cover list ordering, current/upcoming/historical display derivation, date resolution at the exact effective boundary, no version before the first effective date, cloning entries and targets, stale source row version, and rejection when a term is closing/closed/cancelled.

- [ ] **Step 2: Run the focused test and verify failure**

```bash
./scripts/test_backend_school.sh modules::academic::services::timetable_version_service_tests -- --nocapture --test-threads=1
```

Expected: FAIL because the model/service modules do not exist.

- [ ] **Step 3: Implement typed models and pure display-state helper**

Use `#[serde(rename_all = "camelCase")]`, `deny_unknown_fields` for requests, `ToSchema`, and SQLx text enums. Derive display state from the requested/current date and the next published version; never store it in PostgreSQL.

- [ ] **Step 4: Implement list and date resolution**

Use one bounded query per operation. Resolution is:

```sql
SELECT id, academic_term_id, academic_year_id, effective_from, status,
       source_version_id, change_set_id, bell_schedule_id, row_version,
       created_by, published_by, published_at, created_at, updated_at
FROM academic_timetable_versions
WHERE academic_term_id = $1
  AND status = 'published'
  AND effective_from <= $2
ORDER BY effective_from DESC, id
LIMIT 1;
```

Return an actionable not-found error naming the selected date when no version applies.

- [ ] **Step 5: Implement atomic draft clone**

Lock the source version and term, validate row version and term writability, reject a duplicate effective date, insert one draft version, bulk-copy active entries with new IDs while preserving a deterministic old-to-new map for instructor child rows, and bulk-copy version targets. The new draft uses the selected term bell schedule and stores `source_version_id`.

- [ ] **Step 6: Add thin handlers and routes**

Register:

```text
GET  /api/academic/timetable-versions?academicTermId={academic_term_id}
GET  /api/academic/timetable-versions/resolve?academicTermId={academic_term_id}&date={yyyy-mm-dd}
POST /api/academic/timetable-versions/{source_id}/clone
```

List/resolve use Learning Offering read policy. Clone requires Learning Offering manage school because it copies the term-wide recurring schedule. Return standard `ApiResponse<T>` envelopes.

- [ ] **Step 7: Run focused service and static architecture tests**

```bash
./scripts/test_backend_school.sh modules::academic::services::timetable_version_service_tests -- --nocapture --test-threads=1
cd backend-school && cargo test --test static_architecture
```

Run the commands separately. Expected: PASS.

- [ ] **Step 8: Commit the version service**

```bash
git add backend-school/src/modules/academic/models backend-school/src/modules/academic/services \
  backend-school/src/modules/academic/handlers backend-school/src/modules/academic.rs
git commit -m "feat(timetable): add effective timetable versions"
```

---

### Task 4: Make every timetable entry operation version-aware

**Files:**
- Modify: `backend-school/src/modules/academic/models/timetable.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service_tests.rs`
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`
- Modify: `backend-school/src/modules/academic/services/daily_teaching_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_template_service.rs`
- Modify: `backend-school/src/modules/academic/handlers/timetable_templates.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/workspaces.rs`
- Modify: `backend-school/src/modules/parents/handlers.rs`
- Modify: `backend-school/src/modules/parents/services.rs`
- Modify: `backend-school/src/modules/supervision/services/observations.rs`

**Interfaces:**
- `TimetableEntry` adds `timetable_version_id: Uuid`.
- Every mutation request consumes `timetable_version_id: Uuid`; list/occupancy/validation queries consume an explicit version ID for editing views.
- Student/staff/parent date-based reads resolve a published version before listing entries.

- [ ] **Step 1: Write failing version-isolation tests**

Add tests proving the same homeroom/group/teacher/room slot may differ across two versions, conflicts remain blocking inside one version, published-version mutations fail, draft-version mutations pass, swap requires one version, and occupancy never merges versions.

- [ ] **Step 2: Run focused tests and verify failure**

```bash
./scripts/test_backend_school.sh modules::academic::services::timetable_service_tests -- --nocapture --test-threads=1
```

Expected: FAIL because entries and queries do not carry a version ID.

- [ ] **Step 3: Add version identity to DTOs and SQL rows**

Add `timetable_version_id` to `TimetableEntry`, `EntryRow`, `EntryLockRow`, and every create/batch/query/occupancy/validation request that operates on an editable pattern. Include it in all selects/inserts and stable slot-lock keys:

```rust
let key = format!("timetable:{version_id}:{day}:{period_id}");
```

- [ ] **Step 4: Enforce draft-only mutations**

Replace term-only write checks with a helper that locks the version, verifies it belongs to the request term/year and has status `draft`, then validates the owning term is writable. Create, update, deactivate, batch deactivate, swap, validate-move source, template apply, and clear must all use this helper.

- [ ] **Step 5: Scope conflicts and relationships by version**

Every conflict query and occupancy index loads only entries whose `timetable_version_id` equals the selected version. Relationship lookup still reads locked group teachers and homerooms; no teacher snapshot is introduced.

- [ ] **Step 6: Resolve published versions for personal/date reads**

Staff/student/parent routes that show an operational timetable accept a date, resolve one published version through `timetable_version_service::resolve_for_date`, and call a shared list-by-version service. A term-only legacy default is removed rather than silently choosing the latest draft.

Daily-teaching reads resolve by the requested teaching date. Supervision timetable options resolve by
the observation date. The homeroom Delivery workspace uses today's applicable published version when
the selected term is active, the earliest published version when the term is future, and the last
published version when the term is closed; it returns the selected version ID with its timetable
status projection.

- [ ] **Step 7: Update timetable template application**

Template definitions remain reusable and unversioned. Applying or clearing requires the explicit target draft version and writes only into that version. `from_current` reads an explicit published or draft source version.

- [ ] **Step 8: Run focused tests**

Run timetable service tests, timetable template service tests, and parent timetable focused tests separately. Expected: PASS.

- [ ] **Step 9: Commit the entry cutover**

```bash
git add backend-school/src/modules/academic backend-school/src/modules/parents
git commit -m "feat(timetable): scope entries to explicit versions"
```

---

### Task 5: Move operational period targets out of offerings

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/offerings.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/workspaces.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Modify: `backend-school/src/modules/academic/models/timetable_version.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_version_service.rs`
- Test: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Test: `backend-school/src/modules/academic/services/timetable_version_service_tests.rs`

**Interfaces:**
- Removes: `UpdateLearningOfferingRequest.weekly_period_target` and `CourseOfferingSnapshot.weekly_period_target`.
- Keeps: `CourseOfferingSnapshot.standard_periods_per_week`.
- Produces: `TimetableVersionTarget { timetable_version_id, learning_offering_id, weekly_period_target, standard_periods_per_week: Option<i32> }`.

- [ ] **Step 1: Write failing ownership tests**

Add tests proving a course's initial version target equals the catalog standard, cloning preserves the prior operational override, editing the offering cannot change a target, later-term preparation starts from the catalog standard, and catalog credit/hours/periods remain unchanged.

- [ ] **Step 2: Run the focused tests and verify failure**

Run Delivery and timetable-version service tests separately. Expected: FAIL because offering DTO/service still owns `weekly_period_target`.

- [ ] **Step 3: Remove the offering-level mutation and read field**

Delete the weekly-target branch from `offerings::update`, remove the DTO fields, and update hydration/workspace SQL so it no longer reads the dropped migration-051 column. Preserve `standardPeriodsPerWeek` from the exact subject version.

- [ ] **Step 4: Expose targets through timetable versions**

Hydrate targets in one set-based query for all listed versions. For a course, include the standard catalog value for comparison. For an activity, return `standardPeriodsPerWeek = null` and only the explicit operational target.

- [ ] **Step 5: Run focused ownership tests**

Expected: PASS and no runtime SQL references `course_offering_details.weekly_period_target`.

- [ ] **Step 6: Commit clean target ownership**

```bash
git add backend-school/src/modules/academic/delivery backend-school/src/modules/academic/models \
  backend-school/src/modules/academic/services
git commit -m "refactor(academic): make timetable versions own period targets"
```

---

### Task 6: Enforce teacher locking in the group service and UI contract

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/services/groups.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Modify: `backend-school/src/modules/academic/delivery/models.rs`

**Interfaces:**
- `replace_teachers` accepts only a draft group and writable term.
- Learning-group reads expose `teachers_locked: bool`, serialized as `teachersLocked`, derived from group status.

- [ ] **Step 1: Write failing service tests**

Add one test that replaces teachers on a draft group and one that attempts replacement after offering/group publication. The latter must return a conflict and preserve the exact assignment IDs, teachers, roles, and group row version.

- [ ] **Step 2: Run the exact tests and verify failure**

Expected: published replacement currently succeeds and the test fails.

- [ ] **Step 3: Add service-level draft enforcement**

Introduce `require_draft_group_teachers(&GroupLockRow)` and call it before deleting any teacher row. Keep optimistic row-version and active-staff validation. Return the Thai domain message `เผยแพร่กลุ่มเรียนแล้ว ไม่สามารถเปลี่ยนครูผู้สอนได้`.

- [ ] **Step 4: Expose the lock state and rerun tests**

Hydrate `teachers_locked` from `status != draft`. Run the focused Delivery tests and the migration trigger test. Expected: PASS.

- [ ] **Step 5: Commit the teacher boundary**

```bash
git add backend-school/src/modules/academic/delivery
git commit -m "feat(delivery): lock teachers after group publication"
```

---

### Task 7: Register and regenerate the API contract

**Files:**
- Modify: `backend-school/src/api_contract.rs`
- Modify generated: `contracts/openapi/school-api.json`
- Modify generated: `frontend-school/src/lib/api/generated/**`
- Test: `backend-school/src/api_contract.rs`
- Test: `scripts/tests/generate-api-contracts.test.mjs`

**Interfaces:**
- Registers all Release 1 term, timetable-version, target, and version-aware timetable entry DTOs and paths.
- Removes retired term `endDate` and offering `weeklyPeriodTarget` from the generated wire contract.

- [ ] **Step 1: Add failing API-document assertions**

Assert operation IDs for list/resolve/clone versions, required `timetableVersionId` on editable timetable requests, nullable `plannedEndDate`/`closedOn`, `teachersLocked`, target schema fields, and absence of retired owners.

- [ ] **Step 2: Run API contract tests and verify failure**

```bash
cd backend-school && cargo test api_contract::tests -- --nocapture
```

Expected: FAIL until schemas/paths are registered.

- [ ] **Step 3: Register paths and schemas**

Add new handlers to the OpenAPI path list and every named DTO to the components list. Update existing timetable operation schemas rather than adding compatibility alternatives.

- [ ] **Step 4: Regenerate tracked artifacts**

```bash
cd frontend-school && npm run generate:api-contracts
```

Do not edit generated JSON/TypeScript manually.

- [ ] **Step 5: Run contract gates serially**

```bash
cd frontend-school && npm run check:api-contracts
cd frontend-school && npm run test:api-contracts
cd backend-school && cargo test api_contract::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit the generated contract**

```bash
git add backend-school/src/api_contract.rs contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated
git commit -m "feat(api): publish timetable version contracts"
```

---

### Task 8: Cut the current frontend over to Release 1 contracts

**Files:**
- Modify: `frontend-school/src/lib/api/academic-core.ts`
- Modify: `frontend-school/src/lib/api/academic-context.ts`
- Modify: `frontend-school/src/lib/api/learning-delivery.ts`
- Modify: `frontend-school/src/lib/api/timetable.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/core/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/templates/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/templates/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/timetable/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/student/timetable/+page.svelte`
- Modify: `frontend-school/src/lib/components/supervision/SupervisionWorkspace.svelte`
- Modify: `frontend-school/src/lib/api/parents.ts`
- Test: `frontend-school/tests/static/academic-core-cutover-contract.test.mjs`
- Test: `frontend-school/tests/static/learning-delivery-workspace.test.mjs`
- Create: `frontend-school/tests/static/timetable-version-contract.test.mjs`

**Interfaces:**
- API wrappers consume generated DTOs only.
- Timetable page loads versions once, selects a current published or explicit draft version, and passes `timetableVersionId` to every existing query/mutation.
- Release 1 exposes read/select/clone plumbing only; Release 3 owns the complete version-management workspace.

- [ ] **Step 1: Write failing static contract tests**

Assert that term payloads use `plannedEndDate`, term UI labels the expected end as optional, offering UI no longer posts a weekly target, timetable API functions require a version ID, page loader performs one version request rather than per-row requests, and published groups do not render teacher mutation controls when `teachersLocked` is true.

- [ ] **Step 2: Run static tests and verify failure**

```bash
cd frontend-school && node --test tests/static/academic-core-cutover-contract.test.mjs
cd frontend-school && node --test tests/static/learning-delivery-workspace.test.mjs
cd frontend-school && node --test tests/static/timetable-version-contract.test.mjs
```

Run separately. Expected: FAIL on retired fields/missing version wrapper.

- [ ] **Step 3: Update typed API wrappers**

Map form view models explicitly to generated term requests. Add timetable functions:

```ts
export async function listTimetableVersions(academicTermId: string): Promise<TimetableVersion[]>;
export async function resolveTimetableVersion(academicTermId: string, date: string): Promise<TimetableVersion>;
export async function cloneTimetableVersion(sourceId: string, request: CloneTimetableVersionRequest): Promise<TimetableVersion>;
```

Require `timetableVersionId` in entry/list/occupancy/move/template wrappers. Remove all offering weekly-target mapping.

- [ ] **Step 4: Update academic-term form semantics**

The start date remains required. Rename the end input to `วันที่คาดว่าจะปิดภาคเรียน (ไม่บังคับ)`, allow empty value to serialize as `null`, and show `closedOn` read-only when present. Keep year end required.

- [ ] **Step 5: Update timetable selection plumbing**

Load versions after term context is known. Select, in order: explicit URL `timetableVersionId`, current published version for today's date, earliest upcoming published version, then a draft. Store the selected version in the URL and include it in every request. Published versions render read-only; current entry editing remains available only for drafts.

- [ ] **Step 6: Update Delivery detail**

Remove the offering-level weekly-target save control. Display standard periods and the selected timetable-version target when available. Hide teacher editing for `teachersLocked`; keep names visible.

- [ ] **Step 7: Analyze every edited Svelte file with Svelte tooling**

Invoke the required Svelte code analysis/autofix tool for each edited `.svelte` file and resolve all reported issues before continuing.

- [ ] **Step 8: Run focused frontend tests and type checks serially**

Run the three static tests from Step 2, then:

```bash
cd frontend-school && PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: PASS.

- [ ] **Step 9: Commit the frontend cutover**

```bash
git add frontend-school/src/lib/api frontend-school/src/routes/'(app)'/staff/academic \
  frontend-school/tests/static
git commit -m "feat(frontend): consume timetable versions"
```

---

### Task 9: Run Release 1 verification and record the deployment checkpoint

**Files:**
- Modify only if a durable recipe changed: `docs/TESTING.md` or `docs/OPERATIONS.md`
- Remove after implementation is recorded: `docs/superpowers/plans/2026-08-30-academic-operational-change-release-1.md`

**Interfaces:**
- Produces one verified Release 1 commit range ready for push/deployment before Release 2.

- [ ] **Step 1: Run migration 052 tests serially**

Run the three exact migration tests from Task 1, followed by:

```bash
./scripts/test_backend_school.sh -- --test-threads=1
```

Expected: PASS. Do not replace a failure with a narrower test.

- [ ] **Step 2: Run backend gates serially**

```bash
cd backend-school && cargo fmt --all -- --check
cd backend-school && cargo test --test static_architecture
cd backend-school && cargo test api_contract::tests -- --nocapture
cd backend-school && cargo check
```

Expected: PASS.

- [ ] **Step 3: Run frontend and contract gates serially**

```bash
cd frontend-school && npm run check:api-contracts
cd frontend-school && npm run test:api-contracts
cd frontend-school && npm run lint
cd frontend-school && PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
cd frontend-school && npm run test:menu-sync
cd frontend-school && npm run test:static
```

Expected: PASS.

- [ ] **Step 4: Run browser discovery**

```bash
cd frontend-school && npx playwright test --list \
  tests/e2e/academic-context.spec.ts \
  tests/e2e/staff-own-timetable-grid.spec.ts \
  tests/e2e/homeroom-delivery-workspace.spec.ts
```

Expected: discovery PASS. Execution is reported as unrun unless an isolated deployed target and dedicated credentials are explicitly available.

- [ ] **Step 5: Inspect the final repository state**

```bash
git diff --check
git status --short
git log --oneline --decorate -12
```

Review all Release 1 diffs for migration immutability, generated-artifact ownership, accidental PII, untyped API payloads, and unrelated user changes.

- [ ] **Step 6: Remove the completed temporary plan and commit**

After the implementation commits themselves preserve the completed outcome:

```bash
git rm docs/superpowers/plans/2026-08-30-academic-operational-change-release-1.md
git commit -m "docs(academic): retire completed release 1 plan"
```

- [ ] **Step 7: Push and verify the Release 1 deployment checkpoint**

Push `main` only after every local gate passes. Observe the existing automated backend/frontend deployment sequentially. Run authenticated read-only smoke checks for academic context, Delivery, timetable version listing/resolution, and an existing personal timetable on `sandbox`. Do not publish or mutate a live timetable during smoke verification. Keep the protected Neon snapshot until migration verification and smoke checks pass.
