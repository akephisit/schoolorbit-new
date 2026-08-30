# Timetable Teacher Board and School Overview — Release 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an editable teacher timetable view and a read-only whole-school daily overview on top of the exact-instructor and drag-board foundations, without duplicating timetable entries or introducing per-teacher request fanout.

**Architecture:** The teacher board is another projection of the Release 2 normalized workspace: filtering an entry by `timetable_entry_instructors` determines which teacher rows display it, and every move mutates the same timetable entry used by homeroom/group views. A separate day-bounded overview endpoint returns a compact school matrix and typed issue summary so administrators can inspect the whole school without downloading or rendering every five-day relation at once. The existing daily teaching view remains operational but is cut over to exact instructors and shared query helpers where appropriate.

**Tech Stack:** Rust, Axum, SQLx, PostgreSQL, utoipa/OpenAPI, TypeScript, Svelte 5 runes, shadcn-svelte, Node static tests, Playwright.

**Spec:** `docs/superpowers/specs/2026-08-30-timetable-drag-drop-and-effective-teacher-assignment-design.md`

## Global Constraints

- Releases 1 and 2 must be deployed and verified first.
- Read `.rules` before implementation and follow its verification matrix.
- Run all commands serially, with `CARGO_BUILD_JOBS=1` and Playwright `--workers=1`.
- Do not edit applied migrations. This release is expected to require no schema migration.
- Exact `timetable_entry_instructors` are the only source for teacher timetable membership and teacher conflict/load counts.
- A co-taught timetable entry is one entry with multiple instructors. Moving it from any teacher view moves that single entry for the whole team; never fork one teacher's copy.
- Moving an entry in teacher view must not silently change its instructor set. Exact instructors are edited explicitly in the inspector.
- Homeroom, learning-group, and teacher views are editable only for draft versions. Whole-school overview is always read-only.
- Whole-school data is bounded by one selected day and one timetable version. Do not create a full-term, all-day mega response.
- Use generated API/permission contracts and camelCase query fields.
- Preserve keyboard/button movement parity and typed conflict feedback from Release 2.
- Use `frontend-design` before reshaping the overview UI. Use both Svelte skills before editing `.svelte` or `.svelte.ts`, and run the Svelte autofixer on every changed component.
- Do not push or deploy without explicit user approval.

---

### Task 1: Add a compact whole-school day overview contract

**Files:**
- Modify: `backend-school/src/modules/academic/models/timetable.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service_tests.rs`
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**
- Add `WholeSchoolTimetableQuery { academic_year_id, academic_term_id, timetable_version_id, day_of_week }` with `deny_unknown_fields`.
- Add `WholeSchoolTimetableOverview { version, day_of_week, periods, rows, issues, summary }`.
- `WholeSchoolTimetableRow` represents one active homeroom and contains `cells` ordered by bell period.
- `WholeSchoolTimetableCell` contains zero or more compact `WholeSchoolTimetableLesson` values because cross-room/shared groups can legitimately project into the same homeroom cell while conflicts remain visible.
- Each lesson includes `entry_id`, group/offering code/name, covered homeroom IDs, exact instructors, room, and `is_shared_group`.
- Add typed `WholeSchoolTimetableIssueKind::{HomeroomConflict, InstructorConflict, RoomConflict, UnscheduledDemand, OverScheduledDemand, MissingInstructor, MissingRoom}` and issue references.
- Add `GET /api/academic/timetable/whole-school`, operation ID `getWholeSchoolTimetableOverview`.

- [ ] **Step 1: Write failing day-bounded service tests**

Seed a version with ordinary homeroom groups, one shared group, a co-taught entry, a deliberate conflict inserted through a fixture bypass, and unscheduled demand. Assert:

```rust
let overview = timetable_service::get_whole_school_overview(&pool, query).await?;
assert_eq!(overview.day_of_week, "MON");
assert_eq!(overview.rows.len(), active_homeroom_count);
assert!(overview.rows.iter().all(|row| row.cells.len() == period_count));
assert_eq!(shared_entry_occurrences(&overview), covered_homeroom_count);
assert_eq!(unique_entry_ids(&overview), persisted_entry_count_for_monday);
assert!(overview.issues.iter().any(|issue| issue.kind == WholeSchoolTimetableIssueKind::InstructorConflict));
```

Also assert Tuesday entries are absent, exact co-teachers are both present, inactive unreferenced rooms/staff are absent, invalid weekday is rejected, and version/year/term mismatch is rejected.

- [ ] **Step 2: Run the focused tests and confirm failure**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_service_tests::whole_school_overview \
  -- --test-threads=1
```

Expected: FAIL because the contract/service do not exist.

- [ ] **Step 3: Implement a fixed-query overview loader**

Load version/periods, active homerooms, day entries with exact instructors, group coverage, and unscheduled aggregates in set-based queries. Build cells and issue indexes in memory with deterministic ordering. Do not call the Release 2 workspace function then discard four days of data; query the selected day directly.

Calculate issues from exact entry relationships:

- same homeroom + day + period;
- same exact instructor + day + period;
- same physical room + day + period;
- scheduled count below/above the effective weekly target;
- active entry with zero exact instructors;
- missing room only when the offering/group requires a physical room under current domain rules.

- [ ] **Step 4: Add handler, route, permission, and OpenAPI registration**

Require timetable read permission. Document 400/403/404. Return a successful overview even when issues exist; issues are domain findings, not request failures.

- [ ] **Step 5: Re-run focused backend tests**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_service_tests::whole_school_overview \
  -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 6: Commit the overview backend**

```bash
git add backend-school/src/modules/academic/models/timetable.rs \
  backend-school/src/modules/academic/services/timetable_service.rs \
  backend-school/src/modules/academic/services/timetable_service_tests.rs \
  backend-school/src/modules/academic/handlers/timetable.rs \
  backend-school/src/modules/academic.rs backend-school/src/api_contract.rs
git commit -m "feat(timetable): add school day overview"
```

---

### Task 2: Prove every teacher-facing read uses exact period instructors

**Files:**
- Modify: `backend-school/src/modules/academic/services/daily_teaching_service.rs`
- Create: `backend-school/src/modules/academic/services/daily_teaching_service_tests.rs`
- Modify: `backend-school/src/modules/academic/services.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service_tests.rs`

**Interfaces and invariants:**
- Personal timetable, daily teaching overview, Release 2 workspace staff projections, and Release 3 overview all join `timetable_entry_instructors` for actual scheduled teacher membership.
- `learning_group_teachers` may determine eligibility/assignment metadata but cannot make a teacher appear in a period they are not attached to.
- Team-teaching counts equal the exact instructor count on that entry.

- [ ] **Step 1: Add regression tests with split and co-teaching fixtures**

For one learning group assigned to teachers A, B, and C, create:

- Monday period 1 exact A;
- Tuesday period 2 exact B+C;
- Wednesday period 3 exact C.

Assert A's personal/daily view contains only Monday; B only Tuesday; C Tuesday and Wednesday; Tuesday entry reports team teaching; and no read model duplicates an entry for the same teacher.

- [ ] **Step 2: Run focused tests and capture any fallback failure**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::daily_teaching_service_tests -- --test-threads=1
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_service_tests::personal -- --test-threads=1
```

Expected before cutover: at least the legacy `effective_teacher` union in daily teaching makes A/B/C appear on all group periods.

- [ ] **Step 3: Remove group-wide instructor fallback**

Replace the `effective_teacher` CTE union with an exact `timetable_entry_instructors` join for all entry types. Derive the teacher seed set from exact rows in the resolved version, with `include_empty_teachers` separately using effective group assignments only when explicitly requested. Keep exact scheduled rows distinct from eligible-but-empty teachers.

- [ ] **Step 4: Add a source guard against fallback reintroduction**

Add a static architecture assertion that teacher-facing timetable SQL cannot join `learning_group_teachers` as the source of entry membership. A separately named eligible-teacher query is allowed only for empty-teacher display and instructor picker eligibility.

- [ ] **Step 5: Run focused tests**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::daily_teaching_service_tests -- --test-threads=1
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_service_tests -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo test --manifest-path backend-school/Cargo.toml \
  --test static_architecture -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 6: Commit the exact teacher read cutover**

```bash
git add backend-school/src/modules/academic/services/daily_teaching_service.rs \
  backend-school/src/modules/academic/services/daily_teaching_service_tests.rs \
  backend-school/src/modules/academic/services.rs \
  backend-school/src/modules/academic/services/timetable_service.rs \
  backend-school/src/modules/academic/services/timetable_service_tests.rs \
  backend-school/tests/static_architecture.rs
git commit -m "fix(timetable): read exact period teachers"
```

Use the actual static architecture test path reported by `rg --files backend-school | rg 'static_architecture'` if it differs.

---

### Task 3: Generate frontend contracts and extend normalized board state

**Files:**
- Modify (generated): `contracts/openapi/school-api.json`
- Modify (generated): `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/timetable.ts`
- Modify: `frontend-school/src/lib/academic/timetable/board-state.ts`
- Modify: `frontend-school/src/lib/academic/timetable/workspace-controller.svelte.ts`
- Modify: `frontend-school/src/lib/academic/timetable/board-state.test.ts`
- Modify: `frontend-school/tests/static/timetable-version-contract.test.mjs`

**Interfaces:**
- Export `WholeSchoolTimetableOverview`, its typed child schemas, and `getWholeSchoolTimetableOverview(query)` from the wrapper.
- Extend board views to `homeroom | learningGroup | teacher`.
- Teacher selection uses `teacherId`; `visibleEntries` contains entries whose exact `instructors[].id` includes that teacher.
- Selecting a teacher does not filter the conflict occupancy index: moves still check all school entries and every exact instructor on the moving entry.

- [ ] **Step 1: Regenerate and validate contracts**

```bash
cd frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
```

Expected: PASS.

- [ ] **Step 2: Add failing board-state tests for teacher projection**

Assert:

- solo entry is visible only to its exact teacher;
- co-taught entry is visible to each attached teacher but remains one normalized object;
- group-assigned but non-attached teacher does not see the entry;
- moving the co-taught entry updates the slot for all teacher projections;
- changing selected teacher never changes entry instructor IDs;
- teacher load counts count one period per entry per attached teacher.

- [ ] **Step 3: Run the state tests and confirm failure**

```bash
cd frontend-school
node --experimental-strip-types --test src/lib/academic/timetable/board-state.test.ts
```

Expected: FAIL because teacher view is not supported.

- [ ] **Step 4: Implement teacher selectors and overview wrapper**

Use generated types and `satisfies` for query construction. Keep all selected view/owner state URL-serializable. Add contract guards against manual whole-school response interfaces and query snake_case.

- [ ] **Step 5: Re-run state and contract tests**

```bash
cd frontend-school
node --experimental-strip-types --test src/lib/academic/timetable/board-state.test.ts
node --test tests/static/timetable-version-contract.test.mjs
```

Expected: PASS.

- [ ] **Step 6: Commit generated contract and state**

```bash
git add contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  frontend-school/src/lib/api/timetable.ts \
  frontend-school/src/lib/academic/timetable/board-state.ts \
  frontend-school/src/lib/academic/timetable/workspace-controller.svelte.ts \
  frontend-school/src/lib/academic/timetable/board-state.test.ts \
  frontend-school/tests/static/timetable-version-contract.test.mjs
git commit -m "feat(timetable): project schedules by teacher"
```

---

### Task 4: Add the editable teacher board

**Files:**
- Create: `frontend-school/src/lib/components/academic/timetable/TimetableTeacherView.svelte`
- Modify: `frontend-school/src/lib/components/academic/timetable/TimetableViewSelector.svelte`
- Modify: `frontend-school/src/lib/components/academic/timetable/TimetableBoard.svelte`
- Modify: `frontend-school/src/lib/components/academic/timetable/TimetableLessonCard.svelte`
- Modify: `frontend-school/src/lib/components/academic/timetable/TimetableEntryInspector.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`
- Modify: `frontend-school/tests/static/timetable-drag-board-components.test.mjs`
- Create: `frontend-school/tests/e2e/timetable-teacher-board.spec.ts`

**Behavior:**
- Teacher view selects one teacher and shows that teacher's weekly grid.
- The same drag rules, preview endpoint, update/swap transactions, and non-drag move dialog from Release 2 are reused.
- A co-taught card clearly names the team and warns “ย้ายรายการเดียวกันสำหรับครูทุกคนในทีม” before the first move in a session.
- The inspector can explicitly add/remove eligible exact instructors on draft entries. Merely selecting teacher X as the board owner never mutates existing entries; starting a new entry from teacher X's eligible tray preselects X visibly, as required by the approved workflow, and staff may add eligible co-teachers before preview/drop.
- Retain the existing teacher-load XLSX as an action-owned lazy `import('exceljs')`; its rows are calculated only from exact entry instructors and it is not part of workspace loading.

- [ ] **Step 1: Add failing UI contracts and Playwright flow**

Test:

1. select teacher A;
2. only exact A periods appear;
3. move A-only entry to an empty slot;
4. move A+B entry and verify both A and B views show the new slot;
5. attempt a slot where B conflicts although A is free and verify the drop is blocked;
6. create from teacher A's eligible tray and verify A is visibly preselected in the placement candidate before the request;
7. edit instructors explicitly and verify teacher projections update from the returned entry;
8. teacher-load export counts split/co-taught periods from exact instructors without eagerly loading ExcelJS;
9. published version remains read-only.

- [ ] **Step 2: Run focused tests and capture failure**

```bash
cd frontend-school
node --test tests/static/timetable-drag-board-components.test.mjs
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/timetable-teacher-board.spec.ts --workers=1
```

Expected: FAIL because teacher view does not exist.

- [ ] **Step 3: Implement teacher view as a projection, not a new editor**

Compose the existing board/cell/card/inspector components. Add a searchable shadcn-svelte teacher selector with load summary and an eligible-group tray whose counts are group-wide targets, not per-teacher quotas. Use URL `view=teacher&ownerId=<teacher UUID>` and restore it after refresh only when the teacher exists in workspace staff; otherwise select the first exact scheduled teacher. A tray create initializes the candidate with the selected teacher, but does not persist anything until authoritative placement preview succeeds and the user drops/confirms one period.

When a user begins moving a co-taught entry, show the team warning but do not require a second confirmation on every drop. Never clone or split the entry from this view.

- [ ] **Step 4: Analyze/autofix all changed Svelte files**

Use the Svelte skill commands file by file. Resolve duplicate keys, stale derived values, invalid event handlers, and accessibility diagnostics before running the repository check.

- [ ] **Step 5: Run focused UI tests**

```bash
cd frontend-school
node --test tests/static/timetable-drag-board-components.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/timetable-teacher-board.spec.ts --workers=1
```

Expected: PASS.

- [ ] **Step 6: Commit the teacher board**

```bash
git add frontend-school/src/lib/components/academic/timetable \
  'frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte' \
  frontend-school/tests/static/timetable-drag-board-components.test.mjs \
  frontend-school/tests/e2e/timetable-teacher-board.spec.ts
git commit -m "feat(timetable): arrange schedules by teacher"
```

---

### Task 5: Build the read-only whole-school overview

**Files:**
- Create: `frontend-school/src/lib/components/academic/timetable/TimetableWholeSchoolOverview.svelte`
- Create: `frontend-school/src/lib/components/academic/timetable/TimetableIssueSummary.svelte`
- Modify: `frontend-school/src/lib/components/academic/timetable/TimetableViewSelector.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`
- Create: `frontend-school/tests/static/timetable-whole-school-overview.test.mjs`
- Create: `frontend-school/tests/e2e/timetable-whole-school-overview.spec.ts`

**Visual and interaction contract:**
- Add view `wholeSchool` and a weekday selector.
- Freeze homeroom labels and period header for the dense matrix.
- Each compact lesson token shows subject/activity code, teacher initials, and room; details open in a read-only sheet.
- Issue summary groups blockers/warnings by typed kind. A homeroom/cell link opens the exact draft version in homeroom view; a teacher finding opens that same version in teacher view; room-only findings focus the referenced cell before offering the editable homeroom link.
- Clearly label this view “ภาพรวมทั้งโรงเรียน · ดูอย่างเดียว”; it has no drag handles or mutation menu.
- Responsive small screens use one selected period/homeroom slice rather than a horizontally unreadable full matrix.

- [ ] **Step 1: Write failing static and browser tests**

Assert one overview request per selected day, no per-homeroom or per-teacher fanout, visible read-only label, exact homeroom/teacher recovery links carrying version and owner IDs, and complete absence of create/update/swap/delete calls in whole-school mode.

- [ ] **Step 2: Run focused tests and confirm failure**

```bash
cd frontend-school
node --test tests/static/timetable-whole-school-overview.test.mjs
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/timetable-whole-school-overview.spec.ts --workers=1
```

Expected: FAIL because the overview components do not exist.

- [ ] **Step 3: Implement the overview and URL state**

Load only after `view=wholeSchool` is active. Cancel the previous day request on rapid weekday changes. Cache responses by `versionId:dayOfWeek` for the current workspace session and invalidate the selected key on timetable SSE changes. Do not mix overview response rows into editable normalized board state.

- [ ] **Step 4: Analyze/autofix changed Svelte files and run checks**

Use the Svelte skills, then:

```bash
cd frontend-school
node --test tests/static/timetable-whole-school-overview.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/timetable-whole-school-overview.spec.ts --workers=1
```

Expected: PASS.

- [ ] **Step 5: Commit the whole-school overview**

```bash
git add frontend-school/src/lib/components/academic/timetable \
  'frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte' \
  frontend-school/tests/static/timetable-whole-school-overview.test.mjs \
  frontend-school/tests/e2e/timetable-whole-school-overview.spec.ts
git commit -m "feat(timetable): show school wide overview"
```

---

### Task 6: Release 3 verification and sandbox checkpoint

**Files:**
- Modify only if durable guidance changed: `docs/TESTING.md`
- Modify only if rollout/recovery changed: `docs/OPERATIONS.md`

- [ ] **Step 1: Run backend gates sequentially**

```bash
cargo fmt --manifest-path backend-school/Cargo.toml --all -- --check
CARGO_BUILD_JOBS=1 cargo test --manifest-path backend-school/Cargo.toml \
  --test static_architecture -- --test-threads=1
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_service_tests -- --test-threads=1
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::daily_teaching_service_tests -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo check --manifest-path backend-school/Cargo.toml
```

Expected: PASS.

- [ ] **Step 2: Run API and frontend gates sequentially**

```bash
cd frontend-school
npm run check:api-contracts
npm run test:api-contracts
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

Expected: PASS.

- [ ] **Step 3: Run the three timetable browser specs one at a time**

```bash
cd frontend-school
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/timetable-drag-board.spec.ts --workers=1
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/timetable-teacher-board.spec.ts --workers=1
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/timetable-whole-school-overview.spec.ts --workers=1
```

Expected: PASS.

- [ ] **Step 4: Run source and hygiene guards**

```bash
git diff --check
git status --short
rg -n "JOIN learning_group_teachers.*entry|effective_teacher" \
  backend-school/src/modules/academic/services/{timetable_service.rs,daily_teaching_service.rs}
rg -n "view=wholeSchool|wholeSchool" frontend-school/src/lib/components/academic/timetable \
  'frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte'
```

Expected: no group-wide scheduled teacher fallback; whole-school mode is represented and read-only.

- [ ] **Step 5: Push only after explicit approval, then verify sandbox behavior**

After normal deployment, check authenticated sandbox screens serially:

```text
teacher A sees only exact A entries
co-taught move appears in every attached teacher view
conflict of any co-teacher blocks the move
whole-school screen loads one selected day
whole-school issue links focus the right cell
whole-school screen exposes no mutation action
existing daily teaching screen still reports exact teachers
```

- [ ] **Step 6: Record documentation only when needed**

```bash
git add docs/TESTING.md docs/OPERATIONS.md
git commit -m "docs(timetable): record teacher and overview rollout"
```

Skip this commit when neither file changed.
