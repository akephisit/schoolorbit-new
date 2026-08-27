# Learning Delivery Workspace Release 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a term-scoped, human-readable workspace for offerings, learning groups, teachers, rooms, homerooms, and rosters, then connect dependent academic pages through local guidance and deep links.

**Architecture:** Add a bounded read-only delivery overview and a separate lazy management-options contract behind Learning Offering manage access. Keep existing mutations authoritative, enrich roster preview with minimal learner display data without changing its source hash, and split the Svelte workflow into an overview route and a reloadable offering detail route.

**Tech Stack:** Rust, Axum, SQLx, utoipa/OpenAPI, SvelteKit 5, TypeScript, shadcn-svelte, Node static tests, Playwright discovery

**Spec:** `docs/superpowers/specs/2026-08-27-academic-work-organization-and-guided-workflows-design.md`

## Global Constraints

- Release 1 and Release 2 must already be merged and deployed or present in the execution branch.
- Run every command serially; do not start concurrent Cargo, npm, Docker, or test processes.
- Do not change or collapse offering, learning-group, homeroom, student-year, or roster table ownership.
- Do not add automatic learning-group generation, automatic timetable solving, Gradebook, term closure, year closure, or promotion.
- Use `รายการเปิดสอน` for a term offering and `กลุ่มเรียน` for an actual cohort throughout Thai UI copy.
- UUIDs remain internal identifiers and never appear as editable academic text or a student display fallback.
- Read users load only overview/detail read models; management options load only after an exact manage capability and user action.
- Roster display responses contain only student code, display name, grade label, homeroom label, state, and conflict explanation—never national ID, blind index, contact, guardian, medical, or document data.
- Additive JSON contracts originate in Rust DTOs/OpenAPI and are consumed through generated TypeScript operations.
- Overview, options, and roster enrichment use bounded batch queries and never query once per offering, group, or learner.
- Every affected downstream page checks only its own direct prerequisites and remains readable when another module is empty.

---

### Task 1: Add a bounded Learning Delivery overview service

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Create: `backend-school/src/modules/academic/delivery/services/workspaces.rs`
- Modify: `backend-school/src/modules/academic/delivery/services.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`

**Interfaces:**
- Consumes: `LearningOfferingQuery`, `AcademicResourceListFilter`, offering targets, groups, teachers, rosters, grade levels, and study programs.
- Produces: `workspaces::delivery_overview(pool, academic_term_id, filter) -> Result<LearningDeliveryOverview, AppError>`.

- [ ] **Step 1: Write the failing overview service test**

Create course/activity offerings with multiple groups, teacher assignments, and mixed roster states. Assert resolved labels and aggregate coverage:

```rust
let overview = workspaces::delivery_overview(
    &pool,
    context.term_id,
    &AcademicResourceListFilter {
        includes_school_owned: true,
        ..Default::default()
    },
)
.await
.expect("delivery overview should load");

let course = overview
    .offerings
    .iter()
    .find(|item| item.offering.code_snapshot == "ค21101")
    .expect("course summary");
assert_eq!(course.grade_levels[0].name, "มัธยมศึกษาปีที่ 1");
assert_eq!(course.study_programs[0].name, "แผนการเรียนทั่วไป");
assert_eq!(course.group_count, 2);
assert_eq!(course.teacher_assignment_count, 2);
assert_eq!(course.groups_without_primary_teacher, 1);
assert_eq!(course.published_roster_count, 1);
```

Add an organization-filter assertion that an offering outside the caller's union scope is absent.

- [ ] **Step 2: Run the focused test and verify failure**

```bash
./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::delivery_overview_batches_labels_and_group_coverage \
  -- --exact --nocapture --test-threads=1
```

Expected: FAIL because the workspace service and DTOs do not exist.

- [ ] **Step 3: Implement overview DTOs and fixed-count queries**

Add:

```rust
#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LearningOfferingOverviewItem {
    pub offering: LearningOffering,
    pub grade_levels: Vec<GradeLevelLookupItem>,
    pub study_programs: Vec<StudyProgramOption>,
    pub group_count: i64,
    pub teacher_assignment_count: i64,
    pub groups_without_primary_teacher: i64,
    pub published_roster_count: i64,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LearningDeliveryOverview {
    pub academic_term_id: Uuid,
    pub offerings: Vec<LearningOfferingOverviewItem>,
}
```

Implement one scoped offering query plus bounded batch queries for target labels and group/teacher/roster aggregates. Keep the existing 500-offering and group workspace limits explicit; reject oversize input with `ValidationError` instead of truncating coverage silently.

- [ ] **Step 4: Run focused delivery tests**

```bash
./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::delivery_overview_batches_labels_and_group_coverage \
  -- --exact --nocapture --test-threads=1
```

```bash
./scripts/test_backend_school.sh \
  modules::academic::delivery::services::workspaces::tests \
  -- --nocapture --test-threads=1
```

Expected: both commands PASS.

- [ ] **Step 5: Commit the overview service**

```bash
git add backend-school/src/modules/academic/delivery/models.rs \
  backend-school/src/modules/academic/delivery/services/workspaces.rs \
  backend-school/src/modules/academic/delivery/services.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs
git commit -m "feat(academic): add delivery overview service"
```

### Task 2: Add scoped management options and minimal roster display data

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/workspaces.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/groups.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`

**Interfaces:**
- Consumes: selected term/year, Learning Offering manage filter, existing lookup service types, published catalog versions, and structural roster candidates.
- Produces: `delivery_management_options(...) -> DeliveryManagementOptions` and enriched `RosterPreviewStudent`.

- [ ] **Step 1: Write failing management-option and roster-minimization tests**

Add a management-options test that asserts readable, scoped option sets and an enrichment test that serializes a roster student:

```rust
let options = workspaces::delivery_management_options(
    &pool,
    context.term_id,
    context.teacher_id,
    &manage_filter,
)
.await
.expect("options should load");
assert!(options.catalog_versions.iter().any(|item| item.label.contains("ค21101")));
assert!(options.homerooms.iter().all(|item| item.grade_level.is_some()));
assert!(options.teachers.iter().any(|item| item.name.contains("สมชาย")));
assert!(options.rooms.iter().any(|item| item.name == "ห้อง 312"));

let student_json = serde_json::to_value(&preview.students[0]).expect("serialize");
assert_eq!(student_json["studentCode"], "12345");
assert_eq!(student_json["displayName"], "เด็กชาย ทดสอบ ระบบ");
assert_eq!(student_json["gradeLevelName"], "ม.1");
assert_eq!(student_json["homeroomName"], "ม.1/1");
for forbidden in ["nationalId", "nationalIdHash", "phone", "email", "guardian"] {
    assert!(student_json.get(forbidden).is_none());
}
```

Also assert that changing a student's display name does not change `source_hash` because roster concurrency is based on structural IDs and placements.

- [ ] **Step 2: Run focused tests and verify failure**

```bash
./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::delivery_management_options_are_scoped_and_human_readable \
  -- --exact --nocapture --test-threads=1
```

```bash
./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::roster_preview_exposes_minimal_display_data_without_hashing_names \
  -- --exact --nocapture --test-threads=1
```

Expected: both FAIL because the types and enrichment query do not exist.

- [ ] **Step 3: Implement option DTOs and batch roster enrichment**

Add:

```rust
#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryCatalogVersionOption {
    pub id: Uuid,
    pub kind: LearningOfferingKind,
    pub code: String,
    pub name: String,
    pub version_no: i32,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryManagementOptions {
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub catalog_versions: Vec<DeliveryCatalogVersionOption>,
    pub grade_levels: Vec<GradeLevelLookupItem>,
    pub study_programs: Vec<StudyProgramOption>,
    pub organization_units: Vec<OrganizationUnitLookupItem>,
    pub homerooms: Vec<HomeroomLookupItem>,
    pub teachers: Vec<StaffLookupItem>,
    pub rooms: Vec<Room>,
}
```

Use existing lookup service functions in the delivery workspace service, then filter organization choices through `learning_offering_owner_allowed`. Do not broaden resource scope merely because generic lookups contain more rows.

Extend roster DTO:

```rust
pub struct RosterPreviewStudent {
    pub student_academic_year_id: Uuid,
    pub student_id: Uuid,
    pub student_code: Option<String>,
    pub display_name: String,
    pub grade_level_name: String,
    pub homeroom_name: Option<String>,
    pub proposed_active: bool,
    pub currently_active: bool,
    pub conflict_reason: Option<String>,
}
```

Keep `RosterSourceStudent` ID-only and calculate `source_hash` before enrichment. After structural preview creation, issue one query for every preview student-year ID joining `users`, `student_info`, `grade_levels`, current/planned `homeroom_placements`, and `homerooms`; merge by student-year ID and fail closed if a display row is missing.

- [ ] **Step 4: Run focused options, roster, and PII architecture tests**

```bash
./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::delivery_management_options_are_scoped_and_human_readable \
  -- --exact --nocapture --test-threads=1
```

```bash
./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::roster_preview_exposes_minimal_display_data_without_hashing_names \
  -- --exact --nocapture --test-threads=1
```

```bash
cd backend-school
cargo test --test static_architecture -- --nocapture
```

Expected: all commands PASS and no sensitive student field is introduced.

- [ ] **Step 5: Commit options and roster display data**

```bash
git add backend-school/src/modules/academic/delivery/models.rs \
  backend-school/src/modules/academic/delivery/services/workspaces.rs \
  backend-school/src/modules/academic/delivery/services/groups.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs
git commit -m "feat(academic): resolve delivery management labels"
```

### Task 3: Expose and generate Learning Delivery workspace contracts

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/handlers.rs`
- Modify: `backend-school/src/app.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `contracts/openapi/school-api.json` (generated)
- Modify: `frontend-school/src/lib/api/generated/school-api.ts` (generated)
- Modify: `frontend-school/src/lib/api/learning-delivery.ts`
- Create: `frontend-school/tests/static/learning-delivery-workspace.test.mjs`

**Interfaces:**
- Consumes: Task 1 and Task 2 services.
- Produces: `GET /api/academic/delivery/workspace?academicTermId=...` and `GET /api/academic/delivery/management-options?academicTermId=...`, plus typed frontend wrappers and existing detail GET wrappers.

- [ ] **Step 1: Write the failing API/static test**

```js
test('delivery workspace uses generated term query contracts', async () => {
  const api = await readProjectFile('src/lib/api/learning-delivery.ts');
  assert.match(api, /operations\['getLearningDeliveryOverview'\]/);
  assert.match(api, /operations\['getLearningDeliveryManagementOptions'\]/);
  assert.match(api, /getLearningDeliveryOverview/);
  assert.match(api, /getLearningDeliveryManagementOptions/);
  assert.match(api, /getLearningOffering/);
  assert.match(api, /getLearningGroup/);
  assert.doesNotMatch(api, /academic_term_id|ApiResponse<unknown>|Record<string, unknown>/);
});
```

- [ ] **Step 2: Run it and verify failure**

```bash
cd frontend-school
node --test tests/static/learning-delivery-workspace.test.mjs
```

Expected: FAIL because workspace endpoints and detail wrappers are absent.

- [ ] **Step 3: Add handlers, OpenAPI registration, and concrete wrappers**

The overview handler requires `OfferingAction::Read`; management options require `OfferingAction::Manage`. Both accept the existing camel-case `LearningOfferingQuery`. Register static `/api/academic/delivery/...` routes before parameterized offering routes.

Add wrappers:

```ts
type DeliveryWorkspaceQuery = NonNullable<
  operations['getLearningDeliveryOverview']['parameters']['query']
>;

export const getLearningDeliveryOverview = (
  academicTermId: string,
  options: ApiRequestOptions = {}
) => {
  const query = { academicTermId: selectedTerm(academicTermId) } satisfies DeliveryWorkspaceQuery;
  return deliveryData(
    apiClient.get<LearningDeliveryOverview>('/api/academic/delivery/workspace', { ...options, query }),
    'ไม่สามารถโหลดภาพรวมรายการเปิดสอนได้'
  );
};
```

Add the equivalent management-options wrapper and concrete `getLearningOffering(id)` / `getLearningGroup(id)` wrappers. Regenerate:

```bash
npm run generate:api-contracts
```

- [ ] **Step 4: Run backend API and frontend contract tests**

```bash
cd ../backend-school
cargo test api_contract::tests -- --nocapture
```

```bash
cd ../frontend-school
npm run check:api-contracts
```

```bash
npm run test:api-contracts
```

```bash
node --test tests/static/learning-delivery-workspace.test.mjs \
  tests/static/api-query-contract.test.mjs
```

Expected: every command PASS.

- [ ] **Step 5: Commit generated delivery contracts**

```bash
git add backend-school/src/modules/academic/delivery/handlers.rs backend-school/src/app.rs \
  backend-school/src/api_contract.rs contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  frontend-school/src/lib/api/learning-delivery.ts \
  frontend-school/tests/static/learning-delivery-workspace.test.mjs
git commit -m "feat(academic): expose delivery workspaces"
```

### Task 4: Build the term-scoped offering overview

**Files:**
- Create: `frontend-school/src/lib/components/learning-delivery/OfferingOverviewTable.svelte`
- Create: `frontend-school/src/lib/components/learning-delivery/OfferingCreateDialog.svelte`
- Create: `frontend-school/src/lib/components/learning-delivery/OfferingCurriculumPreview.svelte`
- Replace: `frontend-school/src/routes/(app)/staff/academic/delivery/+page.svelte`
- Modify: `frontend-school/tests/static/learning-delivery-workspace.test.mjs`

**Interfaces:**
- Consumes: overview/options contracts, existing preview/apply/create mutations, selected academic context, and Release 1 prerequisite notice.
- Produces: readable course/activity offering overview and two explicit create flows.

- [ ] **Step 1: Extend the failing overview UI test**

```js
assert.match(page, /getLearningDeliveryOverview/);
assert.match(page, /academicTermId/);
assert.match(page, /kind=activity|kindFilter/);
assert.doesNotMatch(page, /getLearningDeliveryManagementOptions\([\s\S]*onMount/);
assert.doesNotMatch(page, /catalogVersionId[^\n]*<Input/);
assert.doesNotMatch(page, /gradeLevelId[^\n]*<Input/);
assert.match(table, /groupsWithoutPrimaryTeacher/);
assert.match(table, /publishedRosterCount/);
```

- [ ] **Step 2: Run the UI test and verify failure**

```bash
cd frontend-school
node --test tests/static/learning-delivery-workspace.test.mjs
```

Expected: FAIL against the current one-page raw-ID delivery editor.

- [ ] **Step 3: Implement overview, filters, and lazy create flows**

Load overview only after a selected term exists and the read capability passes. Derive filters for search, course/activity, status, grade, and study program locally from the one overview response. Read `kind=activity` from the URL as an initial filter without duplicating the route.

`OfferingCreateDialog` loads management options only when opened and only for a manage-capable user. Present two actions: `นำมาจากหลักสูตร` and `เพิ่มรายการเปิดสอนเอง`. Use searchable selectors for catalog versions, targets, programs, and owner units. Preserve existing preview source hash and idempotency behavior.

Rows link to `/staff/academic/delivery/{offeringId}` and show code/name snapshot, kind, resolved targets, status, group count, teacher coverage, and roster coverage. Update local overview state after mutations instead of reloading unrelated modules.

- [ ] **Step 4: Analyze Svelte and run focused request-count tests**

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/learning-delivery/OfferingOverviewTable.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/learning-delivery/OfferingCreateDialog.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/learning-delivery/OfferingCurriculumPreview.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/delivery/+page.svelte' --svelte-version 5
```

Expected: no unresolved analyzer issue.

```bash
node --test tests/static/learning-delivery-workspace.test.mjs \
  tests/static/academic-workspace-request-count.test.mjs
```

Expected: PASS; initial load is one overview request and never fans out per row.

- [ ] **Step 5: Commit the offering overview**

```bash
git add frontend-school/src/lib/components/learning-delivery \
  'frontend-school/src/routes/(app)/staff/academic/delivery/+page.svelte' \
  frontend-school/tests/static/learning-delivery-workspace.test.mjs
git commit -m "feat(academic): redesign offering overview"
```

### Task 5: Build the offering, group, and roster detail workspace

**Files:**
- Create: `frontend-school/src/routes/(app)/staff/academic/delivery/[offeringId]/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte`
- Create: `frontend-school/src/lib/components/learning-delivery/LearningGroupList.svelte`
- Create: `frontend-school/src/lib/components/learning-delivery/LearningGroupEditor.svelte`
- Create: `frontend-school/src/lib/components/learning-delivery/RosterPreviewPanel.svelte`
- Modify: `frontend-school/tests/static/learning-delivery-workspace.test.mjs`

**Interfaces:**
- Consumes: concrete offering/group reads, lazy management options, existing group/teacher/homeroom/roster mutations, and enriched roster preview.
- Produces: reloadable offering detail with named group assignments and minimal roster display.

- [ ] **Step 1: Extend the failing detail-route test**

```js
assert.match(meta, /access:/);
assert.doesNotMatch(meta, /menu:/);
assert.match(page, /getLearningOffering/);
assert.match(page, /listLearningGroups/);
assert.match(editor, /managementOptions\.teachers/);
assert.match(editor, /managementOptions\.homerooms/);
assert.match(editor, /managementOptions\.rooms/);
assert.match(roster, /studentCode/);
assert.match(roster, /displayName/);
assert.match(roster, /gradeLevelName/);
assert.match(roster, /homeroomName/);
assert.doesNotMatch(roster, /student\.studentId\s*\}/);
```

- [ ] **Step 2: Run the detail test and verify failure**

```bash
cd frontend-school
node --test tests/static/learning-delivery-workspace.test.mjs
```

Expected: FAIL because the detail route and named editors do not exist.

- [ ] **Step 3: Implement detail state and human-readable editors**

The detail route uses `_meta.access` with `PERMISSION_MODULES.LEARNING_OFFERING` and no menu item. Load offering plus its groups for readers. Load management options only on an explicit edit/create/roster action and only after a manage capability passes.

Use group deep selection in `?groupId=` so history and reload preserve the selected group. `LearningGroupEditor` uses searchable selections and displays:

```ts
interface LearningGroupEditorProps {
  group: LearningGroup;
  managementOptions: DeliveryManagementOptions;
  onSaveGroup: (request: UpdateLearningGroupRequest) => Promise<void>;
  onReplaceTeachers: (request: ReplaceLearningGroupTeachersRequest) => Promise<void>;
  onReplaceHomerooms: (request: ReplaceLearningGroupHomeroomsRequest) => Promise<void>;
}
```

`RosterPreviewPanel` shows student code, display name, grade, homeroom, proposed/current state, and conflict reason. It uses `studentAcademicYearId` only as the mutation identifier and never renders it. On apply/publish `409`, keep the previous preview visible, mark it stale, and require a new preview.

- [ ] **Step 4: Analyze every detail Svelte file and run focused tests**

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte' --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/learning-delivery/LearningGroupList.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/learning-delivery/LearningGroupEditor.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/learning-delivery/RosterPreviewPanel.svelte --svelte-version 5
```

Expected: no unresolved analyzer issue.

```bash
node --test tests/static/learning-delivery-workspace.test.mjs \
  tests/static/route-preview-meta.test.mjs
```

Expected: PASS.

- [ ] **Step 5: Commit the offering detail workspace**

```bash
git add 'frontend-school/src/routes/(app)/staff/academic/delivery/[offeringId]' \
  frontend-school/src/lib/components/learning-delivery \
  frontend-school/tests/static/learning-delivery-workspace.test.mjs
git commit -m "feat(academic): add offering group workspace"
```

### Task 6: Connect dependent academic pages with local guidance and deep links

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/catalog/activities/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/assessments/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/exam-schedules/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte`
- Modify: `frontend-school/tests/static/academic-page-prerequisites.test.mjs`
- Modify: `frontend-school/tests/static/academic-assessment-structure.test.mjs`
- Modify: `frontend-school/tests/static/academic-exam-schedule.test.mjs`
- Modify: `frontend-school/tests/static/supervision-booking.test.mjs`
- Modify: `frontend-school/tests/static/timetable-request-performance.test.mjs`

**Interfaces:**
- Consumes: Release 1 prerequisite notice and Release 3 offering detail URLs.
- Produces: direct, page-owned next actions without a global checklist or cross-workspace preload.

- [ ] **Step 1: Add failing downstream integration assertions**

```js
assert.match(activityPage, /\/staff\/academic\/delivery\?kind=activity/);
assert.match(assessmentPage, /AcademicPrerequisiteNotice/);
assert.match(assessmentPage, /\/staff\/academic\/delivery/);
assert.match(timetablePage, /AcademicPrerequisiteNotice/);
assert.match(examPages, /\/staff\/academic\/delivery\//);
assert.match(supervisionPage, /AcademicPrerequisiteNotice/);
assert.doesNotMatch(allPages, /readinessScore|completionPercent|ศูนย์เตรียมงานวิชาการ/);
```

Add page-specific request assertions so none of these routes imports curriculum or delivery management-options clients merely to render a read view.

- [ ] **Step 2: Run downstream tests and verify failure**

```bash
cd frontend-school
node --test tests/static/academic-page-prerequisites.test.mjs \
  tests/static/academic-assessment-structure.test.mjs \
  tests/static/academic-exam-schedule.test.mjs \
  tests/static/supervision-booking.test.mjs \
  tests/static/timetable-request-performance.test.mjs
```

Expected: at least the new deep-link and prerequisite assertions FAIL.

- [ ] **Step 3: Implement only direct page dependencies**

- Activity catalog: add `เปิดกิจกรรมในภาคเรียน` linking to `/staff/academic/delivery?kind=activity` when the user can discover Learning Delivery.
- Assessment: when no course offering exists, keep the page readable and show `สร้างรายการเปิดสอนก่อนกำหนดโครงสร้างคะแนน` linking to delivery.
- Timetable: render separate notices for missing groups, teachers, periods, or rooms; link periods to `/staff/academic/core#bell-schedules`, groups to delivery, and rooms to the existing facility owner.
- Exam schedule: retain existing rounds; guide create/schedule actions when eligible offerings/groups are empty and link named items to `/staff/academic/delivery/{offeringId}?groupId={groupId}`.
- Supervision: keep overview/templates usable without a term; show local teacher/group/schedule guidance only for term-scoped booking or observation actions.

Do not add one request that loads all module readiness. Derive each notice from the data that page already owns or from its one typed workspace response.

- [ ] **Step 4: Analyze all edited Svelte files one at a time and rerun focused tests**

Run the Svelte autofixer separately for each of the six edited route files:

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/catalog/activities/+page.svelte' --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/assessments/+page.svelte' --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/timetable/+page.svelte' --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/exam-schedules/+page.svelte' --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte' --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/supervision/+page.svelte' --svelte-version 5
```

Expected: no unresolved analyzer issue for any file.

```bash
node --test tests/static/academic-page-prerequisites.test.mjs \
  tests/static/academic-assessment-structure.test.mjs \
  tests/static/academic-exam-schedule.test.mjs \
  tests/static/supervision-booking.test.mjs \
  tests/static/timetable-request-performance.test.mjs
```

Expected: PASS and request-count guards remain bounded.

- [ ] **Step 5: Commit downstream guidance**

```bash
git add 'frontend-school/src/routes/(app)/staff/academic/catalog/activities/+page.svelte' \
  'frontend-school/src/routes/(app)/staff/academic/assessments/+page.svelte' \
  'frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte' \
  'frontend-school/src/routes/(app)/staff/academic/exam-schedules/+page.svelte' \
  'frontend-school/src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte' \
  'frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte' \
  frontend-school/tests/static/academic-page-prerequisites.test.mjs \
  frontend-school/tests/static/academic-assessment-structure.test.mjs \
  frontend-school/tests/static/academic-exam-schedule.test.mjs \
  frontend-school/tests/static/supervision-booking.test.mjs \
  frontend-school/tests/static/timetable-request-performance.test.mjs
git commit -m "refactor(academic): guide dependent page setup"
```

### Task 7: Verify and prepare Release 3 for deployment

**Files:**
- Verify: every file listed in Tasks 1–6; do not create verification-only files.

**Interfaces:**
- Consumes: Tasks 1–6.
- Produces: a verified Release 3 commit set that completes the approved design.

- [ ] **Step 1: Run backend verification serially**

```bash
cd backend-school
cargo fmt --all -- --check
```

```bash
cargo test --test static_architecture
```

```bash
cargo check
```

Expected: all commands exit 0.

- [ ] **Step 2: Run generated API checks serially**

```bash
cd ../frontend-school
npm run generate:api-contracts
```

```bash
npm run check:api-contracts
```

```bash
npm run test:api-contracts
```

Expected: generated artifacts are reproducible and checks pass.

- [ ] **Step 3: Run frontend verification serially**

```bash
npm run lint
```

```bash
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

```bash
npm run test:static
```

Expected: all commands pass with zero Svelte errors and warnings.

- [ ] **Step 4: Run browser discovery and disposable backend tests**

```bash
npx playwright test --list tests/e2e/academic-context.spec.ts \
  tests/e2e/academic-core-cutover.spec.ts
```

Expected: discovery succeeds; browser execution is reported as unrun unless a dedicated deployed target and account are provided.

```bash
cd ..
./scripts/test_backend_school.sh -- --test-threads=1
```

Expected: all backend-school tests pass and disposable PostgreSQL is removed.

- [ ] **Step 5: Review final Release 3 state**

```bash
git diff --check
```

```bash
git status --short
```

```bash
git log --oneline --decorate -10
```

Expected: no whitespace errors, only intentional Release 3 changes, and task commits are visible. Stop for push/deployment approval; do not create an empty verification commit.
