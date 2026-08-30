# Timetable Effective Teacher Change and Handoff — Release 4 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the existing Delivery “เพิ่ม/ปรับ/หยุดกลางภาค” workflow to effective-dated teacher assignments, with an explicit and conflict-safe handoff into the cloned draft timetable before publication.

**Architecture:** Teacher changes become typed items in `academic_term_change_sets`. Publishing a change set starts or closes effective-dated `learning_group_teachers` episodes on the change set’s `effectiveFrom`; it never rewrites history. The cloned target timetable keeps exact period instructors, and a separate preview/apply handoff contract lets the user choose one replacement, a co-teacher set, or manual timetable editing. Readiness blocks publication while any target entry still references a teacher who is not effective for that group/date, and publication atomically promotes the already-correct target version.

**Tech Stack:** PostgreSQL migrations/triggers, Rust, Axum, SQLx, utoipa/OpenAPI, TypeScript, Svelte 5 runes, shadcn-svelte, Node static tests, Playwright.

**Spec:** `docs/superpowers/specs/2026-08-30-timetable-drag-drop-and-effective-teacher-assignment-design.md`

## Global Constraints

- Releases 1–3 must already be deployed and verified.
- Read `.rules` before implementation and follow the database/API/frontend verification matrix.
- Run commands serially, set `CARGO_BUILD_JOBS=1`, and use Playwright `--workers=1`.
- Never edit migrations 001–054. Add migration `055_academic_effective_teacher_changes.sql` and a new migration test.
- Never delete or rewrite an effective teacher episode that has already affected a published timetable. A change set appends a new episode or closes the prior open episode at `effectiveFrom - 1 day`.
- A change set’s `effectiveFrom` is domain data. Publishing may happen earlier; date-based resolvers choose the effective assignment/version only when that date arrives.
- The end date of the academic term is not required when adding a teacher; an episode may have `endsOn = null`.
- Teacher assignment role (`primary|secondary|assistant`) describes group responsibility. Entry instructor role (`primary|secondary`) describes one scheduled period. Do not conflate or auto-map assistant responsibility into a period.
- Adding a group teacher does not automatically add that teacher to timetable entries. Stopping a teacher does not automatically choose a replacement.
- Handoff is explicit and applies only to selected entries in the target draft version. `assign_one`, `assign_coteachers`, and `manual` are the only supported modes.
- `manual` performs no mutation; it deep-links to the target draft timetable and readiness remains blocked until every invalid exact instructor is resolved.
- A handoff preview and apply are all-or-nothing under change-set, timetable-version, entry, and teacher-assignment row versions. No partial entry updates.
- A conflict never overwrites another timetable entry. The user resolves it on the timetable board by moving/swapping entries or changing the proposed exact teacher set.
- Published timetable versions and published/cancelled change sets remain immutable.
- Use generated API and permission contracts. Remove superseded runtime compatibility paths; do not support two payload shapes.
- Use Svelte skills before any `.svelte`/`.svelte.ts` edit and run the autofixer. Use `frontend-design` for the new handoff panel.
- Do not push, deploy, or run live tenant SQL without explicit user approval and a current Neon snapshot.

---

### Task 1: Add migration 055 for typed teacher change items and append-only episodes

**Files:**
- Create: `backend-school/migrations/055_academic_effective_teacher_changes.sql`
- Modify: `backend-school/src/modules/academic/core/schema_tests.rs`

**Schema changes:**
- Make `academic_term_change_items.learning_offering_id` nullable after dropping/replacing the context FK and old action-shape/unique constraints.
- Add nullable `learning_group_id`, `learning_group_teacher_id`, `teacher_id`, and `teacher_role`.
- Extend `action_kind` with `add_group_teacher`, `adjust_group_teacher_role`, and `stop_group_teacher`.
- Add context FKs for group, assignment episode, and teacher; use `ON DELETE RESTRICT` for academic/user references.
- Add action-shape check:
  - offering actions keep the Release 2 shape;
  - add teacher requires group + teacher + role, no assignment/offering/weekly target;
  - adjust role requires group + assignment + teacher + role, no offering/weekly target;
  - stop teacher requires group + assignment + teacher, role null, no offering/weekly target.
- Add context-safe composite keys/FKs so a teacher episode ID must match the named group, teacher, term, and year.
- Replace the old all-action unique constraint with partial unique indexes for offering actions and with indexes that allow at most one add per `(change_set_id, learning_group_id, teacher_id)`, one role adjustment per `(change_set_id, learning_group_teacher_id)`, and one stop per `(change_set_id, learning_group_teacher_id)`.
- Create `academic_teacher_handoff_runs` with unique `idempotency_key`, change-set/item context, canonical 64-character request hash, actor/time, selected entry IDs, and immutable JSONB response snapshot. This is the authoritative replay receipt; do not reuse an unrelated offering-apply table.
- Replace the Release 1 published-group hard lock trigger with a provenance-aware guard: direct mutations on a published/closed group remain blocked; a transaction may insert a new episode or close one open episode only when `change_set_id` references the currently publishing change set and dates/row versions satisfy the append-only rules.

- [ ] **Step 1: Add a failing migration-055 schema test**

Use `phase_a_fixture`, `record_passing_phase_a_reconciliation_marker`, and `apply_migrations_through` following the migration 052/053 tests. Assert migration 055:

- preserves offering, group, assignment, timetable version, entry, and instructor counts;
- accepts each valid teacher action shape;
- rejects every mixed/partial shape;
- rejects duplicate add/adjust/stop items;
- rejects direct teacher insert/update/delete on published groups;
- permits provenance-bound add and one-way close during a publishing transaction;
- rejects changing a closed episode’s identity/start/date or reopening it;
- rejects stop/role-adjust when `effectiveFrom <= startsOn`, which would create an empty or reversed historical episode;
- enforces handoff receipt idempotency key and request-hash shape;
- leaves all new guard triggers enabled.

- [ ] **Step 2: Run the schema test and capture the expected failure**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_055 -- --test-threads=1
```

Expected: FAIL because migration 055 does not exist.

- [ ] **Step 3: Implement migration preflight before DDL**

Abort with stable error codes when existing data cannot satisfy the new model, including:

```text
ACADEMIC_055_UNKNOWN_CHANGE_ACTION
ACADEMIC_055_INVALID_TEACHER_EPISODE
ACADEMIC_055_OVERLAPPING_TEACHER_EPISODE
ACADEMIC_055_UNMAPPABLE_CHANGE_ITEM
```

Record pre-migration counts in a temporary table. Do not repair or delete tenant data in this migration.

- [ ] **Step 4: Implement DDL, indexes, and provenance-aware triggers**

Reuse Release 1’s teacher episode columns and overlap guard. The publication service must set a transaction-local setting such as `schoolorbit.academic_change_set_id`; the trigger verifies that value, the referenced published change set, and row `change_set_id` before permitting a published-group append/close. Reset is automatic at transaction end.

For a close, require `NEW.ends_on = change_set.effective_from - 1`, `OLD.ends_on IS NULL OR OLD.ends_on >= change_set.effective_from`, and every other field unchanged except row version/update metadata/change provenance.

- [ ] **Step 5: Add migration postflight assertions**

Verify row preservation, constraints/indexes/triggers, an empty handoff receipt table, no invalid action rows, no overlapping teacher episode ranges, and exactly one enabled guard trigger per protected table.

- [ ] **Step 6: Re-run focused schema tests**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_055 -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 7: Commit the migration slice**

```bash
git add backend-school/migrations/055_academic_effective_teacher_changes.sql \
  backend-school/src/modules/academic/core/schema_tests.rs
git commit -m "feat(academic): add effective teacher change schema"
```

---

### Task 2: Extend typed change-set models and item editing

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/change_sets.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Modify: `backend-school/src/modules/academic/delivery/handlers.rs`
- Modify: `backend-school/src/api_contract.rs`

**Interfaces:**
- Add action kinds `AddGroupTeacher`, `AdjustGroupTeacherRole`, `StopGroupTeacher`.
- Add matching variants to `AcademicTermChangeItem` and `UpsertAcademicTermChangeItemRequest`.
- Requests include change-set/item row versions plus the exact group/teacher/assignment IDs required by the action.
- Returned teacher items include stable teacher/group display labels, role, and the affected assignment episode ID where applicable so the frontend needs no lookup fanout.
- Extend findings with `MissingEffectiveTeacher`, `StoppedTeacherStillScheduled`, `EntryInstructorNotEffective`, `TeacherHandoffConflict`, and `TeacherHandoffStale`.

- [ ] **Step 1: Write failing item CRUD and authorization tests**

Cover add, adjust role, stop, edit, and delete in a draft change set. Assert:

- the group belongs to the change-set academic context;
- teacher IDs identify eligible active staff;
- adjust/stop target an effective assignment episode for that group;
- add cannot overlap an existing episode at `effectiveFrom` unless an earlier episode is closed by a matching stop in the same change set;
- a teacher cannot be both added and stopped ambiguously in one set;
- stop/role-adjust requires `effectiveFrom > startsOn`;
- stale item/change-set versions return conflict;
- published/cancelled sets reject edits;
- permission and organization-unit scoping match existing change item operations.

- [ ] **Step 2: Run focused tests and capture failure**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::teacher_change -- --test-threads=1
```

Expected: FAIL because teacher actions are not modeled.

- [ ] **Step 3: Extend row hydration and typed request helpers**

Replace assumptions that every item has `learning_offering_id`. Update `change_set_row_version`, organization scope, resource lookup, normalized request hashing, item list hydration, deterministic ordering, and delete cleanup for all six action kinds. Never serialize nullable teacher action columns into offering variants or vice versa.

- [ ] **Step 4: Implement teacher item upsert/delete transactions**

Lock the change set, target group/assignment, and competing items in stable UUID order. Update the change-set row version once per successful mutation. Keep add/adjust/stop idempotent for the same normalized request while rejecting a reused item identity with different content.

- [ ] **Step 5: Extend OpenAPI and handler response declarations**

The existing item endpoint remains the only mutation route; its discriminated union expands. Register all new schemas and finding enum values.

- [ ] **Step 6: Run focused delivery tests**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 7: Commit typed teacher change items**

```bash
git add backend-school/src/modules/academic/delivery/models.rs \
  backend-school/src/modules/academic/delivery/services/change_sets.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs \
  backend-school/src/modules/academic/delivery/handlers.rs \
  backend-school/src/api_contract.rs
git commit -m "feat(academic): manage teacher change items"
```

---

### Task 3: Add conflict-safe timetable handoff preview and apply APIs

**Files:**
- Create: `backend-school/src/modules/academic/delivery/services/teacher_handoff.rs`
- Create: `backend-school/src/modules/academic/services/effective_teacher_service.rs`
- Modify: `backend-school/src/modules/academic/delivery/services.rs`
- Modify: `backend-school/src/modules/academic/services.rs`
- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Modify: `backend-school/src/modules/academic/delivery/handlers.rs`
- Modify: `backend-school/src/modules/academic.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`

**Interfaces:**
- `TeacherHandoffMode::{AssignOne, AssignCoteachers, Manual}`.
- `PreviewTeacherHandoffRequest { change_set_row_version, target_timetable_version_row_version, teacher_change_item_id, entry_ids, mode, instructor_ids }`.
- `TeacherHandoffPreview { change_set_id, teacher_change_item_id, affected_entries, proposed_entries, conflicts, preview_hash, can_apply }`.
- `ApplyTeacherHandoffRequest` repeats row versions, normalized choice, selected entry IDs, each entry row version, preview hash, and idempotency key.
- `TeacherHandoffEntryPreview` contains before/after exact instructor sets and slot/resource labels.
- Add `POST /api/academic/term-change-sets/{id}/teacher-handoff/preview`, operation ID `previewTeacherHandoff`.
- Add `POST /api/academic/term-change-sets/{id}/teacher-handoff/apply`, operation ID `applyTeacherHandoff`.

**Mode semantics:**
- `assign_one`: replace the stopped/closing teacher on selected target entries with exactly one eligible `instructorId`; retain other co-teachers and preserve the vacated period role.
- `assign_coteachers`: replace the stopped/closing teacher with the supplied non-empty deduplicated exact instructor set; retain pre-existing co-teachers not being replaced, choose primary deterministically only when the removed instructor was the sole primary, and show the resulting roles in preview.
- `manual`: `instructorIds` and apply are forbidden; preview returns affected entries plus a timetable deep link and no mutation hash.
- Add/role-adjust actions may preview their affected entries but only a stop/close action requires replacement/removal handoff. Adding a teacher never alters entries automatically.
- Preview/apply requires manage access to every distinct learning group in the selected entries; school-wide structural entries require the existing school-manage authority.

- [ ] **Step 1: Write failing preview tests for all modes**

Assert preview:

- considers only entries in the target draft version;
- defaults affected entries to those containing the stopped teacher but accepts an explicit selected subset;
- rejects a replacement not effective/eligible for the group on `effectiveFrom` after applying pending change items;
- evaluates conflicts across all proposed exact instructors;
- catches the replacement teacher’s collision, homeroom/group/room collisions, and duplicate IDs;
- leaves database rows unchanged;
- produces a deterministic hash independent of input ID order;
- returns manual deep-link query with exact year/term/version/view/owner context.

- [ ] **Step 2: Run preview tests and capture failure**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::teacher_handoff_preview \
  -- --test-threads=1
```

Expected: FAIL because the handoff service does not exist.

- [ ] **Step 3: Implement pending effective assignment projection**

Build one shared `effective_teacher_service` function that projects effective teacher episodes at `effectiveFrom` after applying the draft teacher items in memory. Use it in handoff eligibility, timetable workspace/mutations, and change-set readiness so they cannot disagree. It must handle stop-old/add-new in the same change set and role adjustment without touching live rows.

- [ ] **Step 4: Implement preview using the shared timetable conflict evaluator**

Reuse Release 2’s exact conflict evaluator. Exclude each source entry from its own slot occupancy, but do not move slots during handoff. Because teacher changes can create conflicts between multiple selected entries, evaluate the proposed set as a batch, not entry-by-entry against only current DB state.

- [ ] **Step 5: Write failing atomic apply tests**

Cover successful solo and co-teacher replacement, repeated idempotency key, stale preview/change set/version/entry, changed eligible assignment, new conflict after preview, and one invalid entry in a multi-entry apply. Assert every failure leaves all instructor child rows and row versions unchanged.

- [ ] **Step 6: Implement all-or-nothing apply**

In one SQL transaction:

1. lock change set, target draft version, teacher item, selected entries, and their instructor rows in stable order;
2. recompute normalized preview/hash;
3. reject stale/conflicting requests;
4. replace exact instructor children for every selected entry;
5. increment each entry row version/update metadata;
6. persist the immutable request/response snapshot in `academic_teacher_handoff_runs`; the same key/hash replays that snapshot and the same key with different content returns conflict;
7. return updated entries and refreshed handoff preview.

In the same transaction, append one audit event per affected entry containing before/after exact instructor IDs, child roles, entry/version/change-set IDs, effective date, actor, and row versions. The handler broadcasts timetable invalidation for each returned entry only after commit.

- [ ] **Step 7: Add handlers/routes/contracts and run tests**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::teacher_handoff -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 8: Commit handoff APIs**

```bash
git add backend-school/src/modules/academic/delivery/services/teacher_handoff.rs \
  backend-school/src/modules/academic/services/effective_teacher_service.rs \
  backend-school/src/modules/academic/delivery/services.rs \
  backend-school/src/modules/academic/services.rs \
  backend-school/src/modules/academic/delivery/models.rs \
  backend-school/src/modules/academic/delivery/handlers.rs \
  backend-school/src/modules/academic.rs backend-school/src/api_contract.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs
git commit -m "feat(timetable): preview and apply teacher handoff"
```

---

### Task 4: Apply teacher episodes and block unsafe change-set publication

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/services/change_sets.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Modify: `backend-school/src/modules/academic/models/timetable.rs`
- Modify: `backend-school/src/modules/academic/services/effective_teacher_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_version_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service_tests.rs`

**Publication rules:**
- Preview reports every target entry whose exact instructor is not effective for that group/date after pending teacher changes.
- `StoppedTeacherStillScheduled` and `EntryInstructorNotEffective` are blocking.
- Add/adjust/stop teacher item impacts are included in `AcademicChangeImpactCounts.teacher_assignments` and normalized preview hash.
- Publication writes teacher episodes and promotes the already-prepared target timetable in one transaction.
- Before `effectiveFrom`, date-based readers use the old assignment/version; on/after it, they use the new episodes/version.
- For a target draft linked to the change set, workspace eligibility, create/update instructor validation, placement preview, and handoff all use the pending projected assignments: newly added teachers are selectable and stopped teachers are ineligible.
- Release 3 whole-school overview adds typed `UnresolvedTeacherHandoff` issues for target-version entries that still reference a projected-ineligible teacher, linking to the exact teacher/homeroom editor.

- [ ] **Step 1: Add failing readiness tests**

Cover:

- stop teacher with untouched target entries -> blocking finding;
- complete assign-one handoff -> no stopped-teacher finding;
- partial selected-entry handoff -> remaining count exact;
- add teacher only -> no automatic timetable change and no false conflict;
- target entry uses a teacher ending before effective date -> blocking;
- proposed co-teacher has another period at same slot -> handoff conflict;
- target draft workspace includes a pending added teacher and excludes a pending stopped teacher, while the base published workspace is unchanged;
- target draft entry update accepts the pending added teacher and rejects the pending stopped teacher;
- warning acknowledgements cannot override blocking teacher findings.

- [ ] **Step 2: Run readiness tests and confirm failure**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::teacher_change_readiness \
  -- --test-threads=1
```

Expected: FAIL because readiness ignores teacher actions.

- [ ] **Step 3: Extend preview hashing and findings**

Hash sorted teacher item values, projected assignment episodes, selected target entry IDs/row versions/exact instructors, and all existing Release 2 inputs. Use stable finding ordering and routes that open either Delivery teacher handoff or the exact target timetable issue. Cut workspace, create/update, and placement-preview eligibility to the shared pending projection whenever the draft version owns a change-set ID; ordinary drafts continue to resolve stored episodes at their version effective date.

- [ ] **Step 4: Add failing publication/date-boundary tests**

Publish a complete change set before its effective date. Assert:

- old date resolves old timetable and teacher episode;
- effective date resolves target timetable and new/closed episodes;
- add creates one episode starting effective date with null end;
- adjust closes old role episode at day -1 and creates new role episode at effective date;
- stop closes old episode at day -1;
- direct published-group mutation remains rejected;
- repeated publish idempotency key returns the original result;
- any mid-transaction failure rolls back episode and version changes together.

- [ ] **Step 5: Implement atomic publication**

Set the transaction-local change-set provenance value expected by migration 055. Lock teacher episodes in stable group/teacher/start order. Apply stop/adjust closes before add/adjust inserts so non-overlap constraints remain valid. Re-run readiness under locks immediately before writes, then promote the target timetable and mark the change set published.

Extend existing academic audit payloads with every teacher episode before/after value and the target timetable/version IDs. After commit, the handler emits both learning-delivery/core invalidation and timetable invalidation; realtime payloads carry IDs/revisions only and remain cache-invalidating signals rather than replacement state.

- [ ] **Step 6: Run focused publication and timetable resolution tests**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::teacher_change -- --test-threads=1
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_service_tests::effective -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 7: Commit safe teacher publication**

```bash
git add backend-school/src/modules/academic/delivery/models.rs \
  backend-school/src/modules/academic/delivery/services/change_sets.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs \
  backend-school/src/modules/academic/models/timetable.rs \
  backend-school/src/modules/academic/services/effective_teacher_service.rs \
  backend-school/src/modules/academic/services/timetable_service.rs \
  backend-school/src/modules/academic/services/timetable_version_service.rs \
  backend-school/src/modules/academic/services/timetable_service_tests.rs
git commit -m "feat(academic): publish effective teacher changes"
```

---

### Task 5: Generate contracts and build Delivery teacher-change UI

**Files:**
- Modify (generated): `contracts/openapi/school-api.json`
- Modify (generated): `frontend-school/src/lib/api/generated/school-api.ts`
- Modify: `frontend-school/src/lib/api/learning-delivery.ts`
- Modify: `frontend-school/src/lib/components/learning-delivery/AcademicChangeSetPanel.svelte`
- Create: `frontend-school/src/lib/components/learning-delivery/AcademicTeacherChangeForm.svelte`
- Create: `frontend-school/src/lib/components/learning-delivery/TeacherHandoffPanel.svelte`
- Modify: `frontend-school/src/lib/components/learning-delivery/AcademicChangeReadiness.svelte`
- Modify: `frontend-school/src/lib/components/timetable/TimetableIssueSummary.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/delivery/+page.svelte`
- Create: `frontend-school/tests/static/academic-teacher-change-contract.test.mjs`
- Create: `frontend-school/tests/e2e/academic-teacher-change-handoff.spec.ts`

**User workflow:**
1. Open Delivery for the selected academic year/term.
2. Open/create “เพิ่ม/ปรับ/หยุดกลางภาค” and choose effective date/reason.
3. Choose “ครูผู้สอน” then add teacher, adjust responsibility role, or stop teacher for a learning group.
4. For a stopped teacher, open “จัดการคาบที่ได้รับผลกระทบ”.
5. Choose one replacement, co-teacher set, or “จัดเองในหน้าตารางสอน”.
6. Preview exact affected entries/conflicts; apply only when conflict-free.
7. Return to readiness; resolve remaining blockers in the target draft timetable.
8. Publish. The new version/teacher responsibility becomes effective on the selected date.

- [ ] **Step 1: Generate and validate contracts**

```bash
cd frontend-school
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
```

Expected: PASS.

- [ ] **Step 2: Write failing static and Playwright tests**

Cover the workflow above plus:

- only generated discriminated union types are used;
- no native `<select>` or handwritten dropdown appears;
- teacher/group options come from bounded Delivery management options, not requests per row;
- effective date wording explicitly says it starts after publication when that date is reached;
- manual choice performs no apply request and links to `academicYearId`, `academicTermId`, `timetableVersionId`, and target view;
- conflict preview disables apply and links to the timetable cell;
- stale response reloads change set/target workspace without losing the chosen mode;
- after apply, affected entry count and readiness update.
- target-version whole-school overview reports unresolved teacher handoffs and links to the same exact editable context.

- [ ] **Step 3: Run tests and confirm failure**

```bash
cd frontend-school
node --test tests/static/academic-teacher-change-contract.test.mjs
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/academic-teacher-change-handoff.spec.ts --workers=1
```

Expected: FAIL because teacher change UI/contracts do not exist.

- [ ] **Step 4: Split the existing panel and implement teacher item forms**

Keep offering actions in `AcademicChangeSetPanel.svelte`, but move teacher-specific selection/validation into `AcademicTeacherChangeForm.svelte`. Use searchable shadcn-svelte selectors and display current effective episode, role, and planned outcome. Disable impossible combinations before submit while preserving backend validation as authoritative.

- [ ] **Step 5: Implement explicit handoff panel**

Show affected timetable entries with checkboxes, current exact teachers, proposed teachers, day/period/room, and conflict badges. Preview whenever normalized choice/selection changes, with abort/cancellation for stale requests. Apply sends the exact preview hash and row versions. After success, refresh change set/readiness and invalidate the target timetable workspace cache.

Manual mode shows a clear link and the message that publication remains blocked until all affected entries are corrected; it never implies automatic replacement.

- [ ] **Step 6: Update readiness presentation**

Group teacher blockers separately from offering/roster/schedule blockers. Routes from `StoppedTeacherStillScheduled` open the handoff panel; entry-level conflicts open the exact target timetable version and selected view. Keep warning acknowledgement behavior unchanged for warnings only.

- [ ] **Step 7: Analyze/autofix Svelte files, then run focused tests**

Use the two Svelte skills on every changed component, then:

```bash
cd frontend-school
node --test tests/static/academic-teacher-change-contract.test.mjs
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/academic-teacher-change-handoff.spec.ts --workers=1
```

Expected: PASS.

- [ ] **Step 8: Commit Delivery teacher change UI**

```bash
git add contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  frontend-school/src/lib/api/learning-delivery.ts \
  frontend-school/src/lib/components/learning-delivery \
  frontend-school/src/lib/components/timetable/TimetableIssueSummary.svelte \
  'frontend-school/src/routes/(app)/staff/academic/delivery/+page.svelte' \
  frontend-school/tests/static/academic-teacher-change-contract.test.mjs \
  frontend-school/tests/e2e/academic-teacher-change-handoff.spec.ts
git commit -m "feat(academic): guide teacher timetable handoff"
```

---

### Task 6: Release 4 verification, migration preflight, and sandbox checkpoint

**Files:**
- Modify when rollout procedure changes: `docs/OPERATIONS.md`
- Modify when durable test procedure changes: `docs/TESTING.md`

- [ ] **Step 1: Run migration/schema gates sequentially**

```bash
cargo fmt --manifest-path backend-school/Cargo.toml --all -- --check
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_055 -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo test --manifest-path backend-school/Cargo.toml \
  --test static_architecture -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 2: Run backend domain gates sequentially**

```bash
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::teacher_change -- --test-threads=1
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::delivery::services_tests::teacher_handoff -- --test-threads=1
CARGO_BUILD_JOBS=1 ./scripts/test_backend_school.sh \
  modules::academic::services::timetable_service_tests -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo check --manifest-path backend-school/Cargo.toml
```

Expected: PASS.

- [ ] **Step 3: Run generated API and frontend gates sequentially**

```bash
cd frontend-school
npm run check:api-contracts
npm run test:api-contracts
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

Expected: PASS.

- [ ] **Step 4: Run end-to-end workflows one at a time**

```bash
cd frontend-school
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/academic-teacher-change-handoff.spec.ts --workers=1
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/timetable-teacher-board.spec.ts --workers=1
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 \
  npx playwright test tests/e2e/timetable-whole-school-overview.spec.ts --workers=1
```

Expected: PASS.

- [ ] **Step 5: Run hygiene and invariant guards**

```bash
git diff --check
git status --short
rg -n "TO[D]O|T[B]D|FIX[M]E|compat|legacy" \
  backend-school/src/modules/academic/delivery \
  frontend-school/src/lib/components/learning-delivery \
  backend-school/migrations/055_academic_effective_teacher_changes.sql
```

Review every result; no unfinished path or compatibility branch is accepted.

- [ ] **Step 6: Prepare migration rollout evidence before push**

Confirm the user has a current Neon snapshot. Review the deployment workflow’s maintenance gate and migration verification without changing live data. Record expected pre/post counts and the migration-055 stable abort codes in `docs/OPERATIONS.md`.

- [ ] **Step 7: Push only after explicit approval and monitor normal auto-deployment**

Do not run migration SQL manually. The existing backend deployment applies migration 055 under maintenance mode. If verification fails, leave maintenance enabled, preserve logs, and investigate the stable abort code; never edit migration 055 after any tenant has applied it.

- [ ] **Step 8: Perform authenticated sandbox acceptance checks**

Use a disposable sandbox change set and verify:

```text
publish before effective date leaves today on old teacher/version
add teacher creates no automatic period assignment
stop teacher with untouched periods blocks publication
assign-one and co-teacher handoff update only selected target entries
handoff collision blocks with no partial update
manual mode links to editable target timetable and remains blocked
after all blockers clear, publication succeeds
date before effective resolves old state; effective date resolves new state
historical published timetable still shows the old exact teacher
```

- [ ] **Step 9: Commit rollout documentation if changed**

```bash
git add docs/OPERATIONS.md docs/TESTING.md
git commit -m "docs(academic): record teacher change rollout"
```

Skip this commit when both files are unchanged.
