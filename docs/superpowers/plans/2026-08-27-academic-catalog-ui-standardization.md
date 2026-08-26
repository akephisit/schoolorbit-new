# Academic Catalog UI and Input Standardization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver readable subject/activity catalog overviews, compact the academic-context topbar, and standardize every application dropdown and date calendar on local shadcn-svelte primitives without N+1 reads.

**Architecture:** Add fixed-query-count academic catalog overview endpoints that return stable identities, deterministic display versions, draft counts, resolved grade levels, and complete grade-level options. Consume generated OpenAPI DTOs in responsive Svelte catalog workspaces, keep history on-demand and cached, and standardize remaining date/calendar controls through shared shadcn-svelte primitives plus static policy guards.

**Tech Stack:** Rust 2021, Axum 0.8, SQLx 0.8/PostgreSQL, Utoipa/OpenAPI, TypeScript 5.9, Svelte 5 runes, SvelteKit 2, Tailwind CSS 4, shadcn-svelte/Bits UI, Node test runner

**Spec:** `docs/superpowers/specs/2026-08-27-academic-catalog-ui-standardization-design.md`

## Global Constraints

- Run commands strictly one at a time; never run test, build, formatter, or generator processes concurrently.
- Never run `cargo clean` and never edit an applied migration.
- Keep catalog identity global; do not add academic-year or academic-term query parameters to the overview pages.
- Use generated OpenAPI contracts for every new backend/frontend interface.
- Keep existing catalog write endpoints and permission policies authoritative.
- Use local shadcn-svelte `Select`, `Popover`, `Command`, `Checkbox`, and `Calendar` primitives; do not add a UI dependency.
- Preserve Kanit, semantic theme tokens, dark mode, keyboard focus, responsive behavior, and current permission scopes.
- Do not replace native time or datetime-local controls in this release.
- Run the Svelte autofixer on every edited `.svelte` file before final verification.

---

### Task 1: Fixed-query-count catalog overview domain service

**Files:**
- Modify: `backend-school/src/modules/academic/core/models.rs`
- Modify: `backend-school/src/modules/academic/core/services/catalog.rs`
- Modify: `backend-school/src/modules/academic/core/services_tests.rs`

**Interfaces:**
- Consumes: `CatalogSubject`, `SubjectVersion`, `CatalogActivity`, `ActivityVersion`, `GradeLevelLookupItem`, `AcademicResourceListFilter`, and `chrono::NaiveDate`.
- Produces: `CatalogDisplayState`, `CatalogSubjectOverviewItem`, `CatalogSubjectOverview`, `CatalogActivityOverviewItem`, `CatalogActivityOverview`, `list_subject_overview(pool, filter, today)`, and `list_activity_overview(pool, filter, today)`.

- [ ] **Step 1: Write failing display-selection and overview integration tests**

Add SQL-backed tests beside the existing `prepare_core_fixture` helper covering current, future, expired, unpublished, draft precedence, grade labels, and organization filtering. Use a fixed date so timezone does not affect assertions. Create rows through the existing catalog services, publish the rows that represent released data, and then call the new overview function:

```rust
#[tokio::test]
async fn catalog_overview_prefers_current_published_version_over_draft_and_future() {
    let pool = prepare_core_fixture("catalog_overview_display_precedence").await;
    let today = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap();
    let grade_level_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM grade_levels WHERE is_active = true ORDER BY level_type, year LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let subject = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "OVERVIEW-1".to_string(),
            owning_organization_unit_id: None,
        },
    )
    .await
    .unwrap();
    let current = catalog::create_subject_version(
        &pool,
        subject.id,
        CreateSubjectVersionRequest {
            name_th: "รายวิชาที่ใช้อยู่".to_string(),
            name_en: None,
            credit: "1.00".to_string(),
            hours_per_semester: Some(40),
            subject_type: "BASIC".to_string(),
            group_id: None,
            description: None,
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
            term_code: None,
            periods_per_week: Some(2),
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    catalog::publish_subject_version(
        &pool,
        current.id,
        PublishVersionRequest { row_version: current.row_version },
    )
    .await
    .unwrap();
    catalog::create_subject_version(
        &pool,
        subject.id,
        CreateSubjectVersionRequest {
            name_th: "ร่างปีถัดไป".to_string(),
            name_en: None,
            credit: "1.50".to_string(),
            hours_per_semester: Some(60),
            subject_type: "ADDITIONAL".to_string(),
            group_id: None,
            description: None,
            effective_from: NaiveDate::from_ymd_opt(2027, 1, 1).unwrap(),
            effective_until: None,
            term_code: None,
            periods_per_week: Some(3),
            grade_level_ids: vec![grade_level_id],
        },
    )
    .await
    .unwrap();
    let overview = catalog::list_subject_overview(
        &pool,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..AcademicResourceListFilter::default()
        },
        today,
    )
    .await
    .unwrap();
    let item = overview.items.iter().find(|item| item.subject.code == "OVERVIEW-1").unwrap();
    assert_eq!(item.display_version.as_ref().unwrap().version_no, 1);
    assert_eq!(item.display_state, CatalogDisplayState::Current);
    assert_eq!(item.draft_count, 1);
}

#[tokio::test]
async fn catalog_overview_returns_grade_options_and_respects_owner_filter() {
    let pool = prepare_core_fixture("catalog_overview_owner_filter").await;
    let overview = catalog::list_subject_overview(
        &pool,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..AcademicResourceListFilter::default()
        },
        NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
    )
    .await
    .unwrap();
    assert!(!overview.grade_level_options.is_empty());
    assert!(overview.items.iter().all(|item| item.subject.owning_organization_unit_id.is_none()));
}
```

- [ ] **Step 2: Run focused backend tests and verify RED**

Run:

```bash
cargo test catalog_overview_prefers_current_published_version_over_draft_and_future --manifest-path backend-school/Cargo.toml
```

Expected: compilation fails because the overview types/functions do not exist.

- [ ] **Step 3: Add typed overview models**

Define camelCase response models and a snake-case serialized enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CatalogDisplayState {
    Current,
    Upcoming,
    Expired,
    Unpublished,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSubjectOverviewItem {
    pub subject: CatalogSubject,
    pub display_version: Option<SubjectVersion>,
    pub display_state: CatalogDisplayState,
    pub draft_count: i64,
    pub grade_levels: Vec<GradeLevelLookupItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSubjectOverview {
    pub items: Vec<CatalogSubjectOverviewItem>,
    pub grade_level_options: Vec<GradeLevelLookupItem>,
}
```

Add equivalent activity structs using `CatalogActivity` and `ActivityVersion`.

- [ ] **Step 4: Implement deterministic display selection and batched reads**

Fetch visible stable rows once, all their versions with `subject_id = ANY($1)` or `activity_id = ANY($1)` once, and active grade levels once. Group versions by stable ID in a `HashMap` and select display data using:

```rust
fn published_state(from: NaiveDate, until: Option<NaiveDate>, today: NaiveDate) -> CatalogDisplayState {
    if from <= today && until.is_none_or(|date| date >= today) {
        CatalogDisplayState::Current
    } else if from > today {
        CatalogDisplayState::Upcoming
    } else {
        CatalogDisplayState::Expired
    }
}
```

Choose current first, then minimum future `effective_from`, then maximum expired `effective_until`/`effective_from`. Count drafts separately. Query `grade_levels WHERE is_active = true` ordered kindergarten, primary, secondary, other then year; format `K/P/M`, full Thai name, and `อ./ป./ม.` labels consistently with lookup services. Resolve each display version's IDs from one lookup map.

- [ ] **Step 5: Run focused academic core tests and verify GREEN**

Run:

```bash
cargo test catalog_overview --manifest-path backend-school/Cargo.toml
```

Expected: all matching tests pass.

- [ ] **Step 6: Format and commit the domain service**

Run:

```bash
cargo fmt --manifest-path backend-school/Cargo.toml -- --check
```

Then commit:

```bash
git add backend-school/src/modules/academic/core/models.rs backend-school/src/modules/academic/core/services/catalog.rs backend-school/src/modules/academic/core/services_tests.rs
git commit -m "feat: add academic catalog overview services"
```

### Task 2: Overview routes and generated API contracts

**Files:**
- Modify: `backend-school/src/modules/academic/core.rs`
- Modify: `backend-school/src/modules/academic/core/handlers.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `frontend-school/src/lib/api/generated/school-api.ts` (generated)
- Modify: `frontend-school/static/openapi/school-api.json` (generated)
- Modify: `frontend-school/src/lib/api/academic-core.ts`
- Modify: `frontend-school/tests/static/academic-core-cutover-contract.test.mjs`

**Interfaces:**
- Consumes: overview service functions from Task 1 and `SCHOOL_TIMEZONE`.
- Produces: `GET /api/academic/catalog/subjects/overview`, `GET /api/academic/catalog/activities/overview`, `getCatalogSubjectOverview()`, and `getCatalogActivityOverview()`.

- [ ] **Step 1: Add failing route/contract assertions**

Extend the static operation list:

```js
['/api/academic/catalog/subjects/overview', 'get', 'getCatalogSubjectOverview'],
['/api/academic/catalog/activities/overview', 'get', 'getCatalogActivityOverview'],
```

Assert generated schemas contain `CatalogSubjectOverview`, `CatalogActivityOverview`, and `CatalogDisplayState`.

- [ ] **Step 2: Run the contract test and verify RED**

Run:

```bash
node --test frontend-school/tests/static/academic-core-cutover-contract.test.mjs
```

Expected: missing OpenAPI operations/schemas.

- [ ] **Step 3: Register routes and permission-filtered handlers**

Register static overview paths before UUID paths. Each handler reuses catalog read access and Bangkok local date:

```rust
let today = Utc::now().with_timezone(&SCHOOL_TIMEZONE).date_naive();
let filter = academic_catalog_access_policy::require_academic_catalog_list_access(
    &pool,
    &actor,
    CatalogAction::Read,
)
.await?;
Ok(ok(catalog::list_subject_overview(&pool, &filter, today).await?))
```

Add Utoipa operation IDs exactly `getCatalogSubjectOverview` and `getCatalogActivityOverview`; register paths and response schemas in `api_contract.rs`.

- [ ] **Step 4: Generate and consume the contract**

Run:

```bash
npm run generate:api-contracts
```

from `frontend-school`, then add generated aliases and wrappers:

```ts
export type CatalogSubjectOverview = Schemas['CatalogSubjectOverview'];
export type CatalogActivityOverview = Schemas['CatalogActivityOverview'];

export const getCatalogSubjectOverview = (options: ApiRequestOptions = {}) =>
	academicData(
		apiClient.get<CatalogSubjectOverview>('/api/academic/catalog/subjects/overview', options),
		'ไม่สามารถโหลดภาพรวมทะเบียนรายวิชาได้'
	);
```

Add the activity equivalent.

- [ ] **Step 5: Verify generated contract and focused tests GREEN**

Run serially:

```bash
npm run check:api-contracts
```

```bash
node --test tests/static/academic-core-cutover-contract.test.mjs
```

Expected: both pass from `frontend-school`.

- [ ] **Step 6: Commit routes and contracts**

```bash
git add backend-school/src/modules/academic/core.rs backend-school/src/modules/academic/core/handlers.rs backend-school/src/api_contract.rs frontend-school/src/lib/api/academic-core.ts frontend-school/src/lib/api/generated/school-api.ts frontend-school/static/openapi/school-api.json frontend-school/tests/static/academic-core-cutover-contract.test.mjs
git commit -m "feat: expose academic catalog overviews"
```

### Task 3: Shared catalog presentation and editing controls

**Files:**
- Create: `frontend-school/src/lib/academic-core/catalog-presentation.ts`
- Create: `frontend-school/src/lib/components/academic-core/GradeLevelMultiSelect.svelte`
- Modify: `frontend-school/src/lib/components/academic-core/CatalogVersionHistory.svelte`
- Create: `frontend-school/tests/static/academic-catalog-ui.test.mjs`

**Interfaces:**
- Consumes: generated `GradeLevelLookupItem`, subject/activity version DTOs, local shadcn Select/Popover/Command/Checkbox/Badge/Button/DatePicker.
- Produces: canonical option arrays and label functions, `CatalogDisplayState` Thai labels/classes, `GradeLevelMultiSelect` with bindable `value: string[]`, and a richer `CatalogVersionHistory` accepting `gradeLevelOptions`.

- [ ] **Step 1: Write failing shared-control policy tests**

Create source assertions that require shadcn primitives and reject UUID text entry:

```js
assert.match(history, /GradeLevelMultiSelect/);
assert.doesNotMatch(history, /รหัสระดับชั้น \(คั่นด้วยจุลภาค\)/);
assert.match(history, /\* as Select from '\$lib\/components\/ui\/select'/);
assert.match(multiselect, /\* as Popover/);
assert.match(multiselect, /\* as Command/);
assert.match(multiselect, /Checkbox/);
```

- [ ] **Step 2: Run the new test and verify RED**

Run:

```bash
node --test frontend-school/tests/static/academic-catalog-ui.test.mjs
```

Expected: missing files/imports and legacy UUID input still present.

- [ ] **Step 3: Add catalog presentation constants**

Define typed choices and fallbacks:

```ts
export const SUBJECT_TYPE_OPTIONS = [
	{ value: 'BASIC', label: 'รายวิชาพื้นฐาน' },
	{ value: 'ADDITIONAL', label: 'รายวิชาเพิ่มเติม' },
	{ value: 'ACTIVITY', label: 'กิจกรรม' }
] as const;

export const ACTIVITY_TYPE_OPTIONS = [
	{ value: 'guidance', label: 'แนะแนว' },
	{ value: 'scout', label: 'ลูกเสือ / เนตรนารี / ยุวกาชาด' },
	{ value: 'club', label: 'ชุมนุม' },
	{ value: 'social', label: 'กิจกรรมเพื่อสังคม' },
	{ value: 'other', label: 'กิจกรรมอื่น ๆ' }
] as const;

export const SCHEDULING_MODE_OPTIONS = [
	{ value: 'synchronized', label: 'จัดพร้อมกัน' },
	{ value: 'independent', label: 'จัดแยกเวลาได้' }
] as const;
```

Add `optionLabel`, `displayStateLabel`, `displayStateClass`, Thai date-range formatting, and normalized search helpers.

- [ ] **Step 4: Build the accessible grade-level multi-select**

Use a bindable array, searchable Command list, keyed options, and Checkbox selection. The trigger summarizes `ทุกระดับชั้น`, one or two short labels, or `เลือกแล้ว N ระดับ`; selected UUIDs never render. Provide `aria-label`, `disabled`, empty-search text, and keyboard-capable shadcn primitives.

- [ ] **Step 5: Refactor version creation controls**

Replace classification text Input with shadcn Select choices based on `kind`. Replace comma-separated grade IDs with `GradeLevelMultiSelect`. Replace both native date Inputs with shared `DatePicker`, using `clearable` only for `effectiveUntil`. Extend history items to show classification, grade labels, and effective-state context without changing publish behavior.

- [ ] **Step 6: Run Svelte autofixer and focused tests**

Run serially:

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic-core/GradeLevelMultiSelect.svelte --svelte-version 5
```

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic-core/CatalogVersionHistory.svelte --svelte-version 5
```

```bash
node --test tests/static/academic-catalog-ui.test.mjs
```

Expected: autofixer reports no unresolved issue and test passes.

- [ ] **Step 7: Commit shared catalog controls**

```bash
git add frontend-school/src/lib/academic-core/catalog-presentation.ts frontend-school/src/lib/components/academic-core/GradeLevelMultiSelect.svelte frontend-school/src/lib/components/academic-core/CatalogVersionHistory.svelte frontend-school/tests/static/academic-catalog-ui.test.mjs
git commit -m "feat: standardize academic catalog controls"
```

### Task 4: Responsive subject catalog workspace

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/catalog/subjects/+page.svelte`
- Modify: `frontend-school/tests/static/academic-catalog-ui.test.mjs`
- Modify: `frontend-school/tests/static/frontend-state-components.test.mjs`

**Interfaces:**
- Consumes: `getCatalogSubjectOverview`, existing version/write APIs, Task 3 presentation helpers and `CatalogVersionHistory`, local Table/Sheet/Select/Input/Badge/Button components.
- Produces: searchable/filterable subject desktop table, mobile cards, on-demand cached history Sheet, and permission-gated create flow.

- [ ] **Step 1: Extend tests for approved subject information architecture**

Require generated overview use, table columns, mobile cards, Sheet, filters, and cache:

```js
assert.match(subjects, /getCatalogSubjectOverview/);
assert.match(subjects, /\* as Table/);
assert.match(subjects, /\* as Sheet/);
assert.match(subjects, /subjectHistoryCache/);
for (const label of ['ชื่อรายวิชา', 'ประเภท', 'ระดับชั้น', 'หน่วยกิต', 'สถานะ']) {
	assert.match(subjects, new RegExp(label));
}
```

- [ ] **Step 2: Run focused test and verify RED**

```bash
node --test frontend-school/tests/static/academic-catalog-ui.test.mjs
```

Expected: subject page still uses the old aside/master-detail layout.

- [ ] **Step 3: Implement overview state, filters, and history cache**

Use `$state.raw` for API envelopes and version arrays, `$derived.by` for filter/sort results, and a non-reactive `Map<string, SubjectVersion[]>` with a reactive selected copy. Search code/Thai/English name; filter type, grade ID, and display state. Sort by Thai-aware code/name comparison. `openSubject` loads once unless invalidated after create/publish.

- [ ] **Step 4: Build responsive table/cards and management Sheet**

Render semantic Table on `md` and larger, with a code spine and monospace code. Render cards below `md`, preserving code, name, type, grades, credit, state, and draft badge order. Use a Sheet sized for history and version creation. Empty state distinguishes no catalog records from no filter matches. Keep add-code form permission-gated and refresh overview after success.

- [ ] **Step 5: Run Svelte autofixer and focused tests GREEN**

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/catalog/subjects/+page.svelte' --svelte-version 5
```

```bash
node --test tests/static/academic-catalog-ui.test.mjs tests/static/frontend-state-components.test.mjs
```

- [ ] **Step 6: Commit subject workspace**

```bash
git add 'frontend-school/src/routes/(app)/staff/academic/catalog/subjects/+page.svelte' frontend-school/tests/static/academic-catalog-ui.test.mjs frontend-school/tests/static/frontend-state-components.test.mjs
git commit -m "feat: redesign subject catalog workspace"
```

### Task 5: Responsive activity catalog workspace

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/catalog/activities/+page.svelte`
- Modify: `frontend-school/tests/static/academic-catalog-ui.test.mjs`
- Modify: `frontend-school/tests/static/frontend-state-components.test.mjs`

**Interfaces:**
- Consumes: `getCatalogActivityOverview`, existing version/write APIs, Task 3 helpers, `CatalogVersionHistory`, and the same shadcn layout primitives as Task 4.
- Produces: searchable/filterable activity desktop table, mobile cards, cached history Sheet, and shadcn activity-type create control.

- [ ] **Step 1: Extend tests for approved activity information architecture**

Require overview API, Table/Sheet, history cache, shadcn activity type, and columns:

```js
assert.match(activities, /getCatalogActivityOverview/);
assert.match(activities, /activityHistoryCache/);
assert.match(activities, /ACTIVITY_TYPE_OPTIONS/);
for (const label of ['ชื่อกิจกรรม', 'ประเภทกิจกรรม', 'รูปแบบการจัด', 'ระดับชั้น', 'ชั่วโมง', 'สถานะ']) {
	assert.match(activities, new RegExp(label));
}
```

- [ ] **Step 2: Run focused test and verify RED**

```bash
node --test frontend-school/tests/static/academic-catalog-ui.test.mjs
```

- [ ] **Step 3: Implement activity overview state and responsive UI**

Mirror the approved interaction model without creating a generic data-grid abstraction. Search code/name/description; filter activity type, scheduling mode, grade ID, and display state. Use labeled Select options for new activity type and filters. Table/cards show display-version details and do not expose raw canonical strings when a Thai label exists.

- [ ] **Step 4: Implement cached history management and mutations**

Load history on first open, cache by catalog ID, and scope loading/error state to the Sheet. After creating or publishing a version, invalidate that ID, reload its history, and refresh the overview once. Preserve manage permission checks and stable code creation.

- [ ] **Step 5: Run Svelte autofixer and focused tests GREEN**

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/catalog/activities/+page.svelte' --svelte-version 5
```

```bash
node --test tests/static/academic-catalog-ui.test.mjs tests/static/frontend-state-components.test.mjs
```

- [ ] **Step 6: Commit activity workspace**

```bash
git add 'frontend-school/src/routes/(app)/staff/academic/catalog/activities/+page.svelte' frontend-school/tests/static/academic-catalog-ui.test.mjs frontend-school/tests/static/frontend-state-components.test.mjs
git commit -m "feat: redesign activity catalog workspace"
```

### Task 6: Compact academic-context topbar

**Files:**
- Modify: `frontend-school/src/lib/components/layout/AcademicContextSwitcher.svelte`
- Modify: `frontend-school/tests/static/academic-context-contract.test.mjs`

**Interfaces:**
- Consumes: existing academic-context store, Select, Sheet, Badge, and dirty-form confirmation.
- Produces: compact closed triggers without visible context/status text and unchanged option status badges.

- [ ] **Step 1: Add failing topbar presentation assertions**

Parse the desktop trigger region separately from Select content and assert:

```js
assert.doesNotMatch(desktopTrigger, /บริบทงาน/);
assert.doesNotMatch(desktopTrigger, /statusLabels/);
assert.match(selectContent, /statusLabels/);
assert.match(selectContent, /Badge/);
```

- [ ] **Step 2: Run focused test and verify RED**

```bash
node --test frontend-school/tests/static/academic-context-contract.test.mjs
```

- [ ] **Step 3: Simplify closed triggers**

Remove the desktop `บริบทงาน` text and selected year/term status badges from Select triggers. Keep Calendar icon, year name, term name, option badges, loading/error states, mobile summary, dirty-form warning, and accessible `aria-label` text. Tighten min/max widths without hiding selected values.

- [ ] **Step 4: Run Svelte autofixer and test GREEN**

```bash
npx @sveltejs/mcp svelte-autofixer src/lib/components/layout/AcademicContextSwitcher.svelte --svelte-version 5
```

```bash
node --test tests/static/academic-context-contract.test.mjs
```

- [ ] **Step 5: Commit topbar change**

```bash
git add frontend-school/src/lib/components/layout/AcademicContextSwitcher.svelte frontend-school/tests/static/academic-context-contract.test.mjs
git commit -m "refactor: simplify academic context topbar"
```

### Task 7: Shadcn Calendar captions and DatePicker migration

**Files:**
- Modify: `frontend-school/src/lib/components/ui/calendar/calendar-caption.svelte`
- Delete: `frontend-school/src/lib/components/ui/calendar/calendar-month-select.svelte`
- Delete: `frontend-school/src/lib/components/ui/calendar/calendar-year-select.svelte`
- Modify: `frontend-school/src/lib/components/ui/calendar/index.ts`
- Modify: `frontend-school/src/lib/components/ui/date-picker/DatePicker.svelte`
- Modify: `frontend-school/src/lib/components/academic-core/StudentYearPlacementEditor.svelte`
- Modify: `frontend-school/src/lib/components/academic-core/StudentYearTransferDialog.svelte`
- Modify: `frontend-school/src/lib/components/academic-core/AcademicYearTermEditor.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/supervision/[id]/+page.svelte`
- Create: `frontend-school/tests/static/shadcn-input-policy.test.mjs`

**Interfaces:**
- Consumes: Calendar bindable `placeholder`, `DateValue`, locale/year/month formatters, shadcn Select, existing DatePicker value contract.
- Produces: Calendar caption with real shadcn month/year Selects; DatePicker props `disabled`, `required`, `clearable`, `ariaLabel`; zero raw HTML `select` and zero `input[type="date"]` in `frontend-school/src`.

- [ ] **Step 1: Write failing global UI policy tests**

Scan all application `.svelte` sources and fail on raw controls:

```js
for (const file of svelteFiles) {
	const source = readFileSync(file, 'utf8');
	assert.doesNotMatch(source, /<select(?:\s|>)/, `${file} must use shadcn Select`);
	assert.doesNotMatch(source, /type=["']date["']/, `${file} must use DatePicker`);
}
assert.match(calendarCaption, /\* as Select/);
assert.match(calendarCaption, /bind:placeholder/);
```

- [ ] **Step 2: Run policy test and verify RED**

```bash
node --test frontend-school/tests/static/shadcn-input-policy.test.mjs
```

Expected: two Calendar selects and thirteen native date fields are reported.

- [ ] **Step 3: Replace Calendar month/year controls**

Build numeric option arrays in `calendar-caption.svelte`; labels use `DateFormatter(locale, ...)`, values remain Gregorian numeric strings, and placeholder updates preserve `monthIndex`:

```ts
function selectMonth(value: string) {
	if (!placeholder) return;
	placeholder = placeholder.set({ month: Number(value) }).subtract({ months: monthIndex });
}

function selectYear(value: string) {
	if (!placeholder) return;
	placeholder = placeholder.set({ year: Number(value) }).subtract({ months: monthIndex });
}
```

Render `Select.Root`/`Trigger`/`Content`/`Item` for each configured caption segment. Mirror the current default year range and preserve caller-supplied `months`/`years`. Remove obsolete native-wrapper files and exports.

- [ ] **Step 4: Extend DatePicker for migrated fields**

Add bindable-safe disabled/required/clear behavior, accessible label forwarding, and a clear button only when `clearable && value && !disabled`. Do not put the clear button inside the Popover trigger. Invalid external ISO strings must render the placeholder instead of throwing during component initialization.

- [ ] **Step 5: Replace all remaining native date fields**

Import `DatePicker` into each listed component/page. Preserve required versus optional semantics and existing value strings. Use `clearable` for optional end/transfer/effective dates and omit it for required starts. Keep time inputs unchanged.

- [ ] **Step 6: Run Svelte autofixer on every edited component**

Run `npx @sveltejs/mcp svelte-autofixer <path> --svelte-version 5` separately for:

1. `src/lib/components/ui/calendar/calendar-caption.svelte`
2. `src/lib/components/ui/date-picker/DatePicker.svelte`
3. `src/lib/components/academic-core/StudentYearPlacementEditor.svelte`
4. `src/lib/components/academic-core/StudentYearTransferDialog.svelte`
5. `src/lib/components/academic-core/AcademicYearTermEditor.svelte`
6. `src/routes/(app)/staff/academic/supervision/[id]/+page.svelte`

Expected: no unresolved Svelte correctness issue.

- [ ] **Step 7: Run policy and relevant static tests GREEN**

```bash
node --test tests/static/shadcn-input-policy.test.mjs tests/static/academic-catalog-ui.test.mjs tests/static/academic-context-contract.test.mjs
```

- [ ] **Step 8: Commit Calendar and DatePicker standardization**

```bash
git add frontend-school/src/lib/components/ui/calendar frontend-school/src/lib/components/ui/date-picker/DatePicker.svelte frontend-school/src/lib/components/academic-core/StudentYearPlacementEditor.svelte frontend-school/src/lib/components/academic-core/StudentYearTransferDialog.svelte frontend-school/src/lib/components/academic-core/AcademicYearTermEditor.svelte 'frontend-school/src/routes/(app)/staff/academic/supervision/[id]/+page.svelte' frontend-school/tests/static/shadcn-input-policy.test.mjs
git commit -m "refactor: standardize shadcn date controls"
```

### Task 8: Full verification, visual critique, and delivery readiness

**Files:**
- Modify if required by verified findings: only files already in Tasks 1–7
- Review: `docs/superpowers/specs/2026-08-27-academic-catalog-ui-standardization-design.md`
- Review: `docs/superpowers/plans/2026-08-27-academic-catalog-ui-standardization.md`

**Interfaces:**
- Consumes: completed Tasks 1–7.
- Produces: verified, review-ready commits with clean generated contracts and no unrelated changes.

- [ ] **Step 1: Run focused backend tests**

```bash
cargo test catalog_overview --manifest-path backend-school/Cargo.toml
```

- [ ] **Step 2: Run backend format and compile/lint matrix serially**

```bash
cargo fmt --manifest-path backend-school/Cargo.toml -- --check
```

```bash
cargo check --manifest-path backend-school/Cargo.toml
```

```bash
cargo clippy --manifest-path backend-school/Cargo.toml --all-targets --all-features -- -D warnings
```

- [ ] **Step 3: Run backend test matrix**

```bash
cargo test --manifest-path backend-school/Cargo.toml --all-targets --all-features
```

- [ ] **Step 4: Run frontend contract and static verification serially**

From `frontend-school`:

```bash
npm run check:api-contracts
```

```bash
npm run check:permissions
```

```bash
npm run test:static
```

- [ ] **Step 5: Run frontend formatting/type verification serially**

```bash
npm run lint
```

```bash
npm run check
```

- [ ] **Step 6: Build frontend**

```bash
npm run build
```

- [ ] **Step 7: Perform frontend-design critique**

Inspect rendered subject/activity pages at desktop and mobile widths if an authenticated local session is available. Verify the code spine communicates stable identity, table/card hierarchy is identical, badges do not dominate names, Thai labels do not wrap ambiguously, filters remain reachable, dark mode has sufficient contrast, focus rings remain visible, and Calendar nested Select content is not clipped. If runtime access is unavailable, report that limitation rather than claiming visual verification.

- [ ] **Step 8: Review repository state and request code review**

```bash
git diff --check c4e19b56..HEAD
```

```bash
git status --short --branch
```

Use `superpowers:requesting-code-review`, address verified findings, rerun affected checks, and commit only necessary corrections.

- [ ] **Step 9: Finish branch and report rollout state**

Use `superpowers:verification-before-completion` and `superpowers:finishing-a-development-branch`. Report commit IDs, exact verification results, whether visual/authenticated runtime checks ran, and whether changes remain local or were explicitly pushed.
