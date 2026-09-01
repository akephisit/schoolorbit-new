# Timetable Blocks and Synchronized Activities Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace entry/batch-owned scheduling with canonical timetable blocks, support synchronized curriculum activities before group creation and structural school blocks, and deliver direct teacher-aware drag-and-drop across every timetable consumer.

**Architecture:** `academic_timetable_blocks` owns the weekly slot and logical event. Explicit group, group-instructor, homeroom, teacher, and sync-state tables own participants; services expose typed block operations and all readers join the same model. One forward migration reconciles and removes the old entry tables, while backend, generated contracts, permissions, frontend, and downstream readers cut over in one maintenance deployment.

**Tech Stack:** PostgreSQL/SQLx migrations, Rust/Axum/utoipa, SvelteKit 5/TypeScript, shadcn-svelte, native HTML drag-and-drop plus existing mobile fallback, Node static tests, Playwright, GitHub Actions deployment.

**Spec:** `docs/superpowers/specs/2026-09-02-timetable-blocks-and-synchronized-activities-design.md`

## Global Constraints

- Never edit an applied migration; add `backend-school/migrations/058_timetable_blocks.sql`.
- Never store or log plaintext national IDs, credentials, tokens, cookies, database URLs, or raw request bodies.
- Rust DTOs and `utoipa` own the HTTP contract; regenerate the tracked OpenAPI and TypeScript artifacts.
- `contracts/permissions.json` owns permissions; regenerate both registries and the lock file.
- Only draft timetable versions mutate. Published versions and every block child remain immutable.
- A draft date never activates a version; a published future version resolves automatically on its effective date.
- Ordinary delivery selects one or more exact instructors per period; synchronized activity groups always use every Delivery teacher.
- One drag creates or moves one bell period. No linked double-period model is introduced.
- Synchronized group sync is per-group; a conflict never moves the block or rolls back other successful groups.
- No old entry/batch API, old schema, dual read/write, fallback derivation, or tenant-specific compatibility branch remains after cutover.
- Run build, test, generation, migration, and browser commands serially.

---

### Task 1: Add the forward-only canonical block migration

**Files:**
- Create: `backend-school/migrations/058_timetable_blocks.sql`
- Modify: `backend-school/src/modules/academic/core/schema_tests.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Produces: `academic_timetable_blocks`, `academic_timetable_block_groups`, `academic_timetable_block_group_instructors`, `academic_timetable_block_homerooms`, `academic_timetable_block_teachers`, and `academic_timetable_block_group_sync`.
- Produces: database functions `assert_timetable_block_mutable(uuid)` and `assert_timetable_block_conflict_free(uuid)` used by child and move triggers.
- Removes: `academic_timetable_entries`, `timetable_entry_instructors`, and their entry/batch triggers after reconciliation.

- [ ] **Step 1: Add a failing migration/schema test**

Add a database test that migrates a fixture through 057, inserts one course entry, one synchronized activity batch, and one structural batch, applies 058, and asserts:

```rust
assert_eq!(table_count(&pool, "academic_timetable_blocks").await, 3);
assert_eq!(table_count(&pool, "academic_timetable_block_groups").await, 3);
assert_eq!(table_count(&pool, "academic_timetable_block_homerooms").await, 2);
assert!(!table_exists(&pool, "academic_timetable_entries").await);
assert!(!table_exists(&pool, "timetable_entry_instructors").await);
```

Also assert the former group-entry ID is retained as its block-group ID and a supervision reference points to that ID after the FK/column rename.

- [ ] **Step 2: Run the focused test and confirm the missing migration fails**

Run:

```bash
scripts/test_backend_school.sh modules::academic::core::schema_tests::migration_058_reconciles_timetable_blocks -- --nocapture --test-threads=1
```

Expected: failure because migration 058 and the canonical block tables do not exist.

- [ ] **Step 3: Write migration preflight and DDL**

Create the six tables with composite academic-context foreign keys, positive row-version checks, shape checks, stable unique keys, and indexes for version/slot/group/homeroom/teacher/room reads. Use these normalized enums:

```sql
block_kind       IN ('COURSE', 'ACTIVITY', 'STRUCTURAL')
scheduling_mode  IN ('independent', 'synchronized') OR NULL
structural_kind  IN ('BREAK', 'HOMEROOM', 'FLAG_CEREMONY', 'TEACHER_MEETING', 'ACADEMIC', 'OTHER') OR NULL
sync_status      IN ('LINKED', 'WAITING_FOR_DATA', 'CONFLICT', 'OUTSIDE_SCOPE', 'EXCLUDED')
```

The migration must fail before destructive work when a batch mixes versions/slots/sources, a delivery entry lacks offering/group context, a referenced entry cannot map one-to-one to a block group, or exact expected target counts cannot be derived.

- [ ] **Step 4: Backfill deterministic blocks and targets**

Map ordinary course and independent-activity entries one-to-one. Group only synchronized activity rows or structural rows that share version, slot, source, and batch. Preserve legacy entry UUIDs in block-group rows, migrate exact instructor order, convert structural homeroom and instructor audiences into explicit target rows, rewire supervision references, and write migration provenance.

- [ ] **Step 5: Add immutable/conflict triggers and remove legacy tables**

The conflict function must reject cross-block duplicate group, covered homeroom/reservation, exact instructor/structural teacher, or physical room in the same version/slot while allowing overlap among children of the same block. Install child mutation and block-move triggers, verify cardinalities, then drop the legacy tables/functions/triggers.

- [ ] **Step 6: Run migration and architecture tests**

Run:

```bash
scripts/test_backend_school.sh modules::academic::core::schema_tests::migration_058_reconciles_timetable_blocks -- --nocapture --test-threads=1
cargo test --test static_architecture active_migrations_are_clean_sequential_timeline
```

Expected: both pass.

- [ ] **Step 7: Commit the schema boundary**

```bash
git add backend-school/migrations/058_timetable_blocks.sql backend-school/src/modules/academic/core/schema_tests.rs backend-school/tests/static_architecture.rs
git commit -m "feat(timetable): add canonical block schema"
```

### Task 2: Introduce dedicated timetable permissions and resource policy

**Files:**
- Modify: `contracts/permissions.json`
- Modify: `contracts/permissions.lock.json`
- Modify: `backend-school/migrations/058_timetable_blocks.sql`
- Modify generated: `backend-school/src/permissions/registry_generated.rs`
- Modify generated: `frontend-school/src/lib/permissions/registry.ts`
- Create: `backend-school/src/policies/timetable_access_policy.rs`
- Modify: `backend-school/src/policies.rs`
- Test: `backend-school/src/policies/timetable_access_policy.rs`
- Test: `frontend-school/tests/static/timetable-block-permissions.test.mjs`

**Interfaces:**
- Produces generated constants for `academic_timetable.read.{assigned,organization_unit,organization_tree,school}`, `academic_timetable.manage.{assigned,organization_unit,organization_tree,school}`, and `academic_timetable.publish.school`.
- Produces `TimetableAction::{Read, Manage, Publish}` and `require_timetable_resources(&PgPool, &ActorContext, TimetableAction, &TimetableResourceSet) -> Result<(), AppError>`.

- [ ] **Step 1: Write permission contract and policy tests**

The static test must assert every exact code exists, timetable route metadata uses `ACADEMIC_TIMETABLE`, and Learning Offering manage is absent from timetable handlers/page. Policy tests cover assigned, organization-unit, organization-tree union, school, denied cross-target, and publish-school-only cases.

- [ ] **Step 2: Run the tests and confirm failure**

```bash
cd frontend-school
node --test tests/static/timetable-block-permissions.test.mjs
```

Expected: missing timetable permission module/constants and policy.

- [ ] **Step 3: Add source permission definitions and migration rows**

Add the nine codes to `contracts/permissions.json`. In migration 058, insert definitions idempotently, map existing active Learning Offering grants to equivalent timetable read/manage scopes, grant publish to roles that currently hold school-level Learning Offering manage, and leave Learning Offering definitions/grants intact for Delivery.

- [ ] **Step 4: Generate registries and implement policy**

```bash
cd frontend-school
npm run generate:permissions
```

The policy loads affected offering owners, learning-group assignments, homeroom organizational context, and whole-school structural scope once, combines independent scopes as a union, and fails closed when the actor lacks authority for any target.

- [ ] **Step 5: Run permission checks**

```bash
cd frontend-school
npm run check:permissions
npm run test:permissions
node --test tests/static/timetable-block-permissions.test.mjs
```

Expected: all pass.

- [ ] **Step 6: Commit permissions**

```bash
git add contracts/permissions.json contracts/permissions.lock.json backend-school/migrations/058_timetable_blocks.sql backend-school/src/permissions/registry_generated.rs frontend-school/src/lib/permissions/registry.ts backend-school/src/policies.rs backend-school/src/policies/timetable_access_policy.rs frontend-school/tests/static/timetable-block-permissions.test.mjs
git commit -m "feat(timetable): add dedicated scheduling permissions"
```

### Task 3: Define canonical typed block contracts

**Files:**
- Create: `backend-school/src/modules/academic/models/timetable_block.rs`
- Modify: `backend-school/src/modules/academic/models.rs`
- Modify: `backend-school/src/api_contract.rs`
- Test: `backend-school/src/api_contract.rs`

**Interfaces:**
- Produces `TimetableBlock`, `TimetableBlockGroup`, `TimetableBlockHomeroom`, `TimetableBlockTeacher`, `TimetableBlockSyncState`, `TimetableBlockWorkspace`, and `TimetableBlockSummary`.
- Produces tagged `TimetableBlockPlacementSource::{ExistingBlock, OrdinaryDemand, SynchronizedOffering}`.
- Produces requests `CreateOrdinaryTimetableBlockRequest`, `CreateSynchronizedTimetableBlockRequest`, `CreateStructuralTimetableBlocksRequest`, `UpdateTimetableBlockRequest`, `RemoveTimetableBlockTargetRequest`, `RetryTimetableBlockSyncRequest`, `RestoreTimetableBlockGroupRequest`, and `TimetableBlockPlacementPreviewRequest`.
- Produces `TimetableTargetKind::{Group, Homeroom, Teacher}` and sync status/conflict enums with snake-case wire values.

- [ ] **Step 1: Add failing OpenAPI shape tests**

Assert canonical schemas are registered, nullable IDs are explicitly nullable/required where shape requires them, tagged placement variants have concrete fields, old entry/batch schemas and operations are absent, and the workspace returns `blocks`, `synchronizedDemands`, and aggregate sync summaries.

- [ ] **Step 2: Run the focused contract test**

```bash
cargo test api_contract::tests::documents_canonical_timetable_blocks --bin backend-school
```

Expected: failure because block DTOs and paths are not registered.

- [ ] **Step 3: Implement the model file**

Use named Rust structs with `Serialize`, `Deserialize`, `ToSchema`, `IntoParams` as appropriate, `#[serde(rename_all = "camelCase", deny_unknown_fields)]` on inputs, and no `serde_json::Value` for block state. Preserve the frontend-needed period labels/times and exact instructor display fields in one bounded workspace response.

- [ ] **Step 4: Register schemas and expected operation IDs**

Use these operation IDs: `getTimetableBlockWorkspace`, `previewTimetableBlockPlacement`, `createOrdinaryTimetableBlock`, `createSynchronizedTimetableBlock`, `createStructuralTimetableBlocks`, `updateTimetableBlock`, `removeTimetableBlockTarget`, `restoreTimetableBlockGroup`, `retryTimetableBlockSync`, `deleteTimetableBlock`, `deleteTimetableBlockSeries`, and `swapTimetableBlocks`.

- [ ] **Step 5: Run focused contract tests**

```bash
cargo test api_contract::tests::documents_canonical_timetable_blocks --bin backend-school
```

Expected: pass.

- [ ] **Step 6: Commit typed models**

```bash
git add backend-school/src/modules/academic/models.rs backend-school/src/modules/academic/models/timetable_block.rs backend-school/src/api_contract.rs
git commit -m "feat(timetable): define block API contracts"
```

### Task 4: Implement block reads, mutations, collision checks, and sync engine

**Files:**
- Create: `backend-school/src/modules/academic/services/timetable_block_queries.rs`
- Create: `backend-school/src/modules/academic/services/timetable_block_conflicts.rs`
- Create: `backend-school/src/modules/academic/services/timetable_block_sync.rs`
- Create: `backend-school/src/modules/academic/services/timetable_block_service.rs`
- Modify: `backend-school/src/modules/academic/services.rs`
- Delete after consumer cutover: `backend-school/src/modules/academic/services/timetable_service.rs`
- Replace tests: `backend-school/src/modules/academic/services/timetable_service_tests.rs`
- Create tests: `backend-school/src/modules/academic/services/timetable_block_service_tests.rs`

**Interfaces:**
- `get_workspace(pool, query, access) -> Result<TimetableBlockWorkspace, AppError>` performs set-based reads.
- `preview_placement(pool, request) -> Result<TimetableBlockPlacementPreview, AppError>` returns stable conflict codes.
- `create_ordinary_block`, `create_synchronized_block`, `create_structural_blocks`, `update_block`, `swap_blocks`, `remove_target`, `restore_group`, `retry_sync`, `deactivate_block`, and `deactivate_series` return changed typed resources.
- `sync_offering_groups_in_tx(&mut Transaction<Postgres>, block_id, actor_id) -> Result<Vec<TimetableBlockSyncState>, AppError>` applies each group independently through savepoints.

- [ ] **Step 1: Port focused tests to canonical terms and make them fail**

Cover solo/split/co-teaching, one-period create/move/swap, same-block overlap, cross-block group/homeroom/teacher/room conflicts, structural target deletion, synchronized zero-group creation, partial sync, sticky exclusion, restore, stale rows, and published immutability.

- [ ] **Step 2: Run service tests against the migrated schema**

```bash
scripts/test_backend_school.sh modules::academic::services::timetable_block_service_tests -- --nocapture --test-threads=1
```

Expected: failure because the new services do not exist.

- [ ] **Step 3: Implement set-based query hydration**

Load blocks, all child targets, exact instructors, learning-group coverage, room labels, activity scheduling mode, targets, and eligible instructors in bounded queries. Group with ordered maps in Rust; never query once per block/target.

- [ ] **Step 4: Implement normalized create/update/move/swap transactions**

Normalize day, UUID sets, teacher order, structural target sets, and series slots before writes. Lock version and slots in stable order, require draft status, check row versions, call the shared conflict engine, write audit metadata without raw payloads, and map named database checks to typed `409` conflicts.

- [ ] **Step 5: Implement per-group synchronized sync**

Resolve eligible Delivery teachers for each group. Use a savepoint per group so one conflict records `CONFLICT` and later groups continue. Missing teachers record `WAITING_FOR_DATA`; outside reservation records `OUTSIDE_SCOPE`; `EXCLUDED` remains untouched; success replaces the exact group instructor set and records `LINKED`.

- [ ] **Step 6: Implement target and series removal**

Course/independent deletion deactivates the whole block. Synchronized group removal deactivates only the group allocation and records `EXCLUDED`. Structural homeroom/teacher removal deactivates only that target and deactivates an empty block. Series removal requires normalized series ID plus version and deactivates every member atomically.

- [ ] **Step 7: Run service tests**

```bash
scripts/test_backend_school.sh modules::academic::services::timetable_block_service_tests -- --nocapture --test-threads=1
```

Expected: pass.

- [ ] **Step 8: Commit block services**

```bash
git add backend-school/src/modules/academic/services.rs backend-school/src/modules/academic/services/timetable_block_queries.rs backend-school/src/modules/academic/services/timetable_block_conflicts.rs backend-school/src/modules/academic/services/timetable_block_sync.rs backend-school/src/modules/academic/services/timetable_block_service.rs backend-school/src/modules/academic/services/timetable_block_service_tests.rs backend-school/src/modules/academic/services/timetable_service_tests.rs
git commit -m "feat(timetable): implement canonical block services"
```

### Task 5: Cut Delivery, versioning, templates, and downstream readers to blocks

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/services/change_sets.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/groups.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/teacher_handoff.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/workspaces.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_version_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_version_service_tests.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_template_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_template_service_tests.rs`
- Modify: `backend-school/src/modules/academic/services/daily_teaching_service.rs`
- Modify: `backend-school/src/modules/supervision/services/observations.rs`
- Modify: `backend-school/src/modules/parents/services.rs`
- Modify any additional active source returned by `rg -l 'academic_timetable_entries|timetable_entry_instructors' backend-school/src`.

**Interfaces:**
- Delivery group create/publish/update calls `retry_sync_for_group(pool, group_id, actor_id)` for matching draft synchronized blocks.
- Version clone copies blocks, every target, exact instructors, sync state, and series identity with new IDs and deterministic maps.
- Template apply creates blocks/targets through the block service instead of direct SQL.
- Personal, student, parent, daily teaching, supervision, export, and readiness reads use canonical blocks.

- [ ] **Step 1: Add failing cross-consumer tests**

Add tests proving a cloned version preserves block structure and instructors; Delivery creation partially syncs; staff/student/parent daily reads agree; supervision resolves a block group; template apply creates canonical blocks; and no active Rust source contains old table names after cutover.

- [ ] **Step 2: Run focused consumer tests**

```bash
scripts/test_backend_school.sh modules::academic::delivery::services_tests modules::academic::services::timetable_version_service_tests modules::academic::services::timetable_template_service_tests -- --nocapture --test-threads=1
```

Expected: failure against removed legacy tables.

- [ ] **Step 3: Port version clone and operational change transactions**

Clone parent blocks first, then child targets/instructors/sync states through ID maps. Teacher handoff replaces instructors on block groups. Stopping offerings removes their draft blocks. Publishing validates canonical readiness only.

- [ ] **Step 4: Connect Delivery synchronization**

After a group/teacher mutation in a draft academic context, find matching synchronized blocks in the target draft and call the sync service. When only a published version exists, use the existing operational-change draft boundary; never mutate published children.

- [ ] **Step 5: Port templates and all readers**

Template source/apply uses block source and targets. Daily/self/student/parent/supervision readers resolve published version by date, then join block slot plus exact targets. Structural teacher targets appear in staff schedules; synchronized student schedules resolve roster membership to a linked block group.

- [ ] **Step 6: Prove old runtime SQL is gone**

```bash
rg -n "academic_timetable_entries|timetable_entry_instructors" backend-school/src
```

Expected: no active runtime matches; test-only historical fixture text may be moved to migration-specific tests only.

- [ ] **Step 7: Run focused consumer tests**

```bash
scripts/test_backend_school.sh modules::academic::delivery::services_tests modules::academic::services::timetable_version_service_tests modules::academic::services::timetable_template_service_tests -- --nocapture --test-threads=1
```

Expected: pass.

- [ ] **Step 8: Commit consumer cutover**

```bash
git add backend-school/src/modules/academic backend-school/src/modules/supervision backend-school/src/modules/parents
git commit -m "refactor(timetable): cut consumers to canonical blocks"
```

### Task 6: Replace handlers, routes, OpenAPI paths, and realtime messages

**Files:**
- Create: `backend-school/src/modules/academic/handlers/timetable_blocks.rs`
- Modify: `backend-school/src/modules/academic/handlers.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Delete: `backend-school/src/modules/academic/handlers/timetable.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_realtime_service.rs`
- Modify: `backend-school/src/modules/academic/websockets.rs`
- Modify: `backend-school/src/api_contract.rs`
- Test: `backend-school/src/api_contract.rs`
- Test: `frontend-school/tests/static/timetable-request-performance.test.mjs`

**Interfaces:**
- Routes live under `/api/academic/timetable-blocks` plus `/workspace`, `/placement-preview`, `/ordinary`, `/synchronized`, `/structural`, `/sync`, `/targets`, `/series`, and `/swap` actions.
- Realtime `TimetableChanged` carries academic term, timetable version, block ID, and summary revision as an invalidation signal.

- [ ] **Step 1: Update route/contract tests to require only block operations**

Tests assert old `/api/academic/timetable`, `/batch`, `/batch-group`, and entry mutation operations are absent; personal/daily endpoints retain their public paths but return block-derived display DTOs.

- [ ] **Step 2: Implement thin handlers and exact resource authorization**

Each handler resolves actor/tenant context, calls `require_timetable_resources`, invokes one service method, wraps the typed result in `ApiResponse`, and broadcasts only after a successful commit. No SQL or raw permission strings enter handlers.

- [ ] **Step 3: Register routes and OpenAPI**

Literal action paths precede `/{block_id}`. Register every path/schema in `api_contract.rs`; remove old entry schemas and operation IDs.

- [ ] **Step 4: Run backend contract tests**

```bash
cargo test api_contract::tests::documents_canonical_timetable_blocks --bin backend-school
cargo test --test static_architecture
```

Expected: pass.

- [ ] **Step 5: Commit HTTP/realtime cutover**

```bash
git add backend-school/src/modules/academic.rs backend-school/src/modules/academic/handlers.rs backend-school/src/modules/academic/handlers/timetable_blocks.rs backend-school/src/modules/academic/handlers/timetable.rs backend-school/src/modules/academic/services/timetable_realtime_service.rs backend-school/src/modules/academic/websockets.rs backend-school/src/api_contract.rs frontend-school/tests/static/timetable-request-performance.test.mjs
git commit -m "refactor(timetable): expose canonical block APIs"
```

### Task 7: Regenerate API contracts and add the typed frontend block client/state

**Files:**
- Modify generated: `contracts/openapi/school-api.json`
- Modify generated: `frontend-school/src/lib/api/generated/school-api.ts`
- Replace: `frontend-school/src/lib/api/timetable.ts`
- Replace: `frontend-school/src/lib/academic/timetable/board-state.ts`
- Modify: `frontend-school/src/lib/academic/timetable/board-state.test.ts`
- Replace: `frontend-school/src/lib/academic/timetable/workspace-controller.svelte.ts`
- Test: `frontend-school/tests/static/timetable-block-contract.test.mjs`

**Interfaces:**
- Frontend exports only generated `TimetableBlock*` wire types plus explicit UI view models.
- Controller stores selected view/owner, per-demand selected instructor IDs, one drag source, preview, and pending mutation.
- `completePlacement(demandId)` clears teacher selection; `failPlacement()` preserves it; `cancelPlacement(demandId)` clears it.

- [ ] **Step 1: Generate the OpenAPI/TypeScript contract**

```bash
cd frontend-school
npm run generate:api-contracts
```

- [ ] **Step 2: Write failing frontend contract/state tests**

Assert old entry/batch API functions are absent, all block methods use generated request/response types, workspace indexing is set-based, same block aggregates across homerooms, and instructor selection reset/preserve rules are exact.

- [ ] **Step 3: Implement the block API wrapper**

Use concrete generated operation request/query types and the standard `ApiResponse` envelope. Preserve typed `409` handling and required academic term/version validation. Do not use casts, `unknown`, or known-shape records.

- [ ] **Step 4: Implement block board state/controller**

Index blocks by slot, target, group, homeroom, teacher, and room once. Aggregate synchronized/structural blocks by block ID. Keep teacher selections keyed by ordinary demand ID so multiple cards never share selection.

- [ ] **Step 5: Run frontend contract/state tests**

```bash
cd frontend-school
npm run check:api-contracts
npm run test:api-contracts
node --experimental-strip-types --test src/lib/academic/timetable/board-state.test.ts
node --test tests/static/timetable-block-contract.test.mjs
```

Expected: pass.

- [ ] **Step 6: Commit generated and state cutover**

```bash
git add contracts/openapi/school-api.json frontend-school/src/lib/api/generated/school-api.ts frontend-school/src/lib/api/timetable.ts frontend-school/src/lib/academic/timetable/board-state.ts frontend-school/src/lib/academic/timetable/board-state.test.ts frontend-school/src/lib/academic/timetable/workspace-controller.svelte.ts frontend-school/tests/static/timetable-block-contract.test.mjs
git commit -m "refactor(timetable): consume generated block contracts"
```

### Task 8: Build the direct drag tray and compact block cards

**Files:**
- Modify: `frontend-school/src/lib/components/academic/timetable/TimetableUnscheduledTray.svelte`
- Replace: `frontend-school/src/lib/components/academic/timetable/TimetableLessonCard.svelte`
- Modify: `frontend-school/src/lib/components/academic/timetable/TimetableCell.svelte`
- Modify: `frontend-school/src/lib/components/academic/timetable/TimetableBoard.svelte`
- Replace: `frontend-school/src/lib/components/academic/timetable/TimetableEntryInspector.svelte`
- Replace: `frontend-school/src/lib/components/academic/timetable/TimetableInstructorPicker.svelte`
- Modify: `frontend-school/src/lib/components/academic/timetable/TimetableMoveDialog.svelte`
- Test: `frontend-school/tests/static/timetable-drag-board-components.test.mjs`
- Test: `frontend-school/tests/e2e/timetable-drag-board.spec.ts`

**Interfaces:**
- Tray emits `dragstart`, `place`, and `cancel` with `TimetableBlockPlacementSource` plus exact instructors.
- Lesson card emits `move`, `inspect`, and context-specific `removeTarget` or `removeBlock` events.

- [ ] **Step 1: Read both Svelte skills and run the current component tests**

Invoke `svelte:svelte-code-writer` and `svelte:svelte-core-bestpractices`, then run:

```bash
cd frontend-school
node --test tests/static/timetable-drag-board-components.test.mjs
```

- [ ] **Step 2: Update tests for the approved interaction**

Require shadcn-svelte multi-select for multiple eligible teachers, automatic one-teacher selection, disabled drag without selection, dedicated drag handle, compact trash button, no large move/edit/remove row, synchronized card without teacher selector, and accessible non-color target text.

- [ ] **Step 3: Implement ordinary and synchronized tray cards**

Ordinary cards show code/title/group/remaining count and exact selection summary. Synchronized cards show reserved homerooms and linked/waiting/conflict/excluded counts. Successful placement/cancel clears the relevant card; failed drop keeps its selection.

- [ ] **Step 4: Implement compact block cards and board feedback**

Drag from a handle, click body to inspect, and use one small context-aware trash icon. The drag ghost includes title, audience/group, and teacher summary. Board cells expose `วางได้`, `สลับคาบ`, or exact conflict text to assistive technology.

- [ ] **Step 5: Run Svelte analyzer/autofixer serially on every changed component**

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic/timetable/TimetableUnscheduledTray.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic/timetable/TimetableLessonCard.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic/timetable/TimetableCell.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic/timetable/TimetableBoard.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer src/lib/components/academic/timetable/TimetableEntryInspector.svelte --svelte-version 5
```

- [ ] **Step 6: Run component and focused browser tests**

```bash
cd frontend-school
node --test tests/static/timetable-drag-board-components.test.mjs
npx playwright test tests/e2e/timetable-drag-board.spec.ts --project=chromium
```

Expected: pass when the configured E2E environment is available; otherwise report the exact missing runtime/account.

- [ ] **Step 7: Commit direct drag UI**

```bash
git add frontend-school/src/lib/components/academic/timetable frontend-school/tests/static/timetable-drag-board-components.test.mjs frontend-school/tests/e2e/timetable-drag-board.spec.ts
git commit -m "feat(timetable): add direct teacher-aware drag cards"
```

### Task 9: Add synchronized/structural workflows and cut the page to block permissions

**Files:**
- Create: `frontend-school/src/lib/components/academic/timetable/TimetableStructuralBlockDialog.svelte`
- Create: `frontend-school/src/lib/components/academic/timetable/TimetableSyncStatusSheet.svelte`
- Modify: `frontend-school/src/lib/components/academic/timetable/TimetableTeacherView.svelte`
- Modify: `frontend-school/src/lib/components/academic/timetable/TimetableWholeSchoolOverview.svelte`
- Modify: `frontend-school/src/lib/components/academic/timetable/TimetableIssueSummary.svelte`
- Replace: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/today/+page.ts`
- Test: `frontend-school/tests/static/timetable-block-workflow.test.mjs`
- Test: `frontend-school/tests/e2e/timetable-teacher-board.spec.ts`
- Test: `frontend-school/tests/e2e/timetable-whole-school-overview.spec.ts`

**Interfaces:**
- Structural dialog submits normalized days, periods, homeroom IDs, teacher IDs, kind, title, room, and note.
- Sync sheet exposes retry, exclude/remove, restore, exact conflict reason, and linked allocation navigation.
- Page requests management options only when exact `academic_timetable.manage.*` passes.

- [ ] **Step 1: Add failing workflow/static tests**

Require the structural dialog audiences, one target removal, explicit block/series delete confirmation, synchronized aggregate status, read-only published state, draft clone prompt, and exact generated permission usage.

- [ ] **Step 2: Implement structural and sync surfaces**

Use local shadcn-svelte Dialog, Select/Combobox, Checkbox, Badge, Alert, Button, and Sheet primitives. Normalize/deduplicate target IDs before submission. Do not fabricate offerings/groups for structural blocks.

- [ ] **Step 3: Replace page entry orchestration with block orchestration**

Load one bounded workspace, patch returned blocks locally after mutation, invalidate whole-school cache by summary revision, and preserve URL year/term/version/view/owner context. The published edit prompt clones/reuses a draft before mutation.

- [ ] **Step 4: Update teacher/whole-school/today presentations**

Teacher view aggregates same-block participation and moves the full block. Whole-school remains read-only and links to exact editable owner/slot. Today view reads block-derived entries and retains its separate daily permission.

- [ ] **Step 5: Run Svelte analyzer/autofixer serially**

Run `npx @sveltejs/mcp svelte-autofixer <file> --svelte-version 5` separately for every changed `.svelte` file and resolve every diagnostic.

- [ ] **Step 6: Run focused static/browser tests**

```bash
cd frontend-school
node --test tests/static/timetable-block-workflow.test.mjs tests/static/timetable-request-performance.test.mjs tests/static/timetable-whole-school-overview.test.mjs
npx playwright test tests/e2e/timetable-teacher-board.spec.ts tests/e2e/timetable-whole-school-overview.spec.ts --project=chromium
```

Expected: pass when E2E credentials/environment are available; otherwise report unrun prerequisites exactly.

- [ ] **Step 7: Commit workflow UI**

```bash
git add frontend-school/src/lib/components/academic/timetable frontend-school/src/routes/'(app)'/staff/academic/timetable frontend-school/tests/static frontend-school/tests/e2e
git commit -m "feat(timetable): add synchronized and structural workflows"
```

### Task 10: Full serial verification, clean artifacts, push, and deploy

**Files:**
- Modify as required by failures: touched implementation/test/contract files only.
- Remove after implementation: `docs/superpowers/specs/2026-09-02-timetable-blocks-and-synchronized-activities-design.md`
- Remove after implementation: `docs/superpowers/plans/2026-09-02-timetable-blocks-and-synchronized-activities.md`

**Interfaces:**
- Produces one main commit range whose backend/frontend/schema/contracts are deployable together.
- Push to `origin/main` triggers backend-school and school frontend deployment workflows.

- [ ] **Step 1: Run all focused backend database suites serially**

```bash
scripts/test_backend_school.sh modules::academic::core::schema_tests::migration_058_reconciles_timetable_blocks -- --nocapture --test-threads=1
scripts/test_backend_school.sh modules::academic::services::timetable_block_service_tests -- --nocapture --test-threads=1
scripts/test_backend_school.sh modules::academic::delivery::services_tests -- --nocapture --test-threads=1
scripts/test_backend_school.sh modules::academic::services::timetable_version_service_tests -- --nocapture --test-threads=1
scripts/test_backend_school.sh modules::academic::services::timetable_template_service_tests -- --nocapture --test-threads=1
```

- [ ] **Step 2: Run backend verification matrix serially**

```bash
cd backend-school
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check
```

- [ ] **Step 3: Run permission and API contract gates serially**

```bash
cd frontend-school
npm run generate:permissions
npm run check:permissions
npm run test:permissions
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
```

- [ ] **Step 4: Run frontend verification matrix serially**

```bash
cd frontend-school
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

- [ ] **Step 5: Run configured browser/sandbox checks**

```bash
cd frontend-school
npx playwright test tests/e2e/timetable-drag-board.spec.ts tests/e2e/timetable-teacher-board.spec.ts tests/e2e/timetable-whole-school-overview.spec.ts --project=chromium
```

Run the authenticated sandbox smoke only through configured secret environment/workflow. Do not print credentials. If local credentials are unavailable, leave the check for the deployment workflow and report it as unrun locally.

- [ ] **Step 6: Remove completed workflow artifacts and review the repository**

Delete the completed design/plan files as required by `.rules`, then run:

```bash
git diff --check
git status --short
rg -n "academic_timetable_entries|timetable_entry_instructors|/api/academic/timetable/batch|LEARNING_OFFERING_MANAGE.*timetable" backend-school/src frontend-school/src
```

Expected: no whitespace errors, only intended changes, and no legacy runtime matches.

- [ ] **Step 7: Commit final generated/cleanup corrections**

```bash
git add -A
git commit -m "feat(timetable): complete canonical block cutover"
```

- [ ] **Step 8: Push and monitor deployment**

```bash
git push origin main
gh run list --branch main --limit 20
```

Wait for required test, contract, backend-school, and frontend tenant workflows. If migration/preflight fails, confirm maintenance remains enabled, inspect only sanitized workflow diagnostics, fix forward with a new migration/code commit, and never edit migration 058 after any tenant applies it.

- [ ] **Step 9: Verify deployed behavior**

Confirm `sandbox.schoolorbit.app` and `snwsb.schoolorbit.app` load the timetable workspace, create an ordinary A/B/A+B placement, create a synchronized block without groups, create/remove one structural target, and show staff/student/parent schedules without 4xx or request-per-target fan-out. Keep the Neon snapshot until these checks pass and the rollback window is explicitly closed.
