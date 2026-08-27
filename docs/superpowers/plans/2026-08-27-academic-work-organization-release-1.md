# Academic Work Organization Release 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the recommended Thai academic-work navigation, explicit preview/apply, one authoritative period editor, and the shared page-local prerequisite presentation foundation.

**Architecture:** Route metadata supplies system-owned recommended workspace/group/order fields while persisted menu placement remains school-owned. A new menu service previews and transactionally applies only frontend-managed academic recommendations; page guidance is a typed frontend view model rendered locally, never a global readiness engine.

**Tech Stack:** Rust, Axum, SQLx, PostgreSQL, utoipa/OpenAPI, SvelteKit 5, TypeScript, shadcn-svelte, Node static tests

**Spec:** `docs/superpowers/specs/2026-08-27-academic-work-organization-and-guided-workflows-design.md`

## Global Constraints

- Run every command serially; do not start concurrent Cargo, npm, Docker, or test processes.
- Never edit migrations `001`–`046`; create only `backend-school/migrations/047_academic_menu_recommendations.sql`.
- Actual menu workspace/section placement, label, icon, active state, and ordering remain school-owned.
- Route synchronization may update recommendation metadata but must not move an existing menu item.
- Navigation placement never grants permission; preview uses `menu.read.all` and apply uses `menu.update.all`.
- Use typed Rust DTOs, register every JSON change in OpenAPI, regenerate TypeScript contracts, and consume generated types.
- Do not add a central academic readiness score, setup center, or mandatory wizard.
- Use `PageState` for whole-page state and local shadcn-svelte primitives for action-specific guidance.
- Remove user-facing `ชุดการเรียน`; use `รายการเปิดสอน` and reserve `กลุ่มเรียน` for actual cohorts.
- Preserve personal `/staff/timetable` and `/staff/exams` placement under `หน้าหลักของฉัน`.

---

### Task 1: Persist route recommendations without changing school placement

**Files:**
- Create: `backend-school/migrations/047_academic_menu_recommendations.sql`
- Modify: `backend-school/src/modules/system/services/route_registration_service.rs`
- Modify: `backend-school/tests/static_architecture.rs`
- Test: `backend-school/src/modules/system/services/route_registration_service.rs`

**Interfaces:**
- Consumes: `RouteItem { workspace, group, order }` from `backend-school/src/modules/menu/models.rs`.
- Produces: nullable `menu_items.recommended_workspace_code`, `recommended_group_code`, and `recommended_display_order`; synchronized by `sync_routes()`.

- [ ] **Step 1: Write the failing route synchronization test**

Extend `StoredMenuRoute` and add a database-backed test that starts with a customized actual group/order and stale recommendation values:

```rust
#[tokio::test]
async fn synchronization_updates_recommendations_without_moving_school_layout() {
    let pool = route_sync_test_pool("route_sync_recommendations").await;
    let custom_group_id: Uuid = sqlx::query_scalar(
        "INSERT INTO menu_groups (code, name, workspace_code, display_order)
         VALUES ('teacher_custom', 'งานที่โรงเรียนจัดเอง', 'academic', 88)
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("custom group should exist");

    sqlx::query(
        "INSERT INTO menu_items
         (code, name, path, group_id, display_order, managed_by)
         VALUES ('staff-academic-core', 'ชื่อที่โรงเรียนตั้ง', '/staff/academic/core', $1, 77, 'frontend')",
    )
    .bind(custom_group_id)
    .execute(&pool)
    .await
    .expect("menu item should exist");

    sync_routes(
        &pool,
        &RouteRegistration {
            routes: vec![route(
                "/staff/academic/core",
                "academic_delivery",
                "academic",
                "staff",
            )],
            environment: Some("test".to_string()),
        },
    )
    .await
    .expect("sync should pass");

    let row = sqlx::query_as::<_, StoredMenuRoute>(
        "SELECT name, icon, group_id, display_order, is_active, path,
                required_permission, user_type, managed_by,
                recommended_workspace_code, recommended_group_code,
                recommended_display_order
         FROM menu_items WHERE code = 'staff-academic-core'",
    )
    .fetch_one(&pool)
    .await
    .expect("route should remain");

    assert_eq!(row.group_id, Some(custom_group_id));
    assert_eq!(row.display_order, 77);
    assert_eq!(row.name, "ชื่อที่โรงเรียนตั้ง");
    assert_eq!(row.recommended_workspace_code.as_deref(), Some("academic"));
    assert_eq!(row.recommended_group_code.as_deref(), Some("academic_delivery"));
    assert_eq!(row.recommended_display_order, Some(10));
}
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run from repository root:

```bash
./scripts/test_backend_school.sh \
  modules::system::services::route_registration_service::tests::synchronization_updates_recommendations_without_moving_school_layout \
  -- --exact --nocapture --test-threads=1
```

Expected: FAIL because migration `047` and the three recommendation fields do not exist.

- [ ] **Step 3: Add the forward migration and synchronize recommendation fields**

Create migration `047` with these columns and recommended sections:

```sql
ALTER TABLE menu_items
    ADD COLUMN recommended_workspace_code character varying(50),
    ADD COLUMN recommended_group_code character varying(50),
    ADD COLUMN recommended_display_order integer;

COMMENT ON COLUMN menu_items.recommended_workspace_code IS
    'Frontend-owned recommended workspace. Actual persisted placement remains school-owned.';
COMMENT ON COLUMN menu_items.recommended_group_code IS
    'Frontend-owned recommended work section. Actual group_id remains school-owned.';
COMMENT ON COLUMN menu_items.recommended_display_order IS
    'Frontend-owned recommended order. Actual display_order remains school-owned.';

INSERT INTO menu_groups
    (code, name, name_en, icon, display_order, is_active, workspace_code)
VALUES
    ('academic_curriculum', 'งานหลักสูตรและกลุ่มสาระ', 'Curriculum and Learning Areas', 'book-open', 10, true, 'academic'),
    ('academic_delivery', 'งานจัดการเรียนการสอน', 'Teaching and Learning Delivery', 'calendar-days', 20, true, 'academic'),
    ('academic_registry', 'งานทะเบียนนักเรียน', 'Student Registry', 'users', 30, true, 'academic'),
    ('academic_assessment', 'งานวัดผลและประเมินผล', 'Measurement and Evaluation', 'badge-check', 40, true, 'academic'),
    ('academic_activities', 'งานกิจกรรมพัฒนาผู้เรียน', 'Learner Development Activities', 'sparkles', 50, true, 'academic'),
    ('academic_supervision', 'งานนิเทศและพัฒนาการสอน', 'Instructional Supervision', 'clipboard-check', 60, true, 'academic'),
    ('academic_admission', 'งานรับนักเรียน', 'Student Admission', 'clipboard-list', 70, true, 'academic')
ON CONFLICT (code) DO NOTHING;
```

Update both INSERT and `ON CONFLICT DO UPDATE` branches in `sync_routes()`:

```rust
recommended_workspace_code = EXCLUDED.recommended_workspace_code,
recommended_group_code = EXCLUDED.recommended_group_code,
recommended_display_order = EXCLUDED.recommended_display_order,
group_id = COALESCE(menu_items.group_id, EXCLUDED.group_id),
display_order = COALESCE(menu_items.display_order, EXCLUDED.display_order)
```

Keep `name`, `icon`, `group_id`, `display_order`, and `is_active` preservation unchanged for existing rows.

- [ ] **Step 4: Run focused and architecture tests**

```bash
./scripts/test_backend_school.sh \
  modules::system::services::route_registration_service::tests::synchronization_updates_recommendations_without_moving_school_layout \
  -- --exact --nocapture --test-threads=1
```

Expected: PASS.

```bash
cd backend-school
cargo test --test static_architecture menu_workspace_contract_is_explicit_and_permission_based -- --exact
```

Expected: PASS with guards for all three recommendation columns and continued school-owned placement preservation.

- [ ] **Step 5: Commit the recommendation boundary**

```bash
git add backend-school/migrations/047_academic_menu_recommendations.sql \
  backend-school/src/modules/system/services/route_registration_service.rs \
  backend-school/tests/static_architecture.rs
git commit -m "feat(menu): persist academic route recommendations"
```

### Task 2: Add transactional academic menu-template preview and apply

**Files:**
- Create: `backend-school/src/modules/menu/services/academic_template_service.rs`
- Modify: `backend-school/src/modules/menu/services.rs`
- Modify: `backend-school/src/modules/menu/models.rs`
- Modify: `backend-school/src/modules/menu/handlers/admin.rs`
- Modify: `backend-school/src/app.rs`
- Test: `backend-school/src/modules/menu/services/academic_template_service.rs`

**Interfaces:**
- Consumes: frontend-managed menu rows with non-null recommendation metadata from Task 1.
- Produces: `preview_academic_template(pool) -> Result<AcademicMenuTemplatePreview, AppError>` and `apply_academic_template(pool, expected_revision) -> Result<AcademicMenuTemplateApplyResult, AppError>`.

- [ ] **Step 1: Write failing service tests for preview, preservation, stale revision, and idempotency**

Define the expected DTO boundary in the test:

```rust
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcademicMenuTemplatePreview {
    pub revision: String,
    pub recommendations_ready: bool,
    pub moves: Vec<AcademicMenuTemplateMove>,
    pub untouched_custom_item_count: usize,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcademicMenuTemplateMove {
    pub menu_item_id: Uuid,
    pub menu_item_name: String,
    pub current_group_name: Option<String>,
    pub target_group_code: String,
    pub target_group_name: String,
    pub current_order: i32,
    pub target_order: i32,
}
```

Add tests that prove `school` and `integration` items remain untouched, labels/icons/active states survive apply, a stale revision returns `AppError::Conflict`, and a second apply reports zero moves.

- [ ] **Step 2: Run the failing service tests**

```bash
./scripts/test_backend_school.sh \
  modules::menu::services::academic_template_service::tests \
  -- --nocapture --test-threads=1
```

Expected: FAIL because the service and DTOs are absent.

- [ ] **Step 3: Implement the service and thin handlers**

Create `academic_template_service.rs` with:

```rust
pub async fn preview_academic_template(
    pool: &PgPool,
) -> Result<AcademicMenuTemplatePreview, AppError>;

pub async fn apply_academic_template(
    pool: &PgPool,
    expected_revision: &str,
) -> Result<AcademicMenuTemplateApplyResult, AppError>;
```

Build the revision with SHA-256 over a deterministically ordered serializable snapshot containing item ID, current group/order, updated timestamp, and recommendation fields. Apply starts a transaction, rebuilds the preview using that transaction, compares revisions, resolves target group codes, then performs one validated bulk update with `UNNEST`. Do not update `name`, `icon`, `is_active`, `path`, `required_permission`, `user_type`, or `managed_by`.

Add handlers and routes:

```text
GET  /api/admin/menu/templates/academic/recommended
POST /api/admin/menu/templates/academic/recommended/apply
```

The GET handler requires `codes::MENU_READ_ALL`. The POST request is:

```rust
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyAcademicMenuTemplateRequest {
    pub revision: String,
}
```

The POST handler requires `codes::MENU_UPDATE_ALL` and returns `ApiResponse<AcademicMenuTemplateApplyResult>`.

- [ ] **Step 4: Run the focused service and static architecture tests**

```bash
./scripts/test_backend_school.sh \
  modules::menu::services::academic_template_service::tests \
  -- --nocapture --test-threads=1
```

Expected: PASS.

```bash
cd backend-school
cargo test --test static_architecture menu_and_feature_handlers_do_not_parse_auth_or_query_permissions_directly -- --exact
```

Expected: PASS; handlers remain context → permission → service → typed response.

- [ ] **Step 5: Commit the backend menu template**

```bash
git add backend-school/src/modules/menu/services/academic_template_service.rs \
  backend-school/src/modules/menu/services.rs \
  backend-school/src/modules/menu/models.rs \
  backend-school/src/modules/menu/handlers/admin.rs \
  backend-school/src/app.rs
git commit -m "feat(menu): preview and apply academic layout"
```

### Task 3: Register and consume the generated menu-template API

**Files:**
- Modify: `backend-school/src/api_contract.rs`
- Modify: `contracts/openapi/school-api.json` (generated)
- Modify: `frontend-school/src/lib/api/generated/school-api.ts` (generated)
- Modify: `frontend-school/src/lib/api/menu-admin.ts`
- Create: `frontend-school/tests/static/academic-menu-template.test.mjs`

**Interfaces:**
- Consumes: Task 2 handlers and DTOs.
- Produces: generated operations `previewRecommendedAcademicMenuTemplate` and `applyRecommendedAcademicMenuTemplate`; typed frontend functions with the same semantic names.

- [ ] **Step 1: Write the failing contract/static test**

```js
test('academic menu template uses generated preview and apply contracts', async () => {
  const api = await readProjectFile('src/lib/api/menu-admin.ts');
  const contract = await readProjectFile('../contracts/openapi/school-api.json');
  assert.match(api, /AcademicMenuTemplatePreview/);
  assert.match(api, /previewRecommendedAcademicMenuTemplate/);
  assert.match(api, /applyRecommendedAcademicMenuTemplate/);
  assert.match(contract, /\/api\/admin\/menu\/templates\/academic\/recommended/);
  assert.match(contract, /ApplyAcademicMenuTemplateRequest/);
});
```

- [ ] **Step 2: Run it and verify failure**

```bash
cd frontend-school
node --test tests/static/academic-menu-template.test.mjs
```

Expected: FAIL because the endpoints and client functions are not registered.

- [ ] **Step 3: Register OpenAPI and add typed client functions**

Add both handlers and every new schema to `backend-school/src/api_contract.rs`, then regenerate from `frontend-school`:

```bash
npm run generate:api-contracts
```

Implement client functions without casts:

```ts
export type AcademicMenuTemplatePreview = Schemas['AcademicMenuTemplatePreview'];
export type AcademicMenuTemplateApplyResult = Schemas['AcademicMenuTemplateApplyResult'];

export async function previewRecommendedAcademicMenuTemplate(): Promise<AcademicMenuTemplatePreview>;
export async function applyRecommendedAcademicMenuTemplate(
  revision: string
): Promise<AcademicMenuTemplateApplyResult>;
```

- [ ] **Step 4: Run contract and focused tests**

```bash
cd frontend-school
npm run check:api-contracts
```

Expected: PASS.

```bash
npm run test:api-contracts
```

Expected: PASS.

```bash
node --test tests/static/academic-menu-template.test.mjs
```

Expected: PASS.

- [ ] **Step 5: Commit the generated contract boundary**

```bash
git add backend-school/src/api_contract.rs contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  frontend-school/src/lib/api/menu-admin.ts \
  frontend-school/tests/static/academic-menu-template.test.mjs
git commit -m "feat(menu): expose academic layout contract"
```

### Task 4: Add menu-template preview/apply UI

**Files:**
- Create: `frontend-school/src/lib/components/menu/AcademicMenuTemplateDialog.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/menu/+page.svelte`
- Modify: `frontend-school/tests/static/academic-menu-template.test.mjs`

**Interfaces:**
- Consumes: `previewRecommendedAcademicMenuTemplate()` and `applyRecommendedAcademicMenuTemplate(revision)` from Task 3.
- Produces: an explicit `ใช้โครงสร้างงานวิชาการแนะนำ` action that refreshes persisted workspaces/groups/items after successful apply.

- [ ] **Step 1: Extend the failing UI test**

Assert that the page is permission-gated and the dialog renders the preview rather than applying immediately:

```js
assert.match(page, /PERMISSIONS\.MENU_READ_ALL/);
assert.match(page, /PERMISSIONS\.MENU_UPDATE_ALL/);
assert.match(dialog, /ใช้โครงสร้างงานวิชาการแนะนำ/);
assert.match(dialog, /previewRecommendedAcademicMenuTemplate/);
assert.match(dialog, /applyRecommendedAcademicMenuTemplate/);
assert.match(dialog, /preview\.revision/);
assert.doesNotMatch(dialog, /onMount\([\s\S]*applyRecommendedAcademicMenuTemplate/);
```

- [ ] **Step 2: Run the UI test and verify failure**

```bash
cd frontend-school
node --test tests/static/academic-menu-template.test.mjs
```

Expected: FAIL because the dialog does not exist.

- [ ] **Step 3: Implement the shadcn-svelte preview dialog**

Use `Dialog`, `Alert`, `Table`/mobile cards, `Button`, and `LoadingButton`. Load preview only when the dialog opens. Show current → target section and order for every move, the untouched custom-link count, and a disabled apply action when `recommendationsReady` is false or the user lacks `menu.update.all`.

The component contract is:

```ts
let {
  open = $bindable(false),
  canApply,
  onApplied
}: {
  open?: boolean;
  canApply: boolean;
  onApplied: () => Promise<void> | void;
} = $props();
```

On `409`, discard the stale preview, reload it, and show `ข้อมูลเมนูเปลี่ยนแล้ว กรุณาตรวจสอบรายการอีกครั้ง`. On success, close the dialog, show the applied move count, and call `onApplied()` once.

- [ ] **Step 4: Analyze the Svelte files and run the focused test**

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/menu/AcademicMenuTemplateDialog.svelte --svelte-version 5
```

Expected: no issues or suggestions requiring a code change.

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/menu/+page.svelte' --svelte-version 5
```

Expected: no issues or suggestions requiring a code change.

```bash
node --test tests/static/academic-menu-template.test.mjs
```

Expected: PASS.

- [ ] **Step 5: Commit the administrator UI**

```bash
git add frontend-school/src/lib/components/menu/AcademicMenuTemplateDialog.svelte \
  'frontend-school/src/routes/(app)/staff/menu/+page.svelte' \
  frontend-school/tests/static/academic-menu-template.test.mjs
git commit -m "feat(menu): add academic layout preview"
```

### Task 5: Map academic routes and consolidate period editing

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/catalog/subject-groups/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/catalog/subjects/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/curricula/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/core/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/delivery/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/homerooms/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/student-years/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/assessments/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/question-bank/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/exam-schedules/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/catalog/activities/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/supervision/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/admission/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/students/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/periods/+page.ts`
- Delete: `frontend-school/src/routes/(app)/staff/academic/periods/+page.svelte`
- Modify: `frontend-school/src/lib/components/academic-core/AcademicYearTermEditor.svelte`
- Create: `frontend-school/tests/static/academic-work-organization.test.mjs`

**Interfaces:**
- Consumes: recommended section codes inserted by Task 1.
- Produces: the exact route-to-section table in the approved spec and a single period editor anchored at `/staff/academic/core#bell-schedules`.

- [ ] **Step 1: Write the failing route-mapping test**

Create a test with the approved map:

```js
const expected = new Map([
  ['catalog/subject-groups', 'academic_curriculum'],
  ['catalog/subjects', 'academic_curriculum'],
  ['curricula', 'academic_curriculum'],
  ['core', 'academic_delivery'],
  ['delivery', 'academic_delivery'],
  ['timetable/today', 'academic_delivery'],
  ['timetable', 'academic_delivery'],
  ['homerooms', 'academic_registry'],
  ['student-years', 'academic_registry'],
  ['assessments', 'academic_assessment'],
  ['question-bank', 'academic_assessment'],
  ['exam-schedules', 'academic_assessment'],
  ['catalog/activities', 'academic_activities'],
  ['supervision', 'academic_supervision'],
  ['admission', 'academic_admission']
]);
```

Also assert `/staff/students` uses `academic_registry`, `/staff/timetable` and `/staff/exams` remain `home`, periods has no `menu`, and the period page throws a redirect to the core anchor.

- [ ] **Step 2: Run the route test and verify failure**

```bash
cd frontend-school
node --test tests/static/academic-work-organization.test.mjs
```

Expected: FAIL because routes still use the generic `academic` group and periods remains a separate editor.

- [ ] **Step 3: Update metadata and replace the period page with a redirect**

Use section-local orders `10, 20, 30...` within each group. Rename:

```ts
// core
title: 'ปีการศึกษา ภาคเรียน และเวลาเรียน'

// delivery
title: 'รายวิชาและกิจกรรมที่เปิดสอน'

// activity catalog
title: 'ทะเบียนกิจกรรมพัฒนาผู้เรียน'
```

Replace periods `+page.ts` with an access-only redirect:

```ts
import { redirect } from '@sveltejs/kit';
import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
  access: {
    user_type: 'staff',
    permission: PERMISSION_MODULES.ACADEMIC_TERM
  }
};

export const load = () => redirect(308, '/staff/academic/core#bell-schedules');
```

Add `id="bell-schedules"` and scroll margin to the authoritative bell-schedule section inside `AcademicYearTermEditor.svelte`.

- [ ] **Step 4: Run focused static and Svelte checks**

```bash
cd frontend-school
node --test tests/static/academic-work-organization.test.mjs \
  tests/static/menu-route-deployment.test.mjs \
  tests/static/sidebar-navigation.test.mjs
```

Expected: PASS.

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic-core/AcademicYearTermEditor.svelte --svelte-version 5
```

Expected: no unresolved issue.

- [ ] **Step 5: Commit the navigation cutover**

```bash
git add 'frontend-school/src/routes/(app)/staff/academic/catalog/subject-groups/+page.ts' \
  'frontend-school/src/routes/(app)/staff/academic/catalog/subjects/+page.ts' \
  'frontend-school/src/routes/(app)/staff/academic/curricula/+page.ts' \
  'frontend-school/src/routes/(app)/staff/academic/core/+page.ts' \
  'frontend-school/src/routes/(app)/staff/academic/delivery/+page.ts' \
  'frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.ts' \
  'frontend-school/src/routes/(app)/staff/academic/timetable/+page.ts' \
  'frontend-school/src/routes/(app)/staff/academic/homerooms/+page.ts' \
  'frontend-school/src/routes/(app)/staff/academic/student-years/+page.ts' \
  'frontend-school/src/routes/(app)/staff/academic/assessments/+page.ts' \
  'frontend-school/src/routes/(app)/staff/academic/question-bank/+page.ts' \
  'frontend-school/src/routes/(app)/staff/academic/exam-schedules/+page.ts' \
  'frontend-school/src/routes/(app)/staff/academic/catalog/activities/+page.ts' \
  'frontend-school/src/routes/(app)/staff/academic/supervision/+page.ts' \
  'frontend-school/src/routes/(app)/staff/academic/admission/+page.ts' \
  'frontend-school/src/routes/(app)/staff/students/+page.ts' \
  'frontend-school/src/routes/(app)/staff/academic/periods/+page.ts' \
  'frontend-school/src/routes/(app)/staff/academic/periods/+page.svelte' \
  frontend-school/src/lib/components/academic-core/AcademicYearTermEditor.svelte \
  frontend-school/tests/static/academic-work-organization.test.mjs
git commit -m "refactor(academic): organize services by school work"
```

### Task 6: Add the page-local prerequisite primitive and terminology guard

**Files:**
- Create: `frontend-school/src/lib/components/academic-workflow/prerequisite.ts`
- Create: `frontend-school/src/lib/components/academic-workflow/AcademicPrerequisiteNotice.svelte`
- Create: `frontend-school/src/lib/components/academic-workflow/index.ts`
- Modify: `frontend-school/src/lib/api/learning-delivery.ts`
- Modify: `frontend-school/src/lib/api/academicAssessments.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/delivery/+page.svelte`
- Create: `frontend-school/tests/static/academic-page-prerequisites.test.mjs`

**Interfaces:**
- Produces: `AcademicPrerequisite` and `AcademicPrerequisiteNotice` for Release 2 and Release 3 pages.

- [ ] **Step 1: Write failing type and language tests**

```js
test('academic prerequisites stay page-local and action-specific', async () => {
  const model = await readProjectFile('src/lib/components/academic-workflow/prerequisite.ts');
  const notice = await readProjectFile('src/lib/components/academic-workflow/AcademicPrerequisiteNotice.svelte');
  assert.match(model, /status: 'missing' \| 'warning'/);
  assert.match(model, /href\?: string/);
  assert.match(notice, /ทางไปต่อ/);
  assert.doesNotMatch(model, /global|completionPercent|readinessScore/i);
});

test('learning delivery no longer calls offerings ชุดการเรียน', async () => {
  const files = await readAcademicWorkflowSources();
  assert.doesNotMatch(files, /ชุดการเรียน/);
  assert.match(files, /รายการเปิดสอน/);
});
```

- [ ] **Step 2: Run the test and verify failure**

```bash
cd frontend-school
node --test tests/static/academic-page-prerequisites.test.mjs
```

Expected: FAIL because the primitive is absent and legacy copy remains.

- [ ] **Step 3: Implement the typed primitive and migrate Release 1 copy**

Create the model:

```ts
export interface AcademicPrerequisite {
  key: string;
  status: 'missing' | 'warning';
  title: string;
  description: string;
  actionLabel?: string;
  href?: string;
}
```

`AcademicPrerequisiteNotice.svelte` accepts one prerequisite, renders `Alert`, and shows `ทางไปต่อ` only when both `actionLabel` and `href` exist. It owns no store, fetch, global registry, or readiness calculation.

Replace legacy delivery and assessment API fallback messages and the current delivery page title, descriptions, error, empty, publish, and toast copy with `รายการเปิดสอน`. Use the new notice for the existing missing-term and no-offering action guidance without changing mutation behavior.

- [ ] **Step 4: Analyze Svelte and run focused tests**

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic-workflow/AcademicPrerequisiteNotice.svelte --svelte-version 5
```

Expected: no unresolved issue.

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/delivery/+page.svelte' --svelte-version 5
```

Expected: no unresolved issue.

```bash
node --test tests/static/academic-page-prerequisites.test.mjs \
  tests/static/academic-workspace-request-count.test.mjs \
  tests/static/academic-assessment-structure.test.mjs
```

Expected: PASS.

- [ ] **Step 5: Commit the local-guidance foundation**

```bash
git add frontend-school/src/lib/components/academic-workflow \
  frontend-school/src/lib/api/learning-delivery.ts \
  frontend-school/src/lib/api/academicAssessments.ts \
  'frontend-school/src/routes/(app)/staff/academic/delivery/+page.svelte' \
  frontend-school/tests/static/academic-page-prerequisites.test.mjs
git commit -m "refactor(academic): add page-local prerequisite guidance"
```

### Task 7: Verify and prepare Release 1 for deployment

**Files:**
- Verify: every file listed in Tasks 1–6; do not create verification-only files.

**Interfaces:**
- Consumes: Tasks 1–6.
- Produces: a verified Release 1 commit set ready to push and deploy; no deployment is performed by this task.

- [ ] **Step 1: Run backend focused and contract checks serially**

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

Expected: every command exits 0.

- [ ] **Step 2: Run API contract checks serially**

```bash
cd frontend-school
npm run generate:api-contracts
```

```bash
npm run check:api-contracts
```

```bash
npm run test:api-contracts
```

Expected: generated artifacts remain clean after generation and every check passes.

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

- [ ] **Step 4: Run the disposable database suite and final Git checks**

```bash
cd ..
./scripts/test_backend_school.sh -- --test-threads=1
```

Expected: all migrations through `047` and all backend-school tests pass; the disposable PostgreSQL container is removed.

```bash
git diff --check
```

```bash
git status --short
```

Expected: no whitespace errors and only intentional Release 1 changes before the final verification commit.

- [ ] **Step 5: Review final Release 1 state**

```bash
git log --oneline --decorate -10
```

Expected: task commits are visible. If verification exposed a defect, return to the owning task, add its regression test and correction there, then rerun this task. Stop for deployment approval; do not create an empty verification commit.
