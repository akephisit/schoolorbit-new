# Daily Teaching Synchronized Activity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render all classroom entries for one synchronized activity as one compact card in each teacher-period cell while retaining complete classroom details.

**Architecture:** Extend the typed daily-teaching response with existing activity-slot identity and scheduling-mode data, then consume the generated OpenAPI DTO in the frontend. A pure display helper groups only synchronized entries sharing an activity slot; the Svelte page uses those groups in both the compact table and detail dialog while filters and summary metrics continue using raw entries.

**Tech Stack:** Rust, Axum, SQLx, Utoipa/OpenAPI, TypeScript, SvelteKit 5, Svelte runes, Node test runner, Tailwind CSS.

## Global Constraints

- Never edit an applied migration; this change requires no migration.
- The school OpenAPI contract and generated TypeScript DTO own the changed wire shape.
- Generated OpenAPI and TypeScript artifacts must be produced by `npm run generate:api-contracts`, never edited by hand.
- Only `ACTIVITY` entries with `activitySchedulingMode === 'synchronized'` and a non-empty `activitySlotId` may merge.
- Course entries, independent activities, entries with incomplete metadata, filters, summary metrics, permissions, and realtime behavior remain unchanged.
- The compact card shows the activity title and unique classroom count; the dialog retains the complete deduplicated classroom/room list.

---

## File Structure

- Modify `backend-school/src/modules/academic/services/daily_teaching_service.rs`: expose activity identity/mode, load them from the existing tables, and protect mapping behavior with unit tests.
- Modify `backend-school/src/modules/academic/handlers/timetable.rs`: document the daily-teaching route in Utoipa.
- Modify `backend-school/src/api_contract.rs`: register the route and daily-teaching schemas.
- Modify `frontend-school/tests/static/api-response-contract.test.mjs`: assert generated contract ownership for the daily-teaching endpoint and its new fields.
- Generate `contracts/openapi/school-api.json` and `frontend-school/src/lib/api/generated/school-api.ts`: update tracked wire artifacts.
- Modify `frontend-school/src/lib/api/timetable.ts`: replace handwritten daily-teaching wire interfaces with generated schema aliases.
- Create `frontend-school/src/lib/utils/daily-teaching-display.ts`: own pure, ordered grouping and classroom-label derivation.
- Create `frontend-school/tests/static/daily-teaching-display.test.mjs`: protect grouping behavior with real helper tests.
- Modify `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte`: render groups in the table and dialog.

---

### Task 1: Carry synchronized activity identity through the backend response

**Files:**
- Modify: `backend-school/src/modules/academic/services/daily_teaching_service.rs`

**Interfaces:**
- Consumes: existing `academic_timetable_entries.activity_slot_id`, `activity_slots.activity_catalog_id`, and `activity_catalog.scheduling_mode` columns.
- Produces: nullable `DailyTeachingEntry.activity_slot_id: Option<Uuid>` and `DailyTeachingEntry.activity_scheduling_mode: Option<String>` serialized as `activitySlotId` and `activitySchedulingMode`.

- [ ] **Step 1: Write the failing service test**

Add a focused unit test beside the existing `build_daily_teaching_overview` tests. Construct one activity seed with a slot ID and synchronized mode, then assert both values survive mapping:

```rust
#[test]
fn build_overview_preserves_synchronized_activity_identity() {
    let period_id = id(1);
    let semester_id = id(2);
    let teacher_id = id(10);
    let activity_slot_id = id(20);

    let overview = build_daily_teaching_overview(
        NaiveDate::from_ymd_opt(2026, 8, 5).unwrap(),
        "WED".to_string(),
        semester_id,
        vec![DailyTeachingPeriod {
            id: period_id,
            name: Some("คาบ 8".to_string()),
            start_time: NaiveTime::from_hms_opt(14, 40, 0).unwrap(),
            end_time: NaiveTime::from_hms_opt(15, 30, 0).unwrap(),
            order_index: 8,
        }],
        vec![DailyTeachingTeacherSeed {
            id: teacher_id,
            display_name: "ครูกิจกรรม".to_string(),
            subject_group_names: vec![],
            sort_order: 0,
        }],
        vec![DailyTeachingEntrySeed {
            teacher_id,
            period_id,
            entry_id: id(30),
            entry_type: "ACTIVITY".to_string(),
            subject_code: None,
            subject_name: None,
            subject_group_name: None,
            classroom_name: Some("ป.1/1".to_string()),
            room_code: None,
            title: Some("ลูกเสือ เนตรนารี".to_string()),
            note: None,
            instructor_count: 1,
            period_order_index: 8,
            activity_slot_id: Some(activity_slot_id),
            activity_scheduling_mode: Some("synchronized".to_string()),
        }],
        false,
    );

    let entry = &overview.teachers[0].periods[0].entries[0];
    assert_eq!(entry.activity_slot_id, Some(activity_slot_id));
    assert_eq!(entry.activity_scheduling_mode.as_deref(), Some("synchronized"));
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run from `backend-school`:

```bash
cargo test modules::academic::services::daily_teaching_service::tests::build_overview_preserves_synchronized_activity_identity --bin backend-school
```

Expected: compilation fails because `DailyTeachingEntrySeed` and `DailyTeachingEntry` do not yet define the activity identity fields.

- [ ] **Step 3: Implement the minimal backend mapping and query joins**

Add the two nullable fields to both entry structs and copy them in `entry_from_seed`. In `list_daily_entries`, select `te.activity_slot_id` and `ac.scheduling_mode AS activity_scheduling_mode`, with joins that keep non-activity entries:

```sql
LEFT JOIN activity_slots activity_slot ON activity_slot.id = te.activity_slot_id
LEFT JOIN activity_catalog ac ON ac.id = activity_slot.activity_catalog_id
```

Update every existing `DailyTeachingEntrySeed` test fixture with `activity_slot_id: None` and `activity_scheduling_mode: None`. Do not change grouping or summary calculations.

- [ ] **Step 4: Run focused backend tests and verify GREEN**

Run from `backend-school`:

```bash
cargo test modules::academic::services::daily_teaching_service::tests --bin backend-school
```

Expected: all daily-teaching service tests pass, including the new identity test.

- [ ] **Step 5: Commit the backend data change**

```bash
git add backend-school/src/modules/academic/services/daily_teaching_service.rs
git commit -m "feat: expose daily activity grouping identity"
```

---

### Task 2: Register the generated daily-teaching API contract

**Files:**
- Modify: `backend-school/src/modules/academic/services/daily_teaching_service.rs`
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `frontend-school/tests/static/api-response-contract.test.mjs`
- Generate: `contracts/openapi/school-api.json`
- Generate: `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/timetable.ts`

**Interfaces:**
- Consumes: Task 1's two serialized entry fields.
- Produces: OpenAPI operation `getDailyTeachingOverview`, generated schemas `DailyTeachingOverview`, `DailyTeachingTeacher`, `DailyTeachingPeriodCell`, `DailyTeachingEntry`, `DailyTeachingPeriod`, and `DailyTeachingSummary`, plus frontend aliases with those exact schema names.

- [ ] **Step 1: Strengthen the contract test and verify RED**

Replace the handwritten-interface assertions in the existing `daily teaching overview API uses typed response contracts` test with assertions that:

```javascript
assert.equal(
	contract.paths['/api/academic/timetable/daily-teaching'].get.operationId,
	'getDailyTeachingOverview'
);
assert.match(generated, /DailyTeachingEntry:[\s\S]*?activitySlotId:\s*string \| null/);
assert.match(generated, /DailyTeachingEntry:[\s\S]*?activitySchedulingMode:/);
assert.doesNotMatch(frontendTimetableApi, /interface\s+DailyTeachingEntry/);
assert.match(
	frontendTimetableApi,
	/export\s+type\s+DailyTeachingEntry\s*=\s*Schemas\['DailyTeachingEntry'\]/
);
```

Read `contracts/openapi/school-api.json` and `frontend-school/src/lib/api/generated/school-api.ts` at the start of that test, following the neighboring generated-contract tests.

Run from `frontend-school`:

```bash
node --test tests/static/api-response-contract.test.mjs
```

Expected: FAIL because the route and schemas are not registered and the API module still owns handwritten interfaces.

- [ ] **Step 2: Add Utoipa derives and the route annotation**

In the service, derive `IntoParams` for `DailyTeachingQuery` with query placement and derive `ToSchema` for all public daily-teaching response structs. Describe the string scheduling mode with the existing `ActivitySchedulingMode` schema while retaining `Option<String>` as the SQL-facing Rust field.

Annotate `daily_teaching_overview` in the handler:

```rust
#[utoipa::path(
    get,
    path = "/api/academic/timetable/daily-teaching",
    operation_id = "getDailyTeachingOverview",
    tag = "academic",
    params(daily_teaching_service::DailyTeachingQuery),
    responses(
        (status = 200, description = "Daily teaching overview", body = ApiResponse<daily_teaching_service::DailyTeachingOverview>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Daily teaching permission required", body = ApiErrorResponse)
    )
)]
```

- [ ] **Step 3: Register the route and schemas**

Import the six public response structs into `backend-school/src/api_contract.rs`. The query type is referenced by the handler annotation and does not belong in `components(schemas(...))`. Add the handler to `paths(...)` and add the response structs plus `ApiResponse<DailyTeachingOverview>` to `components(schemas(...))`. Do not add a second route or duplicate schema entry.

- [ ] **Step 4: Generate the contract artifacts**

Run from `frontend-school`:

```bash
npm run generate:api-contracts
```

Expected: PASS and update only the tracked school API JSON and generated TypeScript file for this endpoint/schema change.

- [ ] **Step 5: Consume generated daily-teaching schema aliases**

In `frontend-school/src/lib/api/timetable.ts`, replace the six handwritten daily-teaching interfaces with aliases:

```typescript
export type DailyTeachingPeriod = Schemas['DailyTeachingPeriod'];
export type DailyTeachingEntry = Schemas['DailyTeachingEntry'];
export type DailyTeachingPeriodCell = Schemas['DailyTeachingPeriodCell'];
export type DailyTeachingTeacher = Schemas['DailyTeachingTeacher'];
export type DailyTeachingSummary = Schemas['DailyTeachingSummary'];
export type DailyTeachingOverview = Schemas['DailyTeachingOverview'];
```

Keep `getDailyTeachingOverview` returning `LoadedApiResponse<DailyTeachingOverview>` and keep its existing query parameter behavior.

- [ ] **Step 6: Run contract tests and verify GREEN**

Run from `frontend-school`:

```bash
npm run check:api-contracts
npm run test:api-contracts
node --test tests/static/api-response-contract.test.mjs
```

Expected: all commands pass and the generated schema exposes both nullable grouping fields.

- [ ] **Step 7: Commit the contract change**

```bash
git add backend-school/src/modules/academic/services/daily_teaching_service.rs backend-school/src/modules/academic/handlers/timetable.rs backend-school/src/api_contract.rs frontend-school/tests/static/api-response-contract.test.mjs contracts/openapi/school-api.json frontend-school/src/lib/api/generated/school-api.ts frontend-school/src/lib/api/timetable.ts
git commit -m "feat: generate daily teaching response contract"
```

---

### Task 3: Build and test the synchronized-entry display helper

**Files:**
- Create: `frontend-school/src/lib/utils/daily-teaching-display.ts`
- Create: `frontend-school/tests/static/daily-teaching-display.test.mjs`

**Interfaces:**
- Consumes: generated-backed `DailyTeachingEntry` from Task 2.
- Produces: `DailyTeachingDisplayGroup`, `groupDailyTeachingEntries(entries)`, and `displayGroupCountLabel(group)` for the Svelte page.

- [ ] **Step 1: Write failing grouping tests**

Create complete entry fixtures containing every generated `DailyTeachingEntry` field. Add separate tests proving:

```javascript
test('merges synchronized entries that share one activity slot', () => {
	const groups = groupDailyTeachingEntries([
		entry({ entryId: 'entry-a', classroomName: 'ป.1/1' }),
		entry({ entryId: 'entry-b', classroomName: 'ป.1/2' })
	]);

	assert.equal(groups.length, 1);
	assert.deepEqual(groups[0].entries.map((item) => item.entryId), ['entry-a', 'entry-b']);
	assert.deepEqual(groups[0].classroomLabels, ['ป.1/1', 'ป.1/2']);
	assert.equal(displayGroupCountLabel(groups[0]), '2 ห้อง');
});
```

Use additional tests for different slot IDs, `independent`, null metadata, stable first-occurrence order, and duplicate `classroomName`/`roomCode` labels. The shared fixture defaults to `entryType: 'ACTIVITY'`, `activitySlotId: 'slot-a'`, `activitySchedulingMode: 'synchronized'`, and uses null for optional course fields.

- [ ] **Step 2: Run helper tests and verify RED**

Run from `frontend-school`:

```bash
node --test tests/static/daily-teaching-display.test.mjs
```

Expected: FAIL with module-not-found because the display helper does not exist.

- [ ] **Step 3: Implement the pure grouping helper**

Define:

```typescript
export interface DailyTeachingDisplayGroup {
	key: string;
	entries: DailyTeachingEntry[];
	isSynchronizedActivity: boolean;
	classroomLabels: string[];
}

export function groupDailyTeachingEntries(
	entries: DailyTeachingEntry[]
): DailyTeachingDisplayGroup[];

export function displayGroupCountLabel(group: DailyTeachingDisplayGroup): string;
```

Use a `Map<string, DailyTeachingDisplayGroup>` only for synchronized slot keys. Push non-groupable entries directly with `entry:${entry.entryId}` keys. Build labels as `classroomName / roomCode` when both exist, otherwise the non-empty value, and deduplicate with insertion order. `displayGroupCountLabel` returns `N ห้อง` when labels exist, `N รายการ` for a multi-entry group without labels, and an empty string for a single ungrouped entry.

- [ ] **Step 4: Run helper tests and verify GREEN**

Run from `frontend-school`:

```bash
node --test tests/static/daily-teaching-display.test.mjs
```

Expected: all grouping, fallback, ordering, and deduplication tests pass.

- [ ] **Step 5: Commit the display model**

```bash
git add frontend-school/src/lib/utils/daily-teaching-display.ts frontend-school/tests/static/daily-teaching-display.test.mjs
git commit -m "feat: group synchronized daily activities"
```

---

### Task 4: Render compact groups in the daily teaching table and dialog

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte`

**Interfaces:**
- Consumes: `groupDailyTeachingEntries` and `displayGroupCountLabel` from Task 3.
- Produces: one compact table card and one detail section per display group.

- [ ] **Step 1: Integrate display groups into the table**

Import the helper functions. For each non-empty cell, derive `displayGroups = groupDailyTeachingEntries(cell.entries)` and iterate groups keyed by `group.key`. Use `group.entries[0]` as the representative entry.

For a synchronized group, render:

```svelte
<Badge variant="secondary">พร้อมกัน</Badge>
<p class="line-clamp-2 text-sm font-medium">{entryTitle(representativeEntry)}</p>
<p class="text-muted-foreground mt-1 truncate text-xs">
	{displayGroupCountLabel(group)}
</p>
```

Keep the existing type and team-teaching badges. For an ungrouped entry, retain the current subject code, subject name, and `entryMeta` presentation.

- [ ] **Step 2: Integrate the same groups into the dialog**

Derive display groups from `selectedCell.entries`. Render one detail card per group. For synchronized groups, show the representative title once and render all `group.classroomLabels` in a compact list under a `ชั้น/ห้อง` label. If the labels are empty, show `displayGroupCountLabel(group)` instead. For ungrouped entries, retain the current four-field detail grid and note.

- [ ] **Step 3: Validate the Svelte component**

Run from `frontend-school`:

```bash
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/timetable/today/+page.svelte' --svelte-version 5
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: the autofixer reports no issues introduced by the change and Svelte/TypeScript check passes. Existing unrelated autofixer suggestions may be reported but must not be expanded into unrelated refactoring.

- [ ] **Step 4: Re-run the focused display and contract tests**

Run from `frontend-school`:

```bash
node --test tests/static/daily-teaching-display.test.mjs
node --test tests/static/api-response-contract.test.mjs
```

Expected: both focused files pass.

- [ ] **Step 5: Commit the Svelte presentation**

```bash
git add 'frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte'
git commit -m "fix: compact synchronized daily activities"
```

---

### Task 5: Run the applicable verification matrix

**Files:**
- Review all files changed in Tasks 1-4.

**Interfaces:**
- Consumes: completed backend contract, frontend grouping helper, and Svelte presentation.
- Produces: evidence that focused behavior and all applicable repository gates pass.

- [ ] **Step 1: Run backend-school verification**

Run from `backend-school`:

```bash
cargo fmt --all -- --check
cargo test modules::academic::services::daily_teaching_service::tests --bin backend-school
cargo test api_contract::tests -- --nocapture
cargo test --test static_architecture
cargo check
```

Expected: all commands pass.

- [ ] **Step 2: Run API-contract verification**

Run from `frontend-school`:

```bash
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
```

Expected: all commands pass and generation leaves the tracked contract artifacts unchanged.

- [ ] **Step 3: Run frontend-school verification**

Run from `frontend-school`:

```bash
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

Expected: all commands pass.

- [ ] **Step 4: Review repository state**

Run from the repository root:

```bash
git diff --check
git status --short
git log -6 --oneline
```

Review every implementation diff against the design. Expected: no whitespace errors, no uncommitted generated drift, no migration changes, and only the planned commits follow the design and plan commits.
