# Academic Operational Change Release 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the timetable-version workspace so staff can create a dated revision, edit only its draft entries, review per-group completion and conflicts, acknowledge warnings, and publish the version without inventing an operational add/stop item.

**Architecture:** Reuse `academic_term_change_sets` as the only publication boundary and keep `academic_timetable_versions` as its dated schedule snapshot. A schedule-only revision becomes publishable only after its cloned target differs from the base; unchanged drafts remain blocked. Extract the existing readiness/publication UI into one shared component used by Delivery and Timetable, then connect the timetable page to the existing typed change-set APIs without adding a second workflow or compatibility path.

**Tech Stack:** Rust, Axum, SQLx/PostgreSQL, Utoipa/OpenAPI, TypeScript, Svelte 5, shadcn-svelte/Bits UI, Node test runner, Playwright.

**Spec:** `docs/superpowers/specs/2026-08-30-academic-operational-change-and-timetable-versioning-design.md`

## Global Constraints

- Run every local command serially; never overlap Rust and frontend work.
- Do not add drag-and-drop, automatic scheduling, alternating-week patterns, or teacher replacement.
- Published timetable versions stay immutable; only a draft with an explicit `timetableVersionId` exposes mutations.
- Creating a revision requires one effective-from date and one reason and creates a linked term change set.
- A schedule-only revision may publish only when the target version differs semantically from its base.
- Target deficits block publication; target excesses require acknowledgement by stable warning code.
- Read-only users must not request management-only options or see mutation/publication controls.
- Use only typed Rust DTOs, generated TypeScript contracts, standard API envelopes, row versions, and existing permissions.
- Do not add a migration: Release 3 uses the Release 1/2 schema and service boundaries already deployed.
- Preserve historical versions, offerings, groups, teachers, rosters, assessments, results, and supervision references.

---

### Task 1: Permit a real schedule-only revision while blocking an unchanged clone

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/services/change_sets.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`

**Interfaces:**
- Consumes: `target_is_pristine(transaction, change_set_id, base_version_id, target_version_id) -> Result<bool, AppError>` and the existing typed change-set preview/publication services.
- Produces: `ChangeSetNoItems` only when both `academic_term_change_items` is empty and the cloned timetable target is still pristine; an edited target is a valid schedule-only change set.

- [ ] **Step 1: Write the failing schedule-only service test**

Add a test named `schedule_only_change_set_can_preview_and_publish_after_a_draft_entry_changes`. Create a runtime change set, update one cloned entry through `timetable_service::update_entry`, and assert the typed preview no longer contains `ChangeSetNoItems`:

```rust
let entry_id: Uuid = sqlx::query_scalar(
    "SELECT id FROM academic_timetable_entries \
     WHERE timetable_version_id = $1 AND is_active ORDER BY id LIMIT 1",
)
.bind(change_set.target_timetable_version_id)
.fetch_one(&pool)
.await
.expect("fixture draft entry");
let entry = timetable_service::get_entry(&pool, entry_id)
    .await
    .expect("typed draft entry");

timetable_service::update_entry(
    &pool,
    entry.id,
    context.teacher_id,
    UpdateTimetableEntryRequest {
        timetable_version_id: change_set.target_timetable_version_id,
        row_version: entry.row_version,
        day_of_week: None,
        bell_schedule_period_id: None,
        room_id: None,
        clear_room: None,
        note: Some("ปรับหมายเหตุในรุ่นใหม่".to_string()),
        clear_note: None,
        title: None,
    },
)
.await?;

let preview = change_sets::preview_change_set(&pool, change_set.id).await?;
assert!(!preview.findings.iter().any(|finding| {
    finding.code == AcademicChangeFindingCode::ChangeSetNoItems
}));
```

Collect the current warning codes, publish with the returned row versions/hash, and assert the change set and target version are both `published` while the source remains unchanged.

- [ ] **Step 2: Run the new test and verify RED**

Run:

```bash
cd backend-school
cargo test modules::academic::delivery::services_tests::schedule_only_change_set_can_preview_and_publish_after_a_draft_entry_changes -- --exact --nocapture --test-threads=1
```

Expected: FAIL because `ChangeSetNoItems` is still blocking after the timetable entry changes.

- [ ] **Step 3: Make the no-change finding depend on target pristine state**

In `build_preview_in_transaction`, calculate the existing semantic clone state after loading the items:

```rust
let target_pristine = target_is_pristine(
    transaction,
    change_set.id,
    base_version_id,
    target_version_id,
)
.await?;

if items.is_empty() && target_pristine {
    findings.push(change_finding(
        AcademicChangeFindingCode::ChangeSetNoItems,
        AcademicChangeFindingSeverity::Blocking,
        "ยังไม่มีการเปลี่ยนแปลง",
        "แก้ตารางในรุ่นแบบร่าง หรือเพิ่มรายการเปลี่ยนแปลงอย่างน้อยหนึ่งรายการก่อนเผยแพร่",
        1,
        None,
        None,
        Some(change_set.id),
    ));
}
```

Do not weaken target, group, teacher, roster, conflict, deficit, excess, preview-hash, row-version, or idempotency checks.

- [ ] **Step 4: Verify GREEN and preserve unchanged-clone behavior**

Run the new exact test, then run:

```bash
cd backend-school
cargo test modules::academic::delivery::services_tests::change_set_preview_blocks_an_empty_change_set_with_a_stable_hash -- --exact --nocapture --test-threads=1
```

Expected: both PASS. The first proves a real draft edit is publishable; the second proves an untouched clone is still blocked.

- [ ] **Step 5: Commit the backend behavior**

```bash
git add backend-school/src/modules/academic/delivery/services/change_sets.rs backend-school/src/modules/academic/delivery/services_tests.rs
git commit -m "feat(timetable): allow schedule-only version publication"
```

---

### Task 2: Extract one shared readiness and publication surface

**Files:**
- Create: `frontend-school/src/lib/components/learning-delivery/AcademicChangeReadiness.svelte`
- Modify: `frontend-school/src/lib/components/learning-delivery/AcademicChangeSetPanel.svelte`
- Modify: `frontend-school/tests/static/academic-operational-change.test.mjs`
- Modify: `frontend-school/tests/e2e/academic-operational-change.spec.ts`

**Interfaces:**
- Consumes: `AcademicTermChangeSet`, `AcademicTermChangeSetPreview`, `getAcademicTermChangeSet`, `previewAcademicTermChangeSet`, `publishAcademicTermChangeSet`, and `cancelAcademicTermChangeSet` from `$lib/api/learning-delivery`.
- Produces: reusable component props `{ changeSet, canManage, onChanged }`; each parent owns a keyed revision counter that invalidates stale preview state after parent-owned draft mutations.

- [ ] **Step 1: Add a failing static ownership assertion**

Extend `academic-operational-change.test.mjs` so it requires both the Delivery panel and the new timetable workspace to consume `AcademicChangeReadiness`, and requires the shared component to own preview, warning acknowledgement, publish, cancel, impact counts, and schedule counts.

```js
assert.match(deliveryPanel, /AcademicChangeReadiness/);
assert.match(readiness, /previewAcademicTermChangeSet/);
assert.match(readiness, /acknowledgedWarningCodes/);
assert.match(readiness, /scheduleCounts/);
assert.match(readiness, /publishAcademicTermChangeSet/);
assert.match(readiness, /cancelAcademicTermChangeSet/);
```

- [ ] **Step 2: Run the static test and verify RED**

Run:

```bash
cd frontend-school
node --test tests/static/academic-operational-change.test.mjs
```

Expected: FAIL because `AcademicChangeReadiness.svelte` does not exist.

- [ ] **Step 3: Move readiness state and behavior into the shared component**

Create `AcademicChangeReadiness.svelte` with these props and state:

```ts
let {
    changeSet,
    canManage,
    onChanged
}: {
    changeSet: AcademicTermChangeSet;
    canManage: boolean;
    onChanged: (changeSet: AcademicTermChangeSet) => void | Promise<void>;
} = $props();

let preview = $state.raw<AcademicTermChangeSetPreview | null>(null);
let acknowledgedWarnings = $state<AcademicChangeFindingCode[]>([]);
```

Move the existing conflict-safe `refreshPreview`, `syncCurrentChangeSet`, `recoverFromConflict`, `publishChangeSet`, and `cancelDraft` behavior without changing request bodies. Deduplicate warnings by `finding.code`, aggregate `affectedCount`, and key warning rows by the stable code. Render:

- blocking findings and recovery links;
- one acknowledgement per warning code;
- per-group `actualPeriods/targetPeriods` rows with complete, deficit, and excess labels;
- impact counts without roster identities;
- disabled publication until no blocking finding remains and all current warning codes are acknowledged;
- cancellation only for manageable draft change sets.

Use a parent-owned revision counter as part of a `{#key}` boundary rather than state-writing `$effect` logic.

- [ ] **Step 4: Replace duplicated Delivery readiness markup**

Keep add/adjust/stop item editing in `AcademicChangeSetPanel.svelte`. Increment `readinessRevision` after each successful item upsert/delete and render:

```svelte
{#key `${changeSet.id}:${readinessRevision}`}
    <AcademicChangeReadiness
        {changeSet}
        {canManage}
        {onChanged}
    />
{/key}
```

Remove only state/functions/markup now owned by the shared component. Preserve the target-version link and all item controls.

- [ ] **Step 5: Verify the existing workflow remains green**

Run the focused static test, Svelte autofixer on both changed components, and the six mocked Playwright scenarios after deployment. Locally, `svelte-check` must report zero errors and zero warnings.

- [ ] **Step 6: Commit the shared review surface**

```bash
git add frontend-school/src/lib/components/learning-delivery/AcademicChangeReadiness.svelte frontend-school/src/lib/components/learning-delivery/AcademicChangeSetPanel.svelte frontend-school/tests/static/academic-operational-change.test.mjs frontend-school/tests/e2e/academic-operational-change.spec.ts
git commit -m "refactor(delivery): share academic change readiness"
```

---

### Task 3: Give the change-set dialog an explicit timetable-revision mode

**Files:**
- Modify: `frontend-school/src/lib/components/learning-delivery/AcademicChangeSetDialog.svelte`
- Modify: `frontend-school/tests/static/timetable-version-contract.test.mjs`

**Interfaces:**
- Consumes: existing `createAcademicTermChangeSet(request)` and `AcademicTermChangeSet` response.
- Produces: optional prop `purpose?: 'operational_change' | 'timetable_revision'`, defaulting to `operational_change`; the timetable variant still calls the same typed create endpoint.

- [ ] **Step 1: Add the failing timetable-dialog contract test**

Require a timetable mode with user-facing copy and the same typed change-set creation call:

```js
assert.match(dialog, /timetable_revision/);
assert.match(dialog, /สร้างรุ่นตารางสอนใหม่/);
assert.match(dialog, /วันที่เริ่มใช้รุ่นใหม่/);
assert.match(dialog, /createAcademicTermChangeSet/);
assert.doesNotMatch(dialog, /cloneTimetableVersion/);
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
cd frontend-school
node --test tests/static/timetable-version-contract.test.mjs
```

Expected: FAIL because the dialog has only operational-change copy.

- [ ] **Step 3: Add the explicit purpose prop and derived copy**

Add:

```ts
type ChangeSetPurpose = 'operational_change' | 'timetable_revision';
let { academicTermId, purpose = 'operational_change', onCreated }: Props = $props();
let isTimetableRevision = $derived(purpose === 'timetable_revision');
```

For `timetable_revision`, render trigger/title/description/field copy equivalent to:

- trigger and title: `สร้างรุ่นตารางสอนใหม่`;
- description: `คัดลอกรุ่นที่เผยแพร่ซึ่งมีผลในวันที่เลือก แล้วแก้เฉพาะแบบร่างชุดใหม่`;
- date label: `วันที่เริ่มใช้รุ่นใหม่`;
- reason placeholder: `เช่น ปรับตารางตั้งแต่สัปดาห์ที่ 8 ตามมติฝ่ายวิชาการ`;
- notice: prior published schedules remain unchanged and the curriculum is not changed.

Keep the existing operational-change copy byte-for-byte where practical so Delivery behavior does not drift.

- [ ] **Step 4: Verify GREEN and autofix the Svelte component**

Run the focused static test and:

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer src/lib/components/learning-delivery/AcademicChangeSetDialog.svelte --svelte-version 5
```

Expected: static test PASS and autofixer reports no issues/suggestions requiring another call.

- [ ] **Step 5: Commit the reusable creation dialog**

```bash
git add frontend-school/src/lib/components/learning-delivery/AcademicChangeSetDialog.svelte frontend-school/tests/static/timetable-version-contract.test.mjs
git commit -m "feat(timetable): add revision creation dialog"
```

---

### Task 4: Integrate revision creation, draft editing, and readiness into Timetable

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`
- Modify: `frontend-school/tests/static/timetable-version-contract.test.mjs`

**Interfaces:**
- Consumes: `AcademicChangeSetDialog`, `AcademicChangeReadiness`, `getAcademicTermChangeSet`, existing version list/entry APIs, and `TimetableVersion.changeSetId`.
- Produces: one URL-backed selected version workspace with `selectedChangeSet`, `draftRevision`, and a refresh path that preserves `timetableVersionId` after create/publish/cancel.

- [ ] **Step 1: Add failing workspace assertions**

Extend the static test to require:

```js
assert.match(page, /purpose="timetable_revision"/);
assert.match(page, /getAcademicTermChangeSet/);
assert.match(page, /AcademicChangeReadiness/);
assert.match(page, /changeSetId/);
assert.match(page, /draftRevision/);
assert.match(page, /selectedVersion\?\.status === 'draft'/);
assert.match(page, /timetableVersionId:\s*selectedVersion\.id/);
assert.doesNotMatch(page, /cloneTimetableVersion/);
```

- [ ] **Step 2: Run the focused test and verify RED**

Run the same focused static file. Expected: FAIL on the missing timetable revision workflow.

- [ ] **Step 3: Load the selected linked change set without request fan-out**

Add:

```ts
let selectedChangeSet = $state.raw<AcademicTermChangeSet | null>(null);
let draftRevision = $state(0);

async function loadSelectedChangeSet(version: TimetableVersion, signal?: AbortSignal) {
    return version.changeSetId
        ? getAcademicTermChangeSet(version.changeSetId, { signal })
        : Promise.resolve(null);
}
```

Load at most one change set for the selected version during initial load and version changes. Never request catalog/teacher/student/room management options for a read-only user merely to render readiness.

- [ ] **Step 4: Select a newly created target version**

Implement `handleRevisionCreated(changeSet)` to re-list versions, choose `changeSet.targetTimetableVersionId`, load its periods/entries/change set, synchronize `timetableVersionId` in the URL, and reset the entry editor. Do not call the standalone `cloneTimetableVersion` wrapper from this page.

- [ ] **Step 5: Invalidate readiness after every draft mutation**

After successful create, update/move, or deactivate operations:

```ts
await refreshEntries();
draftRevision += 1;
resetForm();
```

All mutation payloads must continue to use `selectedVersion.id`; published, cancelled, or missing versions remain read-only.

- [ ] **Step 6: Render the complete version workspace**

Add the timetable revision dialog to PageShell actions for authorized staff with a selected published version. Under the version selector:

- retain explicit current/upcoming/historical/draft labels and effective intervals;
- explain that only the selected draft may change;
- render the shared readiness panel for a linked draft;
- show per-group target completion and conflict/warning findings next to the weekly table;
- after publish/cancel, reload versions and keep the target version selected;
- keep the explicit form-based move controls and do not add drag-and-drop.

- [ ] **Step 7: Run Svelte analysis and focused frontend checks**

Run serially:

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/timetable/+page.svelte' --svelte-version 5
```

```bash
cd frontend-school
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

```bash
cd frontend-school
node --test tests/static/timetable-version-contract.test.mjs tests/static/academic-operational-change.test.mjs
```

Expected: autofixer clean, Svelte check 0 errors/0 warnings, focused static tests PASS.

- [ ] **Step 8: Commit the timetable workspace**

```bash
git add frontend-school/src/routes/'(app)'/staff/academic/timetable/+page.svelte frontend-school/tests/static/timetable-version-contract.test.mjs
git commit -m "feat(timetable): add version revision workspace"
```

---

### Task 5: Prove version states, permissions, readiness, and publication in the browser

**Files:**
- Create: `frontend-school/tests/e2e/timetable-version-workspace.spec.ts`
- Modify: `frontend-school/tests/static/timetable-version-contract.test.mjs`

**Interfaces:**
- Consumes: deployed timetable page and mocked generated-contract response envelopes.
- Produces: serial mocked browser coverage for revision creation, draft-only mutation, read-only access, per-group readiness, warning acknowledgement, conflict recovery, and publication refresh.

- [ ] **Step 1: Add a complete mocked API boundary**

Mirror all documented fields for:

- academic context;
- current, historical, upcoming, and draft timetable versions;
- timetable entries, bell schedules/periods, offerings, groups, homerooms, and rooms;
- one linked `AcademicTermChangeSet`;
- one preview with a complete group, one deficit, one excess warning, and one conflict finding;
- create/update/deactivate timetable entry responses;
- preview/publish/cancel change-set responses.

Record request bodies and counts so tests assert the real page behavior, especially `timetableVersionId`, `previewHash`, target row version, and deduplicated warning codes.

- [ ] **Step 2: Write the six browser scenarios**

Cover:

1. creating a dated timetable revision sends term/date/reason and selects its target draft;
2. current/upcoming/historical/draft options render and only the draft enables entry controls;
3. create/move/deactivate requests all carry the selected draft `timetableVersionId`;
4. readiness renders literal per-group `actual/target` values and blocks deficits/conflicts;
5. an excess warning requires one acknowledgement code before publish and publish refreshes the version to read-only;
6. read-only staff never see create/edit/publish/cancel actions and never request management-only options.

- [ ] **Step 3: Verify test discovery before deployment**

Run:

```bash
cd frontend-school
npx playwright test tests/e2e/timetable-version-workspace.spec.ts --list
```

Expected: exactly 6 tests discovered; do not claim browser GREEN until the frontend is deployed.

- [ ] **Step 4: Run full local gates serially**

Run backend focused tests, then:

```bash
cd backend-school
cargo fmt --all -- --check
```

```bash
cd backend-school
cargo test --test static_architecture -- --test-threads=1
```

```bash
cd backend-school
cargo check
```

Then run API contract generate/check/tests even when no wire shape changed, followed by frontend lint, environment-backed Svelte check, menu sync, all static tests, `git diff --check`, and `git status --short`.

- [ ] **Step 5: Commit test coverage**

```bash
git add frontend-school/tests/e2e/timetable-version-workspace.spec.ts frontend-school/tests/static/timetable-version-contract.test.mjs
git commit -m "test(timetable): cover version revision workspace"
```

---

### Task 6: Review, deploy, and smoke Release 3

**Files:**
- Delete after completion: `docs/superpowers/plans/2026-08-30-academic-operational-change-release-3.md`
- Retain: `docs/superpowers/specs/2026-08-30-academic-operational-change-and-timetable-versioning-design.md`

**Interfaces:**
- Consumes: all Release 3 commits and the repository verification/deployment workflows.
- Produces: clean `main`, automated deployment to `sandbox`/`snwsb`, authenticated backend smoke, and browser evidence against the deployed sandbox.

- [ ] **Step 1: Review the complete Release 3 diff**

Check specifically that:

- no migration or compatibility branch was introduced;
- an untouched clone remains blocked;
- schedule-only edits are preview-hash protected and atomically published;
- no published version can be edited;
- warning acknowledgement is by unique stable code;
- read-only rendering does not load management-only options;
- no roster identity or national ID enters readiness output;
- no request-per-row fan-out or broad page reload was added.

- [ ] **Step 2: Run the final serial verification matrix**

Repeat every applicable `.rules` gate with fresh output. If the monolithic backend suite exhausts PostgreSQL shared memory, preserve its direct pass count and rerun every affected test in smaller serial groups; report both facts exactly.

- [ ] **Step 3: Remove the completed plan artifact and commit**

After every task and final gate passes, delete this plan while retaining the approved spec, then commit:

```bash
git add docs/superpowers/plans/2026-08-30-academic-operational-change-release-3.md
git commit -m "test(timetable): verify version workspace release 3"
```

- [ ] **Step 4: Push main and watch workflows one at a time**

Push `main`. Watch API Contract, Permission Contract, Backend School deploy, and Frontend School deploy serially. Confirm migration verification remains successful even though Release 3 adds no migration, authenticated Academic Core smoke passes, maintenance reopens, and Cloudflare deploy/menu synchronization succeeds for both tenants.

- [ ] **Step 5: Run deployed sandbox browser smoke serially**

Run:

```bash
cd frontend-school
npx playwright test tests/e2e/timetable-version-workspace.spec.ts --workers=1
```

```bash
cd frontend-school
npx playwright test tests/e2e/academic-operational-change.spec.ts --workers=1
```

Expected: 6/6 timetable-version scenarios and 6/6 operational-change regressions PASS against `sandbox.schoolorbit.app`.

- [ ] **Step 6: Record handoff evidence**

Report commit SHA, workflow URLs/conclusions, backend focused/full-test evidence, frontend counts, deployed Playwright counts, and a clean `git status --short`. Then move to the separate Release 4 curriculum-alignment plan.
