# Timetable Homeroom Drag Board — Release 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the form-first timetable workspace with an accessible drag-and-drop board where staff can arrange one period at a time in either homeroom or learning-group view, with authoritative server previews for valid moves, swaps, and conflicts.

**Architecture:** Introduce one typed, set-based workspace endpoint that returns the selected timetable version, bell periods, groups, homerooms, rooms, exact instructors, entries, and unscheduled demand in one response. Keep the database and existing create/update/swap transactions authoritative; the browser builds a local occupancy index for instant highlighting, then confirms every drop through a typed server preview before mutation. Decompose the large route into focused Svelte 5 components and a route-local state module; both editable views consume the same entry collection so a move is reflected everywhere without duplicate schedule records.

**Tech Stack:** Rust, Axum, SQLx, PostgreSQL, utoipa/OpenAPI, TypeScript, Svelte 5 runes, shadcn-svelte, Pointer Events/HTML drag-and-drop fallback, Node static tests, Playwright.

**Spec:** `docs/superpowers/specs/2026-08-30-timetable-drag-drop-and-effective-teacher-assignment-design.md`

## Global Constraints

- Release 1 must already be deployed and verified. Every group timetable entry has exact `timetable_entry_instructors`; never infer a period's teachers from all group teachers.
- Work directly on `main` only when the user explicitly requests inline execution. Run commands sequentially with `CARGO_BUILD_JOBS=1` and Playwright `--workers=1` because this workstation hangs under parallel load.
- Read `.rules` before implementation and use its change-type verification matrix.
- Do not edit migrations 001–054. This release is application-only unless implementation uncovers a schema invariant that cannot be enforced otherwise; stop for design review before adding migration 055.
- Use generated OpenAPI types at the API boundary. Do not maintain handwritten wire-contract duplicates.
- One drag represents exactly one timetable period. Double periods are two independent entries placed twice.
- A populated target is swappable only when the authoritative preview says the two entries can exchange slots. Never overwrite or silently delete the target entry.
- Published and archived timetable versions remain immutable. All mutations require a draft version and optimistic `rowVersion` checks.
- Homeroom and learning-group views edit the same `academic_timetable_entries` rows. Whole-school and teacher views are outside this release.
- Preserve a keyboard/button path for every drag action; drag-and-drop cannot be the only way to move an entry.
- Use `svelte:svelte-code-writer` and `svelte:svelte-core-bestpractices` before editing any `.svelte` or `.svelte.ts` file, and run the Svelte autofixer on each changed component.
- No production SQL, migration repair, deployment, or push is part of an implementation step until the user explicitly approves it.

---

### Task 1: Add a set-based timetable workspace read contract

**Files:**
- Modify: `backend-school/src/modules/academic/models/timetable.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service_tests.rs`
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**
- Add query `TimetableWorkspaceQuery { academic_year_id, academic_term_id, timetable_version_id }` with camelCase serde and `deny_unknown_fields`.
- Add `TimetableWorkspace`, containing `version`, `bell_periods`, `entries`, `learning_groups`, `homerooms`, `rooms`, `staff`, and `unscheduled_demands`.
- Add lightweight typed rows `TimetableWorkspaceLearningGroup`, `TimetableWorkspaceHomeroom`, `TimetableWorkspaceRoom`, `TimetableWorkspaceStaff`, and `TimetableUnscheduledDemand`.
- Add `GET /api/academic/timetable/workspace`, operation ID `getTimetableWorkspace`.
- `TimetableUnscheduledDemand` identifies `learningGroupId`, `learningOfferingId`, `requiredPeriods`, `scheduledPeriods`, `remainingPeriods`, target homerooms, and eligible exact instructor IDs. It is derived from the current offering/group target, never stored.

- [ ] **Step 1: Write failing service tests for one bounded workspace load**

Add tests that seed one draft version with two groups, multiple homerooms, exact solo/co-teachers, rooms, and scheduled entries, then assert:

```rust
let workspace = timetable_service::get_workspace(
    &pool,
    TimetableWorkspaceQuery {
        academic_year_id: year_id,
        academic_term_id: term_id,
        timetable_version_id: version_id,
    },
).await?;

assert_eq!(workspace.entries.len(), 2);
assert_eq!(workspace.entries[0].instructors.len(), 2);
assert_eq!(workspace.unscheduled_demands[0].remaining_periods, 1);
assert!(workspace.learning_groups.iter().all(|group| !group.homeroom_ids.is_empty()));
```

Also assert that a version from another term/year is rejected and that inactive rooms/staff are included only when referenced by an existing active entry so historical drafts remain renderable.

- [ ] **Step 2: Run the focused test and capture the expected failure**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_service_tests::workspace -- --test-threads=1
```

Expected: FAIL because the workspace model and service do not exist.

- [ ] **Step 3: Implement the workspace models and set-based loader**

Implement `get_workspace` with a fixed number of queries, independent of the number of groups or entries:

1. require the requested timetable version and exact academic context;
2. load bell periods for its bell schedule;
3. load all active entries and exact entry instructors;
4. load group/offering/homeroom relationships as sets;
5. load referenced rooms and staff in batches;
6. aggregate scheduled counts by learning group and compute remaining demand from the effective offering/group target.

Use stable ordering for every collection: bell order, grade/homeroom order, group code, entry day/period/ID, and Thai display name. Reject negative remaining counts in presentation by returning `max(required - scheduled, 0)` while retaining `scheduledPeriods` so over-scheduling is visible.

- [ ] **Step 4: Add the handler, route, permission check, and OpenAPI registration**

The handler requires the existing timetable read permission, returns `ApiResponse<TimetableWorkspace>`, and lists 400/403/404 responses. Register it in `backend-school/src/modules/academic.rs` and every request/response schema in `backend-school/src/api_contract.rs`.

- [ ] **Step 5: Re-run the focused backend tests**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_service_tests::workspace -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 6: Commit the backend workspace slice**

```bash
git add backend-school/src/modules/academic/models/timetable.rs \
  backend-school/src/modules/academic/services/timetable_service.rs \
  backend-school/src/modules/academic/services/timetable_service_tests.rs \
  backend-school/src/modules/academic/handlers/timetable.rs \
  backend-school/src/modules/academic.rs backend-school/src/api_contract.rs
git commit -m "feat(timetable): add set based workspace"
```

---

### Task 2: Make create, update, move, and swap previews one typed authoritative contract

**Files:**
- Modify: `backend-school/src/modules/academic/models/timetable.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service_tests.rs`
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**
- Replace `MoveValidityCell.state: String`, `valid: bool`, and free-text-only semantics with typed `TimetablePlacementState::{Source, Move, Swap, Blocked}`. A neutral cell before a drag is frontend presentation state, not an API candidate state.
- Add tagged `TimetablePlacementSource::{ExistingEntry { entry_id, row_version }, UnscheduledDemand { learning_group_id, learning_offering_id }}`.
- Add `TimetablePlacementCandidate { entry_type, learning_group_id, learning_offering_id, homeroom_id, room_id, instructor_ids }`; it is the complete proposed conflict-bearing shape and supports new tray items plus room/instructor edits without ambiguous missing-vs-null fields.
- Add `TimetablePlacementPreviewRequest { timetable_version_id, academic_term_id, source, candidate, target_day_of_week, target_bell_schedule_period_id, expected_target_entry_id, expected_target_row_version }`.
- Add `TimetablePlacementMutationKind::{Create, Update, Move, Swap}`.
- Add `TimetablePlacementPreview { state, source_entry_id, target_entry_id, target_day_of_week, target_bell_schedule_period_id, normalized_candidate, conflicts, mutation }`; `mutation` is absent for blocked/source.
- Reuse `ConflictInfo` but constrain `conflict_type` to serialized `TimetableConflictType::{LearningGroup, Homeroom, Instructor, Room, Version, StaleEntry}`.
- Add operation `previewTimetablePlacement`; retain the existing grid `validateTimetableMoves` endpoint only if current consumers still need whole-board highlighting, but make it return the same typed state/conflict vocabulary.

- [ ] **Step 1: Add failing tests for move, swap, and every collision dimension**

Cover:

- unscheduled demand + empty target -> `Move` with mutation `Create`;
- unscheduled demand + occupied target -> `Blocked` because a new entry has no source slot to swap back into;
- empty target -> `Move`;
- occupied target with conflict-free reverse placement -> `Swap`;
- same slot with a changed exact instructor/room candidate -> `Move` with mutation `Update`;
- group collision -> `Blocked(LearningGroup)`;
- any target homeroom collision -> `Blocked(Homeroom)`;
- any exact entry instructor collision -> `Blocked(Instructor)`;
- physical room collision -> `Blocked(Room)`;
- stale source or target row version -> `Blocked(StaleEntry)` / HTTP 409 at mutation time;
- published version -> `Blocked(Version)`;
- a co-taught entry checks all attached instructors;
- a group teacher not attached to that entry does not create an instructor conflict;
- cross-group swap requires manage access to both groups; and
- successful move/swap audit payloads contain both entries' before/after slot, room, exact teacher set, actor, version, and row versions.

- [ ] **Step 2: Run the focused tests and confirm failure**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_service_tests::placement_preview -- --test-threads=1
```

Expected: FAIL because the typed contract and preview function do not exist.

- [ ] **Step 3: Extract one shared conflict evaluator**

Refactor create, update, swap, grid validation, and placement preview to call the same internal evaluator over exact entry relationships. Validate that an existing entry candidate preserves its immutable group/offering/entry-type identity and that an unscheduled candidate matches the named demand. The preview must simulate both sides of a swap, excluding the two moving entry IDs from occupancy, and must never mutate. Return stable conflict ordering by type then existing entry ID.

Keep permission evaluation separate from collision evaluation: create/update requires the source group scope, swap requires the union of both entries' group scopes, and structural school-wide entries retain the current school-manage requirement. Preview must apply the same scope decision as its proposed mutation so it cannot advertise a move the actor cannot perform.

- [ ] **Step 4: Add the preview handler and OpenAPI contract**

Add `POST /api/academic/timetable/placement-preview`. Require timetable manage permission because it exposes mutation feasibility. Use 400 for invalid academic/slot shape, 403 for permission, 404 for missing version/entry, and 200 with `Blocked` for ordinary scheduling conflicts.

- [ ] **Step 5: Prove preview and mutation cannot disagree under unchanged row versions**

Extend tests to preview then call `create_entry`, `update_entry`, or `swap_entries` with the same normalized candidate and row versions. Assert the resulting source/target slots and exact instructor/room set match the preview. Mutate a row between preview and mutation and assert 409 with no partial move. Audit rows are written inside the same transaction; a forced audit failure rolls back both sides of a swap. Existing post-commit `TimetableChanged` broadcasts continue to invalidate clients for every returned entry and never carry replacement state.

- [ ] **Step 6: Run focused timetable service tests**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_service_tests -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 7: Commit the authoritative placement slice**

```bash
git add backend-school/src/modules/academic/models/timetable.rs \
  backend-school/src/modules/academic/services/timetable_service.rs \
  backend-school/src/modules/academic/services/timetable_service_tests.rs \
  backend-school/src/modules/academic/handlers/timetable.rs \
  backend-school/src/modules/academic.rs backend-school/src/api_contract.rs
git commit -m "feat(timetable): preview moves and swaps"
```

---

### Task 3: Generate the frontend contract and add pure board state

**Files:**
- Modify (generated): `contracts/openapi/school-api.json`
- Modify (generated): `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/timetable.ts`
- Create: `frontend-school/src/lib/academic/timetable/board-state.ts`
- Create: `frontend-school/src/lib/academic/timetable/workspace-controller.svelte.ts`
- Create: `frontend-school/src/lib/academic/timetable/board-state.test.ts`
- Modify: `frontend-school/tests/static/timetable-version-contract.test.mjs`

**Interfaces:**
- Export generated aliases `TimetableWorkspace`, `TimetablePlacementPreviewRequest`, `TimetablePlacementPreview`, and `TimetablePlacementState` from `timetable.ts`.
- Add `getTimetableWorkspace(query, options)` and `previewTimetablePlacement(payload, options)` using generated query/body types.
- Pure `board-state.ts` functions normalize the workspace and return visible rows, entries for a cell, unscheduled demand, local candidate state, and issue counts.
- `createTimetableWorkspaceController(workspace)` in the Svelte module owns selected view/owner, drag source, preview, pending mutation, and refresh state while delegating domain calculations to the pure module.

- [ ] **Step 1: Regenerate and validate the API contract**

From `frontend-school`:

```bash
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
```

Expected: PASS and generated names match the Rust operation/schema names exactly.

- [ ] **Step 2: Write failing state tests before the state module**

Test a fixed workspace fixture for:

- homeroom rows show entries whose group covers that homeroom;
- a shared group entry appears in each covered homeroom without duplicating the normalized entry;
- learning-group rows show only that group's entries;
- local occupancy marks an obvious exact-teacher/group/homeroom/room conflict;
- an occupied cell is only a swap candidate, never an overwrite candidate;
- remaining demand decrements after adding an entry and increments after deletion;
- published versions expose `canEdit === false`;
- selecting an entry is independent from beginning a drag.

- [ ] **Step 3: Run the state test and capture failure**

```bash
cd frontend-school
node --experimental-strip-types --test src/lib/academic/timetable/board-state.test.ts
```

Expected: FAIL because the pure state module does not exist.

- [ ] **Step 4: Implement the normalized Svelte state module**

Use pure TypeScript for normalized maps and occupancy calculations, and Svelte 5 runes only in `workspace-controller.svelte.ts`. Do not fetch inside either module. Store IDs, not copied domain objects, for selection and drag state. Treat the local evaluator as a responsiveness hint only; only a successful server preview may enable the final mutation call.

- [ ] **Step 5: Update the handwritten API wrapper and contract guard**

Build query/body values with `satisfies` generated types. Remove the route's multi-request workspace fanout once Task 5 adopts the endpoint. Extend the static contract test to reject snake_case query keys and handwritten replicas of generated workspace/preview interfaces.

- [ ] **Step 6: Run state and contract tests**

```bash
cd frontend-school
node --experimental-strip-types --test src/lib/academic/timetable/board-state.test.ts
node --test tests/static/timetable-version-contract.test.mjs
```

Expected: PASS.

- [ ] **Step 7: Commit contract and state**

```bash
git add contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  frontend-school/src/lib/api/timetable.ts \
  frontend-school/src/lib/academic/timetable/board-state.ts \
  frontend-school/src/lib/academic/timetable/workspace-controller.svelte.ts \
  frontend-school/src/lib/academic/timetable/board-state.test.ts \
  frontend-school/tests/static/timetable-version-contract.test.mjs
git commit -m "feat(timetable): add drag board state"
```

Use the actual test path in `git add` if Step 3 required the static-test fallback.

---

### Task 4: Build the reusable accessible drag board components

**Files:**
- Create: `frontend-school/src/lib/components/timetable/TimetableWorkspaceHeader.svelte`
- Create: `frontend-school/src/lib/components/timetable/TimetableViewSelector.svelte`
- Create: `frontend-school/src/lib/components/timetable/TimetableUnscheduledTray.svelte`
- Create: `frontend-school/src/lib/components/timetable/TimetableBoard.svelte`
- Create: `frontend-school/src/lib/components/timetable/TimetableCell.svelte`
- Create: `frontend-school/src/lib/components/timetable/TimetableLessonCard.svelte`
- Create: `frontend-school/src/lib/components/timetable/TimetableEntryInspector.svelte`
- Create: `frontend-school/src/lib/components/timetable/TimetableMoveDialog.svelte`
- Reuse: `frontend-school/src/lib/components/timetable/TimetableInstructorPicker.svelte`
- Modify: `frontend-school/src/lib/components/MobileDragDropPolyfill.svelte`
- Modify: `frontend-school/tests/static/mobile-drag-drop-loading.test.mjs`
- Create: `frontend-school/tests/static/timetable-drag-board-components.test.mjs`

**Visual contract:**
- Header: selected version/status/date range, draft/published badge, save state, and explicit view switch.
- Left/top tray: unscheduled lessons grouped by homeroom or learning group, with `remaining / required` count and teacher chips.
- Board: weekday columns and bell-period rows; lesson cards show code, short name, owner, exact teacher initials/names, and room.
- States: neutral, dragging, valid move, valid swap, blocked, saving, stale. Encode state with icon/text/border as well as color.
- Inspector: edit exact teacher/room/details for drafts and read-only details for published versions.

- [ ] **Step 1: Write failing static component-contract tests**

Assert the component set exists and that:

- draggable cards have an accessible label and keyboard move control;
- cells expose day/period labels and state text;
- the move dialog can select day/period without drag;
- the board has no HTML `<select>` and uses the established shadcn-svelte selector components;
- mobile drag polyfill is loaded only on timetable routes that need it;
- no component directly calls the API wrapper.

- [ ] **Step 2: Run the component tests and confirm failure**

```bash
cd frontend-school
node --test tests/static/timetable-drag-board-components.test.mjs
```

Expected: FAIL because the components do not exist.

- [ ] **Step 3: Implement presentation-only components**

Components receive generated domain values and callbacks/snippets; they do not own fetching or mutation. Use one lesson card DOM item per actual entry even when rendering the same shared-group entry in multiple homeroom rows; give render instances composite DOM keys such as `${entry.id}:${row.id}` to avoid Svelte duplicate-key errors.

Use pointer/drag events only to announce intent. The parent controller performs local preview, calls the server preview, and chooses update vs swap. Provide buttons/menu actions “ย้ายคาบ”, “แก้รายละเอียด”, and “นำออกจากตาราง” so touch, keyboard, and assistive technology users have full parity.

- [ ] **Step 4: Make drag behavior route-local and mobile-safe**

Update `MobileDragDropPolyfill.svelte` so the timetable route opts in without affecting every authenticated page. Keep scroll available outside a deliberate lesson-card drag. Add an escape/cancel path that clears all target highlighting.

- [ ] **Step 5: Run Svelte analysis/autofix on each new component**

Use the commands prescribed by `svelte:svelte-code-writer`; apply every correctness recommendation and document any declined style-only recommendation in the implementation handoff.

- [ ] **Step 6: Run component tests and Svelte check**

```bash
cd frontend-school
node --test tests/static/timetable-drag-board-components.test.mjs
node --test tests/static/mobile-drag-drop-loading.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Expected: PASS with 0 Svelte errors and 0 warnings.

- [ ] **Step 7: Commit reusable components**

```bash
git add frontend-school/src/lib/components/timetable \
  frontend-school/src/lib/components/MobileDragDropPolyfill.svelte \
  frontend-school/tests/static/timetable-drag-board-components.test.mjs \
  frontend-school/tests/static/mobile-drag-drop-loading.test.mjs
git commit -m "feat(timetable): build accessible drag board"
```

---

### Task 5: Replace the timetable route with homeroom and group workspaces

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`
- Modify: `frontend-school/tests/static/timetable-request-performance.test.mjs`
- Modify: `frontend-school/tests/e2e/timetable-version-workspace.spec.ts`
- Create: `frontend-school/tests/e2e/timetable-drag-board.spec.ts`

**Interfaces and behavior:**
- URL view values are `view=homeroom|learningGroup`, with `ownerId` for the selected homeroom/group. Unknown values fall back to homeroom without throwing.
- Route loads exactly one workspace request after academic context/version resolution.
- Drop flow is `local hint -> POST placement-preview -> create, update, or swap -> replace returned entries -> refresh workspace on stale/SSE`.
- Create from unscheduled flow fixes the demand's group/offering. When exactly one teacher is eligible, preselect that teacher visibly; when several are eligible, require the staff member to select one or more rather than assuming all of them teach the period.
- Published versions render the same board but all create/update/swap/delete controls are disabled.

- [ ] **Step 1: Extend the request-performance test to fail on fanout**

Assert the route imports and calls `getTimetableWorkspace`, does not call `loadTimetableCollections`, and does not loop over groups/homerooms to issue entry or teacher requests. Retain version resolution as one bounded request.

- [ ] **Step 2: Add a failing Playwright scenario**

Use the existing local mock harness to cover:

1. open a draft in homeroom view;
2. drag one unscheduled period to an empty slot, preview returns `move/create`, entry is created once;
3. drag an existing entry to an occupied compatible slot, preview returns `swap`, swap is called once;
4. drag into teacher conflict, preview returns `blocked`, no mutation is called and Thai reason is visible;
5. switch to learning-group view and observe the same moved entry;
6. use the non-drag move dialog to move it back;
7. open a published version and verify all mutation controls are unavailable.

- [ ] **Step 3: Run focused tests and capture failure**

```bash
cd frontend-school
node --test tests/static/timetable-request-performance.test.mjs
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/timetable-drag-board.spec.ts --workers=1
```

Expected: FAIL because the route still uses the form-first/fanout workspace.

- [ ] **Step 4: Rebuild the route as a thin controller**

Keep academic context/version URL synchronization, SSE reconnect, and draft-version operations. Replace embedded table/form markup with the Task 4 components. Centralize error mapping:

- 409 stale -> reload workspace and announce that another user changed the timetable;
- 409 scheduling conflict -> retain drag source and show current typed conflicts;
- network failure -> retain current board, show retry, do not apply optimistic placement;
- successful mutation -> update normalized entries from server response and clear preview.

Do not copy an entry per homeroom. Render projections from normalized state and ensure shared-group cards always move the single source row.

- [ ] **Step 5: Verify one-period creation and unscheduled counts**

Add route tests for a demand requiring 3 periods: each drop creates one entry and changes the count 3→2→1→0. The UI must not offer a “two periods” drop option. Also prove a one-teacher group defaults visibly, while a multiple-teacher group cannot be dropped until one or more exact teachers are selected.

- [ ] **Step 6: Run focused static and browser tests**

```bash
cd frontend-school
node --test tests/static/timetable-request-performance.test.mjs
node --test tests/static/timetable-drag-board-components.test.mjs
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/timetable-version-workspace.spec.ts --workers=1
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/timetable-drag-board.spec.ts --workers=1
```

Expected: PASS.

- [ ] **Step 7: Commit the route cutover**

```bash
git add 'frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte' \
  frontend-school/tests/static/timetable-request-performance.test.mjs \
  frontend-school/tests/e2e/timetable-version-workspace.spec.ts \
  frontend-school/tests/e2e/timetable-drag-board.spec.ts
git commit -m "feat(timetable): arrange homerooms by drag and drop"
```

---

### Task 6: Release 2 verification and sandbox checkpoint

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
CARGO_BUILD_JOBS=1 cargo check --manifest-path backend-school/Cargo.toml
```

Expected: PASS.

- [ ] **Step 2: Run API/frontend gates sequentially**

```bash
cd frontend-school
npm run check:api-contracts
npm run test:api-contracts
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

Expected: PASS with no stale generated contract and no Svelte diagnostics.

- [ ] **Step 3: Run focused browser tests with one worker**

```bash
cd frontend-school
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/timetable-version-workspace.spec.ts --workers=1
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/timetable-drag-board.spec.ts --workers=1
```

Expected: PASS on desktop plus the project’s configured mobile viewport.

- [ ] **Step 4: Run hygiene and scope checks**

```bash
git diff --check
git status --short
rg -n "loadTimetableCollections|instructors_by_group|state: String" \
  frontend-school/src/routes/'(app)'/staff/academic/timetable \
  backend-school/src/modules/academic/{models/timetable.rs,services/timetable_service.rs}
```

Expected: no timetable-route fanout, no group-teacher fallback, no stringly typed placement state.

- [ ] **Step 5: Push only after explicit approval and run authenticated sandbox checks**

After normal auto-deployment, verify in the sandbox tenant:

```text
workspace load is bounded (no repeated group requests)
homeroom and group boards show the same entry placement
one drop schedules one period
solo/co-teacher conflicts block correctly
valid swaps exchange entries without deletion
keyboard move works
published version is read-only
```

Keep the database snapshot through the checkpoint. No production tenant mutation is required for this application-only release.

- [ ] **Step 6: Record documentation only when it changed**

```bash
git add docs/TESTING.md docs/OPERATIONS.md
git commit -m "docs(timetable): record drag board rollout"
```

Skip this commit when both files are unchanged.
