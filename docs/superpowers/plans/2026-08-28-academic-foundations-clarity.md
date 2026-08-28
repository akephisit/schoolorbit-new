# Academic Foundations Setup Clarity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Academic Core and its immediate foundation registries planning-only, human-readable, and impossible to save with contradictory duplicate or raw-ID inputs.

**Architecture:** Typed Rust DTOs remove client ownership of derived year, term, bell-schedule, and homeroom identity fields while service helpers derive and validate authoritative values. The frontend uses a four-step Academic Core path plus list-first homeroom and student-year registries; existing catalog, curriculum, and delivery workspaces remain authoritative and receive regression guards rather than a rewrite.

**Tech Stack:** Rust, Axum, SQLx, PostgreSQL, utoipa/OpenAPI, SvelteKit 5, TypeScript, Tailwind CSS, shadcn-svelte, Node test runner, Playwright

**Spec:** `docs/superpowers/specs/2026-08-28-academic-foundations-clarity-design.md`

## Global Constraints

- Work directly on `main` as requested; do not create a worktree or use subagents.
- Run every command serially; do not overlap npm, Rust, database, or browser commands.
- Never edit an applied migration and add no migration unless a deterministic persisted-data repair is proven necessary.
- Never store, log, or add national IDs to a foundation response; student display contracts contain only name and school student code.
- Rust DTOs and utoipa own JSON; regenerate `contracts/openapi/school-api.json` and generated TypeScript types.
- Existing generated permission codes and read/manage boundaries remain unchanged.
- Official subject, activity, curriculum, study-program, and learning-group codes remain editable school data.
- UUIDs, row versions, bell-schedule codes, term codes, and English weekday codes are never ordinary editable content.
- Creating or selecting a planning year/term never activates, closes, or promotes anything.
- Use local shadcn-svelte primitives, SchoolOrbit semantic tokens, PageShell, PageState, PageSkeleton, and LoadingButton.
- Run the Svelte autofixer on every created or edited `.svelte` file before final verification.

---

### Task 1: Derive academic year, term, and bell-schedule identity in the backend

**Files:**
- Modify: `backend-school/src/modules/academic/core/models.rs`
- Modify: `backend-school/src/modules/academic/core/services.rs`
- Modify: `backend-school/src/modules/academic/core/services/years_terms.rs`
- Modify: `backend-school/src/modules/academic/core/services/bell_schedules.rs`
- Modify: `backend-school/src/modules/academic/core/services_tests.rs`

**Interfaces:**
- Consumes: existing `AcademicTermType`, planning-state checks, optimistic `row_version`, and academic audit events.
- Produces: `derive_academic_year_name(year, custom_name)`, `derive_term_identity(term_type, sequence, custom_name)`, server-owned schedule code/default selection, and replacement typed request DTOs used by Tasks 4 and 5.

- [ ] **Step 1: Write failing pure helper and serialization tests**

Add focused tests proving these concrete outcomes:

```rust
assert_eq!(
    years_terms::derive_academic_year_name(2571, None).unwrap(),
    "ปีการศึกษา 2571"
);
assert_eq!(
    years_terms::derive_academic_year_name(2571, Some("ปีแห่งการอ่าน")).unwrap(),
    "ปีแห่งการอ่าน"
);
assert!(years_terms::derive_academic_year_name(2571, Some("   ")).is_err());

let identity = years_terms::derive_term_identity(AcademicTermType::Regular, 2, None).unwrap();
assert_eq!(identity.code, "2");
assert_eq!(identity.name, "ภาคเรียนที่ 2");

let summer = years_terms::derive_term_identity(
    AcademicTermType::Summer,
    3,
    Some("ภาคฤดูร้อนเพิ่มเติม"),
).unwrap();
assert_eq!(summer.code, "SUMMER");
assert_eq!(summer.name, "ภาคฤดูร้อนเพิ่มเติม");
```

Also deserialize `CreateAcademicYearRequest`, `CreateAcademicTermRequest`, and `CreateBellScheduleRequest` JSON and assert that removed fields such as `name`, `sequence`, and `code` fail under `deny_unknown_fields`.

- [ ] **Step 2: Run the focused tests and confirm failure**

Run from `backend-school`:

```bash
cargo test academic_foundation_identity --lib -- --nocapture
```

Expected: FAIL because the derivation helpers and replacement DTO fields do not exist yet.

- [ ] **Step 3: Replace the request contracts**

Use these request shapes while keeping response models unchanged:

```rust
pub struct CreateAcademicYearRequest {
    pub year: i32,
    pub custom_name: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub school_days: Vec<String>,
}

pub struct UpdateAcademicYearRequest {
    pub year: i32,
    pub custom_name: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub school_days: Vec<String>,
    pub row_version: i64,
}

pub struct CreateAcademicTermRequest {
    pub academic_year_id: Uuid,
    pub term_type: AcademicTermType,
    pub custom_name: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub included_in_year_result: bool,
    pub blocks_year_closure: bool,
    pub bell_schedule_id: Uuid,
}

pub struct UpdateAcademicTermRequest {
    pub term_type: AcademicTermType,
    pub custom_name: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub included_in_year_result: bool,
    pub blocks_year_closure: bool,
    pub bell_schedule_id: Uuid,
    pub row_version: i64,
}

pub struct CreateBellScheduleRequest {
    pub academic_year_id: Uuid,
    pub name: String,
    pub owning_organization_unit_id: Option<Uuid>,
}

pub struct UpdateBellScheduleRequest {
    pub name: String,
    pub is_default: bool,
    pub owning_organization_unit_id: Option<Uuid>,
    pub row_version: i64,
}
```

- [ ] **Step 4: Implement minimal derivation and transactional allocation**

In `years_terms.rs`, derive standard year names, lock the planning year before calculating `MAX(sequence_no) + 1`, derive a stable term code/name, and retain existing `sequence_no`/`code` during update. In `bell_schedules.rs`, lock the year, allocate `DEFAULT` for the first schedule and `SCHEDULE-{n}` thereafter, make the first schedule default, and keep update from changing its code.

Normalize school days in canonical Monday-to-Sunday order before persistence. Preserve custom year/term names only when the explicit optional value is nonblank.

Remove the obsolete `validate_term_definitions(&[CreateAcademicTermRequest])` helper and its duplicate-code/sequence fixture tests; uniqueness is now owned by transactional allocation plus the existing database keys.

- [ ] **Step 5: Strengthen period validation with per-day overlap**

Change `validate_periods` to reject unknown/duplicate weekdays and to compare overlaps only where two active rows share an applicable day. Add a database context check that every period day belongs to the owning academic year's `school_days`. Inactive periods remain validated for shape but do not create active overlap conflicts.

- [ ] **Step 6: Run focused pure and database service tests**

Run serially:

```bash
cargo test academic_foundation_identity --lib -- --nocapture
```

```bash
../scripts/test_backend_school.sh cargo test modules::academic::core::services_tests::planning_year_and_term_updates_reject_stale_versions_and_unused_term_deletes --lib -- --nocapture
```

```bash
../scripts/test_backend_school.sh cargo test modules::academic::core::services_tests::academic_setup_workspace_matches_bounded_collections --lib -- --nocapture
```

Expected: PASS, including updated fixtures that use the replacement request shapes.

- [ ] **Step 7: Commit the backend academic setup invariant slice**

```bash
git add backend-school/src/modules/academic/core/models.rs backend-school/src/modules/academic/core/services.rs backend-school/src/modules/academic/core/services/years_terms.rs backend-school/src/modules/academic/core/services/bell_schedules.rs backend-school/src/modules/academic/core/services_tests.rs
git commit -m "feat(academic): derive foundation setup identities"
```

---

### Task 2: Derive homeroom identity and constrain advisor roles

**Files:**
- Modify: `backend-school/src/modules/academic/core/models.rs`
- Modify: `backend-school/src/modules/academic/core/services/student_years.rs`
- Modify: `backend-school/src/modules/academic/core/services_tests.rs`

**Interfaces:**
- Consumes: planning-year validation and grade-level `level_type`/`year` rows.
- Produces: `derive_homeroom_identity(level_type, grade_year, room_number, custom_name)` and create/update DTOs with no client-authored code/name, consumed by Task 6.

- [ ] **Step 1: Write failing homeroom identity tests**

```rust
let identity = student_years::derive_homeroom_identity("secondary", 1, "3", None).unwrap();
assert_eq!(identity.code, "M1-3");
assert_eq!(identity.name, "ม.1/3");

let custom = student_years::derive_homeroom_identity(
    "primary",
    2,
    "1",
    Some("ห้องส่งเสริมวิทยาศาสตร์"),
).unwrap();
assert_eq!(custom.code, "P2-1");
assert_eq!(custom.name, "ห้องส่งเสริมวิทยาศาสตร์");
```

Add cases for trimmed room numbers, blank room number/custom name, unsupported grade types, duplicate homeroom identity, stale row version, and advisor roles outside `primary | secondary`.

- [ ] **Step 2: Run the focused test and confirm failure**

```bash
cargo test academic_foundation_homeroom_identity --lib -- --nocapture
```

Expected: FAIL because the helper and new DTO ownership do not exist.

- [ ] **Step 3: Replace homeroom create/update request fields**

```rust
pub struct CreateHomeroomRequest {
    pub academic_year_id: Uuid,
    pub custom_name: Option<String>,
    pub grade_level_id: Uuid,
    pub room_number: String,
    pub study_program_id: Uuid,
    pub capacity: i32,
}

pub struct UpdateHomeroomRequest {
    pub custom_name: Option<String>,
    pub grade_level_id: Uuid,
    pub room_number: String,
    pub study_program_id: Uuid,
    pub capacity: i32,
    pub row_version: i64,
}
```

- [ ] **Step 4: Implement homeroom derivation in the service transaction**

Load `grade_levels.level_type/year` inside `validate_homeroom_context`, derive code and standard Thai short name, and persist those values. Preserve the database unique constraint on `(academic_year_id, grade_level_id, room_number)` and map it to a named Thai conflict. Do not rewrite existing rows outside a deliberate update.

- [ ] **Step 5: Run focused tests**

```bash
cargo test academic_foundation_homeroom_identity --lib -- --nocapture
```

```bash
../scripts/test_backend_school.sh cargo test modules::academic::core::services_tests::homeroom --lib -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit the homeroom invariant slice**

```bash
git add backend-school/src/modules/academic/core/models.rs backend-school/src/modules/academic/core/services/student_years.rs backend-school/src/modules/academic/core/services_tests.rs
git commit -m "feat(academic): derive homeroom identity"
```

---

### Task 3: Return readable student-year records and eligible create candidates

**Files:**
- Modify: `backend-school/src/modules/academic/core/models.rs`
- Modify: `backend-school/src/modules/academic/core/services/student_years.rs`
- Modify: `backend-school/src/modules/academic/core/handlers.rs`
- Modify: `backend-school/src/modules/academic/core.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `backend-school/src/modules/academic/core/services_tests.rs`

**Interfaces:**
- Consumes: `student_academic_years`, `users`, `student_info`, grade levels, study programs, generated `STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL` permission.
- Produces: enriched `StudentAcademicYear` fields and `GET /api/academic/student-years/candidates?academicYearId=...&search=...&limit=...` returning `StudentYearCandidate[]`.

- [ ] **Step 1: Write failing service/contract tests**

Assert list/get/create results contain:

```rust
assert!(!record.student_name.is_empty());
assert!(!record.grade_level_name.is_empty());
assert!(!record.study_program_name.is_empty());
assert_ne!(record.student_name, record.student_id.to_string());
```

Assert candidate search returns active student accounts that do not yet have a `student_academic_years` row for the target year, excludes existing rows, applies a bounded limit, and serializes only `id`, `studentCode`, and `name`.

- [ ] **Step 2: Run the focused test and confirm failure**

```bash
../scripts/test_backend_school.sh cargo test modules::academic::core::services_tests::student_year_read_models_are_human_readable --lib -- --nocapture
```

Expected: FAIL because display fields and candidate service are absent.

- [ ] **Step 3: Enrich the typed model without sensitive fields**

Add these response-only fields to `StudentAcademicYear`:

```rust
pub student_code: Option<String>,
pub student_name: String,
pub grade_level_name: String,
pub study_program_name: String,
```

Join authoritative tables in list/get queries. Do not select national ID, blind index, contact, guardian, medical, or document fields.

- [ ] **Step 4: Add typed candidate query, service, handler, route, and OpenAPI registration**

Use a camelCase `StudentYearCandidateQuery` with required `academic_year_id`, optional `search`, and optional `limit` clamped to `1..=100`. Require the existing manage permission and keep the general read endpoint unchanged.

- [ ] **Step 5: Run service, router, and static architecture tests**

Run serially:

```bash
../scripts/test_backend_school.sh cargo test modules::academic::core::services_tests::student_year_read_models_are_human_readable --lib -- --nocapture
```

```bash
cargo test --test static_architecture academic -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit the readable student-year contract slice**

```bash
git add backend-school/src/modules/academic/core/models.rs backend-school/src/modules/academic/core/services/student_years.rs backend-school/src/modules/academic/core/handlers.rs backend-school/src/modules/academic/core.rs backend-school/src/api_contract.rs backend-school/src/modules/academic/core/services_tests.rs
git commit -m "feat(academic): add readable student year options"
```

---

### Task 4: Regenerate contracts and add frontend regression tests first

**Files:**
- Modify generated: `contracts/openapi/school-api.json`
- Modify generated: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/academic-core.ts`
- Create: `frontend-school/src/lib/academic-core/foundation-presentation.ts`
- Create: `frontend-school/tests/runtime/academic-foundation-presentation.test.ts`
- Create: `frontend-school/tests/static/academic-foundation-clarity.test.mjs`
- Modify: `frontend-school/tests/e2e/academic-core-cutover.spec.ts`

**Interfaces:**
- Consumes: Tasks 1–3 Rust/OpenAPI DTOs.
- Produces: generated TypeScript request types, `listStudentYearCandidates`, pure presentation helpers, and failing UI expectations for Tasks 5–7.

- [ ] **Step 1: Generate the API artifacts**

Run from `frontend-school`:

```bash
npm run generate:api-contracts
```

Expected: tracked OpenAPI and generated TypeScript reflect `customName`, removed identity inputs, readable student-year fields, and the candidate endpoint.

- [ ] **Step 2: Update the typed API wrapper**

Export `StudentYearCandidate` and implement `listStudentYearCandidates(academicYearId, search, options)` using the generated operation query type and camelCase parameters. Keep concrete envelopes and `requiredContextValue`; do not cast responses.

- [ ] **Step 3: Write pure presentation tests**

Test exact Thai previews and override detection:

```ts
assert.equal(standardAcademicYearName(2571), 'ปีการศึกษา 2571');
assert.equal(standardTermName('regular', 2), 'ภาคเรียนที่ 2');
assert.equal(standardTermName('summer', 3), 'ภาคฤดูร้อน');
assert.equal(customNameFromStored('ปีการศึกษา 2571', 'ปีการศึกษา 2571'), '');
assert.equal(customNameFromStored('ปีแห่งการอ่าน', 'ปีการศึกษา 2571'), 'ปีแห่งการอ่าน');
assert.deepEqual(normalizeSchoolDays(['FRI', 'MON', 'MON']), ['MON', 'FRI']);
```

- [ ] **Step 4: Write static tests for the intended Svelte structure**

Require the Academic Core orchestrator and four focused step components; shadcn Checkbox/Collapsible/Dialog/Table usage; no editable `DEFAULT`, `MON,TUE`, term sequence/code, homeroom code/name, advisor-role Input, or raw UUID fallback. Require lazy candidate loading only after the create-student dialog opens.

- [ ] **Step 5: Update Playwright mocks and assertions to the new request shapes**

Assert the planning year POST contains `year`, `customName`, dates, and selected school days but no `name`; assert term/schedule POSTs contain no `code` or `sequence`; assert the page has no activation/closure/promotion action.

- [ ] **Step 6: Run frontend tests and confirm the new UI tests fail**

Run from `frontend-school` serially:

```bash
node --experimental-strip-types --test tests/runtime/academic-foundation-presentation.test.ts
```

```bash
node --test tests/static/academic-foundation-clarity.test.mjs
```

Expected: presentation helpers may pass after minimal helper implementation; static UI expectations FAIL until Tasks 5–7.

- [ ] **Step 7: Commit contracts and red tests**

```bash
git add contracts/openapi/school-api.json frontend-school/src/lib/api/generated/school-api.ts frontend-school/src/lib/api/academic-core.ts frontend-school/src/lib/academic-core/foundation-presentation.ts frontend-school/tests/runtime/academic-foundation-presentation.test.ts frontend-school/tests/static/academic-foundation-clarity.test.mjs frontend-school/tests/e2e/academic-core-cutover.spec.ts
git commit -m "test(academic): define foundation clarity contracts"
```

---

### Task 5: Build the four-step Academic Core planning path

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/core/+page.svelte`
- Rewrite: `frontend-school/src/lib/components/academic-core/AcademicYearTermEditor.svelte`
- Create: `frontend-school/src/lib/components/academic-core/setup/AcademicYearSetupStep.svelte`
- Create: `frontend-school/src/lib/components/academic-core/setup/BellScheduleSetupStep.svelte`
- Create: `frontend-school/src/lib/components/academic-core/setup/BellSchedulePeriodsStep.svelte`
- Create: `frontend-school/src/lib/components/academic-core/setup/AcademicTermSetupStep.svelte`
- Test: `frontend-school/tests/static/academic-foundation-clarity.test.mjs`
- Test: `frontend-school/tests/e2e/academic-core-cutover.spec.ts`

**Interfaces:**
- Consumes: generated Task 4 request DTOs and presentation helpers.
- Produces: one orchestrated planning-year selection and four focused saved-record steps.

- [ ] **Step 1: Adapt page mutation handlers to the generated contracts**

Remove hard-coded `schoolDays: ['MON', ...]`. Pass selected weekdays and `customName`. Schedule creation passes only year/name/owner. Term creation passes type/custom name/dates/flags/schedule while sequence/code/name come from the response.

- [ ] **Step 2: Implement `AcademicYearSetupStep.svelte`**

Use number input, shadcn DatePicker, weekday Checkbox controls, a live standard-name preview, and a Collapsible advanced custom-name override. Preserve an existing custom label when editing; changing the numeric year must not erase it silently.

- [ ] **Step 3: Implement `BellScheduleSetupStep.svelte`**

Inherit the selected year, ask only for the readable name, show which schedule is default, and expose `ตั้งเป็นตารางหลัก` as an explicit update action. Do not render the internal code as an editable or primary identifier.

- [ ] **Step 4: Implement `BellSchedulePeriodsStep.svelte`**

Render dense period rows with order, optional name, time inputs, `ใช้ทุกวันเรียน`, expanded weekday checkboxes, and active Checkbox/Switch. Serialize arrays of canonical weekday codes; never use comma-splitting user input.

- [ ] **Step 5: Implement `AcademicTermSetupStep.svelte`**

Use preset cards/select for regular, summer, remedial, and custom; preview the derived name; place custom label and the two annual-result/closure choices in Collapsible advanced settings with plain Thai help. Do not render code or numeric sequence inputs.

- [ ] **Step 6: Rewrite the orchestrator and summaries**

Keep existing years readable. Maintain an in-page selected planning year. Open the first incomplete step, collapse completed steps to summaries, allow edit, and show `ฉบับเตรียมการ`. Add explicit copy that activation, closure, and promotion are separate workflows. Read-only users see summaries without management forms.

- [ ] **Step 7: Run Svelte autofixer on each edited/created component**

Run one command per file from `frontend-school`, for example:

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic-core/setup/AcademicYearSetupStep.svelte --svelte-version 5
```

Repeat serially for the other three steps, `AcademicYearTermEditor.svelte`, and the core `+page.svelte`; apply every actionable issue.

- [ ] **Step 8: Run focused static and Playwright tests**

```bash
node --test tests/static/academic-foundation-clarity.test.mjs
```

```bash
npx playwright test tests/e2e/academic-core-cutover.spec.ts --grep "planning year|term count"
```

Expected: PASS.

- [ ] **Step 9: Commit the guided setup UI**

```bash
git add frontend-school/src/routes/'(app)'/staff/academic/core/+page.svelte frontend-school/src/lib/components/academic-core/AcademicYearTermEditor.svelte frontend-school/src/lib/components/academic-core/setup frontend-school/tests/static/academic-foundation-clarity.test.mjs frontend-school/tests/e2e/academic-core-cutover.spec.ts
git commit -m "feat(frontend): guide academic foundation setup"
```

---

### Task 6: Make homerooms and student years list-first and human-readable

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/homerooms/+page.svelte`
- Rewrite: `frontend-school/src/lib/components/academic-core/HomeroomEditor.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/student-years/+page.svelte`
- Modify: `frontend-school/src/lib/components/academic-core/StudentYearPlacementEditor.svelte`
- Modify: `frontend-school/src/lib/components/academic-core/StudentYearTransferDialog.svelte`
- Modify: `frontend-school/src/lib/workspaces/academic-batch.ts`
- Modify: `frontend-school/tests/runtime/academic-batch-loader.test.ts`
- Test: `frontend-school/tests/static/academic-foundation-clarity.test.mjs`

**Interfaces:**
- Consumes: derived homeroom DTOs, readable `StudentAcademicYear`, and lazy `listStudentYearCandidates`.
- Produces: responsive registry tables/cards and shadcn dialogs with no raw identity fallback.

- [ ] **Step 1: Add failing batch/lazy-load tests**

Prove that read-only homeroom collection loading does not request staff management options, and that student candidates are not requested during page mount. Candidate search starts only after the authorized create dialog opens.

- [ ] **Step 2: Rewrite `HomeroomEditor.svelte` list-first**

Use Table on desktop and cards on mobile. Show standard code as read-only supporting text, readable room name, grade, program, capacity, advisor names/roles, and status. Create/edit uses shadcn Dialog with grade, room number, program, capacity, and advanced custom display name; do not ask for code or standard name.

Advisor management uses named staff selection and a shadcn Select with exactly:

```text
primary   -> ครูที่ปรึกษาหลัก
secondary -> ครูที่ปรึกษาร่วม
```

Load staff options only when an authorized user opens advisor management.

- [ ] **Step 3: Patch homeroom page state after create/update/advisor mutations**

Use returned typed resources and advisor arrays. Add `updateHomeroom`; avoid broad workspace reloads. Preserve stale conflict messages from `academicData`.

- [ ] **Step 4: Convert the student-year page to a registry table plus dialogs**

Display `studentCode`, `studentName`, `gradeLevelName`, `studyProgramName`, current named homeroom, class number, and status. Open a detail/placement dialog for the selected row instead of rendering one large editor per row. The create dialog searches `StudentYearCandidate` lazily and uses grade/program named options.

- [ ] **Step 5: Remove all UUID fallbacks from placement and transfer UI**

When a homeroom relationship cannot resolve, render `ไม่พบห้องประจำชั้นที่อ้างอิง` with destructive/integrity styling and a reload action. Never render `studentId`, `gradeLevelId`, `studyProgramId`, or `homeroomId` as user-facing replacement text.

- [ ] **Step 6: Replace the custom transfer overlay with shadcn Dialog**

Keep DatePicker and Select controls, focus behavior, cancel/submit states, and current idempotency behavior. Do not change transfer service semantics.

- [ ] **Step 7: Run Svelte autofixer serially**

Run these separately and apply actionable findings:

```bash
npx @sveltejs/mcp svelte-autofixer src/routes/'(app)'/staff/academic/homerooms/+page.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic-core/HomeroomEditor.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer src/routes/'(app)'/staff/academic/student-years/+page.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic-core/StudentYearPlacementEditor.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic-core/StudentYearTransferDialog.svelte --svelte-version 5
```

- [ ] **Step 8: Run focused runtime and static tests**

```bash
node --experimental-strip-types --test tests/runtime/academic-batch-loader.test.ts
```

```bash
node --test tests/static/academic-foundation-clarity.test.mjs
```

Expected: PASS.

- [ ] **Step 9: Commit the foundation registries**

```bash
git add frontend-school/src/routes/'(app)'/staff/academic/homerooms/+page.svelte frontend-school/src/lib/components/academic-core/HomeroomEditor.svelte frontend-school/src/routes/'(app)'/staff/academic/student-years/+page.svelte frontend-school/src/lib/components/academic-core/StudentYearPlacementEditor.svelte frontend-school/src/lib/components/academic-core/StudentYearTransferDialog.svelte frontend-school/src/lib/workspaces/academic-batch.ts frontend-school/tests/runtime/academic-batch-loader.test.ts frontend-school/tests/static/academic-foundation-clarity.test.mjs
git commit -m "feat(frontend): clarify homeroom and student year registries"
```

---

### Task 7: Remove dead raw-ID editors and guard the already-modern foundation pages

**Files:**
- Delete: `frontend-school/src/lib/components/learning-delivery/LearningOfferingEditor.svelte`
- Delete: `frontend-school/src/lib/components/learning-delivery/CurriculumOfferingPreview.svelte`
- Modify: `frontend-school/tests/static/academic-core-cutover-contract.test.mjs`
- Modify: `frontend-school/tests/static/academic-page-prerequisites.test.mjs`
- Modify: `frontend-school/tests/static/academic-catalog-ui.test.mjs`
- Modify: `frontend-school/tests/static/academic-curriculum-workspace.test.mjs`
- Modify: `frontend-school/tests/static/learning-delivery-workspace.test.mjs`
- Modify: `frontend-school/tests/static/academic-foundation-clarity.test.mjs`

**Interfaces:**
- Consumes: direct import/re-export/call-site audit and the active `OfferingCreateDialog` / `OfferingCurriculumPreview` implementations.
- Produces: one supported delivery UI and durable guards for catalog, curriculum, homeroom, student-year, and delivery field ownership.

- [ ] **Step 1: Re-run the call-site audit before deletion**

```bash
rg -n "LearningOfferingEditor|CurriculumOfferingPreview" frontend-school/src frontend-school/tests
```

Expected: only the two component definitions, static file-presence expectations, generated API type names, and the active `OfferingCurriculumPreview` type import. Confirm that the active type name is not the deleted component.

- [ ] **Step 2: Delete the two confirmed dead components**

Remove static expectations that required these legacy files. Do not delete the generated `CurriculumOfferingPreview` DTO or active `OfferingCurriculumPreview.svelte`.

- [ ] **Step 3: Strengthen the foundation ownership guards**

Scan only active foundation forms and assert:

- catalog/curriculum official codes remain present;
- no input label asks for UUID/internal IDs;
- no `?? placement.homeroomId`, `?? studentYear.studentId`, `?? studentYear.gradeLevelId`, or `?? studentYear.studyProgramId` human-label fallback;
- no editable `DEFAULT`, comma-separated weekday codes, term sequence/code, homeroom code, or unconstrained advisor role;
- catalog/curriculum/delivery continue using their existing typed human-readable controls and lazy management options.

- [ ] **Step 4: Run the affected static tests**

```bash
node --test tests/static/academic-core-cutover-contract.test.mjs tests/static/academic-page-prerequisites.test.mjs tests/static/academic-catalog-ui.test.mjs tests/static/academic-curriculum-workspace.test.mjs tests/static/learning-delivery-workspace.test.mjs tests/static/academic-foundation-clarity.test.mjs
```

Expected: PASS.

- [ ] **Step 5: Commit legacy removal and regression guards**

```bash
git add -A frontend-school/src/lib/components/learning-delivery/LearningOfferingEditor.svelte frontend-school/src/lib/components/learning-delivery/CurriculumOfferingPreview.svelte frontend-school/tests/static
git commit -m "refactor(frontend): remove raw id academic editors"
```

---

### Task 8: Verify serially, inspect data requirements, and publish main

**Files:**
- Modify if required by generated formatting only: files already owned by Tasks 1–7
- Inspect: `backend-school/migrations/001_baseline.sql` through the latest migration
- Inspect: `git diff`, `git status`, and deployment workflow results

**Interfaces:**
- Consumes: all previous task commits.
- Produces: verified main branch and deployable artifacts; no migration unless the deterministic audit proves one is necessary.

- [ ] **Step 1: Run the deterministic persisted-data audit against disposable fixture data**

Use focused SQLx tests to check standard year names, weekday membership, term identities, homeroom identities, and cross-year references. Because existing custom names are not marked separately, do not add an automatic data rewrite based only on inequality. Record ambiguous values as preserved behavior in test assertions.

If and only if a recognized old generated pattern is provably inconsistent, add the next sequential migration after `047`; test it through the disposable migration gate. Otherwise add no migration.

- [ ] **Step 2: Format and verify backend-school**

Run from `backend-school`, one command at a time:

```bash
cargo fmt --all -- --check
```

```bash
cargo test --test static_architecture
```

```bash
cargo check
```

```bash
../scripts/test_backend_school.sh cargo test modules::academic::core --lib -- --nocapture
```

- [ ] **Step 3: Verify generated API contracts**

Run from `frontend-school`, one at a time:

```bash
npm run check:api-contracts
```

```bash
npm run test:api-contracts
```

- [ ] **Step 4: Verify frontend-school**

Run from `frontend-school`, one at a time:

```bash
npm run lint
```

```bash
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

```bash
npm run test:static
```

```bash
node --experimental-strip-types --test tests/runtime/academic-foundation-presentation.test.ts tests/runtime/academic-batch-loader.test.ts
```

- [ ] **Step 5: Run focused browser coverage**

```bash
npx playwright test tests/e2e/academic-core-cutover.spec.ts
```

If deployed sandbox credentials are available through existing `E2E_*` environment variables, run the production-compatible smoke subset against `sandbox.schoolorbit.app`. Never write credentials into the plan, Git, output, or fixtures.

- [ ] **Step 6: Review the final diff and worktree**

Run from repository root:

```bash
git diff --check
```

```bash
git log --oneline --decorate origin/main..HEAD
```

```bash
git status --short --branch
```

Review every changed file for scope, generated-artifact ownership, permissions, PDPA, no migration edits, no raw-ID fallback, and no lifecycle action.

- [ ] **Step 7: Commit any final mechanical fixes**

```bash
git add -u
git commit -m "fix(academic): finalize foundation clarity verification"
```

Skip this commit when there are no final fixes.

- [ ] **Step 8: Push main and monitor the existing automatic deployment**

```bash
git push origin main
```

Inspect the repository's existing deployment workflow serially. Confirm backend/API-contract/frontend jobs and sandbox deployment succeed before reporting readiness for user testing. Do not manually mutate production data or activate a year/term.
