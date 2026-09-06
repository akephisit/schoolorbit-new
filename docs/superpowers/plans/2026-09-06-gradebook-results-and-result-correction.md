# Gradebook, Academic Results, and Result Correction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the complete Release 2 Gradebook, course/activity/learner-evaluation result preparation, immutable academic-affairs locking, and append-only result correction cutover.

**Architecture:** Keep Assessment responsible only for the shared four-phase plan, put group score items and student scores behind a new Gradebook vertical module, put course/activity official outcomes behind a Results vertical module, and put the two course-linked learner-evaluation domains behind a Learner Evaluation vertical module. A forward-only migration creates the new ownership boundaries, migrates verified legacy activity results, removes overlapping storage, and all browser clients consume generated typed contracts with backend resource policies as the authority.

**Tech Stack:** PostgreSQL 16 migrations, Rust/Axum/SQLx/utoipa, SvelteKit 5 runes, TypeScript, shadcn-svelte, Tailwind CSS, Node test runner, Playwright, generated OpenAPI and permission registries.

**Spec:** `docs/superpowers/specs/2026-09-05-gradebook-results-and-result-correction-design.md`

## Global Constraints

- Never edit an applied migration; the first new tenant migration is `060`.
- Never store, return, log, or emit plaintext national IDs or unnecessary student-sensitive data.
- Use `contracts/permissions.json` and Rust/utoipa DTOs as sources of truth; regenerate artifacts instead of editing generated files.
- Every request carries `academicYearId` and `academicTermId`, and services validate all referenced rows against that context.
- Fixed phases are exactly `before_midterm`, `midterm`, `after_midterm`, and `final`.
- Gradebook cell states are only an exact decimal number or an absent row; clearing deletes and never writes zero.
- Learner-evaluation values are exactly `0`, `1`, `2`, or `3`; absence remains blank and blocks confirmation.
- Course grading is school-wide criterion grading only. Do not add group-referenced grading, percentile/standard-deviation grading, quotas, or a per-subject method selector.
- Course teacher selections are only derived, explicit `0`, `ร`, or `มส`; grades above zero are derived until academic-affairs correction.
- Activity outcomes are only `ผ`/`มผ` (`pass`/`fail`).
- Initial locks are immutable, corrections are append-only, and no compatibility API, view, dual write, or fallback read survives the cutover.
- Handlers remain session/context/policy/service/typed-response/realtime only; SQL belongs in services.
- New Svelte code uses runes, local shadcn-svelte primitives, `PageShell`, shared app states, and the project Svelte autofixer.
- Release 2 does not close terms, calculate GPA/GPAX, promote students, create transcripts, or implement report cards.

---

### Task 1: Extend the generated permission vocabulary and contract

**Files:**
- Modify: `contracts/permissions.schema.json`
- Modify: `contracts/permissions.json`
- Modify: `scripts/generate-permissions.mjs`
- Modify: `scripts/tests/generate-permissions.test.mjs`
- Modify: `frontend-school/src/lib/permissions/registry.ts`
- Generate: `contracts/permissions.lock.json`
- Generate: `backend-school/src/permissions/registry_generated.rs`
- Generate: `frontend-school/src/lib/permissions/registry.generated.ts`

**Interfaces:**
- Produces actions `lock` and `correct` in the validated contract vocabulary.
- Produces generated constants for the 19 approved `academic_gradebook`, `academic_result`, and `academic_learner_evaluation` permissions.

- [ ] **Step 1: Add failing generator tests for actions and exact permission codes**

```js
assert.equal(validatePermissionAction('lock'), true);
assert.equal(validatePermissionAction('correct'), true);
assert.deepEqual(
  generatedCodes.filter((code) => code.startsWith('academic_gradebook.')),
  [
    'academic_gradebook.manage.assigned',
    'academic_gradebook.manage.school',
    'academic_gradebook.read.assigned',
    'academic_gradebook.read.organization_unit',
    'academic_gradebook.read.school'
  ]
);
```

- [ ] **Step 2: Run the permission tests and verify they fail on the unrecognized actions/modules**

Run: `cd frontend-school && npm run test:permissions`

- [ ] **Step 3: Add `lock`/`correct`, Thai action labels, and all approved permission records**

Use only these modules/scopes:

```text
academic_gradebook: read.assigned, read.organization_unit, read.school, manage.assigned, manage.school
academic_result: read.assigned, read.organization_unit, read.school, manage.assigned, manage.school, lock.school, correct.school
academic_learner_evaluation: read.assigned, read.organization_unit, read.school, manage.assigned, manage.school, lock.school, correct.school
```

Do not add `organization_tree` because it is not in the approved permission list.

- [ ] **Step 4: Generate and verify permission artifacts**

Run:

```bash
cd frontend-school
npm run generate:permissions
npm run check:permissions
npm run test:permissions
```

- [ ] **Step 5: Commit the permission contract**

```bash
git add contracts/permissions.json contracts/permissions.schema.json contracts/permissions.lock.json \
  scripts/generate-permissions.mjs scripts/tests/generate-permissions.test.mjs \
  backend-school/src/permissions/registry_generated.rs \
  frontend-school/src/lib/permissions/registry.ts \
  frontend-school/src/lib/permissions/registry.generated.ts
git commit -m "feat: add gradebook and result permissions"
```

### Task 2: Add the transactional Release 2 tenant cutover

**Files:**
- Create: `backend-school/migrations/060_gradebook_results_and_learner_evaluations.sql`
- Modify: `backend-school/src/modules/academic/core/schema_tests.rs`
- Modify: `backend-school/src/modules/academic/cutover_test_support.rs`

**Interfaces:**
- Consumes the exact permission codes from Task 1.
- Produces the physical tables and constraints named in the approved spec.
- Produces one active grading policy with grade lower bounds `0/50/55/60/65/70/75/80` and one active learner-evaluation aggregation policy with lower bounds `0.00/1.00/1.50/2.50`.
- Produces migrated but unconfirmed `academic_activity_evaluations`, then removes `learning_results` and `activity_result_details`.

- [ ] **Step 1: Add schema tests for successful cutover and control ownership**

```rust
#[tokio::test]
async fn migration_060_creates_gradebook_result_boundaries() {
    let pool = migrated_pool_through(60).await;
    assert_columns(&pool, "academic_assessment_phase_controls", &["plan_editing_enabled"]).await;
    assert_missing_column(&pool, "academic_assessment_phase_controls", "score_entry_enabled").await;
    assert_row_count(&pool, "academic_gradebook_phase_controls", 4).await;
    assert_table_exists(&pool, "learning_group_student_scores").await;
    assert_table_exists(&pool, "academic_result_corrections").await;
}
```

Add separate exact tests for: 8+3 catalog seeds, policy seeds, score-control value copy, score-item identity/lifecycle preservation, permission and grant copy, recognized legacy activity migration, unknown outcome rejection, context mismatch rejection, invalid correction target rejection, and legacy-table removal.

- [ ] **Step 2: Run the focused tests and verify migration 060 is missing**

Run:

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_060_creates_gradebook_result_boundaries \
  -- --exact --nocapture --test-threads=1
```

- [ ] **Step 3: Implement preflight and Gradebook-owned schema**

The migration must first collect source counts and abort on unknown legacy outcomes or cross-context rows. Then:

```sql
CREATE TABLE academic_gradebook_phase_controls (... UNIQUE (academic_term_id, phase_code));
INSERT INTO academic_gradebook_phase_controls (..., score_entry_enabled, ...)
SELECT ..., score_entry_enabled, ... FROM academic_assessment_phase_controls;
ALTER TABLE academic_assessment_phase_controls DROP COLUMN score_entry_enabled;
ALTER TABLE learning_group_score_items
  ADD COLUMN lifecycle TEXT NOT NULL DEFAULT 'active'
    CHECK (lifecycle IN ('active', 'cancelled')),
  ADD COLUMN cancelled_at TIMESTAMPTZ,
  ADD COLUMN cancelled_by UUID REFERENCES users(id) ON DELETE SET NULL,
  ADD CONSTRAINT learning_group_score_items_cancelled_shape_check CHECK (
    (lifecycle = 'active' AND cancelled_at IS NULL AND cancelled_by IS NULL)
    OR (lifecycle = 'cancelled' AND cancelled_at IS NOT NULL)
  );
CREATE TABLE learning_group_student_scores (... UNIQUE (score_item_id, student_academic_year_id));
CREATE TABLE learning_group_phase_confirmations (... UNIQUE (learning_group_id, assessment_phase_id));
```

Use `NUMERIC(10,2)` and positive `row_version` constraints. Store ordered roster/source checksums and contextual composite foreign keys.

- [ ] **Step 4: Implement grading, learner-evaluation, activity, lock, and correction schema**

Create every table listed in the design. Enforce mutually exclusive correction targets with a check equivalent to:

```sql
CHECK (num_nonnulls(course_result_id, activity_result_id, subject_student_evaluation_id) = 1)
```

Store typed old/new outcome columns per target family rather than one unconstrained text payload. Seed the 8 desirable-characteristic rows, 3 reading/thinking/writing rows, grading bands, and learner-evaluation bands.

- [ ] **Step 5: Replace the per-offering grading JSON ownership**

Preflight that every existing offering total is representable, add `assessment_total_score NUMERIC(10,2)`, copy the existing total, and remove `grading_policy`. Do not retain `policyCode` or `passingScore` in storage.

- [ ] **Step 6: Migrate legacy activity values, verify identity/counts, and drop legacy tables**

Map only normalized `pass` and `fail`. Migrated rows remain unconfirmed. Verify every source identity exists in the target before:

```sql
DROP TABLE activity_result_details;
DROP TABLE learning_results;
```

- [ ] **Step 7: Insert permission definitions and copy grants by lineage**

Copy matching existing `academic_assessment` grants into the new modules across `role_permissions`, `organization_permission_grants`, and `organization_permission_delegations`. Copy assigned/read/manage capabilities separately from school manage/lock/correct; do not infer grants from role names.

- [ ] **Step 8: Run all migration 060 cases**

Run each exact migration test through `./scripts/test_backend_school.sh ... --test-threads=1`, ending with the recognized migration/count/removal case and both atomic rejection cases.

- [ ] **Step 9: Commit the cutover**

```bash
git add backend-school/migrations/060_gradebook_results_and_learner_evaluations.sql \
  backend-school/src/modules/academic/core/schema_tests.rs \
  backend-school/src/modules/academic/cutover_test_support.rs
git commit -m "feat: add gradebook result schema cutover"
```

### Task 3: Separate Assessment controls and initialize new terms

**Files:**
- Modify: `backend-school/src/modules/academic/models/assessment.rs`
- Modify: `backend-school/src/modules/academic/services/assessment_service.rs`
- Modify: `backend-school/src/modules/academic/services/assessment_service_tests.rs`
- Modify: `backend-school/src/modules/academic/core/services/years_terms.rs`
- Modify: `backend-school/src/modules/academic/core/services_tests.rs`
- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/offerings.rs`
- Test: `backend-school/src/modules/academic/delivery/services_tests.rs`

**Interfaces:**
- Produces `UpdateAssessmentPhaseControlRequest { plan_editing_enabled, row_version }` with no score-entry field.
- Produces `AssessmentPhaseControl { id, academic_year_id, academic_term_id, phase_code, plan_editing_enabled, row_version }`.
- Produces `assessment_total_score: Decimal` on offering/assessment DTOs; removes `CourseGradingPolicy`.
- New terms receive four Assessment controls, four Gradebook controls, and two learner-evaluation controls in one transaction.

- [ ] **Step 1: Change tests to reject Assessment score-entry ownership and assert all new controls**

```rust
assert_eq!(assessment_controls.len(), 4);
assert_eq!(gradebook_controls.len(), 4);
assert_eq!(learner_evaluation_controls.len(), 2);
assert_eq!(domains, ["desirable_characteristic", "reading_thinking_writing"]);
```

Add a service test proving an unchanged phase upsert does not increment `course_assessment_phases.row_version`.

- [ ] **Step 2: Run the focused Assessment and term-creation tests and verify failures**

Run:

```bash
./scripts/test_backend_school.sh modules::academic::services::assessment_service_tests -- --nocapture
./scripts/test_backend_school.sh modules::academic::core::services_tests::create_term_seeds_phase_controls -- --exact --nocapture
```

- [ ] **Step 3: Remove score-entry fields/SQL and seed the new control rows**

Update `create_term` to insert all three control families before the audit/commit. Keep plan control mutations limited to `plan_editing_enabled`.

- [ ] **Step 4: Prevent no-op Assessment autosaves from invalidating confirmations**

In phase updates, apply:

```sql
... SET max_score = $n, exam_arrangement = $n, ...,
        row_version = row_version + 1
WHERE id = $id
  AND row_version = $expected
  AND (max_score, exam_arrangement, exam_duration_minutes)
      IS DISTINCT FROM ($max, $arrangement, $duration)
```

When values are unchanged, return the current row without treating it as an optimistic conflict.

- [ ] **Step 5: Replace `grading_policy` DTO/SQL usage with `assessment_total_score`**

Remove `policyCode`, `passingScore`, and `CourseGradingPolicy` from runtime contracts. Validate that the four phase maxima equal `assessment_total_score` when the plan is ready.

- [ ] **Step 6: Run focused tests and commit**

```bash
git add backend-school/src/modules/academic backend-school/src/modules/academic/delivery
git commit -m "refactor: separate assessment and gradebook controls"
```

### Task 4: Implement Gradebook authorization, services, and HTTP API

**Files:**
- Create: `backend-school/src/modules/academic/gradebook.rs`
- Create: `backend-school/src/modules/academic/gradebook/models.rs`
- Create: `backend-school/src/modules/academic/gradebook/handlers.rs`
- Create: `backend-school/src/modules/academic/gradebook/services.rs`
- Create: `backend-school/src/modules/academic/gradebook/services/controls.rs`
- Create: `backend-school/src/modules/academic/gradebook/services/workspace.rs`
- Create: `backend-school/src/modules/academic/gradebook/services/items.rs`
- Create: `backend-school/src/modules/academic/gradebook/services/scores.rs`
- Create: `backend-school/src/modules/academic/gradebook/services/confirmations.rs`
- Create: `backend-school/src/modules/academic/gradebook/services_tests.rs`
- Create: `backend-school/src/policies/gradebook_access_policy.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/src/policies.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Produces `gradebook::routes()` under `/gradebook`.
- Produces `list_subjects`, `list_controls`, `update_control`, `get_group_phase_workspace`, `create_item`, `update_item`, `remove_item`, `save_scores_batch`, and `confirm_phase`.
- Score mutations use `enum ScoreCellMutation { Set { score_item_id, student_academic_year_id, value, row_version }, Clear { ... } }` with `#[serde(tag = "operation", rename_all = "snake_case")]`.
- Item removal returns `ScoreItemRemovalOutcome { disposition: Deleted | Cancelled, item_id, row_version }`.

- [ ] **Step 1: Write policy/service tests for access, blank/zero, item lifecycle, and confirmation rules**

Cover assigned, organization-unit, school, denied, and union list scopes; primary-only confirmation; closed-window denial and school override; decimal boundaries; clear deletes; lower maximum rejection; hard-delete/cancel; exact phase total; blank warning count; roster/source checksum invalidation; and rename/reorder non-invalidation.

- [ ] **Step 2: Run the Gradebook tests and verify the module is missing**

Run: `./scripts/test_backend_school.sh modules::academic::gradebook::services_tests -- --nocapture`

- [ ] **Step 3: Implement resource policy and deterministic source helpers**

Expose policy decisions for `can_read_group`, `can_manage_group`, `can_confirm_group_phase`, and `can_manage_school`. Resolve current effective teacher episodes and primary role inside the term/date boundary. Build ordered checksums from active roster IDs, active item IDs/maxima, phase row version, and score revisions.

- [ ] **Step 4: Implement controls/workspace/item services**

List endpoints must union authorized scopes and issue bounded queries. Create/update/remove item operations must validate context, window, lock state, row version, and maximum rules in a transaction.

- [ ] **Step 5: Implement atomic score batches**

Normalize and deduplicate `(score_item_id, student_academic_year_id)`, validate every cell before writing, and use one transaction. `Set(0)` inserts/updates exact zero; `Clear` deletes. Return updated/deleted cell versions and a new workspace revision.

- [ ] **Step 6: Implement phase confirmation and targeted invalidation**

Confirmation snapshots calculation-relevant source checksums and reports `blank_score_count`. Mutations that affect calculation delete/mark stale only the affected group-phase confirmation. Control toggles, item rename, and item reorder do not invalidate it.

- [ ] **Step 7: Add typed handlers, route registration, and static architecture ownership**

Every handler must use `AuthenticatedSession`, actor tenant context, policy, service, `ApiResponse`, and a PII-free realtime signal. Register literal routes before `{id}` routes.

- [ ] **Step 8: Run focused tests and commit**

```bash
git add backend-school/src/modules/academic/gradebook.rs \
  backend-school/src/modules/academic/gradebook \
  backend-school/src/modules/academic.rs backend-school/src/policies.rs \
  backend-school/src/policies/gradebook_access_policy.rs backend-school/tests/static_architecture.rs
git commit -m "feat: add gradebook services and api"
```

### Task 5: Implement Learner Evaluation services and independent domain locks

**Files:**
- Create: `backend-school/src/modules/academic/learner_evaluation.rs`
- Create: `backend-school/src/modules/academic/learner_evaluation/models.rs`
- Create: `backend-school/src/modules/academic/learner_evaluation/handlers.rs`
- Create: `backend-school/src/modules/academic/learner_evaluation/services.rs`
- Create: `backend-school/src/modules/academic/learner_evaluation/services/catalog.rs`
- Create: `backend-school/src/modules/academic/learner_evaluation/services/configuration.rs`
- Create: `backend-school/src/modules/academic/learner_evaluation/services/entry.rs`
- Create: `backend-school/src/modules/academic/learner_evaluation/services/confirmation.rs`
- Create: `backend-school/src/modules/academic/learner_evaluation/services/locking.rs`
- Create: `backend-school/src/modules/academic/learner_evaluation/services/summary.rs`
- Create: `backend-school/src/modules/academic/learner_evaluation/services_tests.rs`
- Create: `backend-school/src/policies/learner_evaluation_access_policy.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/src/policies.rs`

**Interfaces:**
- Produces `LearnerEvaluationDomain::{DesirableCharacteristic, ReadingThinkingWriting}` and `LearnerEvaluationLevel` validated to `0..=3`.
- Produces catalog/configuration/control/workspace/batch-response/group-confirmation/subject-domain-lock/summary endpoints under `/learner-evaluations`.
- `summarize_student_term(pool, context, student_academic_year_id)` returns per-domain exact average, policy level, completeness, missing subjects, catalog-criterion averages, and subject drilldown.

- [ ] **Step 1: Write failing tests for catalog copies, independent controls, configuration lifecycle, response entry, confirmation, locking, summary, and correction recalculation**

Include: new subject config copies all active catalog criteria; later catalog edits do not silently change it; response-backed criteria deactivate instead of delete; closed-domain override; blank blocks confirmation but level 0 is valid; independent domain locks; all-room atomicity; equal subject weighting; subject-only criteria do not create catalog summary rows; threshold edges `0/1.00/1.50/2.50`; provisional missing-subject reasons.

- [ ] **Step 2: Run the focused tests and verify failure**

Run: `./scripts/test_backend_school.sh modules::academic::learner_evaluation::services_tests -- --nocapture`

- [ ] **Step 3: Implement catalog and subject-term configuration**

Use the persisted Assessment coordinator for subject configuration. Create the configuration transactionally on first access. Treat criterion rename/reorder as non-calculation changes; activation/deactivation invalidates affected confirmations.

- [ ] **Step 4: Implement controls, batch entry, and group confirmation**

Batch writes validate all group/student/criterion/domain/context rows before mutation. Confirmation requires every active criterion for every current student and returns typed missing pairs.

- [ ] **Step 5: Implement subject-domain locks and immutable source rows**

Lock all active course groups for one `subject_id + term + domain` in one transaction, after revalidating every group confirmation. Write immutable response/configuration snapshots.

- [ ] **Step 6: Implement equal-subject term summaries**

First average active criteria within each locked subject, then average those subject values. Apply the active versioned aggregation bands and return provisional status until every expected subject-domain lock exists.

- [ ] **Step 7: Add policies, typed handlers, routes, and commit**

```bash
git add backend-school/src/modules/academic/learner_evaluation.rs \
  backend-school/src/modules/academic/learner_evaluation \
  backend-school/src/modules/academic.rs backend-school/src/policies.rs \
  backend-school/src/policies/learner_evaluation_access_policy.rs
git commit -m "feat: add course learner evaluations"
```

### Task 6: Implement criterion grading and course result preparation

**Files:**
- Create: `backend-school/src/modules/academic/results.rs`
- Create: `backend-school/src/modules/academic/results/models.rs`
- Create: `backend-school/src/modules/academic/results/handlers.rs`
- Create: `backend-school/src/modules/academic/results/services.rs`
- Create: `backend-school/src/modules/academic/results/services/policies.rs`
- Create: `backend-school/src/modules/academic/results/services/course_preparation.rs`
- Create: `backend-school/src/modules/academic/results/services/activities.rs`
- Create: `backend-school/src/modules/academic/results/services/readiness.rs`
- Create: `backend-school/src/modules/academic/results/services/locking.rs`
- Create: `backend-school/src/modules/academic/results/services/corrections.rs`
- Create: `backend-school/src/modules/academic/results/services_tests.rs`
- Create: `backend-school/src/policies/academic_result_access_policy.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/src/policies.rs`

**Interfaces:**
- Produces immutable `GradingPolicyVersion` and ordered `GradingPolicyBand` services.
- Produces `CourseOutcomeSelection::{Derived, ExplicitZero, Incomplete, InsufficientAttendance}`.
- Produces course preparation workspace and `confirm_group_results` endpoints under `/results`.
- Produces assigned activity entry/confirmation and school readiness APIs.

- [ ] **Step 1: Write failing pure/service tests for policy validation and prepared outcomes**

Cover inclusive lower bounds, exact decimal totals, blank score as calculation zero without storage mutation, only derived/0/ร/มส teacher choices, activated-policy invalidation of unlocked confirmations, policy snapshot stability after lock, and no method/group-grading DTO.

- [ ] **Step 2: Write failing activity tests**

Cover pass/fail only, blank completeness, membership invalidation, primary-only confirmation, independent group readiness, and bulk ready-lock skip reasons.

- [ ] **Step 3: Run result tests and verify failure**

Run: `./scripts/test_backend_school.sh modules::academic::results::services_tests -- --nocapture`

- [ ] **Step 4: Implement versioned school grading policies**

Creating a policy validates exactly eight grade outcomes and ordered inclusive lower bounds. Activating it is transactional, deactivates the prior version, keeps activated versions immutable, and invalidates only unlocked group result confirmations.

- [ ] **Step 5: Implement course preparation and group confirmation**

Require current confirmations for all four phases. Calculate totals from active items and current numeric rows, treating missing rows as zero only in calculation. Persist explicit 0/ร/มส selections with optimistic versions. Group confirmation snapshots all phase confirmation revisions, active policy version, and selections.

- [ ] **Step 6: Implement activity entry/confirmation and readiness**

Use current activity participants and effective assigned teachers. Blank outcomes block confirmation. Return typed per-group readiness without issuing one query per group.

- [ ] **Step 7: Add authorization, handlers, routes, and commit**

```bash
git add backend-school/src/modules/academic/results.rs \
  backend-school/src/modules/academic/results \
  backend-school/src/modules/academic.rs backend-school/src/policies.rs \
  backend-school/src/policies/academic_result_access_policy.rs
git commit -m "feat: add academic result preparation"
```

### Task 7: Implement initial locks, effective results, and append-only corrections

**Files:**
- Modify: `backend-school/src/modules/academic/results/models.rs`
- Modify: `backend-school/src/modules/academic/results/handlers.rs`
- Modify: `backend-school/src/modules/academic/results/services/readiness.rs`
- Modify: `backend-school/src/modules/academic/results/services/locking.rs`
- Modify: `backend-school/src/modules/academic/results/services/corrections.rs`
- Modify: `backend-school/src/modules/academic/results/services_tests.rs`
- Modify: `backend-school/src/modules/academic/learner_evaluation/services/summary.rs`
- Modify: `backend-school/src/modules/academic/learner_evaluation/services_tests.rs`

**Interfaces:**
- Produces course lock by `subject_id + academic_term_id`, activity lock by group, learner-evaluation lock by `subject_id + domain`, and `lock_all_ready_activities`.
- Produces correction requests tagged `course`, `activity`, or `learner_evaluation`, each consuming the expected effective version.
- Produces effective result views as immutable initial row plus ordered correction history and latest effective value.

- [ ] **Step 1: Add failing transactional lock tests**

Assert course all-room atomicity, exact failing room/phase reasons, policy/source snapshot, locked-source write rejection, independent activity locks, independent learner-evaluation domain locks, and bulk activity skips.

- [ ] **Step 2: Add failing correction tests**

Assert append-only rows, unchanged initial rows/scores/totals, valid target-specific outcomes, stale effective-version rejection, academic-affairs-only access, and learner-summary recalculation after a criterion correction.

- [ ] **Step 3: Run both service suites and verify failure**

Run:

```bash
./scripts/test_backend_school.sh modules::academic::results::services_tests -- --nocapture
./scripts/test_backend_school.sh modules::academic::learner_evaluation::services_tests -- --nocapture
```

- [ ] **Step 4: Implement lock transactions and source immutability guards**

Revalidate all source revisions under row locks, then write headers and immutable detail rows. Every normal Assessment, Gradebook, result-preparation, and learner-evaluation mutation must call a shared lock-state guard before changing a locked scope.

- [ ] **Step 5: Implement target-specific corrections**

Resolve current effective value, compare `expected_effective_version`, validate the new value family, append one correction, and return the updated effective view. Do not add a free-text remark.

- [ ] **Step 6: Implement readiness queue and effective-result search**

Return bounded subject/group/domain summaries with actionable blockers in one workspace request. Search results expose school/student identifiers needed for authorized work but never national IDs.

- [ ] **Step 7: Run focused tests and commit**

```bash
git add backend-school/src/modules/academic/results \
  backend-school/src/modules/academic/learner_evaluation
git commit -m "feat: lock and correct academic results"
```

### Task 8: Remove legacy result runtime references and repair delivery impact counts

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/services/activities.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/change_sets.rs`
- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Replaces legacy `ActivityResult` and legacy impact counters with new activity evaluation/lock ownership.
- Produces no runtime token `learning_results`, `activity_result_details`, `CourseGradingPolicy`, `policyCode`, `passingScore`, or Assessment-owned `scoreEntryEnabled`.

- [ ] **Step 1: Add failing static and delivery tests for the clean boundary**

```rust
assert!(!runtime_sources_contain("learning_results"));
assert!(!runtime_sources_contain("activity_result_details"));
assert!(!runtime_sources_contain("CourseGradingPolicy"));
```

Change destructive impact expectations to count new mutable evaluations and immutable locks/results separately so a user receives an actionable stop/delete warning.

- [ ] **Step 2: Run static/delivery tests and verify legacy references fail**

Run:

```bash
cd backend-school
cargo test --test static_architecture
cd ..
./scripts/test_backend_school.sh modules::academic::delivery::services_tests -- --nocapture
```

- [ ] **Step 3: Replace runtime SQL/models and delete legacy API schema registrations**

Do not add compatibility aliases. Historical migration text remains untouched and is excluded from runtime-source assertions.

- [ ] **Step 4: Run focused tests and commit**

```bash
git add backend-school/src/modules/academic/delivery backend-school/src/api_contract.rs \
  backend-school/tests/static_architecture.rs
git commit -m "refactor: remove legacy academic result runtime"
```

### Task 9: Register and generate the complete HTTP contract

**Files:**
- Modify: `backend-school/src/api_contract.rs`
- Generate: `contracts/openapi/school-api.json`
- Generate: `frontend-school/src/lib/api/generated/school-api.ts`
- Create: `frontend-school/src/lib/api/academicGradebook.ts`
- Create: `frontend-school/src/lib/api/academicLearnerEvaluations.ts`
- Create: `frontend-school/src/lib/api/academicResults.ts`
- Modify: `frontend-school/src/lib/api/academicAssessments.ts`
- Modify: `frontend-school/tests/static/api-query-contract.test.mjs`
- Modify: `frontend-school/tests/static/api-response-contract.test.mjs`
- Create: `frontend-school/tests/static/academic-gradebook-results-contract.test.mjs`

**Interfaces:**
- Every route from Tasks 4–7 has `utoipa::path`, a unique operation ID, required camel-case academic context query fields, and a standard `ApiResponse<T>` schema.
- Frontend wrappers export aliases from `components['schemas']`, derive query/path shapes from `operations[...]`, and return concrete typed resources.

- [ ] **Step 1: Add failing contract tests for exact routes, DTOs, envelopes, and removed fields**

Assert all Gradebook/Learner Evaluation/Results operations exist, `academicYearId` and `academicTermId` are required where applicable, Assessment controls omit `scoreEntryEnabled`, and no group-grading/method-selector schema exists.

- [ ] **Step 2: Run backend contract tests and verify missing registrations**

Run: `cd backend-school && cargo test api_contract::tests --bin backend-school -- --nocapture`

- [ ] **Step 3: Register every handler path and schema in `SchoolApiDoc`**

Use the feature DTOs directly. Wrap list/single/empty results with named `ApiResponse` aliases following existing contract patterns.

- [ ] **Step 4: Generate OpenAPI/TypeScript and implement strict wrappers**

Wrapper calls use the generated operation query/path types with `satisfies`; no handwritten wire interface, `unknown`, `Record<string, unknown>`, response cast, snake-case query alias, or unbounded per-row request is allowed.

- [ ] **Step 5: Run contract generation/check/tests and commit**

```bash
cd frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
node --test tests/static/academic-gradebook-results-contract.test.mjs
cd ..
git add backend-school/src/api_contract.rs contracts/openapi/school-api.json \
  frontend-school/src/lib/api frontend-school/tests/static
git commit -m "feat: publish gradebook result api contracts"
```

### Task 10: Build Gradebook ledger state, keyboard navigation, and save queue

**Files:**
- Create: `frontend-school/src/lib/academic/gradebook/ledger.ts`
- Create: `frontend-school/src/lib/academic/gradebook/save-queue.ts`
- Create: `frontend-school/tests/runtime/gradebook-ledger.test.ts`
- Create: `frontend-school/tests/runtime/gradebook-save-queue.test.ts`

**Interfaces:**
- `nextEditableCell(position, selectedItemIds, studentIds, direction)` traverses selected columns only.
- `normalizeScorePaste(text, origin, selectedItemIds, studentIds)` returns a bounded rectangular mutation list or a typed validation error.
- `createGradebookSaveQueue({ delayMs: 750, saveBatch })` exposes `enqueue`, `flush`, `retry`, `discard`, `status`, and `subscribe`.

- [ ] **Step 1: Write failing pure tests**

Cover initial no-selected-columns, select-all/clear, new-column selection, unchecked read-only traversal, Tab/Enter directions, paste bounds, blank-to-clear, explicit `0`, decimal validation, debounce/coalescing, flush-before-close, retry, and retained local values on 409.

- [ ] **Step 2: Run runtime tests and verify failure**

Run:

```bash
cd frontend-school
node --experimental-strip-types --test \
  tests/runtime/gradebook-ledger.test.ts \
  tests/runtime/gradebook-save-queue.test.ts
```

- [ ] **Step 3: Implement the pure ledger helpers and save queue**

Keep server totals out of these helpers. The queue batches only cells for one current group/phase context and requires flush/discard before context changes.

- [ ] **Step 4: Run tests and commit**

```bash
git add frontend-school/src/lib/academic/gradebook frontend-school/tests/runtime
git commit -m "feat: add gradebook ledger state"
```

### Task 11: Build the responsive Gradebook and learner-evaluation entry workspace

**Files:**
- Create: `frontend-school/src/lib/components/academic/gradebook/GradebookWorkspaceHeader.svelte`
- Create: `frontend-school/src/lib/components/academic/gradebook/GradebookEntryControls.svelte`
- Create: `frontend-school/src/lib/components/academic/gradebook/ScoreLedger.svelte`
- Create: `frontend-school/src/lib/components/academic/gradebook/ScoreItemDialog.svelte`
- Create: `frontend-school/src/lib/components/academic/gradebook/LearnerEvaluationLedger.svelte`
- Create: `frontend-school/src/lib/components/academic/gradebook/SubjectCriteriaDialog.svelte`
- Create: `frontend-school/src/lib/components/academic/gradebook/GradebookMobileEditor.svelte`
- Create: `frontend-school/src/lib/components/academic/gradebook/PhaseConfirmationDialog.svelte`
- Create: `frontend-school/src/routes/(app)/staff/academic/gradebook/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/academic/gradebook/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/assessments/+page.svelte`
- Modify: `frontend-school/tests/static/academic-assessment-structure.test.mjs`
- Modify: `frontend-school/tests/e2e/assessment-editor-reopen.spec.ts`
- Create: `frontend-school/tests/static/academic-gradebook.test.mjs`
- Create: `frontend-school/tests/e2e/gradebook-workflow.spec.ts`

**Interfaces:**
- Route metadata uses `academicContext: 'term_required'`, `workspace: 'academic'`, `group: 'academic_assessment'`, and discovery permission alternatives for Gradebook/Learner Evaluation readers.
- URL state uses `subjectId`, `learningGroupId`, `tab`, and `phase`.
- Score and evaluation matrices expose local column checkbox state but never send it to the API.

- [ ] **Step 1: Add failing static/E2E discovery tests**

Assert page/permission metadata, three top tabs, four score phase tabs, manager controls only after exact permission, read-only users make no action-only request, mobile explicit close, sticky save state, and Assessment no longer mutates Gradebook controls.

- [ ] **Step 2: Implement the Svelte components using the approved design**

Desktop uses fixed readable widths, horizontally scrolling content, sticky student identity columns, checkbox-selected editable columns, compact item actions, server-derived totals, and shared app states. Mobile uses a full-screen Sheet with sticky header, explicit back/X, student/item context, and sticky save status.

- [ ] **Step 3: Implement orchestration and context safety**

Use `$state.raw` for replaced API workspaces, `$derived` for computed presentation, `LatestRequest` for stale cancellation, and `registerAcademicContextDirtySource()` to flush or offer retry/discard before year/term/subject/group/tab changes.

- [ ] **Step 4: Make Assessment show only read-only Gradebook status/link**

Only load that status when `$can.hasAny()` includes a Gradebook read permission; never issue Gradebook manager calls from the Assessment page.

- [ ] **Step 5: Run the Svelte autofixer on every created/modified component**

Run: `npx @sveltejs/mcp svelte-autofixer <path> --svelte-version 5` for each `.svelte` path and resolve every reported issue.

- [ ] **Step 6: Run focused static/runtime checks and commit**

```bash
cd frontend-school
node --test tests/static/academic-gradebook.test.mjs tests/static/academic-assessment-structure.test.mjs
node --experimental-strip-types --test tests/runtime/gradebook-ledger.test.ts tests/runtime/gradebook-save-queue.test.ts
cd ..
git add frontend-school/src/lib/components/academic/gradebook \
  frontend-school/src/routes/'(app)'/staff/academic/gradebook \
  frontend-school/src/routes/'(app)'/staff/academic/assessments \
  frontend-school/tests
git commit -m "feat: add responsive gradebook workspace"
```

### Task 12: Build result preparation, locking, summary, and correction workspaces

**Files:**
- Create: `frontend-school/src/lib/academic/results/presentation.ts`
- Create: `frontend-school/src/lib/academic/learner-evaluation/presentation.ts`
- Create: `frontend-school/src/lib/components/academic/results/ResultPreparationTable.svelte`
- Create: `frontend-school/src/lib/components/academic/results/ActivityEvaluationTable.svelte`
- Create: `frontend-school/src/lib/components/academic/results/LearnerEvaluationSummary.svelte`
- Create: `frontend-school/src/lib/components/academic/results/ResultLockQueue.svelte`
- Create: `frontend-school/src/lib/components/academic/results/ResultCorrectionDialog.svelte`
- Create: `frontend-school/src/routes/(app)/staff/academic/results/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/academic/results/+page.svelte`
- Create: `frontend-school/src/routes/(app)/staff/academic/result-locks/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/academic/result-locks/+page.svelte`
- Create: `frontend-school/src/routes/(app)/staff/academic/result-corrections/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/academic/result-corrections/+page.svelte`
- Create: `frontend-school/tests/runtime/learner-evaluation-summary.test.ts`
- Create: `frontend-school/tests/static/academic-results.test.mjs`
- Create: `frontend-school/tests/e2e/academic-result-locking.spec.ts`

**Interfaces:**
- Result preparation allows only derived/explicit 0/ร/มส and group-primary confirmation.
- Lock queue groups course by subject, learner evaluation by subject/domain, and activity by group with blockers visible before mutation.
- Correction UI displays immutable initial, effective current, and ordered history; it appends and never edits history.

- [ ] **Step 1: Write failing presentation/runtime/static tests**

Assert no group-grading/method-selector UI, owned subjects first, exact Thai outcome labels, subject readiness automatic after all groups, activity blank blocking, provisional learner summary reasons, independent domain locks, exact route permissions, and no mutation-only request for readers.

- [ ] **Step 2: Implement pure presentation helpers and route metadata**

The preparation route accepts result/learner-evaluation read alternatives. Lock and correction routes use exact lock/correct school permission alternatives and create separate menu services.

- [ ] **Step 3: Implement preparation UI**

Show policy version read-only to teachers, course previews and per-room confirmations, activity pass/fail entry, learner-evaluation readiness, and derived term summaries. Do not add a redundant submit button.

- [ ] **Step 4: Implement academic-affairs lock and correction UI**

Use typed blockers, action-specific loading states, optimistic effective versions, and affected-row patching. A failed/stale correction retains the user's selected value and offers refresh.

- [ ] **Step 5: Run Svelte autofixer on every created component/page**

Run the project autofixer with Svelte 5 for every `.svelte` path and resolve every reported issue.

- [ ] **Step 6: Run focused tests and commit**

```bash
cd frontend-school
node --test tests/static/academic-results.test.mjs
node --experimental-strip-types --test tests/runtime/learner-evaluation-summary.test.ts
cd ..
git add frontend-school/src/lib/academic frontend-school/src/lib/components/academic/results \
  frontend-school/src/routes/'(app)'/staff/academic/results \
  frontend-school/src/routes/'(app)'/staff/academic/result-locks \
  frontend-school/src/routes/'(app)'/staff/academic/result-corrections \
  frontend-school/tests
git commit -m "feat: add academic result workspaces"
```

### Task 13: Integrate menu/context/prerequisite/request-discipline contracts

**Files:**
- Modify: `frontend-school/tests/static/academic-context-contract.test.mjs`
- Modify: `frontend-school/tests/static/academic-work-organization.test.mjs`
- Modify: `frontend-school/tests/static/api-global-contract.test.mjs`
- Modify: `frontend-school/tests/static/academic-page-prerequisites.test.mjs`
- Modify: `frontend-school/tests/static/academic-workspace-request-count.test.mjs`
- Modify: `frontend-school/tests/runtime/menu-route-registration.test.mjs`
- Modify: `backend-school/src/modules/menu/services/academic_template_service.rs`

**Interfaces:**
- Produces discoverable services under the existing `academic_assessment` work section only after their backend workflow exists.
- Preserves system-owned paths/permission gates and school-owned labels/order through menu synchronization.

- [ ] **Step 1: Add failing route registration, context, prerequisite, and request-count assertions**

Gradebook depends on Delivery+Assessment, Results depends on Gradebook confirmations, and locks/corrections depend on prepared/locked Results respectively. Each page uses bounded workspace endpoints and never requests once per room/student.

- [ ] **Step 2: Update the academic template and route metadata expectations**

Add Gradebook, Results, Result Locks, and Result Corrections beneath `academic_assessment`; do not create a new work section. Navigation placement must not grant access.

- [ ] **Step 3: Run menu/static tests and commit**

```bash
cd frontend-school
npm run test:menu-sync
node --test tests/static/academic-context-contract.test.mjs \
  tests/static/academic-work-organization.test.mjs \
  tests/static/academic-page-prerequisites.test.mjs \
  tests/static/academic-workspace-request-count.test.mjs
cd ..
git add frontend-school/tests backend-school/src/modules/menu/services/academic_template_service.rs
git commit -m "feat: register academic result workflows"
```

### Task 14: Add cutover operations, smoke coverage, and narrow the future backlog

**Files:**
- Modify: `backend-school/src/modules/system/handlers/migration.rs`
- Modify: `.github/workflows/backend-school-neon-compatibility.yml`
- Modify: `.github/workflows/deploy-backend-school.yml`
- Modify: `scripts/smoke_test.sh`
- Modify: `frontend-school/tests/static/deployment-installer.test.mjs`
- Modify: `docs/TESTING.md`
- Modify: `docs/OPERATIONS.md`
- Modify: `TODO.md`

**Interfaces:**
- Produces a bounded `gradebookResultsCutover` migration-status audit with no score/student payload.
- Deployment keeps maintenance enabled unless latest migration equality and cutover audit both pass.
- Leaves `SCH-002` only for term closure/reopen, GPA/GPAX, promotion, graduation, transcripts/report cards, and ปพ.

- [ ] **Step 1: Add failing deployment/static assertions for the new destructive cutover gate**

Assert the compatibility workflow runs the migration 060 cases against a direct non-pooler disposable Neon branch and deploy requires the typed cutover audit before disabling maintenance.

- [ ] **Step 2: Implement the bounded migration audit and workflow gate**

Report only counts/status such as controls copied, expected seed versions, legacy tables absent, and new tables present. Do not return names, scores, outcomes, or student identifiers.

- [ ] **Step 3: Extend the authenticated smoke test with read-only bounded endpoints**

Probe at most two terms and keep response payloads in the script's private temporary directory. Log only HTTP/status summaries.

- [ ] **Step 4: Update canonical operations/testing documentation and backlog**

Document: fresh protected Neon snapshot, maintenance enabled, one reviewed SHA, direct-endpoint compatibility, centralized migration, latest-version and cutover audit, menu sync, authenticated smoke, go/no-go, snapshot rollback only before the first accepted write, and forward repair after it.

- [ ] **Step 5: Run the full deployment/topology verification matrix and commit**

Run every installer/workflow command required by `.rules` because workflow files changed, then:

```bash
git add backend-school/src/modules/system/handlers/migration.rs \
  .github/workflows/backend-school-neon-compatibility.yml \
  .github/workflows/deploy-backend-school.yml scripts/smoke_test.sh \
  frontend-school/tests/static/deployment-installer.test.mjs \
  docs/TESTING.md docs/OPERATIONS.md TODO.md
git commit -m "ops: gate gradebook result cutover"
```

### Task 15: Complete cross-stack verification and release review

**Files:**
- Review: all files changed by Tasks 1–14
- Remove after the implementation review is recorded: `docs/superpowers/specs/2026-09-05-gradebook-results-and-result-correction-design.md`
- Remove after the implementation review is recorded: `docs/superpowers/plans/2026-09-06-gradebook-results-and-result-correction.md`

**Interfaces:**
- Produces one clean Release 2 cutover commit sequence with no legacy runtime/compatibility boundary.

- [ ] **Step 1: Run focused backend database/service suites**

```bash
./scripts/test_backend_school.sh modules::academic::core::schema_tests::migration_060_creates_gradebook_result_boundaries -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh modules::academic::gradebook::services_tests -- --nocapture
./scripts/test_backend_school.sh modules::academic::learner_evaluation::services_tests -- --nocapture
./scripts/test_backend_school.sh modules::academic::results::services_tests -- --nocapture
```

- [ ] **Step 2: Run complete backend gates**

```bash
cd backend-school
cargo fmt --all -- --check
cargo test --test static_architecture
cargo test api_contract::tests --bin backend-school -- --nocapture
cargo check
```

- [ ] **Step 3: Run complete contract/frontend gates**

```bash
cd frontend-school
npm run generate:permissions
npm run check:permissions
npm run test:permissions
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:menu-sync
npm run test:static
node --experimental-strip-types --test tests/runtime/gradebook-ledger.test.ts \
  tests/runtime/gradebook-save-queue.test.ts tests/runtime/learner-evaluation-summary.test.ts
npx playwright test --list tests/e2e/gradebook-workflow.spec.ts \
  tests/e2e/academic-result-locking.spec.ts
```

- [ ] **Step 4: Run Svelte analysis and repository hygiene checks**

Run `npx @sveltejs/mcp svelte-autofixer <path> --svelte-version 5` for every changed `.svelte` file, then:

```bash
git diff --check
git status --short
git diff --stat
git diff
```

- [ ] **Step 5: Request code review and resolve every accepted finding**

Review against the approved spec, authorization boundaries, migration atomicity, absence of legacy compatibility, request counts, PII minimization, and mobile close/save safety. Re-run the affected focused tests after every fix.

- [ ] **Step 6: Record unavailable external gates accurately**

Actual Playwright execution, disposable Neon compatibility, deployment, and authenticated production smoke remain unrun unless their dedicated accounts, workflow authorization, target, and runtime secrets are present. Never report discovery or local generation as deployed success.
