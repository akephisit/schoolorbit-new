# Academic Core Cutover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to
> implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace SchoolOrbit's legacy academic-year, semester, curriculum, classroom, enrollment,
course-planning, and activity-delivery model with one explicit Academic Core and Learning Delivery
model, preserve valid tenant data, convert every current consumer, and remove the legacy runtime
schema without a compatibility layer.

**Architecture:** Stable catalog identities own immutable versions; configurable academic years and
terms provide explicit URL/API context; student-year records and placement history replace global
active enrollments; term offerings and learning groups provide the common delivery boundary for
courses and student-development activities. Release 1 is a hard cutover delivered through a
write-frozen Phase A migration/runtime artifact and a separately gated Phase B cleanup migration.

**Tech Stack:** PostgreSQL/SQLx sequential migrations, Rust/Axum/utoipa, generated permission and
OpenAPI/TypeScript contracts, SvelteKit 5 runes, shadcn-svelte, Node static tests, Playwright, and the
centralized all-tenant migration runner.

**Spec:** `docs/superpowers/specs/2026-08-23-academic-core-lifecycle-redesign-design.md`

## Global Constraints

- Read and follow `.rules`; migrations `001` through `040` are immutable.
- This plan implements Release 1 only. Gradebook score entry and result calculation are Release 2,
  term closing/activation is Release 3, and annual closure/promotion is Release 4.
- The new model is the only runtime model. Do not add old-route aliases, compatibility DTOs,
  dual-read, dual-write, per-tenant feature flags, database views that imitate legacy tables, or a
  fallback to `is_active` queries.
- Phase A contains migrations `041` through `044`, all new backend/frontend code, generated
  contracts, preflight, reconciliation, and operational gates. Phase B contains migration `045`
  and final legacy-schema guards. Production remains in maintenance mode between the phases.
- Migration `045` must be a separate reviewed cleanup pull request or release artifact. It must not
  be present in the Phase A image because the centralized runner applies every pending migration.
- Never run a production migration, snapshot, deployment, permission reassignment, smoke mutation,
  cleanup, push, pull request, or merge merely because this implementation plan was approved.
- Release 1 imports the currently active year and term but exposes no status mutation or activation
  endpoint. Schedule production cutover early enough that Release 3 is deployed before the next
  real term/year transition; otherwise postpone Release 1 production cutover.
- Academic-page reads never infer active state. Term-scoped endpoints require `academicTermId` and
  year-scoped endpoints require `academicYearId`; active IDs are defaults returned by the context
  API only.
- Selecting a Topbar context never changes a database status. The URL query string is the source of
  truth for the selected context and must support deep links, reload, and browser history.
- Preserve source UUIDs where a source row and target row represent the same entity. Use the fixed
  UUID-v5 namespace `5c33b984-10df-58db-bf80-62dbc4a03d1b` for split or generated entities.
- Preserve every valid source row. A source row must map, be classified by preflight as an empty
  obsolete draft, or block cutover. Do not silently discard data.
- Historical closed years receive migration provenance but no fabricated score, term-result, or
  year-result rows. They remain readable and are ineligible as promotion input.
- Use PostgreSQL `NUMERIC`, `bigdecimal::BigDecimal`, and validated decimal-string wire fields for
  credit, hour, weight, and score values. Do not introduce new authoritative `f32`/`f64` fields.
- New draft mutations require `rowVersion`; stale writes return HTTP 409. Published/closed data is
  not silently edited or deleted.
- Preserve parent/student isolation and current resource-policy semantics. Context IDs narrow an
  already-authorized result; they never grant access.
- Never log database URLs, secrets, raw request bodies, plaintext national IDs, learner names, or
  row-level PII from preflight/reconciliation. Reports contain finding codes, tenant/schema labels,
  counts, and non-sensitive resource IDs only.
- Author permission JSON and Rust DTOs first; regenerate permission/OpenAPI artifacts. Never edit
  generated artifacts directly.
- Before creating, editing, or analyzing any `.svelte`, `.svelte.ts`, or `.svelte.js` file, invoke
  `svelte:svelte-code-writer` and `svelte:svelte-core-bestpractices`, then run the Svelte analyzer and
  resolve every finding.
- Use `superpowers:using-git-worktrees` before implementation,
  `superpowers:test-driven-development` for every behavior change,
  `superpowers:requesting-code-review` at each Phase A/Phase B boundary, and
  `superpowers:verification-before-completion` before any completion claim.

## Release Boundaries

| Artifact | Contents | Database state after artifact | Traffic |
|---|---|---|---|
| Phase A | migrations 041-044, new runtime, new frontend, contracts | new tables authoritative and writable; inert legacy tables retained for reconciliation only | maintenance |
| Phase B | migration 045, final schema/static guards | legacy academic tables/columns removed | maintenance |
| Go-live | same Phase B backend/frontend image | only new schema exists | opened after smoke |

Phase A and Phase B are not independently usable product releases. They are checkpoints inside one
maintenance cutover. No new-system write is accepted until Phase B reconciliation and authenticated
smoke tests pass. Before that first write, rollback restores the captured snapshot and previous
release. After that first write, recovery is fix-forward or an explicitly approved snapshot restore
with post-cutover-write reconciliation.

## Canonical Release 1 Contracts

### Status and context types

```rust
pub enum AcademicYearStatus { Planning, Ready, Active, Closing, Closed, Archived }
pub enum AcademicTermStatus { Planning, Ready, Active, Closing, Closed, Cancelled }
pub enum AcademicTermType { Regular, Summer, Remedial, Custom }
pub enum AcademicContextRequirement { None, YearRequired, TermRequired, TermOptional }
pub enum LearningOfferingKind { Course, Activity }
pub enum LearningOfferingStatus { Draft, Published, Closed }
pub enum StudentAcademicYearStatus { Planned, Active, Completed, Withdrawn, Graduated }
```

Release 1 services may create and edit `planning` years/terms and `draft` delivery records. They may
publish offerings and rosters because current timetable, assessment, attendance, and parent views
need an authoritative operational snapshot. They may not transition year/term status.

### Explicit context API

```text
GET /api/academic/context/options

GET  /api/academic/years
POST /api/academic/years
GET  /api/academic/years/{yearId}
PATCH /api/academic/years/{yearId}

GET  /api/academic/terms?academicYearId={yearId}
POST /api/academic/terms
GET  /api/academic/terms/{termId}
PATCH /api/academic/terms/{termId}
DELETE /api/academic/terms/{termId}     planning and dependency-free only

GET  /api/academic/bell-schedules?academicYearId={yearId}
POST /api/academic/bell-schedules
GET  /api/academic/bell-schedules/{scheduleId}
PATCH /api/academic/bell-schedules/{scheduleId}
GET  /api/academic/bell-schedules/{scheduleId}/periods
PUT  /api/academic/bell-schedules/{scheduleId}/periods

GET  /api/academic/grade-progressions
PUT  /api/academic/grade-progressions
```

`POST` always creates `planning`; year/term `PATCH` DTOs contain no `status` field. The context
response is:

```rust
pub struct AcademicContextOptions {
    pub years: Vec<AcademicYearOption>,
    pub terms: Vec<AcademicTermOption>,
    pub active_academic_year_id: Option<Uuid>,
    pub active_academic_term_id: Option<Uuid>,
}
```

Every term option includes its owning year ID, sequence, type, status, dates, and inclusion/blocking
flags. The frontend filters terms from this one response; changing a year clears a term that does
not belong to the selected year.

### Catalog, curriculum, student-year, and delivery APIs

```text
GET|POST       /api/academic/catalog/subjects
GET|PATCH      /api/academic/catalog/subjects/{subjectId}
GET|POST       /api/academic/catalog/subjects/{subjectId}/versions
GET|PATCH      /api/academic/catalog/subject-versions/{versionId}
POST           /api/academic/catalog/subject-versions/{versionId}/publish
GET|PUT        /api/academic/catalog/subjects/{subjectId}/default-teachers
GET|POST       /api/academic/catalog/subject-groups
GET|PATCH|DELETE /api/academic/catalog/subject-groups/{groupId}

GET|POST       /api/academic/catalog/activities
GET|PATCH      /api/academic/catalog/activities/{activityId}
GET|POST       /api/academic/catalog/activities/{activityId}/versions
GET|PATCH      /api/academic/catalog/activity-versions/{versionId}
POST           /api/academic/catalog/activity-versions/{versionId}/publish
GET|PUT        /api/academic/catalog/activities/{activityId}/default-teachers

GET|POST       /api/academic/curricula
GET|PATCH      /api/academic/curricula/{curriculumId}
GET|POST       /api/academic/curricula/{curriculumId}/versions
GET|PATCH      /api/academic/curriculum-versions/{versionId}
POST           /api/academic/curriculum-versions/{versionId}/publish
GET|POST       /api/academic/curriculum-versions/{versionId}/programs
GET|PATCH      /api/academic/study-programs/{programId}
GET|PUT        /api/academic/study-programs/{programId}/requirements

GET|POST       /api/academic/homerooms?academicYearId={yearId}
GET|PATCH      /api/academic/homerooms/{homeroomId}
GET|PUT        /api/academic/homerooms/{homeroomId}/advisors
GET|POST       /api/academic/student-years?academicYearId={yearId}
GET|PATCH      /api/academic/student-years/{studentYearId}
POST           /api/academic/student-years/{studentYearId}/placements
POST           /api/academic/placements/{placementId}/transfer

GET|POST       /api/academic/offerings?academicTermId={termId}
POST           /api/academic/offerings/preview-from-curriculum
POST           /api/academic/offerings/apply-from-curriculum
GET|PATCH      /api/academic/offerings/{offeringId}
POST           /api/academic/offerings/{offeringId}/publish
GET|POST       /api/academic/offerings/{offeringId}/groups
GET|PATCH      /api/academic/learning-groups/{groupId}
GET|PUT        /api/academic/learning-groups/{groupId}/homerooms
GET|PUT        /api/academic/learning-groups/{groupId}/teachers
GET|PUT        /api/academic/learning-groups/{groupId}/roster
POST           /api/academic/learning-groups/{groupId}/roster/publish

GET            /api/students/me/academic-context
GET            /api/parents/students/{studentId}/academic-context
```

The student-year list uses filters in its query string and never accepts a bulk request body for a
read. Transfer closes the current placement and creates the next placement in one transaction.
Publishing an offering or roster snapshots source details and rejects stale `rowVersion` values.

### Route context metadata

Every staff academic `+page.ts` exports one of:

```ts
export const _meta = {
  // existing menu/access metadata,
  academicContext: 'term_required' as const
};
```

Use `none` for catalog/curriculum and the all-years core setup page; `year_required` for homerooms,
student-years, bell schedules, and admission placement; `term_required` for delivery, assessments,
timetable, exam scheduling, teacher timetable, and term activity operations; `term_optional` for
supervision and annual calendar views. Parent/student pages use page-local selectors and do not use
the staff Topbar switcher.

### Permission contract

Add these Release 1 permissions exactly:

```text
academic_context.read.school
academic_year.read.school
academic_year.manage.school
academic_term.read.school
academic_term.manage.school
academic_catalog.read.school
academic_catalog.manage.organization_unit
academic_catalog.manage.organization_tree
academic_catalog.manage.school
academic_curriculum.read.organization_unit
academic_curriculum.read.organization_tree
academic_curriculum.read.school
academic_curriculum.manage.organization_unit
academic_curriculum.manage.organization_tree
academic_curriculum.manage.school
homeroom.read.school
homeroom.manage.school
student_academic_year.read.school
student_academic_year.manage.school
learning_offering.read.assigned
learning_offering.read.organization_unit
learning_offering.read.organization_tree
learning_offering.read.school
learning_offering.manage.assigned
learning_offering.manage.organization_unit
learning_offering.manage.organization_tree
learning_offering.manage.school
```

Retain specialized `academic_assessment`, `academic_exam_schedule`, `academic_question_bank`, and
current timetable permissions where their semantics remain valid. Remove/deactivate legacy
`academic_structure.*.all`, `academic_classroom.*.all`, `academic_enrollment.*.all`,
`academic_course_plan.*.all`, old CRUD-style `academic_curriculum.*.all`, activity permissions that
own the superseded delivery model, and unimplemented coarse `academic_promotion.*.all` permissions
in Phase B. Do not create aliases.

### Deterministic migration identity

Use `uuid_generate_v5` and the fixed namespace from Global Constraints:

```text
stable subject       subject:<normalized-code>
stable activity      activity:<normalized-type>:<normalized-name>
default program      program:<legacy-curriculum-version-id>
student-year         student-year:<student-id>:<academic-year-id>
course offering      course-offering:<academic-term-id>:<subject-version-id>
generated group      activity-group:<activity-slot-id>:<homeroom-id>
default bell schedule bell-schedule:<academic-year-id>
```

Normalization is Unicode NFKC, trim, internal whitespace collapse, and lowercase for preflight.
Migration SQL performs the same transformation with
`lower(regexp_replace(normalize(btrim(...), NFKC), '\s+', ' ', 'g'))`. Preflight must compare Rust
and SQL mapping output on the same fixture before Phase A.

## Target Database Shape

### Migration 041 — core catalog and curriculum

- Alter `academic_years` in place: add status, `row_version`, migration provenance, and status/date
  checks; retain IDs and current descriptive columns.
- Rename `academic_semesters` to `academic_terms`; add `sequence_no`, stable code, term type,
  `included_in_year_result`, `blocks_year_closure`, status, `row_version`, and
  year/date/uniqueness checks.
- Rename legacy `subjects` to `subject_versions`; create stable `subjects`; backfill
  `subject_versions.subject_id`, effective dates, immutable/published state, and exact decimal
  credits/hours.
- Rename `subject_grade_levels` to `subject_version_grade_levels`; keep subject-group classification
  on the exact version; convert default-instructor relations to stable subject ownership.
- Rename `activity_catalog` to `activity_versions`; create stable `activities`; backfill stable IDs,
  effective dates, immutable/published state, and exact hours.
- Convert activity default-instructor relations to stable activity ownership.
- Rename `study_plans` to `curricula` and `study_plan_versions` to `curriculum_versions`; create one
  default `study_programs` row per legacy version; rename/backfill course and activity requirement
  tables against the default program.
- Create `grade_level_progressions`, backfill the legacy `next_grade_level_id` relationships, and
  leave the old column inert until migration 045.
- Create `bell_schedules`; rename `academic_periods` to `bell_schedule_periods`; backfill one
  deterministic default schedule per year and link each term to the owning-year schedule.
- Create `academic_audit_events` and `academic_core_cutover_audits`. Audit JSON excludes row-level
  PII and records migration/version provenance.
- Add deferred constraint triggers that serialize on the stable catalog row and reject overlapping
  subject/activity effective ranges without adding an unprovisioned PostgreSQL extension.

### Migration 042 — student-year and learning delivery

- Rename `class_rooms` to `homerooms`, preserve IDs, and replace the legacy plan-version reference
  with `study_program_id`.
- Create `student_academic_years`, `homeroom_placements`, and current-placement uniqueness. Preserve
  each legacy enrollment ID as its placement ID; generate deterministic student-year IDs.
- Create `learning_offerings`, `course_offering_details`, `activity_offering_details`,
  `learning_offering_targets`, `learning_groups`, `learning_group_homerooms`,
  `learning_group_teachers`, `learning_group_students`, and `learning_group_preferred_rooms`.
- Generate one course offering for each `(academic_term_id, subject_version_id)` represented by
  legacy `classroom_courses`; preserve each course row ID as its learning-group ID.
- Preserve each activity slot ID as an activity offering ID and each existing activity-group ID as
  a learning-group ID. Generate deterministic groups for independent slot/homeroom combinations
  that have no legacy group row.
- Backfill instructor, homeroom coverage, preferred-room, and authoritative roster relationships.
- Create the Release 1 minimum `learning_results` header and `activity_result_details`; backfill
  legacy activity-member pass/fail values without inventing course results. Release 2 extends this
  boundary.
- Keep `academic_core_entity_map` for Phase A reconciliation. It records source table/ID, target
  table/ID, mapping rule, and migration number; it contains no names or national IDs.
- Enforce same-year/same-term relationships through composite unique keys and composite foreign
  keys, not only application checks.
- Stable subjects, activities, curricula, and offerings carry nullable
  `owning_organization_unit_id`; migrated null owners are explicitly school-owned.

### Migration 043 — affected consumers and permissions

- Rename the assessment plan/category/item tables to `course_assessment_plans`,
  `course_assessment_categories`, and `course_assessment_items`; replace semester/subject coupling
  with `learning_offering_id`; convert all weight/score columns to explicit `NUMERIC` precision.
- Replace timetable course/classroom/semester references with academic-term, learning-group, and
  optional homeroom references while preserving entry IDs and instructor rows.
- Replace exam-round/item/session references with academic-term, offering/group, and new assessment
  plan references; add same-context constraints.
- Convert question-bank ownership to stable `subject_id`; instructor access joins through course
  offering details and learning-group teachers.
- Convert supervision cycles to year plus optional term and observations to learning-group plus
  optional homeroom; remove the duplicate free-text semester authority in migration 045.
- Convert admission tracks to `study_program_id`, room assignments to `homeroom_id`, and successful
  enrollment to student-year plus placement semantics.
- Convert parent, student, lookup, calendar, dashboard, daily-teaching, and certificate joins to the
  new core. Certificate campaigns retain `academic_year_id` because that table is transformed in
  place.
- Insert new permission definitions, map equivalent role/user grants without escalation, mark old
  definitions inactive, invalidate permission revisions, and write aggregate audit counts.
- Finish with SQL assertions that every source row has an entity-map row and every target foreign
  key resolves. Any mismatch raises and leaves migration 043 unapplied.

### Migration 044 — clean runtime contract

- Make transitional legacy term/period columns nullable so the clean API never has to manufacture
  compatibility values while Phase A is under maintenance.
- Add optimistic revision ownership for subject groups and the grade-progression rule set.
- Allow multiple study programs per curriculum version while enforcing at most one non-archived
  default program.
- Add stable catalog archival timestamps and let term audit references become null when an unused
  planning term is deleted; the audit entity ID and payload remain durable.
- Keep every legacy column/table in place until the separately gated destructive migration.

### Migration 045 — final cleanup

- Require a successful Phase A reconciliation marker for the tenant and the expected migration
  version before dropping anything.
- Drop old columns, constraints, helper tables, empty legacy junctions, obsolete status fields,
  legacy permission definitions/grants, and `academic_core_entity_map`.
- Keep `academic_core_cutover_audits` as the non-PII durable record of counts/checksums and cutover
  version.
- Assert no legacy relation or column from the cleanup manifest remains. Migration 045 is the only
  destructive schema step and is never included in the Phase A image.

## File Structure

### New backend and migration files

- `backend-school/migrations/041_academic_core_catalog.sql`
- `backend-school/migrations/042_academic_delivery_backfill.sql`
- `backend-school/migrations/043_academic_consumer_cutover.sql`
- `backend-school/migrations/044_academic_core_legacy_cleanup.sql` — Phase B only
- `backend-school/src/bin/preflight_academic_core.rs`
- `backend-school/src/modules/academic/cutover_preflight.rs`
- `backend-school/src/modules/academic/core.rs`
- `backend-school/src/modules/academic/core/models.rs`
- `backend-school/src/modules/academic/core/handlers.rs`
- `backend-school/src/modules/academic/core/services.rs`
- `backend-school/src/modules/academic/core/services/context.rs`
- `backend-school/src/modules/academic/core/services/years_terms.rs`
- `backend-school/src/modules/academic/core/services/bell_schedules.rs`
- `backend-school/src/modules/academic/core/services/progressions.rs`
- `backend-school/src/modules/academic/core/services/catalog.rs`
- `backend-school/src/modules/academic/core/services/curriculum.rs`
- `backend-school/src/modules/academic/core/services/student_years.rs`
- `backend-school/src/modules/academic/core/schema_tests.rs`
- `backend-school/src/modules/academic/core/services_tests.rs`
- `backend-school/src/modules/academic/delivery.rs`
- `backend-school/src/modules/academic/delivery/models.rs`
- `backend-school/src/modules/academic/delivery/handlers.rs`
- `backend-school/src/modules/academic/delivery/services.rs`
- `backend-school/src/modules/academic/delivery/services/offerings.rs`
- `backend-school/src/modules/academic/delivery/services/groups.rs`
- `backend-school/src/modules/academic/delivery/services/activities.rs`
- `backend-school/src/modules/academic/delivery/services_tests.rs`
- `backend-school/src/modules/academic/reconciliation.rs`
- `backend-school/src/policies/academic_catalog_access_policy.rs`
- `backend-school/src/policies/academic_curriculum_access_policy.rs`
- `backend-school/src/policies/learning_offering_access_policy.rs`

### New frontend and test files

- `frontend-school/src/lib/api/academic-context.ts`
- `frontend-school/src/lib/api/academic-core.ts`
- `frontend-school/src/lib/api/learning-delivery.ts`
- `frontend-school/src/lib/academic-context/types.ts`
- `frontend-school/src/lib/academic-context/route-context.ts`
- `frontend-school/src/lib/academic-context/store.ts`
- `frontend-school/src/lib/components/layout/AcademicContextSwitcher.svelte`
- `frontend-school/src/routes/(app)/staff/academic/core/+page.ts`
- `frontend-school/src/routes/(app)/staff/academic/core/+page.svelte`
- `frontend-school/src/routes/(app)/staff/academic/catalog/subjects/+page.ts`
- `frontend-school/src/routes/(app)/staff/academic/catalog/subjects/+page.svelte`
- `frontend-school/src/routes/(app)/staff/academic/catalog/activities/+page.ts`
- `frontend-school/src/routes/(app)/staff/academic/catalog/activities/+page.svelte`
- `frontend-school/src/routes/(app)/staff/academic/curricula/+page.ts`
- `frontend-school/src/routes/(app)/staff/academic/curricula/+page.svelte`
- `frontend-school/src/routes/(app)/staff/academic/homerooms/+page.ts`
- `frontend-school/src/routes/(app)/staff/academic/homerooms/+page.svelte`
- `frontend-school/src/routes/(app)/staff/academic/student-years/+page.ts`
- `frontend-school/src/routes/(app)/staff/academic/student-years/+page.svelte`
- `frontend-school/src/routes/(app)/staff/academic/delivery/+page.ts`
- `frontend-school/src/routes/(app)/staff/academic/delivery/+page.svelte`
- `frontend-school/tests/static/academic-core-cutover-contract.test.mjs`
- `frontend-school/tests/static/academic-context-contract.test.mjs`
- `frontend-school/tests/e2e/academic-context.spec.ts`
- `frontend-school/tests/e2e/academic-core-cutover.spec.ts`

### Primary modified areas

- `contracts/permissions.json` and all generated permission artifacts.
- `backend-school/Cargo.toml`, `backend-school/src/main.rs`, `backend-school/src/api_contract.rs`,
  `backend-school/src/modules/academic.rs`, `backend-school/src/modules/academic/{handlers,models,services}.rs`,
  `backend-school/src/policies.rs`, and `backend-school/tests/static_architecture.rs`.
- Existing academic assessment, activity, timetable, timetable-template, exam-schedule, daily-teaching,
  and websocket files under `backend-school/src/modules/academic/`.
- Admission, calendar, lookup, parent, question-bank, supervision, staff dashboard, student, and
  certificate services identified in Task 9.
- `frontend-school/src/lib/components/layout/Header.svelte`,
  `frontend-school/src/routes/(app)/+layout.svelte`, existing academic API wrappers, affected staff
  academic routes, teacher routes, and parent/student academic views identified in Tasks 11-13.
- `contracts/openapi/school-api.json` and
  `frontend-school/src/lib/api/generated/school-api.ts` through generation only.
- `docs/TESTING.md`, `docs/OPERATIONS.md`, and `TODO.md` at the final durable-documentation task.

## Execution Order and Review Gates

| Order | Task | Depends on | Review gate |
|---|---|---|---|
| 1 | Freeze cutover inventory and read-only preflight | approved plan | mapping rules and blocking findings |
| 2 | Migration 041 core catalog/curriculum | Task 1 | schema and deterministic identity |
| 3 | Migration 042 student-year/delivery | Task 2 | cardinality and cross-context integrity |
| 4 | Migration 043 consumers/permission data | Task 3 | all consumer FKs and grant equivalence |
| 5 | Permission source and resource policies | Task 4 | allowed/denied/union behavior |
| 6 | Academic Core backend | Task 5 | typed DTO/service/handler boundary |
| 7 | Learning Delivery backend | Task 6 | publishing and roster invariants |
| 8 | Academic consumer backend cutover | Task 7 | timetable/assessment/exam/activity |
| 9 | Cross-module backend cutover | Task 8 | admission/supervision/portals/lookups |
| 10 | OpenAPI and generated contracts | Tasks 6-9 | offline generation and exact DTOs |
| 11 | URL context and Topbar | Task 10 | no activation and unsaved-edit guard |
| 12 | New core/delivery frontend | Task 11 | permissions and exact context |
| 13 | Existing consumer frontend cutover | Task 12 | no legacy API usage |
| 14 | Phase A rehearsal, docs, and verification | Tasks 1-13 | Phase A release candidate |
| 15 | Phase B cleanup migration and final verification | successful Phase A production reconciliation | no legacy schema/runtime |

Tasks 1-14 form the Phase A pull request. Task 15 is a separate pull request and release artifact.
Do not begin production Task 15 merely because local Phase A tests pass.

---

### Task 1: Freeze the Legacy Inventory and Build Read-Only Preflight

**Files:**

- Create: `backend-school/src/bin/preflight_academic_core.rs`
- Create: `backend-school/src/lib.rs`
- Create: `backend-school/src/modules/academic/cutover_preflight.rs`
- Create: `backend-school/src/modules/academic/cutover_preflight_database_tests.rs`
- Create: `backend-school/src/modules/academic/cutover_test_support.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/src/main.rs`
- Test: `backend-school/src/modules/academic/cutover_preflight.rs`
- Test: `backend-school/tests/static_architecture.rs`

**Interfaces:**

```rust
pub struct AcademicCorePreflightReport {
    pub schema: String,
    pub generated_at: DateTime<Utc>,
    pub can_cut_over: bool,
    pub source_counts: BTreeMap<String, i64>,
    pub expected_target_counts: BTreeMap<String, i64>,
    pub findings: Vec<AcademicCorePreflightFinding>,
}

pub struct AcademicCorePreflightFinding {
    pub code: AcademicCorePreflightCode,
    pub severity: PreflightSeverity,
    pub affected_count: i64,
    pub resource_ids: Vec<Uuid>, // capped at 20; never learner/user IDs
    pub guidance_th: String,
}

pub async fn run_academic_core_preflight(
    pool: &PgPool,
    schema_label: &str,
    cutover_date: NaiveDate,
) -> Result<AcademicCorePreflightReport, PreflightError>;
```

The CLI accepts `PREFLIGHT_SCHEMA_DATABASE_URL`, `PREFLIGHT_SCHEMA_NAME`, optional
`PREFLIGHT_CUTOVER_DATE=YYYY-MM-DD`, and the explicit public-schema safety opt-in
`PREFLIGHT_SCHEMA_ALLOW_PUBLIC=1`. It uses a read-only transaction, validates the schema name with
the same ASCII identifier/public-schema guard as `migrate_tenant_schema`, writes one JSON document
to stdout, and exits `0` only when `canCutOver` is true. Connection strings and query rows never
appear in output or errors. A small library target exposes the shared runtime to the CLI without
pulling the backend binary's database-test modules into the command.

- [x] **Step 1: Add RED normalization and status-classification tests**

Add table-driven pure tests for NFKC/case/whitespace normalization and the exact legacy state rules:

```rust
#[test]
fn classifies_legacy_status_only_when_dates_and_active_flags_are_unambiguous() {
    assert_eq!(classify_year(false, date(2024, 5, 1), date(2025, 3, 31), date(2025, 8, 23)),
               Ok(AcademicYearStatus::Closed));
    assert_eq!(classify_year(false, date(2026, 5, 1), date(2027, 3, 31), date(2025, 8, 23)),
               Ok(AcademicYearStatus::Planning));
    assert_eq!(classify_year(true, date(2025, 5, 1), date(2026, 3, 31), date(2025, 8, 23)),
               Ok(AcademicYearStatus::Active));
    assert_eq!(classify_year(false, date(2025, 5, 1), date(2026, 3, 31), date(2025, 8, 23)),
               Err(AcademicCorePreflightCode::InactiveCurrentYearAmbiguous));
}
```

- [x] **Step 2: Run the focused tests and verify RED**

```bash
cd backend-school
cargo test modules::academic::cutover_preflight::tests --bin backend-school -- --nocapture
```

Expected: FAIL because the preflight module and classifiers do not exist.

- [x] **Step 3: Implement pure mapping rules and the legacy fixture helper**

`cutover_test_support.rs` is test-only and applies active migrations through an explicit inclusive
version using `sqlx::migrate::Migrator` plus `sqlx::migrate::Migrate`; it must assert that versions
`001`-`040` are contiguous before creating a legacy fixture. It exposes:

```rust
pub async fn apply_migrations_through(
    pool: &PgPool,
    version: i64,
) -> Result<(), Box<dyn Error + Send + Sync>>;
pub async fn seed_academic_cutover_fixture(pool: &PgPool, fixture: CutoverFixture) -> TestResult<()>;
```

Fixtures use synthetic UUIDs/names and contain no national IDs. Add fixtures for:

- two historical years, one active year, and one future year;
- two regular terms plus summer;
- two versions of one subject and activity;
- a curriculum with course/activity requirements;
- a learner with historical/current/future placements;
- synchronized and independent activity slots;
- assessment, timetable, exam, supervision, and admission references;
- one intentionally ambiguous variant per blocking finding family.

- [x] **Step 4: Implement all blocking and warning findings**

The preflight query set must emit these stable codes and no ad-hoc messages:

```text
ACTIVE_YEAR_COUNT_INVALID
ACTIVE_TERM_COUNT_INVALID
ACTIVE_YEAR_DATE_MISMATCH
ACTIVE_TERM_DATE_MISMATCH
INACTIVE_CURRENT_YEAR_AMBIGUOUS
INACTIVE_CURRENT_TERM_AMBIGUOUS
YEAR_DATE_RANGE_INVALID
TERM_DATE_RANGE_INVALID
TERM_OUTSIDE_YEAR
TERM_SEQUENCE_AMBIGUOUS
SUBJECT_IDENTITY_BLANK
SUBJECT_IDENTITY_CONFLICT
SUBJECT_VERSION_RANGE_OVERLAP
ACTIVITY_IDENTITY_CONFLICT
ACTIVITY_VERSION_RANGE_OVERLAP
CURRICULUM_VERSION_UNRESOLVED
ENROLLMENT_YEAR_CONFLICT
ENROLLMENT_STATUS_INVALID
HOMEROOM_PROGRAM_UNRESOLVED
COURSE_TERM_YEAR_MISMATCH
SYNCHRONIZED_ACTIVITY_PATTERN_CONFLICT
ACTIVITY_MEMBER_DUPLICATE
ASSESSMENT_REFERENCE_ORPHAN
TIMETABLE_REFERENCE_ORPHAN
EXAM_REFERENCE_ORPHAN
SUPERVISION_REFERENCE_ORPHAN
ADMISSION_PROGRAM_UNRESOLVED
PERMISSION_MAPPING_UNRESOLVED
HISTORICAL_RESULTS_UNAVAILABLE
```

`HISTORICAL_RESULTS_UNAVAILABLE` is a warning with counts only. Every other unresolved relationship
above is blocking. Preflight also calculates exact expected counts for stable identities, versions,
programs, student-years, placements, offerings, groups, rosters, assessment plans/items, timetable
entries, and exam items.

- [x] **Step 5: Add database-backed preflight tests**

Test one passing legacy fixture and one fixture for each finding family. Assert the passing report's
expected counts and deterministic IDs, and assert that a failed run performs no writes by comparing
table counts and checksums before/after.

```bash
./scripts/test_backend_school.sh \
  modules::academic::cutover_preflight_database_tests -- --nocapture --test-threads=1
```

Expected after implementation: PASS.

- [x] **Step 6: Add CLI and static privacy guards**

The CLI starts its transaction with `SET TRANSACTION READ ONLY`, caps non-sensitive sample IDs,
serializes camel-case JSON, and maps connection/query failures to bounded codes. Extend
`static_architecture.rs` to reject `println!`/`eprintln!` in the preflight library, SQL containing
national-ID columns, and debug formatting of `PgConnectOptions`/database URLs. Intentional final
JSON and bounded CLI errors use `std::io::Write`.

```bash
cd backend-school
cargo test --bin preflight_academic_core
cargo test --test static_architecture academic_core_preflight -- --nocapture
cargo fmt --all -- --check
```

- [x] **Step 7: Commit Task 1**

```bash
git add backend-school/src/bin/preflight_academic_core.rs \
  backend-school/src/lib.rs \
  backend-school/src/modules/academic/cutover_preflight.rs \
  backend-school/src/modules/academic/cutover_preflight_database_tests.rs \
  backend-school/src/modules/academic/cutover_test_support.rs \
  backend-school/src/modules/academic.rs backend-school/src/main.rs \
  backend-school/tests/static_architecture.rs
git commit -m "feat(academic): add cutover preflight"
```

---

### Task 2: Add Migration 041 for Academic Core, Catalog, and Curriculum

**Files:**

- Create: `backend-school/migrations/041_academic_core_catalog.sql`
- Create: `backend-school/src/modules/academic/core.rs`
- Create: `backend-school/src/modules/academic/core/schema_tests.rs`
- Modify: `backend-school/Cargo.toml`
- Modify: `backend-school/Cargo.lock`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/src/modules/academic/cutover_test_support.rs`
- Test: `backend-school/tests/static_architecture.rs`

**Interfaces:** Migration 041 produces the target tables and constraints listed under Target
Database Shape, plus reusable SQL functions `academic_normalize_identity(text)` and
`academic_assert_version_range()` used only by new-schema constraints.

- [x] **Step 1: Add RED schema and migration tests**

Add tests that apply through migration 040, seed the passing fixture, apply 041, and assert:

- source IDs are preserved for year, term, version, curriculum, and requirement rows;
- stable subject/activity/default-program IDs equal the Rust UUID-v5 mapping;
- year/term states equal preflight classifications;
- summer flags remain explicit and are not inferred from sequence number;
- every version points to one stable identity and no effective ranges overlap;
- credits/hours round-trip as `NUMERIC` without binary-float casts;
- published/version rows reject destructive edits through constraints/triggers;
- the preflight source counts equal migration audit source counts.

Add failure tests for blank/duplicate codes, overlapping ranges, ambiguous term sequence/date, and a
foreign key that crosses an academic year.

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_041_maps_core_fixture -- --exact --nocapture --test-threads=1
```

Expected: FAIL because migration 041 does not exist.

- [x] **Step 2: Write migration 041 preconditions**

Start migration 041 with SQL equivalents of every core preflight blocker. Each `RAISE EXCEPTION`
uses a stable code such as `ACADEMIC_CORE_041_SUBJECT_IDENTITY_CONFLICT`; do not include row names or
PII. Assert migration 040 is the current applied predecessor through SQLx ordering rather than
editing `_sqlx_migrations`.

- [x] **Step 3: Transform years and terms in place**

Add enum-like `CHECK` constraints with text columns, set migrated statuses, add partial unique
indexes for at most one active year and one active term, and add composite uniqueness
`(id, academic_year_id)` on terms. Term sequence is assigned by parsed numeric legacy term when
unambiguous, otherwise chronological `(start_date, end_date, id)` ordering verified by preflight.
Generate a stable uppercase code `TERM-<sequence>` only when the legacy value is not a usable unique
code.

- [x] **Step 4: Split stable catalog identities from versions**

Rename source tables before creating stable tables so PostgreSQL preserves existing foreign-key
targets until migration 043. Populate stable subjects by normalized code and stable activities by
normalized type/name. Add `effective_from`, nullable exclusive `effective_until`, `version_no`,
`status`, `row_version`, and migration provenance to version rows. Close each version range at the
next version's start date and leave the newest open.

Add deferred constraint triggers that lock the owning stable identity and reject overlap at commit.
Trigger errors contain stable codes and IDs, not names.

- [x] **Step 5: Transform curriculum and bell-schedule ownership**

Rename curricula/version/requirement tables, preserve source IDs, create deterministic default
programs, and retain legacy curriculum publication dates. Requirements point to exact subject or
activity versions and carry grade, recommended term, exact credit/hour, and requirement-kind
fields. Create bell schedules and attach all legacy periods and terms to the owning-year default
schedule.

- [x] **Step 6: Backfill progression and audit records**

Backfill permitted `promote` transitions from every non-null legacy next-grade value. Do not infer
repeat/graduate rules in Release 1. Write one aggregate `academic_core_cutover_audits` row containing
migration version, mapping algorithm version `academic-core-v1`, source/target counts, and SHA-256
checksums over sorted non-PII identifiers.

- [x] **Step 7: Run focused migration/schema verification**

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests -- --nocapture --test-threads=1
cd backend-school
cargo test --test static_architecture migration_timeline -- --nocapture
cargo fmt --all -- --check
```

Expected: PASS; the active migration timeline is contiguous through 041.

- [x] **Step 8: Commit Task 2**

```bash
git add backend-school/migrations/041_academic_core_catalog.sql \
  backend-school/src/modules/academic/core.rs \
  backend-school/src/modules/academic/core/schema_tests.rs \
  backend-school/src/modules/academic.rs \
  backend-school/src/modules/academic/cutover_test_support.rs \
  backend-school/tests/static_architecture.rs
git commit -m "feat(academic): migrate core catalog and curriculum"
```

---

### Task 3: Add Migration 042 for Student-Year and Learning Delivery

**Files:**

- Create: `backend-school/migrations/042_academic_delivery_backfill.sql`
- Create: `backend-school/src/modules/academic/delivery.rs`
- Create: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/src/modules/academic/core/schema_tests.rs`
- Modify: `backend-school/src/modules/academic/cutover_test_support.rs`
- Test: `backend-school/tests/static_architecture.rs`

**Interfaces:** Migration 042 creates student-year, placement, offering, group, teacher, coverage,
roster, preferred-room, and minimum activity-result tables plus `academic_core_entity_map`.

- [x] **Step 1: Add RED cardinality and integrity tests**

After applying 041 to the passing fixture, assert migration 042 produces:

```text
student_academic_years = distinct(student_id, academic_year_id) from legacy enrollments
homeroom_placements    = legacy enrollment rows
course offerings       = distinct(term_id, subject_version_id) from classroom_courses
course groups          = classroom_courses rows
activity offerings     = activity_slots rows
activity groups        = legacy groups + missing independent slot/homeroom groups
group students         = migrated course rosters + activity members without duplicates
```

Add negative tests for duplicate current placement, duplicate group student, course group attached
to a homeroom from another year, group/offering term mismatch, and a course/activity subtype mismatch.

```bash
./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::migration_042_maps_delivery_fixture -- --exact --nocapture --test-threads=1
```

Expected: FAIL because migration 042 does not exist.

- [x] **Step 2: Transform homerooms and placement history**

Rename `class_rooms` to `homerooms` and `classroom_advisors` to `homeroom_advisors`. Resolve each
homeroom's program from its exact legacy curriculum version's deterministic default program.
Create one student-year per source student/year and preserve every enrollment as a placement.

Map source statuses as follows:

```text
active + active year       -> active student-year/current placement
active + future year       -> planned student-year/planned current placement
completed + closed year    -> completed student-year/ended placement
transferred|moved_out      -> retained ended placement
dropped                    -> withdrawn student-year/ended placement when no later active row exists
```

Any other combination is a preflight/migration blocker. Do not end a current-year placement when a
future-year placement exists.

- [x] **Step 3: Create and backfill course delivery**

Generate one course offering per exact term/subject-version pair. Snapshot version name/code,
credits, hours, requirement source, and grading-policy metadata. Preserve each classroom-course ID
as a learning-group ID, link its homeroom, instructors, and preferred rooms, and populate its roster
from same-year placements whose known interval/status overlaps the offering term. Historical ended
placements remain eligible for their historical term; an interval that cannot be resolved is a
preflight blocker. Record roster source `migration_homeroom_snapshot` and published timestamp
derived from source timestamps.

- [x] **Step 4: Create and backfill activity delivery**

Preserve slot and group IDs as specified. A synchronized slot becomes one offering with all source
groups and shared schedule configuration. An independent slot creates a deterministic group per
assigned homeroom where no legacy group exists. Migrate instructors and members; reject one student
appearing twice in the same group. Convert pass/fail membership results to `learning_results` plus
`activity_result_details`, preserving source timestamps and marking provenance `legacy_activity`.

- [x] **Step 5: Add database-enforced subtype and context invariants**

Use deferred constraint triggers for the exactly-one-subtype invariant and composite foreign keys
for year/term consistency. Partial unique indexes enforce one current placement and one active roster
membership per group/student. Published offering snapshots reject edits to version, term, kind,
targets, and requirement source.

- [x] **Step 6: Populate entity mappings and reconcile inside migration**

Insert one map row for every renamed, split, generated, or merged source row. For merged course
offerings, multiple classroom-course map rows may target the same offering but every source group
maps uniquely. End migration 042 with count assertions matching preflight expectations and write
aggregate audit checksums.

- [x] **Step 7: Run focused tests**

```bash
./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests -- --nocapture --test-threads=1
cd backend-school
cargo test --test static_architecture migration_timeline -- --nocapture
cargo fmt --all -- --check
```

- [x] **Step 8: Commit Task 3**

```bash
git add backend-school/migrations/042_academic_delivery_backfill.sql \
  backend-school/src/modules/academic/delivery.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs \
  backend-school/src/modules/academic.rs \
  backend-school/src/modules/academic/core/schema_tests.rs \
  backend-school/src/modules/academic/cutover_test_support.rs \
  backend-school/tests/static_architecture.rs
git commit -m "feat(academic): migrate student years and delivery"
```

---

### Task 4: Add Migration 043 for Consumers and Permission Data

**Files:**

- Create: `backend-school/migrations/043_academic_consumer_cutover.sql`
- Create: `backend-school/src/modules/academic/reconciliation.rs`
- Modify: `backend-school/src/modules/academic/core/schema_tests.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Modify: `backend-school/src/modules/academic/cutover_preflight.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Test: `backend-school/tests/static_architecture.rs`

**Interfaces:**

```rust
pub struct AcademicCutoverReconciliation {
    pub migration_version: i64,
    pub passed: bool,
    pub checks: Vec<ReconciliationCheck>,
}

pub async fn reconcile_academic_core_cutover(
    pool: &PgPool,
) -> Result<AcademicCutoverReconciliation, AppError>;
```

The internal migration-status response adds only aggregate `academicCoreCutover` state: migration
version, pass/fail, check codes, and counts. It never returns entity-map rows.

- [x] **Step 1: Add RED consumer-mapping tests**

Seed every affected consumer in the legacy fixture, apply 041-042, then require 043 to preserve IDs
and counts for assessment plans/categories/items, timetable entries/instructors, exam rounds/days/
items/sessions/room assignments, supervision cycles/observations, question-bank questions,
admission tracks/assignments, daily-teaching references, and certificate year joins.

For decimals, assert values such as `7.25`, `12.50`, and `0.10` round-trip exactly. Add negative tests
for cross-term exam/group references, assessment/offering mismatch, unresolved admission program,
and an old permission grant with no declared mapping.

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_043_maps_all_consumers -- --exact --nocapture --test-threads=1
```

Expected: FAIL because migration 043 does not exist.

- [x] **Step 2: Transform assessment, timetable, and exam ownership**

Rename assessment tables and bind each plan to the deterministic course offering for its legacy
term/subject-version. Preserve category/item IDs and ordering. Convert numeric columns with explicit
rounding checks that raise when source precision cannot be represented.

Replace timetable and exam foreign keys using entity-map joins, validate every relation shares one
term, and then make the new key columns non-null where the row kind requires them. Generic break or
homeroom timetable entries may have no group but must have a homeroom or explicit entry kind.

- [x] **Step 3: Transform supervision, question bank, admission, and portals**

Map question-bank version IDs to stable subject IDs. Map supervision free-text semester only when it
agrees with its academic-semester relation; otherwise block. Map admission tracks through the
curriculum-version default program selected for the round year and assignments to homerooms.
Introduce new admission columns before retiring old ones in 044.

Add new context columns/indexes needed by parent, student, calendar, lookup, dashboard, and teaching
queries. Do not materialize names or PII into academic audit rows.

- [x] **Step 4: Insert and map permissions transactionally**

Insert the exact permission contract from this plan. Map grants by capability:

```text
academic_structure.read.all              -> context/year/term/catalog read.school
academic_structure.manage.all            -> year/term/catalog manage.school
academic_classroom.*.all                  -> homeroom read/manage.school by action
academic_enrollment.read/update.all       -> student_academic_year read/manage.school
academic_course_plan.read/manage.all      -> learning_offering read/manage.school
academic_curriculum.read.all              -> academic_curriculum.read.school
academic_curriculum create/update/delete  -> academic_curriculum.manage.school
academic_curriculum organization scopes  -> same action/scope in new family
activity.read.all                         -> academic_catalog.read.school + learning_offering.read.school
activity.manage.all                       -> academic_catalog.manage.school + learning_offering.manage.school
activity.manage_members.all               -> learning_offering.manage.school
activity.manage.own                       -> learning_offering.manage.assigned
```

Read never maps to manage. Assigned/unit/tree grants retain their exact scope. Do not map promotion
permissions to any Release 1 capability. Grant `academic_context.read.school` to every principal
that retains at least one staff academic read/manage capability so the selector cannot block an
otherwise authorized workspace; this grants context labels only, not academic records. Insert target
grants with `ON CONFLICT DO NOTHING`, compare source/target distinct principal counts, and raise on
an unmapped active grant.

- [x] **Step 5: Implement reconciliation checks**

The Rust reconciliation service independently recomputes migration SQL assertions and returns stable
codes for source-to-target counts, orphan counts, cross-context counts, permission principal counts,
active-state uniqueness, and sorted-ID checksums. Add tests that deliberately tamper with one target
row after migration and assert reconciliation fails with the matching code.

- [x] **Step 6: Wire aggregate reconciliation into internal migration status**

Modify the existing internal migration-status handler/service so an operator can verify every tenant
after 043 without querying entity rows. Keep this read-only and protected by the existing internal
service identity. A tenant not yet on 043 reports `notApplicable`, not success.

- [x] **Step 7: Run focused tests and permission-data assertions**

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests -- --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  modules::academic::reconciliation::tests -- --nocapture --test-threads=1
cd backend-school
cargo test --test static_architecture migration_timeline -- --nocapture
cargo fmt --all -- --check
```

- [x] **Step 8: Commit Task 4**

```bash
git add backend-school/migrations/043_academic_consumer_cutover.sql \
  backend-school/src/modules/academic/reconciliation.rs \
  backend-school/src/modules/academic/core/schema_tests.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs \
  backend-school/src/modules/academic/cutover_preflight.rs \
  backend-school/src/modules/academic.rs \
  backend-school/src/modules/system/handlers/migration.rs \
  backend-school/tests/static_architecture.rs
git commit -m "feat(academic): migrate consumers and permission data"
```

---

---

### Task 5: Replace the Permission Contract and Add Resource Policies

**Files:**

- Modify: `contracts/permissions.json`
- Generate: `contracts/permissions.lock.json`
- Generate: `backend-school/src/permissions/registry_generated.rs`
- Generate: `frontend-school/src/lib/permissions/registry.generated.ts`
- Create: `backend-school/src/policies/academic_catalog_access_policy.rs`
- Create: `backend-school/src/policies/academic_curriculum_access_policy.rs`
- Create: `backend-school/src/policies/learning_offering_access_policy.rs`
- Modify: `backend-school/src/policies/resource_access_policy.rs`
- Modify: `backend-school/src/policies.rs`
- Modify: `backend-school/src/modules/academic/core/schema_tests.rs`
- Test: `backend-school/src/policies/academic_catalog_access_policy.rs`
- Test: `backend-school/src/policies/academic_curriculum_access_policy.rs`
- Test: `backend-school/src/policies/learning_offering_access_policy.rs`
- Test: `backend-school/tests/static_architecture.rs`

**Interfaces:**

```rust
pub enum AcademicResourceAccess { None, Assigned, OrganizationUnit, OrganizationTree, School }

pub async fn academic_catalog_access(
    pool: &PgPool,
    actor: &ActorContext,
    resource: CatalogResourceRef,
    action: CatalogAction,
) -> Result<AcademicResourceAccess, AppError>;

pub async fn learning_offering_access(
    pool: &PgPool,
    actor: &ActorContext,
    offering_id: Uuid,
    action: OfferingAction,
) -> Result<AcademicResourceAccess, AppError>;
```

Stable catalogs, curricula, and offerings carry nullable `owning_organization_unit_id`. A null owner
means school-owned and requires school scope. New organization-scoped records receive an explicitly
selected authorized owner; ownership is never inferred from `is_primary`. Assigned delivery access
comes only from `learning_group_teachers` or another explicit assignment table.

- [x] **Step 1: Add RED contract and policy tests**

Write tests requiring every exact permission in Canonical Release 1 Contracts and rejecting the old
permission constants after generation. Add policy tests for:

- school scope accessing school-owned and organization-owned resources;
- exact organization-unit scope;
- explicit organization-tree scope using `resource_access_policy.rs`;
- assigned teacher access to an offering;
- union of assigned + unit + tree results in list filters;
- no access for unrelated staff;
- read permission cannot mutate;
- null school-owned resource is not exposed to organization-only actors.

```bash
cd frontend-school
npm run test:permissions
cd ../backend-school
cargo test policies::academic_catalog_access_policy --bin backend-school -- --nocapture
cargo test policies::learning_offering_access_policy --bin backend-school -- --nocapture
```

Expected: FAIL because the source contract and policies are still legacy.

- [x] **Step 2: Edit the handwritten permission source**

Add exactly the 27 permission definitions from this plan with Thai names/descriptions. Remove the
legacy definitions from `contracts/permissions.json` only after migration 043 contains their DB/grant
mapping. Keep specialized assessment/exam/question-bank/timetable entries unchanged.

- [x] **Step 3: Generate registries and lock**

```bash
cd frontend-school
npm run generate:permissions
npm run check:permissions
npm run test:permissions
```

Do not hand-edit any generated output.

- [x] **Step 4: Implement reusable resource policies**

Use `ActorContext` and shared organization-tree decisions. Policies return a typed decision/filter,
not SQL fragments from handlers. List services apply the union of independent scopes; only school
scope may short-circuit. Mutation policies verify the resource owner and action-specific manage
permission.

- [x] **Step 5: Test migration-to-contract equivalence**

Extend schema tests to parse the generated lock and compare active academic permission codes after
043. Assert each migrated principal gains only the target capabilities declared in Task 4. Assert
old promotion grants produce no new capability.

- [x] **Step 6: Run permission verification**

```bash
cd frontend-school
npm run check:permissions
npm run test:permissions
cd ../backend-school
cargo test policies::academic_catalog_access_policy --bin backend-school -- --nocapture
cargo test policies::academic_curriculum_access_policy --bin backend-school -- --nocapture
cargo test policies::learning_offering_access_policy --bin backend-school -- --nocapture
cargo test --test static_architecture -- --nocapture
cargo fmt --all -- --check
```

**Maintenance-only checkpoint review (2026-08-23):** no blocking Task 4-5 finding. Keep
academic traffic closed until all three pre-go-live integration gaps below are removed without
legacy compatibility:

- Task 6/8 must make scoped learning-offering endpoints consume
  `learning_offering_access_policy`; transitional handlers still require school scope.
- Task 6 must replace the transitional curriculum read path with
  `academic_curriculum_access_policy`, including unit-read and school-manage-as-read semantics.
- Task 8 must remove the transitional activity registration-open shortcut; assigned scope must
  require an explicit teacher/group assignment before any slot-assignment data is reachable.

- [x] **Step 7: Commit Task 5**

```bash
git add contracts/permissions.json contracts/permissions.lock.json \
  backend-school/src/permissions/registry_generated.rs \
  frontend-school/src/lib/permissions/registry.generated.ts \
  backend-school/src/policies.rs \
  backend-school/src/policies/academic_catalog_access_policy.rs \
  backend-school/src/policies/academic_curriculum_access_policy.rs \
  backend-school/src/policies/learning_offering_access_policy.rs \
  backend-school/src/modules/academic/core/schema_tests.rs \
  backend-school/tests/static_architecture.rs
git commit -m "feat(academic): replace academic permission contract"
```

---

### Task 6: Implement the Academic Core Backend

**Files:**

- Modify: `backend-school/Cargo.toml`
- Create: `backend-school/src/modules/academic/core/models.rs`
- Create: `backend-school/src/modules/academic/core/handlers.rs`
- Create: `backend-school/src/modules/academic/core/services.rs`
- Create: `backend-school/src/modules/academic/core/services/context.rs`
- Create: `backend-school/src/modules/academic/core/services/years_terms.rs`
- Create: `backend-school/src/modules/academic/core/services/bell_schedules.rs`
- Create: `backend-school/src/modules/academic/core/services/progressions.rs`
- Create: `backend-school/src/modules/academic/core/services/catalog.rs`
- Create: `backend-school/src/modules/academic/core/services/curriculum.rs`
- Create: `backend-school/src/modules/academic/core/services/student_years.rs`
- Create: `backend-school/src/modules/academic/core/services_tests.rs`
- Modify: `backend-school/src/modules/academic/core.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/src/modules/academic/websockets.rs`
- Modify: `backend-school/src/api_contract.rs`
- Test: `backend-school/tests/static_architecture.rs`

**Interfaces:** Implement all explicit context, catalog, curriculum, homeroom, student-year, and
placement endpoints defined earlier. Add `bigdecimal = { version = "0.4", features = ["serde"] }`
and the SQLx `bigdecimal` feature. Wire DTO decimal fields are validated canonical strings and
OpenAPI documents them as strings with a decimal pattern.

- [ ] **Step 1: Add RED model/validation tests**

Test enum serialization in `snake_case`, camel-case DTO fields, decimal validation, date containment,
term code/sequence uniqueness messages, optimistic row-version parsing, immutable published
versions, and planning-only delete rules.

```bash
cd backend-school
cargo test modules::academic::core::services_tests --bin backend-school -- --nocapture
```

Expected: FAIL because core models/services do not exist.

- [ ] **Step 2: Implement typed models and pure validators**

Define separate wire DTOs and DB rows. Requests use `#[serde(rename_all = "camelCase",
deny_unknown_fields)]`; stable responses expose IDs/statuses/provenance state but no dynamic metadata
unless it has a named type. `UpdateAcademicYearRequest` and `UpdateAcademicTermRequest` omit status.
All mutable requests include `row_version`.

- [ ] **Step 3: Implement context and year/term services**

`context::list_options` returns all authorized contexts in one bounded query ordered by year
descending and term sequence ascending. It reads active IDs only to suggest defaults.

Year/term services create planning records, validate dates/flags, use `UPDATE ... WHERE row_version
= $expected`, return conflict on zero updated rows, and append non-PII audit events. Term deletion
requires planning status and zero dependent offerings/schedules/assessment/exam/supervision rows.
Bell-schedule services validate non-overlapping ordered periods and same-year term selection.
Progression services replace a complete validated transition set transactionally and reject an
invalid grade/curriculum scope.

- [ ] **Step 4: Implement catalog and curriculum services**

Stable identity updates affect only stable code/archival metadata permitted by policy. Version detail
updates are draft-only and effective-range checked in the transaction. Publishing a curriculum
version validates at least one program, no duplicate requirement, valid version ranges, and exact
credit/hour values, then freezes version/program/requirements atomically.

List services accept typed filters and resource-policy decisions. Do not put `sqlx::query*` in
handlers.

- [ ] **Step 5: Implement student-year and placement services**

Creating a future student-year never queries for or closes rows from another academic year. Enforce
one student/year row. Placement transfer locks the student-year and current placement, validates
same-year target homeroom/program, ends the old placement, creates the new one, appends an audit
event, and returns both rows. An idempotency key prevents a retried transfer from duplicating rows.

- [ ] **Step 6: Implement thin handlers and route registration**

Handlers perform session context, exact permission/policy, service call, `ApiResponse`, and a bounded
`academic_core_changed` signal. Register the clean paths only. Remove the old structure/year-active/
semester/classroom/enrollment/subject/study-plan handlers and routes after their replacements compile;
do not retain aliases.

- [ ] **Step 7: Add service and HTTP tests**

Cover allowed/denied access, union-scoped listing, future-year coexistence, stale update conflict,
published immutability, planning-only deletion, same-year placement enforcement, transfer retry,
and context options with no active term. Assert the context endpoint performs no update and emits no
activation event.

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::services_tests -- --nocapture --test-threads=1
cd backend-school
cargo test --test static_architecture -- --nocapture
cargo fmt --all -- --check
cargo check
```

- [ ] **Step 8: Commit Task 6**

```bash
git add backend-school/Cargo.toml backend-school/src/modules/academic/core.rs \
  backend-school/src/modules/academic/core/models.rs \
  backend-school/src/modules/academic/core/handlers.rs \
  backend-school/src/modules/academic/core/services.rs \
  backend-school/src/modules/academic/core/services/context.rs \
  backend-school/src/modules/academic/core/services/years_terms.rs \
  backend-school/src/modules/academic/core/services/bell_schedules.rs \
  backend-school/src/modules/academic/core/services/progressions.rs \
  backend-school/src/modules/academic/core/services/catalog.rs \
  backend-school/src/modules/academic/core/services/curriculum.rs \
  backend-school/src/modules/academic/core/services/student_years.rs \
  backend-school/src/modules/academic/core/services_tests.rs \
  backend-school/src/modules/academic.rs \
  backend-school/src/modules/academic/websockets.rs \
  backend-school/src/api_contract.rs backend-school/tests/static_architecture.rs
git commit -m "feat(academic): implement academic core API"
```

---

### Task 7: Implement the Learning Delivery Backend

**Files:**

- Create: `backend-school/src/modules/academic/delivery/models.rs`
- Create: `backend-school/src/modules/academic/delivery/handlers.rs`
- Create: `backend-school/src/modules/academic/delivery/services.rs`
- Create: `backend-school/src/modules/academic/delivery/services/offerings.rs`
- Create: `backend-school/src/modules/academic/delivery/services/groups.rs`
- Create: `backend-school/src/modules/academic/delivery/services/activities.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Modify: `backend-school/src/modules/academic/delivery.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/src/modules/academic/websockets.rs`
- Modify: `backend-school/src/api_contract.rs`
- Test: `backend-school/tests/static_architecture.rs`

**Interfaces:** Implement offering/group endpoints defined in Canonical Release 1 Contracts. Course
and activity create requests are tagged enums so a course request cannot carry activity details and
vice versa.

```rust
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CreateLearningOfferingRequest {
    Course(CreateCourseOfferingRequest),
    Activity(CreateActivityOfferingRequest),
}

pub struct PublishRosterRequest {
    pub row_version: i64,
    pub idempotency_key: Uuid,
}
```

- [ ] **Step 1: Add RED service tests**

Test exactly-one-subtype, selected term/year consistency, version effective date, organization owner,
duplicate group codes, teacher assignment, homeroom coverage, roster overrides, stale publish,
idempotent publish, curriculum preview/apply source-hash conflict, and closed-term write rejection.

```bash
./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests -- --nocapture --test-threads=1
```

Expected: FAIL because delivery services are not implemented.

- [ ] **Step 2: Implement offering draft and publish services**

Course creation snapshots the exact subject version, requirement, credit/hour, and grading policy.
Activity creation snapshots the exact activity version, assignment mode, capacity, attendance rule,
and pass criteria. Both require an explicit term and owner. Publishing validates effective range,
targets, at least one valid group, and optimistic version, then freezes snapshot fields atomically.

Implement curriculum generation as `preview-from-curriculum -> apply-from-curriculum`. Preview
resolves both course and activity requirements for the selected term/year/programs and returns
create/retain/conflict items plus a source-version hash. Apply locks the curriculum version and term,
verifies the hash, and idempotently creates draft offerings/targets. It never copies a prior term's
scores, results, attendance, teaching logs, exams, or supervision records.

- [ ] **Step 3: Implement groups, teachers, and homeroom coverage**

All group operations verify the group and offering share a term. Teacher writes require explicit
role and prevent duplicate active assignment. Homeroom coverage requires the homeroom's year to
match the term's year. Preferred rooms remain planning hints and are validated against active room
resources.

- [ ] **Step 4: Implement draft and published rosters**

Draft generation previews students from current placements in covered homerooms and returns added,
removed, retained, and conflict counts. Apply requires the preview source hash and writes a draft
roster. Manual elective overrides are explicit. Publish verifies no duplicate student, enrollment in
the same academic year, capacity policy, and stale hash; it marks the roster authoritative without
changing homeroom placements.

- [ ] **Step 5: Implement activity-specific delivery semantics**

Support assigned and self-registration modes, synchronized and independent schedules, capacity, and
membership state through common group/roster tables. Preserve activity pass/fail result reads from
the minimum Release 1 result tables. Do not add course scoring or GPA calculation.

- [ ] **Step 6: Implement handlers, policies, and realtime signal**

Handlers use `learning_offering_access_policy`, return typed envelopes, and emit one
`learning_delivery_changed` signal carrying term/offering/group IDs and revision only. The signal
contains no roster/student data.

- [ ] **Step 7: Run focused verification**

```bash
./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests -- --nocapture --test-threads=1
cd backend-school
cargo test --test static_architecture -- --nocapture
cargo fmt --all -- --check
cargo check
```

- [ ] **Step 8: Commit Task 7**

```bash
git add backend-school/src/modules/academic/delivery.rs \
  backend-school/src/modules/academic/delivery/models.rs \
  backend-school/src/modules/academic/delivery/handlers.rs \
  backend-school/src/modules/academic/delivery/services.rs \
  backend-school/src/modules/academic/delivery/services/offerings.rs \
  backend-school/src/modules/academic/delivery/services/groups.rs \
  backend-school/src/modules/academic/delivery/services/activities.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs \
  backend-school/src/modules/academic.rs \
  backend-school/src/modules/academic/websockets.rs \
  backend-school/src/api_contract.rs backend-school/tests/static_architecture.rs
git commit -m "feat(academic): implement learning delivery API"
```

---

### Task 8: Cut Over Assessment, Timetable, Exam, and Activity Consumers

**Files:**

- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/src/modules/academic/handlers.rs`
- Modify: `backend-school/src/modules/academic/models.rs`
- Modify: `backend-school/src/modules/academic/services.rs`
- Modify: `backend-school/src/modules/academic/handlers/assessment.rs`
- Modify: `backend-school/src/modules/academic/models/assessment.rs`
- Modify: `backend-school/src/modules/academic/services/assessment_service.rs`
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`
- Modify: `backend-school/src/modules/academic/handlers/timetable_templates.rs`
- Modify: `backend-school/src/modules/academic/models/timetable.rs`
- Modify: all files under `backend-school/src/modules/academic/services/timetable_service/`
- Modify: `backend-school/src/modules/academic/services/timetable_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_template_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_realtime_service.rs`
- Modify: `backend-school/src/modules/academic/services/period_service.rs`
- Modify: `backend-school/src/modules/academic/services/daily_teaching_service.rs`
- Modify: `backend-school/src/modules/academic/handlers/exam_schedule.rs`
- Modify: `backend-school/src/modules/academic/models/exam_schedule.rs`
- Modify: all files under `backend-school/src/modules/academic/services/exam_schedule_service/`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service.rs`
- Delete: `backend-school/src/modules/academic/handlers/activity.rs`
- Delete: `backend-school/src/modules/academic/handlers/course_planning.rs`
- Delete: `backend-school/src/modules/academic/handlers/study_plans.rs`
- Delete: `backend-school/src/modules/academic/handlers/subjects.rs`
- Delete: `backend-school/src/modules/academic/models/activity.rs`
- Delete: `backend-school/src/modules/academic/models/course_planning.rs`
- Delete: `backend-school/src/modules/academic/models/study_plans.rs`
- Delete: `backend-school/src/modules/academic/services/academic_structure_service.rs`
- Delete: `backend-school/src/modules/academic/services/academic_structure_service_tests.rs`
- Delete: `backend-school/src/modules/academic/services/activity_service.rs`
- Delete: `backend-school/src/modules/academic/services/activity_service_tests.rs`
- Delete: `backend-school/src/modules/academic/services/course_planning_service.rs`
- Delete: `backend-school/src/modules/academic/services/course_planning_service_tests.rs`
- Delete: `backend-school/src/modules/academic/services/study_plan_service.rs`
- Delete: `backend-school/src/modules/academic/services/study_plan_service_tests.rs`
- Delete: `backend-school/src/modules/academic/services/subject_service.rs`
- Delete: `backend-school/src/modules/academic/services/subject_service_tests.rs`
- Modify: the corresponding assessment, timetable, and exam service test files.
- Test: `backend-school/tests/static_architecture.rs`

**Interfaces:** Assessment planning is now offering-scoped:

```text
GET /api/academic/assessments/plans?academicTermId={termId}
GET /api/academic/assessments/offerings/{offeringId}
PUT /api/academic/assessments/offerings/{offeringId}
POST /api/academic/assessments/offerings/{offeringId}/submit
```

Timetable and exam endpoints retain their feature paths but require `academicTermId` and expose
`learningGroupId`, `offeringId`, `homeroomId`, stable subject ID, and version display snapshot. They
must not expose `semesterId`, `classroomCourseId`, or legacy subject-version IDs as stable subject
identity.

- [ ] **Step 1: Add RED consumer contract tests**

Update service tests first so fixtures contain a term, offering, group, homeroom, and roster. Assert:

- one assessment plan can be shared by multiple groups in one offering;
- a group from another offering/term is rejected;
- timetable occupancy and swaps operate on group and term;
- activity and course entries share group conflict detection;
- exam imports resolve offering/group/assessment plan in one term;
- teacher published/today views use explicit term;
- daily-teaching views use group snapshots;
- no consumer service query contains an unqualified active-year/active-term lookup.

```bash
./scripts/test_backend_school.sh modules::academic::services::assessment_service -- --nocapture
./scripts/test_backend_school.sh modules::academic::services::timetable_service_tests -- --nocapture
./scripts/test_backend_school.sh modules::academic::services::exam_schedule_service_tests -- --nocapture
```

Expected: FAIL against the legacy consumers.

- [ ] **Step 2: Port assessment planning without adding Gradebook**

Replace subject/semester plan lookup with offering lookup. Preserve categories/items and current
saved/submitted structure state. Use validated decimal strings and `BigDecimal`; remove float casts.
Validate category/item totals against the offering's snapshotted grading policy. Do not add student
score sheets, course results, GPA, term results, or year results.

- [ ] **Step 3: Port timetable and bell schedules**

Replace classroom-course references with learning groups and academic semesters with terms. Period
queries resolve the term's bell schedule. Group coverage determines affected homerooms; the
authoritative roster does not control room conflict checks. Batch moves/swaps lock entries in stable
ID order and enforce same-term operations.

Timetable templates remain reusable drafts. Applying a template creates term/group draft entries and
never copies attendance, teaching logs, or result data.

- [ ] **Step 4: Port exam scheduling**

Exam rounds belong to one explicit term. Import candidates are offering/group combinations with a
published assessment plan. Publishing validates all items, sessions, rooms, and rosters share the
term and writes existing publish audit state. Published parent/student views filter by authoritative
group roster and requested term.

- [ ] **Step 5: Replace legacy activity and course-planning runtime paths**

Move surviving activity operations to delivery services/routes. Delete old handlers/models/services
and module exports. Ensure there is no compiled endpoint under `/planning/courses`,
`/planning/classrooms/*/activities`, `/subjects`, `/study-plans`, `/classrooms`, `/enrollments`, or
`/semesters` using the old contract.

- [ ] **Step 6: Port daily teaching and realtime payloads**

Daily teaching resolves schedule entry -> learning group -> offering snapshot. Realtime timetable
signals contain term/group revision and force an authoritative HTTP reload; they contain no student
roster or old entity IDs.

- [ ] **Step 7: Add static legacy-query guard for converted academic services**

Extend `static_architecture.rs` to scan compiled academic runtime files and reject legacy relation/
field tokens. Allow legacy tokens only in migrations 001-044, cutover preflight/test support,
reconciliation, and migration tests. The Phase A allowlist does not include handlers or runtime
services.

- [ ] **Step 8: Run focused verification**

```bash
./scripts/test_backend_school.sh modules::academic::services::assessment_service -- --nocapture
./scripts/test_backend_school.sh modules::academic::services::timetable_service_tests -- --nocapture
./scripts/test_backend_school.sh modules::academic::services::exam_schedule_service_tests -- --nocapture
./scripts/test_backend_school.sh modules::academic::delivery::services_tests -- --nocapture
cd backend-school
cargo test --test static_architecture -- --nocapture
cargo fmt --all -- --check
cargo check
```

- [ ] **Step 9: Commit Task 8**

```bash
git add -A backend-school/src/modules/academic backend-school/tests/static_architecture.rs
git commit -m "refactor(academic): cut consumers over to learning groups"
```

---

### Task 9: Cut Over Admission, Supervision, Portals, Lookups, and Other Backend Consumers

**Files:**

- Modify: `backend-school/src/bin/seed_sandbox.rs`
- Modify: `backend-school/src/modules/admission/models/rounds.rs`
- Modify: `backend-school/src/modules/admission/services/application_service.rs`
- Modify: `backend-school/src/modules/admission/services/portal_service.rs`
- Modify: `backend-school/src/modules/admission/services/round_service.rs`
- Modify: `backend-school/src/modules/admission/services/selection_service.rs`
- Modify: `backend-school/src/modules/calendar/services/notifications.rs`
- Modify: `backend-school/src/modules/calendar/services/visibility.rs`
- Modify: `backend-school/src/modules/calendar/services_tests.rs`
- Modify: `backend-school/src/modules/lookup/handlers.rs`
- Modify: `backend-school/src/modules/lookup/services.rs`
- Modify: `backend-school/src/modules/parents/services.rs`
- Modify: `backend-school/src/modules/question_bank/models.rs`
- Modify: `backend-school/src/modules/question_bank/services.rs`
- Modify: `backend-school/src/policies/question_bank_access_policy.rs`
- Modify: all service files under `backend-school/src/modules/supervision/services/` that read a
  year, semester, classroom course, classroom, subject, or enrollment.
- Modify: `backend-school/src/modules/staff/models.rs`
- Modify: `backend-school/src/modules/staff/services/dashboard_service.rs`
- Modify: `backend-school/src/modules/staff/services/staff_service.rs`
- Modify: `backend-school/src/modules/students/services.rs`
- Modify: `backend-school/src/policies/student_access_policy.rs`
- Modify: `backend-school/src/modules/certificates/schema_tests.rs`
- Modify: `backend-school/src/modules/certificates/services_tests.rs`
- Modify: relevant admission, parent, student, question-bank, supervision, staff, and lookup tests.
- Test: `backend-school/tests/static_architecture.rs`

**Interfaces:** Every term-scoped endpoint gains a required `academicTermId`; year-scoped endpoints
gain `academicYearId`. Parent/student timetable, exam, activity, and calendar responses are filtered
by the requested context plus self/child policy. Lookup responses use `academicTerm` and `homeroom`
terminology and return stable subject IDs with a selected version display label.

- [ ] **Step 1: Add RED cross-module tests**

Add tests proving:

- admission for a future year selects a study program/homeroom without changing current placement;
- enrollment retry creates one student-year and placement;
- supervision year-only and term-filtered views return the correct groups;
- question-bank assigned access follows group teachers across all accessible offerings;
- parent/student views return only rostered self/child rows in the explicit term;
- dashboards and lookups do not infer active context inside SQL;
- certificate year rendering still works after the in-place academic-year transformation.

```bash
./scripts/test_backend_school.sh modules::admission::services -- --nocapture
./scripts/test_backend_school.sh modules::supervision::services -- --nocapture
./scripts/test_backend_school.sh modules::question_bank -- --nocapture
./scripts/test_backend_school.sh modules::parents -- --nocapture
./scripts/test_backend_school.sh modules::certificates -- --nocapture
```

Expected: at least the new context and future-year assertions FAIL.

- [ ] **Step 2: Port admission transactionally**

Admission tracks reference `study_program_id`. Capacity derives from target-year homerooms. Final
enrollment locks the application/assignment, upserts one planned or active student-year according to
the already-migrated year status, creates one placement idempotently, and leaves other years intact.
Retain the existing admission identity/PII protections; this task never logs applicant data.

- [ ] **Step 3: Port supervision and question-bank authorization**

Supervision cycles use year plus optional term and observations use learning groups. Whole-year
cycles union groups from all selected-year terms. Question-bank records reference stable subjects;
assigned access joins stable subject -> course offering detail -> group teacher and unions all
authorized organization scopes.

- [ ] **Step 4: Port parent, student, calendar, and staff views**

Require context DTOs at service boundaries. Parent/learner policies first establish self/child
access, then term/group rosters narrow results. Staff dashboard callers explicitly resolve the active
default from the context service and pass IDs to queries; SQL never owns that default selection.
Annual calendar accepts year plus optional term.

- [ ] **Step 5: Port generic lookups and test fixtures**

Replace legacy lookup names and tables; return only fields needed by the caller and no national-ID/
contact data. Update sandbox and certificate fixtures to insert stable identities/versions,
curriculum/program, student-year, homeroom, offering, and group records using shared test builders.

- [ ] **Step 6: Audit all runtime consumers**

Run both searches and classify every hit. After Task 9, hits outside migrations/cutover tests must be
zero:

```bash
rg -n "academic_semesters|class_rooms|classroom_courses|student_class_enrollments|activity_catalog|study_plans|study_plan_versions|academic_assessment_plans" \
  backend-school/src \
  -g '!modules/academic/cutover_preflight.rs' \
  -g '!modules/academic/cutover_test_support.rs'
rg -n "WHERE[^;]*(is_active\s*=\s*true).*academic_(years|terms)" backend-school/src
```

Expected: no runtime query hits. Any legitimate migration/reconciliation constant belongs in the
static allowlist by exact file and symbol, not a directory-wide exemption.

- [ ] **Step 7: Run focused and broad backend verification**

```bash
./scripts/test_backend_school.sh modules::admission::services -- --nocapture
./scripts/test_backend_school.sh modules::supervision::services -- --nocapture
./scripts/test_backend_school.sh modules::question_bank -- --nocapture
./scripts/test_backend_school.sh modules::parents -- --nocapture
./scripts/test_backend_school.sh modules::certificates -- --nocapture
cd backend-school
cargo test --test static_architecture -- --nocapture
cargo fmt --all -- --check
cargo check
```

- [ ] **Step 8: Commit Task 9**

```bash
git add backend-school/src/bin/seed_sandbox.rs \
  backend-school/src/modules/admission backend-school/src/modules/calendar \
  backend-school/src/modules/lookup backend-school/src/modules/parents \
  backend-school/src/modules/question_bank backend-school/src/modules/supervision \
  backend-school/src/modules/staff backend-school/src/modules/students \
  backend-school/src/modules/certificates backend-school/src/policies \
  backend-school/tests/static_architecture.rs
git commit -m "refactor(academic): port cross-module academic consumers"
```

---

### Task 10: Finalize OpenAPI and Generated TypeScript Contracts

**Files:**

- Modify: `backend-school/src/api_contract.rs`
- Generate: `contracts/openapi/school-api.json`
- Generate: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/tests/static/api-global-contract.test.mjs`
- Create: `frontend-school/tests/static/academic-core-cutover-contract.test.mjs`

**Interfaces:** The generated contract contains every clean endpoint from Tasks 6-9 and no legacy
academic paths or legacy wire field names.

- [ ] **Step 1: Add RED OpenAPI ownership tests**

The new static test parses OpenAPI and requires:

- all context/core/delivery paths and methods;
- `academicYearId`/`academicTermId` on every scoped read;
- decimal strings instead of `number` for exact values;
- tagged course/activity offering requests;
- response envelopes for every JSON endpoint;
- no `/api/academic/semesters`, `/structure`, `/classrooms`, `/enrollments`, `/planning/courses`,
  `/subjects`, or `/study-plans` legacy path;
- no schema properties `semesterId`, `classroomCourseId`, or `isActive` for academic year/term DTOs.

```bash
cd frontend-school
node --test tests/static/academic-core-cutover-contract.test.mjs
```

Expected: FAIL until OpenAPI is regenerated.

- [ ] **Step 2: Complete utoipa registration**

Register all paths and schemas in `api_contract.rs`; remove deleted handler registrations. Every
operation ID is unique and uses `AcademicTerm`, `Homeroom`, `StudentAcademicYear`, `LearningOffering`,
or `LearningGroup` vocabulary.

- [ ] **Step 3: Regenerate offline artifacts**

```bash
cd frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
node --test tests/static/academic-core-cutover-contract.test.mjs
```

Do not patch generated JSON/TypeScript by hand.

- [ ] **Step 4: Run backend contract verification**

```bash
cd backend-school
cargo test api_contract::tests -- --nocapture
cargo fmt --all -- --check
cargo check
```

- [ ] **Step 5: Commit Task 10**

```bash
git add backend-school/src/api_contract.rs contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  frontend-school/tests/static/api-global-contract.test.mjs \
  frontend-school/tests/static/academic-core-cutover-contract.test.mjs
git commit -m "feat(academic): publish core cutover API contract"
```

---

### Task 11: Add URL-Owned Academic Context and the Topbar Switcher

**Required skills:** Before touching the Svelte files in this task, invoke
`svelte:svelte-code-writer` and `svelte:svelte-core-bestpractices`. Run the Svelte analyzer on each
changed component and apply its fixes before the task commit.

**Files:**

- Create: `frontend-school/src/lib/api/academic-context.ts`
- Create: `frontend-school/src/lib/academic-context/types.ts`
- Create: `frontend-school/src/lib/academic-context/route-context.ts`
- Create: `frontend-school/src/lib/academic-context/store.ts`
- Create: `frontend-school/src/lib/components/layout/AcademicContextSwitcher.svelte`
- Modify: `frontend-school/src/lib/components/layout/Header.svelte`
- Modify: `frontend-school/src/routes/(app)/+layout.svelte`
- Create: `frontend-school/tests/static/academic-context-contract.test.mjs`
- Create: `frontend-school/tests/e2e/academic-context.spec.ts`

**Interfaces:**

```ts
export type AcademicContextRequirement =
  | 'none'
  | 'year_required'
  | 'term_required'
  | 'term_optional';

export type SelectedAcademicContext = {
  academicYearId: string | null;
  academicTermId: string | null;
};

export type AcademicContextState = {
  requirement: AcademicContextRequirement;
  options: AcademicContextOptionsResponse | null;
  selected: SelectedAcademicContext;
  status: 'hidden' | 'loading' | 'ready' | 'unavailable' | 'error';
};

export function registerAcademicContextDirtySource(
  key: string,
  isDirty: () => boolean
): () => void;
```

The URL parameters are exactly `academicYearId` and `academicTermId`. They are not stored as an
independent selected-value copy in local storage. `store.ts` may cache context options for the
authenticated session, but the URL remains authoritative.

- [ ] **Step 1: Add RED route-context and store tests**

The static test creates synthetic route modules and asserts metadata inheritance, valid literal
values, no context on non-academic routes, URL parsing, term/year consistency, and absence of any
activation API call. Add browser tests for:

- missing required params default to active options with `replaceState`;
- selecting a year clears an incompatible term;
- term optional offers `ทั้งปี` and removes only `academicTermId`;
- browser back/forward restores the selected context;
- a dirty page requires confirmation before context change;
- cancelling confirmation preserves URL/store/page state;
- mobile button displays `year · term` and opens both controls;
- closed/planning/ready/active status labels are visible;
- selector failure leaves the page in an actionable error state instead of guessing active context;
- selector never calls a mutation endpoint.

```bash
cd frontend-school
node --test tests/static/academic-context-contract.test.mjs
npx playwright test --list tests/e2e/academic-context.spec.ts
```

Expected: static test FAIL because the context modules do not exist; Playwright discovery succeeds
only after the test file is created.

- [ ] **Step 2: Implement route metadata discovery**

Use an eager `import.meta.glob('/src/routes/(app)/**/+page.ts')` parallel to route access. Walk parent
route IDs so child detail pages inherit context requirements. Reject invalid metadata at build/test
time. Do not merge context semantics into permission checks; access and context are orthogonal.

- [ ] **Step 3: Implement the URL-owned store**

On an authenticated route change:

1. resolve requirement;
2. hide/clear in-memory context for `none` without rewriting unrelated query params;
3. load options once when context is required;
4. validate URL IDs and ownership;
5. choose active year/term defaults only when required params are missing;
6. write defaults with `replaceState` and user choices with normal history;
7. expose a ready state only after the URL is valid.

If no valid required year/term exists, expose `unavailable`; pages do not issue scoped API calls.
Use `goto` with preserved unrelated query params, hash, scroll, and focus behavior.

- [ ] **Step 4: Implement dirty-source coordination**

Pages register a stable key and getter. Before a user-initiated context change, evaluate all current
route sources. Show an existing AlertDialog with Thai copy naming the unsaved-change risk. On confirm,
perform navigation and clear only sources unregistered by page teardown; never mutate page draft
state directly from the Topbar.

- [ ] **Step 5: Implement responsive switcher and Header integration**

Desktop uses linked year and term Select controls with status badges. Mobile uses one compact button
opening a Sheet/Popover. Show it only for staff routes whose resolved metadata is not `none`. Keep
search, theme, notifications, and profile controls usable at all breakpoints; shrink/hide the search
before truncating the context label.

- [ ] **Step 6: Initialize only after authentication**

The app layout starts context resolution after `/api/auth/me` succeeds and disposes route/dirty
subscriptions on teardown. A permission denial redirects before a protected page loads context data.
Signing out clears cached options and selected in-memory state.

- [ ] **Step 7: Run Svelte and focused browser verification**

Run the Svelte code-writer analyzer/autofixer on:

```text
frontend-school/src/lib/components/layout/AcademicContextSwitcher.svelte
frontend-school/src/lib/components/layout/Header.svelte
frontend-school/src/routes/(app)/+layout.svelte
```

Then run:

```bash
cd frontend-school
node --test tests/static/academic-context-contract.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run lint
npx playwright test --list tests/e2e/academic-context.spec.ts
```

- [ ] **Step 8: Commit Task 11**

```bash
git add frontend-school/src/lib/api/academic-context.ts \
  frontend-school/src/lib/academic-context \
  frontend-school/src/lib/components/layout/AcademicContextSwitcher.svelte \
  frontend-school/src/lib/components/layout/Header.svelte \
  'frontend-school/src/routes/(app)/+layout.svelte' \
  frontend-school/tests/static/academic-context-contract.test.mjs \
  frontend-school/tests/e2e/academic-context.spec.ts
git commit -m "feat(academic): add explicit topbar context"
```

---

### Task 12: Build the New Core and Learning Delivery Workspaces

**Required skills:** Invoke `svelte:svelte-code-writer` and
`svelte:svelte-core-bestpractices` before analyzing or editing the Svelte files. Prefer extracting
typed controller/state modules when an existing workspace is oversized; do not move behavior into
untyped component-local objects.

**Files:**

- Create: `frontend-school/src/lib/api/academic-core.ts`
- Create: `frontend-school/src/lib/api/learning-delivery.ts`
- Create: all new core/catalog/curriculum/homeroom/student-year/delivery routes listed in File
  Structure.
- Create: focused components under `frontend-school/src/lib/components/academic-core/` for
  year/term editor, version history, curriculum programs/requirements, homeroom placements, and
  student-year transfer.
- Create: focused components under `frontend-school/src/lib/components/learning-delivery/` for
  offering editor, groups/teachers, roster preview, and roster publication.
- Delete: `frontend-school/src/lib/api/academic.ts` after all consumers move to typed replacements.
- Delete: `frontend-school/src/routes/(app)/staff/academic/structure/+page.ts`
- Delete: `frontend-school/src/routes/(app)/staff/academic/structure/+page.svelte`
- Delete: `frontend-school/src/routes/(app)/staff/academic/subjects/+page.ts`
- Delete: `frontend-school/src/routes/(app)/staff/academic/subjects/+page.svelte`
- Delete: `frontend-school/src/routes/(app)/staff/academic/study-plans/+page.ts`
- Delete: `frontend-school/src/routes/(app)/staff/academic/study-plans/+page.svelte`
- Delete: `frontend-school/src/routes/(app)/staff/academic/classrooms/+page.ts`
- Delete: `frontend-school/src/routes/(app)/staff/academic/classrooms/+page.svelte`
- Delete: `frontend-school/src/routes/(app)/staff/academic/enrollments/+page.ts`
- Delete: `frontend-school/src/routes/(app)/staff/academic/enrollments/+page.svelte`
- Delete: `frontend-school/src/routes/(app)/staff/academic/planning/+page.ts`
- Delete: `frontend-school/src/routes/(app)/staff/academic/planning/+page.svelte`
- Delete: `frontend-school/src/routes/(app)/staff/academic/activities/+page.ts`
- Delete: `frontend-school/src/routes/(app)/staff/academic/activities/+page.svelte`
- Delete: `frontend-school/src/routes/(app)/staff/academic/activities/[id]/+page.svelte`
- Move/replace: subject-group routes with
  `frontend-school/src/routes/(app)/staff/academic/catalog/subject-groups/` using the stable catalog
  API and `academicContext: 'none'`.
- Modify: relevant menu route metadata and static tests for removed/replacement routes.
- Create: `frontend-school/tests/e2e/academic-core-cutover.spec.ts`

**Interfaces:** API modules alias generated wire DTOs and map to explicit UI view models only where
display state differs. They do not use `unknown`, casts, or `Record<string, unknown>` for known
responses.

- [ ] **Step 1: Add RED typed API and route tests**

Extend `academic-core-cutover-contract.test.mjs` to require new routes, generated DTO imports, exact
permissions, context metadata, and no legacy wrapper/path strings. Add Playwright component-style
flows with controlled API stubs for:

- manually creating a future planning year and regular/summer/custom terms;
- confirming term count is derived from rows and no `numberOfTerms` field exists;
- creating a new subject/activity version without changing prior version display;
- publishing a curriculum version with a default program and requirements;
- creating a future student-year/placement without changing current-year placement;
- creating course and activity offerings and publishing a roster;
- rejecting stale row version and preserving the user's draft.

```bash
cd frontend-school
node --test tests/static/academic-core-cutover-contract.test.mjs
npx playwright test --list tests/e2e/academic-core-cutover.spec.ts
```

Expected: static test FAIL until routes/wrappers are replaced.

- [ ] **Step 2: Implement strictly typed API wrappers**

Use the generated envelope/data types and one shared query helper that requires a selected context
for scoped calls. Mutations return typed resources and pages patch only affected rows. A 409 maps to
Thai reload guidance without swallowing the server message/code.

- [ ] **Step 3: Build the academic core setup page**

The all-years page lists statuses and term rows. Users with manage permission may add/edit planning
years/terms and configure sequence, code, type, dates, inclusion, blocking, and bell schedule. There
is no active toggle and no stored term-count field. Active/closing/closed rows are read-only in
Release 1 with guidance that lifecycle transitions arrive through the controlled workflow.

- [ ] **Step 4: Build catalog and curriculum pages**

Catalog lists stable identities separately from version history. Editors never overwrite a
published historical version. Curriculum UI shows version -> program -> requirement hierarchy and
the exact subject/activity version selected. Publishing uses a confirmation summary and patches the
returned immutable state.

- [ ] **Step 5: Build year-scoped homeroom and student-year pages**

Read selected year from the context store. Homeroom management includes advisor/program/capacity.
Student-year management shows academic status and placement history. Future placement creation and
mid-year transfer are separate actions; transfer requires date/reason and displays both ended/new
placement responses.

- [ ] **Step 6: Build term-scoped delivery workspace**

Present course and activity offerings through one workspace with kind-specific details. Group
management covers homerooms, teachers, rooms, and authoritative roster. Roster preview visibly
separates added/removed/retained/conflict rows; publish uses the source hash and row version. Do not
show score/GPA controls. Curriculum generation first displays course/activity create/retain/conflict
items and applies only the reviewed source hash; a stale curriculum/term asks the user to preview
again.

- [ ] **Step 7: Delete legacy routes/wrapper and verify menu synchronization inputs**

Remove old route files without redirects or aliases. Replacement routes own new system route IDs;
the normal menu synchronization removes stale frontend-owned records while preserving school-owned
workspace placement for surviving route records. Add/update menu sync fixtures for the replacements.

- [ ] **Step 8: Run Svelte analyzer and frontend verification**

Run the Svelte tooling on every created/modified Svelte component, then:

```bash
cd frontend-school
node --test tests/static/academic-core-cutover-contract.test.mjs
npm run test:menu-sync
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run lint
npx playwright test --list tests/e2e/academic-core-cutover.spec.ts
```

- [ ] **Step 9: Commit Task 12**

```bash
git add -A frontend-school/src/lib/api frontend-school/src/lib/components/academic-core \
  frontend-school/src/lib/components/learning-delivery \
  'frontend-school/src/routes/(app)/staff/academic' \
  frontend-school/tests/static frontend-school/tests/runtime \
  frontend-school/tests/e2e/academic-core-cutover.spec.ts
git commit -m "feat(academic): build core and delivery workspaces"
```

---

### Task 13: Cut Over Every Existing Frontend Consumer

**Required skills:** Invoke `svelte:svelte-code-writer` and
`svelte:svelte-core-bestpractices` before editing any affected component and run the analyzer on every
changed Svelte file.

**Files:**

- Modify: `frontend-school/src/lib/api/academicAssessments.ts`
- Modify: `frontend-school/src/lib/api/timetable.ts`
- Modify: `frontend-school/src/lib/api/examSchedule.ts`
- Modify: `frontend-school/src/lib/api/questionBank.ts`
- Modify: `frontend-school/src/lib/api/supervision.ts`
- Modify: `frontend-school/src/lib/api/admission.ts`
- Modify: `frontend-school/src/lib/api/parents.ts`
- Modify: `frontend-school/src/lib/api/lookup.ts`
- Modify: certificate/staff API wrappers only where generated academic DTO names changed.
- Modify: all `+page.ts` and `+page.svelte` files under:
  - `frontend-school/src/routes/(app)/staff/academic/assessments/`
  - `frontend-school/src/routes/(app)/staff/academic/timetable/`
  - `frontend-school/src/routes/(app)/staff/academic/periods/`
  - `frontend-school/src/routes/(app)/staff/academic/exam-schedules/`
  - `frontend-school/src/routes/(app)/staff/academic/question-bank/`
  - `frontend-school/src/routes/(app)/staff/academic/supervision/`
  - `frontend-school/src/routes/(app)/staff/academic/admission/`
- Modify: `frontend-school/src/routes/(app)/staff/timetable/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/timetable/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/exams/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/exams/+page.svelte`
- Modify: student timetable/activity/exam/calendar routes under
  `frontend-school/src/routes/(app)/student/`.
- Modify: parent student timetable/exam/calendar routes under
  `frontend-school/src/routes/(app)/parent/student/[id]/`.
- Modify: affected static tests under `frontend-school/tests/static/`.
- Modify: affected Playwright tests under `frontend-school/tests/e2e/`.

**Context matrix:**

| Workspace | Requirement |
|---|---|
| assessment structure | `term_required` |
| timetable, templates, today | `term_required` |
| bell schedules/periods | `year_required` |
| exam schedules and details | `term_required` |
| question-bank catalog | `none` |
| supervision list/detail/workflows | `term_optional` |
| admission round setup | `year_required` |
| staff own timetable/exams | `term_required` |
| student/parent timetable/activity/exam | page-local term selector |
| student/parent calendar | page-local year plus optional term selector |

- [ ] **Step 1: Add RED context-propagation tests**

Static tests scan wrappers/pages and require selected IDs in every scoped call, metadata on every
staff academic route, generated DTO usage, and no legacy path/wire token. Extend Playwright flows to
change term and verify each workspace reloads only its selected term without activating anything.

```bash
cd frontend-school
node --test tests/static/academic-context-contract.test.mjs \
  tests/static/academic-core-cutover-contract.test.mjs \
  tests/static/academic-assessment-structure.test.mjs \
  tests/static/academic-exam-schedule.test.mjs
```

Expected: FAIL until every consumer passes explicit context.

- [ ] **Step 2: Port assessment and timetable pages**

Assessment structure selects offerings rather than subject/classroom-course rows and shows the
offering snapshot source. Timetable works with groups/homeroom coverage and the selected term's bell
schedule. Register dirty sources for unsaved assessment/timetable edits so Topbar changes require
confirmation.

- [ ] **Step 3: Port exam, supervision, question-bank, and admission pages**

Exam candidates show offering/group context. Supervision supports `ทั้งปี` and term-specific queries.
Question bank uses stable subjects and no term context for catalog authoring; assigned filters remain
backend-authoritative. Admission uses the target year, study program, homeroom capacity, and explicit
placement outcome.

- [ ] **Step 4: Port teacher, student, and parent views**

Teacher timetable/exam pages use the staff Topbar context. Student and parent pages show a local
history selector populated only with authorized years/terms; they pass IDs on every request and
cannot infer or browse another learner. Default to active context locally only when URL selection is
absent.

- [ ] **Step 5: Remove legacy API and type vocabulary**

Delete manual types that duplicate generated DTOs. Replace user-facing Thai labels where the old
word “ภาคเรียน” remains valid, but replace internal identifiers such as semester/classroom-course/
enrollment with term/learning-group/student-year. Do not add response casts.

- [ ] **Step 6: Run static, Svelte, and browser discovery checks**

Run Svelte tooling on every changed Svelte file, then:

```bash
cd frontend-school
rg -n "/api/academic/(semesters|structure|classrooms|enrollments|planning/courses|subjects|study-plans)" src tests
rg -n "semesterId|classroomCourseId|studentClassEnrollment" src tests
npm run test:menu-sync
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run lint
npx playwright test --list tests/e2e/academic-context.spec.ts \
  tests/e2e/academic-core-cutover.spec.ts
```

Expected: both `rg` commands return no runtime/manual-contract hits; generated historical test
fixtures are updated rather than globally exempted.

- [ ] **Step 7: Commit Task 13**

```bash
git add -A frontend-school/src/lib/api \
  'frontend-school/src/routes/(app)/staff' \
  'frontend-school/src/routes/(app)/student' \
  'frontend-school/src/routes/(app)/parent' \
  frontend-school/tests
git commit -m "refactor(academic): port frontend consumers to explicit context"
```

---

### Task 14: Rehearse and Produce the Phase A Release Candidate

**Files:**

- Modify: `backend-school/src/modules/system/handlers/migration.rs`
- Modify: `backend-school/src/modules/academic/reconciliation.rs`
- Modify: `backend-school/src/app.rs`
- Modify: `docs/TESTING.md`
- Modify: `docs/OPERATIONS.md`
- Modify: `TODO.md`
- Modify: `frontend-school/tests/static/documentation-policy.test.mjs` only if the canonical
  documentation-policy assertions need a new allowed in-document section; do not add a new document.
- Test: all Phase A focused/static/integration/E2E suites.

**Interfaces:** Add one internal, service-authenticated operation:

```text
POST /internal/academic-core/reconcile-all
```

`reconcile-all` is valid only after 043, reruns every aggregate check, and records a success marker
in `academic_core_cutover_audits` only when all checks pass. It returns bounded per-tenant status and
counts, never row data, connection strings, or tenant secrets. Existing internal authentication and
bounded tenant concurrency apply; no second migration runner is introduced. Preflight remains the
read-only CLI from Task 1 so obtaining a tenant pool cannot trigger lazy migration before validation.

- [ ] **Step 1: Add RED internal-operation tests**

Test authentication failure, mixed tenant pass/fail aggregation, no success marker on failed
reconciliation, idempotent success-marker recording, and bounded failure responses. Assert the
operation never invokes `run_tenant_migrations` itself.

```bash
./scripts/test_backend_school.sh \
  modules::system::handlers::migration::tests -- --nocapture --test-threads=1
```

Expected: new endpoint tests FAIL until handlers are registered.

- [ ] **Step 2: Implement internal reconciliation orchestration**

Reuse the admin tenant inventory and pool manager already owned by migration operations.
Reconciliation rejects any tenant not exactly on Phase A's expected latest version 043. A
successful marker stores migration version, mapping algorithm version, check codes, aggregate
counts/checksums, and timestamp; it stores no actor/learner names.

- [ ] **Step 3: Document repeatable local and clone rehearsal**

Add to `docs/TESTING.md`:

- the focused legacy -> 041 -> 042 -> 043 -> 044 fixture commands;
- the full disposable PostgreSQL command;
- expected preflight/reconciliation assertions;
- manual Neon compatibility gate and the fact it is external/unrun without repository credentials;
- Playwright discovery versus execution requirements;
- exact reporting requirements for skipped database/browser/deployment checks.

Add no copied table inventory; link to migrations, typed services, and canonical operations.

- [ ] **Step 4: Document the two-artifact maintenance cutover**

Add one durable Academic Core section to `docs/OPERATIONS.md` with these mandatory gates:

1. confirm Phase A contains migrations 041-044 and does not contain 045;
2. confirm Release 3 timing is before the next operational term transition;
3. enter global maintenance and stop academic writes/workers/realtime mutations;
4. run `preflight_academic_core` separately against every tenant through the authorized secret-backed
   connection inventory while tenants remain on 040, aggregate only schema label/status/counts, and
   resolve every blocker;
5. confirm stable source counts and take a recoverable snapshot under retention policy;
6. apply 041-044 through `/internal/migrate-all` using the Phase A image;
7. deploy the Phase A backend/frontend while traffic remains closed;
8. run `/internal/academic-core/reconcile-all` and require a success marker for every tenant;
9. perform selected-tenant read-only/authenticated workflow checks in multiple year/term contexts;
10. deploy the separately approved Phase B image and apply 045 through the same migration runner;
11. verify latest version, cleanup manifest, generated contracts, permissions, `/ready`, and selected
    authenticated workflows;
12. explicitly record the go/no-go decision, then open traffic and mark the first accepted write as
    the snapshot rollback boundary.

The document must say: any preflight/reconciliation/migration/smoke failure keeps maintenance active;
before first write, restore snapshot + previous release; after first write, do not deploy the old app
against the new schema.

- [ ] **Step 5: Rehearse Phase A and the future Phase B migration on isolated data**

Use three disposable datasets:

- a complete representative synthetic fixture;
- each blocking inconsistent fixture;
- an authorized protected clone/snapshot of representative tenant data with output redacted.

For the protected clone, record only duration, aggregate counts/checksums, finding codes, and pass/
fail outside the repository. Apply 041-044, run the new backend/frontend smoke workflows, then apply a
review copy of 045 on the clone and verify cleanup. Never commit clone data or output.

- [ ] **Step 6: Update unfinished backlog accurately**

Keep `SCH-002` open. Clarify that after Release 1 it still owns Gradebook/results, term lifecycle,
annual closure/promotion, and Thai academic documents. Do not mark promotion or term transition
complete and do not add a completed-work report.

- [ ] **Step 7: Run the complete Phase A change-type matrix**

```bash
# Repository and documentation
git diff --check
cd frontend-school
npm run check:docs

# Permission contract
npm run generate:permissions
npm run check:permissions
npm run test:permissions

# API contract
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts

# Frontend
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:menu-sync
npm run test:static
npx playwright test --list tests/e2e/academic-context.spec.ts \
  tests/e2e/academic-core-cutover.spec.ts

# Backend and database
cd ../backend-school
cargo fmt --all -- --check
cargo test --test static_architecture
cargo test api_contract::tests -- --nocapture
cargo check
cd ..
./scripts/test_backend_school.sh

# Final worktree evidence
git diff --check
git status --short
```

Execute Playwright, deployed smoke, and manual Neon compatibility only when their dedicated accounts,
deployment target, and explicit authority exist. Report them as unrun with the missing dependency;
do not imply discovery equals execution.

- [ ] **Step 8: Request Phase A code review**

Use `superpowers:requesting-code-review` for two passes:

1. spec/plan coverage and migration/data correctness;
2. authorization, PII, API, frontend context, operational rollback, and final diff.

Apply accepted findings through `superpowers:receiving-code-review`, rerun affected checks, and
request review again. Do not merge/deploy.

- [ ] **Step 9: Commit the Phase A release candidate**

```bash
git add backend-school/src/modules/system/handlers/migration.rs \
  backend-school/src/modules/academic/reconciliation.rs backend-school/src/app.rs \
  docs/TESTING.md docs/OPERATIONS.md TODO.md \
  frontend-school/tests/static/documentation-policy.test.mjs
git commit -m "docs(academic): define core cutover operations"
```

Tag/build the exact reviewed Phase A commit only after explicit release approval. Confirm with
`git ls-tree` that `backend-school/migrations/045_academic_core_legacy_cleanup.sql` is absent from
that artifact.

---

### Task 15: Apply the Separately Gated Phase B Legacy Cleanup

**Precondition:** Every production tenant has successfully applied 041-044, Phase A runtime is
deployed under maintenance, `/internal/academic-core/reconcile-all` has recorded a current success
marker, selected authenticated context workflows pass, and an operator has explicitly authorized
the cleanup. Local Phase A completion alone does not satisfy this precondition.

**Files:**

- Create: `backend-school/migrations/045_academic_core_legacy_cleanup.sql`
- Modify: `backend-school/src/modules/academic/core/schema_tests.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Modify: `backend-school/src/modules/academic/reconciliation.rs`
- Modify: `backend-school/src/modules/system/handlers/migration.rs`
- Delete: `backend-school/src/bin/preflight_academic_core.rs`
- Delete: `backend-school/src/modules/academic/cutover_preflight.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/src/main.rs`
- Modify: `backend-school/tests/static_architecture.rs`
- Modify: `docs/OPERATIONS.md`
- Delete after the Phase B pull request records the completed outcome:
  `docs/superpowers/specs/2026-08-23-academic-core-lifecycle-redesign-design.md`
- Delete after the Phase B pull request records the completed outcome:
  `docs/superpowers/plans/2026-08-23-academic-core-cutover.md`

`cutover_test_support.rs` and migration fixture tests remain under `#[cfg(test)]` so clean-database
CI continues to prove the full 040 -> 045 transformation. They are not compiled runtime code.

- [ ] **Step 1: Start a separate Phase B branch/worktree from the exact Phase A release commit**

Use `superpowers:using-git-worktrees`. Reconfirm production reconciliation evidence through the
authorized operational channel without copying tenant data into Git or chat. If any tenant is not
green or traffic is open for writes, stop.

- [ ] **Step 2: Add RED cleanup-manifest tests**

Apply 040, seed the complete fixture, run preflight, apply 041-044, record a successful reconciliation
marker, then apply 045. Assert all final target rows remain and every legacy relation/column is absent.
Also assert 045 fails when the marker is missing, stale, has a mismatched checksum, or reconciliation
currently fails.

The cleanup manifest includes:

```text
student_class_enrollments
classroom_courses
classroom_course_instructors
classroom_course_preferred_rooms
activity_slots
activity_slot_classrooms
activity_slot_classroom_assignments
activity_slot_instructors
activity_groups
activity_group_instructors
activity_group_members
legacy academic year/term is_active columns
grade_levels.next_grade_level_id
legacy admission track/room-assignment columns
legacy timetable/exam/supervision foreign-key and free-text semester columns
academic_core_entity_map
obsolete legacy permission grants and definitions
```

Renamed authoritative relations such as `academic_terms`, `subject_versions`, `activity_versions`,
`curricula`, `curriculum_versions`, `homerooms`, and `course_assessment_plans` must remain.

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_045_removes_legacy_schema -- --exact --nocapture --test-threads=1
```

Expected: FAIL because migration 045 does not exist.

- [ ] **Step 3: Write migration 045 with fail-closed prerequisites**

At the beginning, lock the tenant audit marker and verify migration 044, mapping algorithm
`academic-core-v1`, successful check codes, expected checksum, and no writes since the recorded
maintenance reconciliation. Raise a stable bounded error before any drop if a prerequisite differs.

Drop dependencies in foreign-key order, then old tables/columns/functions. Deactivate/delete legacy
permissions only after asserting no active database permission contract row or unmatched grant
remains; static/generated-contract tests separately prove the source contract has no old code. End
with catalog queries asserting the cleanup manifest is absent and target manifest present, then
append a `cleanup_completed` aggregate audit record.

- [ ] **Step 4: Remove one-time Phase A runtime tools**

Delete the one-time preflight CLI/library. Replace reconcile-all with a read-only completed-audit
status in the existing migration-status response; remove code that queries the deleted entity map.
Keep migration fixture helpers test-only. Tighten static guards so legacy tokens are allowed only in
immutable migrations 001-044 and exact test-support files.

- [ ] **Step 5: Run the full migration chain and final schema guard**

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests -- --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests -- --nocapture --test-threads=1
./scripts/test_backend_school.sh
cd backend-school
cargo test --test static_architecture -- --nocapture
cargo fmt --all -- --check
cargo check
```

Expected: PASS with contiguous migrations 001-044 and zero final runtime legacy-query hits.

- [ ] **Step 6: Run all generated-contract and frontend checks again**

```bash
cd frontend-school
npm run check:permissions
npm run test:permissions
npm run check:api-contracts
npm run test:api-contracts
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:menu-sync
npm run test:static
npm run check:docs
```

- [ ] **Step 7: Request Phase B code review and verify completion evidence**

Use `superpowers:requesting-code-review` for cleanup safety, migration ordering, retained-data counts,
permissions, one-time tool removal, rollback boundary, and documentation. Apply accepted findings and
rerun every affected check. Then use `superpowers:verification-before-completion` and review:

```bash
git diff --check
git diff --stat
git status --short
```

- [ ] **Step 8: Commit the Phase B cleanup artifact**

```bash
git add -A backend-school/migrations/044_academic_core_legacy_cleanup.sql \
  backend-school/src backend-school/tests docs/OPERATIONS.md \
  docs/superpowers/specs/2026-08-23-academic-core-lifecycle-redesign-design.md \
  docs/superpowers/plans/2026-08-23-academic-core-cutover.md
git commit -m "refactor(academic): remove legacy academic schema"
```

Do not merge or deploy this commit without the explicit production precondition at the top of Task
15. The reviewed commit becomes the Phase B image; the centralized runner applies 045 while
maintenance remains active.

---

## Final Acceptance Checklist

- [ ] Future and current student-year/placement records coexist without cross-year mutation.
- [ ] Terms are configurable rows; two, three, summer, remedial, and custom sequences require no
  separate term-count setting.
- [ ] Staff Topbar selection is URL-owned, route-aware, permission-safe, and never activates state.
- [ ] Every scoped backend query receives explicit year/term context from its caller.
- [ ] Subjects, activities, curricula, and programs retain stable identity and immutable versions.
- [ ] Courses and activities share offerings/groups but keep distinct detail/result semantics.
- [ ] Existing assessment structures, timetables, exams, activities, supervision, admission,
  portals, lookups, teaching views, and certificates preserve valid migrated data.
- [ ] Historical years contain no fabricated results and cannot feed promotion.
- [ ] Permission migration preserves equivalent access without read-to-manage or scope escalation.
- [ ] Generated permission and API contracts match runtime and frontend consumers.
- [ ] Phase A preflight/reconciliation pass every tenant before cleanup.
- [ ] Migration 045 leaves no legacy runtime table/column/permission/path and preserves target counts.
- [ ] Snapshot rollback remains available until the explicitly recorded first new-system write.
- [ ] Release 3 timing is approved before production Release 1 cutover so the next term can transition
  through the safe lifecycle rather than a manual status edit.
- [ ] All applicable `.rules` verification commands have exact pass/fail/unrun evidence.
