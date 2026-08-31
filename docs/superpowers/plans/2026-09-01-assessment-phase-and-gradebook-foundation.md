# Assessment Phase and Gradebook Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace free-form assessment categories with an auto-saved fixed four-phase course plan,
add coordinator inference and per-phase controls, establish group-owned score items, and make exam
schedule source changes explicit and safe.

**Architecture:** One course assessment plan owns the four shared phase maxima and exam intention;
future score items belong to learning groups. Readiness is derived from saved data rather than a
submit status. Imported exam items remain snapshots and draft rounds synchronize only through an
explicit preview/apply boundary.

**Tech Stack:** PostgreSQL 18 migrations, Rust/Axum/SQLx/utoipa, generated OpenAPI TypeScript,
SvelteKit 5 runes, shadcn-svelte, Tailwind CSS, Node static tests, Rust database/service tests

**Spec:** `docs/superpowers/specs/2026-09-01-assessment-phase-and-gradebook-foundation-design.md`

## Global Constraints

- Read and obey `.rules`; never edit an applied migration.
- Run commands serially with one Rust build/test process at a time.
- Use exact `NUMERIC(10,2)`/decimal strings; do not introduce floats for scores.
- Use typed Rust DTOs, utoipa registration, and generated frontend contracts only.
- Do not add permission codes; reuse the existing assessment and exam-schedule permissions.
- Do not retain free-form/custom assessment phases, the `practical` phase mode, submit endpoints,
  global assessment teacher-access settings, or compatibility reads/writes after cutover.
- Never store, expose, or log plaintext national IDs or other unnecessary PII.
- Preserve assessment phase IDs and all existing exam schedule item relationships.
- Use `apply_patch` for source edits and run the Svelte autofixer for every changed `.svelte` file.
- Run the focused tests after each task and the full applicable `.rules` verification matrix at the
  end.

---

### Task 1: Forward Migration and Fixed-Phase Schema

**Files:**
- Create: `backend-school/migrations/056_assessment_phase_gradebook_foundation.sql`
- Modify: `backend-school/src/modules/academic/core/schema_tests.rs`

**Interfaces:**
- Produces: `course_assessment_phases`, `assessment_coordinator_id`,
  `academic_assessment_phase_controls`, `learning_group_score_items`, and canonical phase/exam
  constraints used by all later tasks.
- Preserves: existing phase UUIDs and exam-schedule foreign-key identities.

- [ ] **Step 1: Write failing migration schema tests**

Add focused tests that query the migrated schema and assert:

```rust
assert_eq!(phase_codes, vec![
    "after_midterm",
    "before_midterm",
    "final",
    "midterm",
]);
assert!(plan_has_assessment_coordinator);
assert_eq!(phase_control_count_per_term, 4);
assert!(group_score_items_reference_learning_group_and_phase);
assert!(legacy_category_table_is_absent);
assert!(legacy_course_item_table_is_absent);
assert!(legacy_submit_columns_are_absent);
```

The test must also insert a valid in-timetable phase and verify the renamed
`assessment_phase_id` on `academic_exam_schedule_items` keeps the same UUID.

- [ ] **Step 2: Run the test and verify it fails before migration 056 exists**

Run:

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_056_establishes_fixed_assessment_phases \
  -- --exact --nocapture --test-threads=1
```

Expected: FAIL because migration 056 objects/columns do not exist.

- [ ] **Step 3: Add migration 056**

Implement one forward-only transactional migration that:

```sql
-- Reject unrepresentable tenants before mutation.
-- Require exactly one of each canonical code per persisted plan.
-- Reject NULL/custom/duplicate codes and practical modes.
-- Preserve category IDs while renaming/rebuilding as course_assessment_phases.
-- Rename academic_exam_schedule_items.assessment_category_id to assessment_phase_id.
-- Drop editable phase name/display order and derive them in application code.
-- Restrict exam_arrangement to none/outside_timetable/in_timetable.
-- Add course_assessment_plans.assessment_coordinator_id and remove status/submit/lock columns.
-- Add four academic_assessment_phase_controls rows per term with both flags false.
-- Create learning_group_score_items with exact phase/group/context foreign keys.
-- Deterministically expand every legacy course item to every active group in its offering.
-- Reconcile source and expanded counts, then drop course_assessment_items.
```

Use `uuid_generate_v5` with a fixed namespace and source-item/group IDs for deterministic item
expansion. Add named checks and unique constraints for plan/phase and term/phase controls. Infer a
coordinator only when every active group has exactly one active primary and the distinct teacher-ID
count is one. Never guess a missing duration.

- [ ] **Step 4: Run the focused migration test**

Run the Step 2 command again. Expected: PASS.

- [ ] **Step 5: Run existing Academic Core schema coverage**

Run:

```bash
./scripts/test_backend_school.sh modules::academic::core::schema_tests \
  -- --nocapture --test-threads=1
```

Expected: PASS with no migration checksum edits.

- [ ] **Step 6: Commit the schema boundary**

```bash
git add backend-school/migrations/056_assessment_phase_gradebook_foundation.sql \
  backend-school/src/modules/academic/core/schema_tests.rs
git commit -m "feat(academic): establish fixed assessment phases"
```

### Task 2: Typed Assessment Model and Pure Readiness Rules

**Files:**
- Modify: `backend-school/src/modules/academic/models/assessment.rs`
- Modify: `backend-school/src/modules/academic/services/assessment_service.rs`

**Interfaces:**
- Produces: `AssessmentPhaseCode`, `AssessmentExamArrangement`, `AssessmentReadiness`,
  `SaveAssessmentPhaseRequest`, and pure readiness/coordinator-suggestion helpers.
- Consumes: Task 1 fixed schema names.

- [ ] **Step 1: Replace old validation tests with failing fixed-phase tests**

Add pure tests covering:

```rust
assert!(validate_phase_snapshot(&valid_four_phases()).is_ok());
assert!(validate_phase_snapshot(&missing_final()).is_err());
assert!(validate_phase_snapshot(&duplicate_midterm()).is_err());
assert!(validate_phase_snapshot(&before_midterm_with_exam()).is_err());
assert!(readiness(&over_allocated_plan()).findings.contains(&"total_mismatch"));
assert!(readiness(&missing_duration_plan()).findings.contains(&"missing_exam_duration"));
assert!(readiness(&complete_plan()).ready);
```

Add coordinator helper cases for common primary, different primaries, missing primary, and explicit
coordinator preservation.

- [ ] **Step 2: Run the pure tests and verify failure**

```bash
CARGO_BUILD_JOBS=1 cargo test --manifest-path backend-school/Cargo.toml \
  modules::academic::services::assessment_service::tests -- --nocapture --test-threads=1
```

Expected: FAIL because the new enums and helpers do not exist.

- [ ] **Step 3: Implement the typed model and readiness helpers**

Replace free-form category/item DTOs with:

```rust
pub enum AssessmentPhaseCode {
    BeforeMidterm,
    Midterm,
    AfterMidterm,
    Final,
}

pub enum AssessmentExamArrangement {
    None,
    OutsideTimetable,
    InTimetable,
}

pub struct SaveAssessmentPlanRequest {
    pub row_version: Option<i64>,
    pub assessment_coordinator_id: Option<Uuid>,
    pub phases: Vec<SaveAssessmentPhaseRequest>,
}

pub struct AssessmentReadiness {
    pub ready: bool,
    pub findings: Vec<String>,
}
```

Phase labels and order come from enum methods. Allow an auto-saved combined total below or above the
policy total; reject negative/unrepresentable values and invalid phase/arrangement combinations.
Readiness, not save validation, requires exact total, coordinator, and in-timetable durations.

- [ ] **Step 4: Run pure tests**

Run the Step 2 command. Expected: PASS.

- [ ] **Step 5: Commit typed assessment rules**

```bash
git add backend-school/src/modules/academic/models/assessment.rs \
  backend-school/src/modules/academic/services/assessment_service.rs
git commit -m "refactor(academic): model assessment readiness by fixed phase"
```

### Task 3: Assessment Service, Phase Controls, and Authorization

**Files:**
- Modify: `backend-school/src/modules/academic/services/assessment_service.rs`
- Modify: `backend-school/src/modules/academic/services/assessment_service_tests.rs`
- Modify: `backend-school/src/modules/academic/handlers/assessment.rs`
- Modify: `backend-school/src/modules/academic.rs`

**Interfaces:**
- Produces:
  - `list_assessment_plans(pool, query, access) -> Vec<AssessmentPlanSummary>`
  - `get_assessment_plan(pool, offering_id, access) -> AssessmentPlanDetail`
  - `save_assessment_plan(pool, offering_id, actor_user_id, payload) -> AssessmentPlanDetail`
  - `get_phase_controls(pool, academic_term_id) -> Vec<AssessmentPhaseControl>`
  - `update_phase_control(pool, academic_term_id, phase_code, payload) -> AssessmentPhaseControl`
- Removes: settings and submit service/handler/routes.

- [ ] **Step 1: Write failing database/service tests**

Cover one set-based list returning fixed phase summaries, common-primary suggestion, manual
coordinator persistence, unrelated-teacher denial, manager override, over/under total auto-save,
derived readiness, four default controls, control row-version conflict, and strict camelCase DTOs.

Include assertions equivalent to:

```rust
assert_eq!(detail.phases.len(), 4);
assert_eq!(detail.suggested_coordinator_id, Some(common_primary_id));
assert!(!saved.readiness.ready);
assert_eq!(saved.total_score, "105");
assert_eq!(controls.len(), 4);
assert!(matches!(stale_update, Err(AppError::Conflict(_))));
```

- [ ] **Step 2: Run focused service tests and verify failure**

```bash
./scripts/test_backend_school.sh modules::academic::services::assessment_service_tests \
  -- --nocapture --test-threads=1
```

- [ ] **Step 3: Implement set-based service queries and mutations**

Read summaries with one phase aggregation query, group/candidate queries in bounded batches, and no
per-row API loop. Persist the suggested coordinator only on first authorized save. Require the
existing school-manage permission for coordinator selection/controls and the exact assigned-manage
permission plus persisted coordinator identity for coordinator edits.

Save the complete four-phase snapshot in one transaction with plan row-version locking and upsert
all phases by canonical code. Return the saved detail and readiness. Remove settings/feature-toggle
logic and submit transitions.

- [ ] **Step 4: Replace handlers and routes**

Expose:

```text
GET /api/academic/assessments/plans?academicTermId=...
GET /api/academic/assessments/offerings/{offering_id}
PUT /api/academic/assessments/offerings/{offering_id}
GET /api/academic/assessments/phase-controls?academicTermId=...
PUT /api/academic/assessments/phase-controls/{phase_code}?academicTermId=...
```

Delete `/settings` and `/offerings/{offering_id}/submit` registrations and implementations.

- [ ] **Step 5: Run assessment service tests**

Run the Step 2 command. Expected: PASS.

- [ ] **Step 6: Run thin-handler architecture test**

```bash
CARGO_BUILD_JOBS=1 cargo test --manifest-path backend-school/Cargo.toml \
  --test static_architecture academic_handlers_keep_sql_in_services \
  -- --exact --test-threads=1
```

- [ ] **Step 7: Commit assessment API behavior**

```bash
git add backend-school/src/modules/academic/services/assessment_service.rs \
  backend-school/src/modules/academic/services/assessment_service_tests.rs \
  backend-school/src/modules/academic/handlers/assessment.rs \
  backend-school/src/modules/academic.rs
git commit -m "feat(academic): auto-save fixed assessment plans"
```

### Task 4: Exam Source Preview and Draft Synchronization

**Files:**
- Modify: `backend-school/src/modules/academic/models/exam_schedule.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service/workspace.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service/tests.rs`
- Modify: `backend-school/src/modules/academic/services/exam_schedule_service_tests.rs`
- Modify: `backend-school/src/modules/academic/handlers/exam_schedule.rs`
- Modify: `backend-school/src/modules/academic.rs`

**Interfaces:**
- Produces:
  - `preview_exam_source_changes(pool, round_id) -> ExamSourceChangePreview`
  - `apply_exam_source_changes(pool, round_id, actor_user_id, request) -> ExamSourceChangeApplyResult`
- Preserves: published round rows and imported item snapshots.

- [ ] **Step 1: Write failing source-classification tests**

Test `unchanged`, `newly_eligible`, `changed`, and `no_longer_eligible`; prove a points-only phase
change is `unchanged`. Add database cases for unplaced duration update, valid placed update,
conflicting placed update, explicit removal, stale preview, and published preview/apply rejection.

- [ ] **Step 2: Run focused exam tests and verify failure**

```bash
./scripts/test_backend_school.sh modules::academic::services::exam_schedule_service_tests \
  -- --nocapture --test-threads=1
```

- [ ] **Step 3: Implement typed preview/apply DTOs and service logic**

Use named enums/DTOs:

```rust
pub enum ExamSourceChangeKind {
    Unchanged,
    NewlyEligible,
    Changed,
    NoLongerEligible,
}

pub struct ApplyExamSourceChangesRequest {
    pub round_row_version: i64,
    pub decisions: Vec<ExamSourceChangeDecision>,
}
```

Preview compares current ready phase sources with imported snapshot duration/identity. Apply locks
the draft round, source plans/phases, affected items, sessions, days, and conflicts in stable order.
It inserts selected new items, updates selected safe durations, and deletes only explicitly selected
no-longer-eligible items. Reuse the existing session conflict helpers; never mutate a published
round. Return `409` when the preview fingerprint/row versions are stale.

- [ ] **Step 4: Register preview/apply endpoints**

```text
GET  /api/academic/exam-schedules/{round_id}/source-changes
POST /api/academic/exam-schedules/{round_id}/source-changes/apply
```

Keep legacy import/clear endpoints only if still used internally during the same atomic cutover;
remove them from runtime/frontend by the end of Task 6 so no compatibility workflow remains.

- [ ] **Step 5: Run focused exam tests**

Run the Step 2 command. Expected: PASS.

- [ ] **Step 6: Commit exam synchronization backend**

```bash
git add backend-school/src/modules/academic/models/exam_schedule.rs \
  backend-school/src/modules/academic/services/exam_schedule_service.rs \
  backend-school/src/modules/academic/services/exam_schedule_service/workspace.rs \
  backend-school/src/modules/academic/services/exam_schedule_service/tests.rs \
  backend-school/src/modules/academic/services/exam_schedule_service_tests.rs \
  backend-school/src/modules/academic/handlers/exam_schedule.rs \
  backend-school/src/modules/academic.rs
git commit -m "feat(exams): preview assessment source changes"
```

### Task 5: OpenAPI and Generated Frontend Contract

**Files:**
- Modify: `backend-school/src/api_contract.rs`
- Regenerate: `contracts/openapi/school-api.json`
- Regenerate: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/academicAssessments.ts`
- Modify: `frontend-school/src/lib/api/examSchedule.ts`
- Modify: `frontend-school/tests/static/academic-assessment-structure.test.mjs`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`

**Interfaces:**
- Consumes: Tasks 2–4 Rust DTOs/handlers.
- Produces: generated DTO aliases and concrete frontend API wrappers.

- [ ] **Step 1: Update static contract tests first**

Require fixed phase/coordinator/readiness/control schemas, auto-save operation, and exam source
preview/apply operations. Assert absence of submit/settings/free-form category/item/practical APIs.

- [ ] **Step 2: Run static tests and verify failure**

```bash
node --test frontend-school/tests/static/academic-assessment-structure.test.mjs \
  frontend-school/tests/static/academic-exam-schedule.test.mjs
```

- [ ] **Step 3: Register new paths/schemas and remove retired registrations**

Update `api_contract.rs` path and component lists using the exact DTO names created in Tasks 2–4.

- [ ] **Step 4: Generate contracts serially**

```bash
cd frontend-school && npm run generate:api-contracts
```

- [ ] **Step 5: Rewrite typed API wrappers**

`academicAssessments.ts` exports generated aliases for plan summary/detail, phase, readiness,
coordinator option, phase control, and save/update requests. It calls only the new plan/control
endpoints. `examSchedule.ts` adds typed preview/apply calls and removes retired clear/import calls
after the page cutover.

- [ ] **Step 6: Run focused static tests and contract checks**

```bash
node --test frontend-school/tests/static/academic-assessment-structure.test.mjs \
  frontend-school/tests/static/academic-exam-schedule.test.mjs
```

```bash
cd frontend-school && npm run check:api-contracts
```

```bash
cd frontend-school && npm run test:api-contracts
```

- [ ] **Step 7: Commit generated contract**

```bash
git add backend-school/src/api_contract.rs contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  frontend-school/src/lib/api/academicAssessments.ts \
  frontend-school/src/lib/api/examSchedule.ts \
  frontend-school/tests/static/academic-assessment-structure.test.mjs \
  frontend-school/tests/static/academic-exam-schedule.test.mjs
git commit -m "feat(api): publish fixed assessment contracts"
```

### Task 6: Assessment Workspace UI

**Files:**
- Create: `frontend-school/src/lib/components/academic/assessment/AssessmentPhaseControls.svelte`
- Create: `frontend-school/src/lib/components/academic/assessment/AssessmentPlanTable.svelte`
- Create: `frontend-school/src/lib/components/academic/assessment/AssessmentPlanEditor.svelte`
- Create: `frontend-school/src/lib/components/academic/assessment/assessmentPresentation.ts`
- Rewrite: `frontend-school/src/routes/(app)/staff/academic/assessments/+page.svelte`
- Modify: `frontend-school/tests/static/academic-assessment-structure.test.mjs`

**Interfaces:**
- Consumes: Task 5 generated assessment wrappers.
- Produces: compact table workspace, fixed phase editor, coordinator selection, phase controls, and
  debounced auto-save.

- [ ] **Step 1: Write failing static UI assertions**

Assert that the page/components contain the fixed phase codes, overview table, coordinator
selection, two switches per phase, auto-save state, readiness filters, and no category/item
add/delete, submit, settings, or practical controls.

- [ ] **Step 2: Run the assessment static test and verify failure**

```bash
node --test frontend-school/tests/static/academic-assessment-structure.test.mjs
```

- [ ] **Step 3: Implement presentation helpers and components**

Use shadcn-svelte Table, Select, Switch, Input, Badge, Button, and PageState. The four-phase rail is
the repeated visual signature. Components receive typed props and emit narrow callbacks; they do
not call APIs directly.

`AssessmentPlanEditor.svelte` keeps one local complete snapshot and emits it after a 600 ms debounce
or blur. Select changes flush immediately. It renders `กำลังบันทึก`, `บันทึกแล้ว`, or a persistent
retry state and never clears failed local input.

- [ ] **Step 4: Rewrite the route controller**

The route loads plans and controls only after exact read/manage permissions. Patch one saved plan
or control in local state rather than broadly reload. Register a dirty source only while save is
pending/failed. Preserve context selection and prerequisite states.

- [ ] **Step 5: Run Svelte autofixer on each changed Svelte file**

From `frontend-school`, run serially:

```bash
npx @sveltejs/mcp svelte-autofixer \
  'src/lib/components/academic/assessment/AssessmentPhaseControls.svelte' --svelte-version 5
npx @sveltejs/mcp svelte-autofixer \
  'src/lib/components/academic/assessment/AssessmentPlanTable.svelte' --svelte-version 5
npx @sveltejs/mcp svelte-autofixer \
  'src/lib/components/academic/assessment/AssessmentPlanEditor.svelte' --svelte-version 5
npx @sveltejs/mcp svelte-autofixer \
  'src/routes/(app)/staff/academic/assessments/+page.svelte' --svelte-version 5
```

Apply every valid issue, then rerun until each reports zero issues.

- [ ] **Step 6: Run focused static test and frontend check**

```bash
node --test frontend-school/tests/static/academic-assessment-structure.test.mjs
```

```bash
cd frontend-school && PUBLIC_BACKEND_URL=http://localhost:3000 \
  PUBLIC_VAPID_KEY=test npm run check
```

- [ ] **Step 7: Commit assessment UI**

```bash
git add frontend-school/src/lib/components/academic/assessment \
  'frontend-school/src/routes/(app)/staff/academic/assessments/+page.svelte' \
  frontend-school/tests/static/academic-assessment-structure.test.mjs
git commit -m "feat(academic): redesign assessment phase workspace"
```

### Task 7: Exam Schedule Source-Change UI

**Files:**
- Create: `frontend-school/src/lib/components/academic/exam-schedule/ExamSourceChangesPanel.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/exam-schedules/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`

**Interfaces:**
- Consumes: Task 5 typed exam source preview/apply API.
- Produces: round-level attention indicator and detailed safe synchronization workflow.

- [ ] **Step 1: Write failing UI assertions**

Require `ExamSourceChangesPanel`, `ตรวจสอบการเปลี่ยนแปลง`, new/changed/no-longer-eligible counts,
explicit removal decisions, published read-only copy, and absence of legacy import/clear buttons.

- [ ] **Step 2: Run focused static test and verify failure**

```bash
node --test frontend-school/tests/static/academic-exam-schedule.test.mjs
```

- [ ] **Step 3: Implement source-change panel and route integration**

Load preview with the workspace. A draft round lets managers select eligible import, duration sync,
or explicit removal decisions and apply one preview. Display per-item conflict results and retain
unchanged rows on partial rejection. A published round shows old/current values read-only and no
apply action. Extend the Task 4 round-list DTO/query with a set-based `sourceChangeCount`; the list
page consumes that value and never makes one source-preview request per round.

- [ ] **Step 4: Remove legacy import/clear UI and API usage**

Delete `importExamItems` and `clearMismatchedExamItems` calls/buttons/dialogs after the new apply
workflow fully covers their behavior. Remove retired backend routes/handlers/contracts in the same
commit if Task 4 temporarily retained them.

- [ ] **Step 5: Run Svelte autofixer serially**

```bash
cd frontend-school && npx @sveltejs/mcp svelte-autofixer \
  'src/lib/components/academic/exam-schedule/ExamSourceChangesPanel.svelte' --svelte-version 5
cd frontend-school && npx @sveltejs/mcp svelte-autofixer \
  'src/routes/(app)/staff/academic/exam-schedules/+page.svelte' --svelte-version 5
cd frontend-school && npx @sveltejs/mcp svelte-autofixer \
  'src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte' --svelte-version 5
```

- [ ] **Step 6: Run focused static and frontend checks**

```bash
node --test frontend-school/tests/static/academic-exam-schedule.test.mjs
```

```bash
cd frontend-school && PUBLIC_BACKEND_URL=http://localhost:3000 \
  PUBLIC_VAPID_KEY=test npm run check
```

- [ ] **Step 7: Commit exam source UI**

```bash
git add frontend-school/src/lib/components/academic/exam-schedule/ExamSourceChangesPanel.svelte \
  'frontend-school/src/routes/(app)/staff/academic/exam-schedules/+page.svelte' \
  'frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte' \
  frontend-school/tests/static/academic-exam-schedule.test.mjs \
  frontend-school/src/lib/api/examSchedule.ts backend-school/src/modules/academic.rs \
  backend-school/src/modules/academic/handlers/exam_schedule.rs \
  backend-school/src/api_contract.rs contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts
git commit -m "feat(exams): review assessment source changes"
```

### Task 8: Serial Final Verification and Tenant-Safe Review

**Files:**
- Review: every file changed in Tasks 1–7

**Interfaces:**
- Consumes: complete implementation.
- Produces: verified branch ready for user testing/deployment decision.

- [ ] **Step 1: Run backend focused suites serially**

```bash
./scripts/test_backend_school.sh modules::academic::services::assessment_service_tests \
  -- --nocapture --test-threads=1
```

```bash
./scripts/test_backend_school.sh modules::academic::services::exam_schedule_service_tests \
  -- --nocapture --test-threads=1
```

```bash
./scripts/test_backend_school.sh modules::academic::core::schema_tests \
  -- --nocapture --test-threads=1
```

- [ ] **Step 2: Run API contract matrix serially**

```bash
cd frontend-school && npm run check:api-contracts
```

```bash
cd frontend-school && npm run test:api-contracts
```

- [ ] **Step 3: Run frontend matrix serially**

```bash
cd frontend-school && npm run lint
```

```bash
cd frontend-school && PUBLIC_BACKEND_URL=http://localhost:3000 \
  PUBLIC_VAPID_KEY=test npm run check
```

```bash
cd frontend-school && npm run test:static
```

- [ ] **Step 4: Run complete backend suite once**

```bash
./scripts/test_backend_school.sh -- --test-threads=1
```

- [ ] **Step 5: Perform read-only SNWSB reconciliation**

Using the secret-backed direct Neon endpoint without printing credentials, run one read-only
transaction that checks only safe aggregates:

```sql
SELECT count(*) FROM course_assessment_plans;
SELECT phase_code, count(*) FROM course_assessment_phases GROUP BY phase_code;
SELECT count(*) FROM academic_exam_schedule_items;
SELECT count(*) FROM academic_assessment_phase_controls;
```

Do not apply migration 056 manually to production. This step is only available after deployment or
on an explicitly authorized disposable Neon branch; otherwise report it as unrun.

- [ ] **Step 6: Review repository state**

```bash
git diff --check
git status --short
git log --oneline --decorate -8
```

Review the complete diff for generated artifacts, migration immutability, authorization, N+1
queries, stale compatibility branches, sensitive data, and accidental unrelated edits.

- [ ] **Step 7: Commit verification-only corrections when the final diff contains them**

```bash
git commit -am "test(academic): complete assessment cutover coverage"
```

When verification required no correction, record the step as a no-op and do not create an empty
commit.
