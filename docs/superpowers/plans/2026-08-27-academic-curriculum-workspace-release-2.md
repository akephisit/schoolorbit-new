# Academic Curriculum Workspace Release 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the developer-oriented curriculum page with a readable overview and deep-linked curriculum workspace that uses human labels for every academic selection.

**Architecture:** Add bounded curriculum overview and management-option read models beside the existing Academic Core curriculum services. Keep existing write endpoints and published-version immutability; split the Svelte UI into an overview route and an access-guarded detail route that lazily loads management options only for authorized actions.

**Tech Stack:** Rust, Axum, SQLx, utoipa/OpenAPI, SvelteKit 5, TypeScript, shadcn-svelte, Node static tests

**Spec:** `docs/superpowers/specs/2026-08-27-academic-work-organization-and-guided-workflows-design.md`

## Global Constraints

- Release 1 must already be merged and deployed or present in the execution branch.
- Run every command serially; do not start concurrent Cargo, npm, Docker, or test processes.
- Do not change the Academic Core schema or edit any applied migration.
- Curriculum overview is independent of topbar year; year selection may be a visual comparison only.
- Published curriculum versions remain immutable; edits create or modify a draft version through existing services.
- UUIDs may cross API and persistence boundaries but never appear as editable text, labels, placeholders, errors, or exports.
- Read users receive resolved existing data and must not request action-only management options.
- Reuse generated academic curriculum permissions and existing resource policies; do not add department-named permissions.
- Additive JSON endpoints must use typed Rust DTOs and generated OpenAPI/TypeScript contracts.
- Initial overview and detail reads must use bounded batch queries and may not request one row at a time.
- Use Release 1 `AcademicPrerequisiteNotice` only for the action that lacks input; keep the rest of the page readable.

---

### Task 1: Add a bounded curriculum overview read model

**Files:**
- Modify: `backend-school/src/modules/academic/core/models.rs`
- Modify: `backend-school/src/modules/academic/core/services/workspaces.rs`
- Modify: `backend-school/src/modules/academic/core/services_tests.rs`

**Interfaces:**
- Consumes: existing `Curriculum`, `CurriculumVersion`, `GradeLevelLookupItem`, academic-year, study-program, and curriculum access-filter data.
- Produces: `workspaces::curriculum_overview(pool, filter) -> Result<CurriculumOverview, AppError>`.

- [ ] **Step 1: Write failing service coverage for display-version selection and bounded summaries**

Add a database-backed test that creates current published, future published, expired published, draft-only, and organization-filtered curricula. Assert one ordered item per visible stable curriculum and no draft replacing published display data:

```rust
let overview = workspaces::curriculum_overview(&pool, &school_filter)
    .await
    .expect("overview should load");

assert_eq!(overview.items.len(), 4);
assert_eq!(overview.items[0].curriculum.code, "CUR-A");
assert_eq!(overview.items[0].display_state, CurriculumDisplayState::Current);
assert_eq!(overview.items[0].study_program_count, 2);
assert_eq!(overview.items[0].draft_count, 1);
assert_eq!(overview.items[0].grade_levels[0].name, "มัธยมศึกษาปีที่ 1");
assert_eq!(overview.items[0].start_academic_year_name.as_deref(), Some("2569"));
assert!(!overview.items.iter().any(|item| item.curriculum.code == "OUTSIDE"));
```

- [ ] **Step 2: Run the focused test and verify failure**

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::services_tests::curriculum_overview_resolves_display_versions_and_labels \
  -- --exact --nocapture --test-threads=1
```

Expected: FAIL because `CurriculumOverview` and `curriculum_overview()` do not exist.

- [ ] **Step 3: Implement typed overview DTOs and batch queries**

Add these DTOs in `models.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CurriculumDisplayState {
    Current,
    Upcoming,
    Expired,
    Unpublished,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CurriculumOverviewItem {
    pub curriculum: Curriculum,
    pub display_version: Option<CurriculumVersion>,
    pub display_state: CurriculumDisplayState,
    pub grade_levels: Vec<GradeLevelLookupItem>,
    pub start_academic_year_name: Option<String>,
    pub end_academic_year_name: Option<String>,
    pub study_program_count: i64,
    pub draft_count: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CurriculumOverview {
    pub items: Vec<CurriculumOverviewItem>,
}
```

Implement `curriculum_overview()` with bounded SQL that loads visible curriculum identities, candidate versions, version-grade links, year labels, and aggregate program/draft counts in a fixed number of queries. Reuse the catalog display ordering rule: current published → nearest future published → most recently expired published → unpublished.

- [ ] **Step 4: Run the focused service tests**

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::services_tests::curriculum_overview_resolves_display_versions_and_labels \
  -- --exact --nocapture --test-threads=1
```

Expected: PASS.

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::services::workspaces::tests \
  -- --nocapture --test-threads=1
```

Expected: PASS, including workspace collection limits.

- [ ] **Step 5: Commit the overview service**

```bash
git add backend-school/src/modules/academic/core/models.rs \
  backend-school/src/modules/academic/core/services/workspaces.rs \
  backend-school/src/modules/academic/core/services_tests.rs
git commit -m "feat(academic): add curriculum overview service"
```

### Task 2: Resolve requirement labels and add lazy management options

**Files:**
- Modify: `backend-school/src/modules/academic/core/models.rs`
- Modify: `backend-school/src/modules/academic/core/services/workspaces.rs`
- Modify: `backend-school/src/modules/academic/core/services_tests.rs`

**Interfaces:**
- Consumes: `program_workspace(pool, version_id, filter)` and existing catalog/academic-year masters.
- Produces: resolved `CurriculumRequirementView`, `CurriculumCreateOptions`, and `CurriculumManagementOptions`.

- [ ] **Step 1: Write failing tests for resolved labels and management-only options**

Extend the existing `program_workspace` test to assert that each requirement contains grade and catalog labels:

```rust
let requirement = &workspace.requirements[0];
assert_eq!(requirement.grade_level.name, "มัธยมศึกษาปีที่ 1");
assert_eq!(requirement.catalog.code, "ค21101");
assert_eq!(requirement.catalog.name, "คณิตศาสตร์พื้นฐาน 1");
assert_eq!(requirement.catalog.resource_kind, RequirementResourceKind::Course);
```

Add tests that `curriculum_create_options()` returns only active global grade levels and that `curriculum_management_options()` returns ordered academic years plus published subject/activity versions visible to the same curriculum scope.

- [ ] **Step 2: Run focused tests and verify failure**

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::services_tests::curriculum_program_workspace_resolves_requirement_labels \
  -- --exact --nocapture --test-threads=1
```

Expected: FAIL because requirement view fields do not exist.

- [ ] **Step 3: Implement the resolved DTO and option services**

Add DTOs:

```rust
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CurriculumCatalogVersionOption {
    pub id: Uuid,
    pub resource_kind: RequirementResourceKind,
    pub code: String,
    pub name: String,
    pub version_no: i32,
    pub effective_from: NaiveDate,
    pub effective_until: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CurriculumRequirementView {
    pub study_program_id: Uuid,
    pub requirement: ProgramRequirement,
    pub grade_level: GradeLevelLookupItem,
    pub catalog: CurriculumCatalogVersionOption,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CurriculumCreateOptions {
    pub grade_levels: Vec<GradeLevelLookupItem>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CurriculumManagementOptions {
    pub academic_years: Vec<AcademicYearLookupItem>,
    pub grade_levels: Vec<GradeLevelLookupItem>,
    pub catalog_versions: Vec<CurriculumCatalogVersionOption>,
}
```

Change `CurriculumProgramWorkspace.requirements` to `Vec<CurriculumRequirementView>`. Query all referenced grade/catalog IDs in batches and fail with an integrity error when an existing requirement cannot be resolved; do not silently display a UUID fallback.

Implement:

```rust
pub async fn curriculum_create_options(
    pool: &PgPool,
    filter: &AcademicResourceListFilter,
) -> Result<CurriculumCreateOptions, AppError>;

pub async fn curriculum_management_options(
    pool: &PgPool,
    version_id: Uuid,
    filter: &AcademicResourceListFilter,
) -> Result<CurriculumManagementOptions, AppError>;
```

- [ ] **Step 4: Run focused workspace tests**

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::services_tests::curriculum_program_workspace_resolves_requirement_labels \
  -- --exact --nocapture --test-threads=1
```

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::services_tests::curriculum_management_options_are_published_scoped_and_ordered \
  -- --exact --nocapture --test-threads=1
```

Expected: both commands PASS.

- [ ] **Step 5: Commit resolved curriculum reads**

```bash
git add backend-school/src/modules/academic/core/models.rs \
  backend-school/src/modules/academic/core/services/workspaces.rs \
  backend-school/src/modules/academic/core/services_tests.rs
git commit -m "feat(academic): resolve curriculum workspace labels"
```

### Task 3: Expose and generate curriculum workspace contracts

**Files:**
- Modify: `backend-school/src/modules/academic/core/handlers.rs`
- Modify: `backend-school/src/app.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `contracts/openapi/school-api.json` (generated)
- Modify: `frontend-school/src/lib/api/generated/school-api.ts` (generated)
- Modify: `frontend-school/src/lib/api/academic-core.ts`
- Create: `frontend-school/tests/static/academic-curriculum-workspace.test.mjs`

**Interfaces:**
- Consumes: Task 1 and Task 2 services.
- Produces: `GET /api/academic/curricula/overview`, `GET /api/academic/curricula/management-options`, and `GET /api/academic/curriculum-versions/{id}/management-options`.

- [ ] **Step 1: Write the failing contract/static test**

```js
test('curriculum workspace clients use generated contracts', async () => {
  const api = await readProjectFile('src/lib/api/academic-core.ts');
  assert.match(api, /getCurriculumOverview/);
  assert.match(api, /getCurriculumCreateOptions/);
  assert.match(api, /getCurriculumManagementOptions/);
  assert.match(api, /operations\['getCurriculumOverview'\]/);
  assert.doesNotMatch(api, /ApiResponse<unknown>|Record<string, unknown>| as Curriculum/);
});
```

- [ ] **Step 2: Run it and verify failure**

```bash
cd frontend-school
node --test tests/static/academic-curriculum-workspace.test.mjs
```

Expected: FAIL because the endpoints and wrappers do not exist.

- [ ] **Step 3: Add permission-correct handlers and regenerate contracts**

Register static `/overview` and `/management-options` routes before `/curricula/{id}`. The overview handler requires `CurriculumAction::Read`; both management-option handlers require `CurriculumAction::Manage`. The version-scoped handler also resolves the version's curriculum and enforces exact resource access.

Add typed wrappers:

```ts
export type CurriculumOverview = Schemas['CurriculumOverview'];
export type CurriculumCreateOptions = Schemas['CurriculumCreateOptions'];
export type CurriculumManagementOptions = Schemas['CurriculumManagementOptions'];

export const getCurriculumOverview = (options: ApiRequestOptions = {}) =>
  academicData(
    apiClient.get<CurriculumOverview>('/api/academic/curricula/overview', options),
    'ไม่สามารถโหลดภาพรวมหลักสูตรได้'
  );
```

Add equivalent concrete wrappers for both management endpoints, then run:

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
node --test tests/static/academic-curriculum-workspace.test.mjs
```

Expected: every command PASS.

- [ ] **Step 5: Commit the curriculum contracts**

```bash
git add backend-school/src/modules/academic/core/handlers.rs backend-school/src/app.rs \
  backend-school/src/api_contract.rs contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  frontend-school/src/lib/api/academic-core.ts \
  frontend-school/tests/static/academic-curriculum-workspace.test.mjs
git commit -m "feat(academic): expose curriculum workspaces"
```

### Task 4: Build the curriculum overview route

**Files:**
- Create: `frontend-school/src/lib/components/academic-core/CurriculumOverviewTable.svelte`
- Create: `frontend-school/src/lib/components/academic-core/CurriculumCreateDialog.svelte`
- Replace: `frontend-school/src/routes/(app)/staff/academic/curricula/+page.svelte`
- Modify: `frontend-school/tests/static/academic-curriculum-workspace.test.mjs`

**Interfaces:**
- Consumes: `getCurriculumOverview()`, `getCurriculumCreateOptions()`, and existing create mutation.
- Produces: read-first curriculum overview with a lazy, permission-gated create dialog and links to `/staff/academic/curricula/{id}`.

- [ ] **Step 1: Extend the failing UI test**

```js
assert.match(page, /getCurriculumOverview/);
assert.match(page, /canManageAcademicCurriculum/);
assert.doesNotMatch(page, /getCurriculumCreateOptions\([\s\S]*onMount/);
assert.doesNotMatch(page, /gradeLevelIds:\s*''/);
assert.doesNotMatch(page, /รหัสระดับชั้น/);
assert.match(table, /startAcademicYearName/);
assert.match(table, /studyProgramCount/);
```

- [ ] **Step 2: Run the UI test and verify failure**

```bash
cd frontend-school
node --test tests/static/academic-curriculum-workspace.test.mjs
```

Expected: FAIL against the current one-page raw-ID editor.

- [ ] **Step 3: Implement overview, table/cards, and lazy create dialog**

The route loads only `getCurriculumOverview()` after read permission. The create dialog loads `getCurriculumCreateOptions()` when opened and only when `$can.hasAny(...)` contains a curriculum manage permission.

Use a desktop table and mobile cards with stable code, name, grade labels, display version, effective years, state, program count, and draft count. `CurriculumCreateDialog` uses `GradeLevelMultiSelect`; its draft is:

```ts
let draft = $state({
  code: '',
  nameTh: '',
  nameEn: '',
  gradeLevelIds: [] as string[]
});
```

Patch the new curriculum into the overview state after create instead of reloading unrelated workspaces. If grade options are empty, render `AcademicPrerequisiteNotice` without a route because no grade-level editor exists.

- [ ] **Step 4: Analyze Svelte and run the focused test**

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic-core/CurriculumOverviewTable.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic-core/CurriculumCreateDialog.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/curricula/+page.svelte' --svelte-version 5
```

Expected: no unresolved analyzer issue.

```bash
node --test tests/static/academic-curriculum-workspace.test.mjs
```

Expected: PASS for overview and lazy create behavior.

- [ ] **Step 5: Commit the overview UI**

```bash
git add frontend-school/src/lib/components/academic-core/CurriculumOverviewTable.svelte \
  frontend-school/src/lib/components/academic-core/CurriculumCreateDialog.svelte \
  'frontend-school/src/routes/(app)/staff/academic/curricula/+page.svelte' \
  frontend-school/tests/static/academic-curriculum-workspace.test.mjs
git commit -m "feat(academic): redesign curriculum overview"
```

### Task 5: Build the deep-linked curriculum detail workspace

**Files:**
- Create: `frontend-school/src/routes/(app)/staff/academic/curricula/[id]/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/academic/curricula/[id]/+page.svelte`
- Modify: `frontend-school/src/lib/components/academic-core/CurriculumProgramEditor.svelte`
- Create: `frontend-school/src/lib/components/academic-core/CurriculumVersionPanel.svelte`
- Modify: `frontend-school/tests/static/academic-curriculum-workspace.test.mjs`

**Interfaces:**
- Consumes: curriculum/version/program workspace reads, Task 2 resolved requirements, lazy management options, and existing curriculum mutation functions.
- Produces: a reloadable detail route with human-readable version, program, and requirement editing.

- [ ] **Step 1: Extend the failing detail-route test**

```js
assert.match(meta, /_meta\s*=\s*\{[\s\S]*access:/);
assert.doesNotMatch(meta, /menu:/);
assert.match(page, /getCurriculumProgramWorkspace/);
assert.match(page, /getCurriculumManagementOptions/);
assert.match(editor, /catalogVersions/);
assert.match(editor, /gradeLevels/);
assert.doesNotMatch(editor, /catalogVersionId[^\n]*<Input/);
assert.doesNotMatch(editor, /gradeLevelId[^\n]*<Input/);
```

- [ ] **Step 2: Run the detail test and verify failure**

```bash
cd frontend-school
node --test tests/static/academic-curriculum-workspace.test.mjs
```

Expected: FAIL because the detail route and labeled editors do not exist.

- [ ] **Step 3: Implement the detail route and refactor the editor**

The route metadata uses `_meta.access` with `PERMISSION_MODULES.ACADEMIC_CURRICULUM` and no menu record. Load curriculum identity, versions, selected version, and its program workspace as read data. Encode selected version in `?versionId=` so reload and browser history preserve it.

Load management options only when the user opens create/edit requirement or version controls. `CurriculumProgramEditor` accepts:

```ts
{
  version: CurriculumVersion;
  programs: StudyProgram[];
  requirements: CurriculumRequirementView[];
  managementOptions: CurriculumManagementOptions | null;
  canManage: boolean;
  onCreateProgram: (draft: CreateStudyProgramRequest) => Promise<void>;
  onReplaceRequirements: (
    program: StudyProgram,
    requirements: ProgramRequirementInput[]
  ) => Promise<void>;
}
```

Render requirements grouped by grade and recommended term with catalog code/name/version. Use shadcn Select or searchable Popover/Command for all IDs. A published version renders management controls as read-only and offers creation of a new draft version rather than editing published rows.

- [ ] **Step 4: Analyze every edited Svelte file and run focused tests**

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/curricula/[id]/+page.svelte' --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic-core/CurriculumProgramEditor.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic-core/CurriculumVersionPanel.svelte --svelte-version 5
```

Expected: no unresolved analyzer issue.

```bash
node --test tests/static/academic-curriculum-workspace.test.mjs \
  tests/static/academic-workspace-request-count.test.mjs \
  tests/static/route-preview-meta.test.mjs
```

Expected: PASS with no per-row request fan-out.

- [ ] **Step 5: Commit the detail workspace**

```bash
git add 'frontend-school/src/routes/(app)/staff/academic/curricula/[id]' \
  frontend-school/src/lib/components/academic-core/CurriculumProgramEditor.svelte \
  frontend-school/src/lib/components/academic-core/CurriculumVersionPanel.svelte \
  frontend-school/tests/static/academic-curriculum-workspace.test.mjs
git commit -m "feat(academic): add curriculum detail workspace"
```

### Task 6: Verify and prepare Release 2 for deployment

**Files:**
- Verify: every file listed in Tasks 1–5; do not create verification-only files.

**Interfaces:**
- Consumes: Tasks 1–5.
- Produces: a verified Release 2 commit set ready for deployment approval.

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

- [ ] **Step 2: Run API contract verification serially**

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

- [ ] **Step 4: Run focused browser discovery and the disposable backend suite**

```bash
npx playwright test --list tests/e2e/academic-core-cutover.spec.ts
```

Expected: discovery succeeds; execution is reported as unrun unless a dedicated deployed target and account are supplied.

```bash
cd ..
./scripts/test_backend_school.sh -- --test-threads=1
```

Expected: all backend-school tests pass and disposable PostgreSQL is removed.

- [ ] **Step 5: Review final repository state**

```bash
git diff --check
```

```bash
git status --short
```

```bash
git log --oneline --decorate -8
```

Expected: no whitespace errors, only intentional Release 2 changes, and each implementation task has its own commit. Stop at the deployment approval checkpoint; do not create an empty verification commit.
