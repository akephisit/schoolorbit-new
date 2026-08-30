# Academic Operational Change Release 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let authorized staff add or stop a course/activity, adjust its operational weekly-period target, and add/remove students on explicit dates during a writable term while preserving history and publishing every cross-domain change atomically with one effective-from timetable version.

**Architecture:** Release 2 builds a typed `change_sets` application service above the immutable schema introduced by migration 052. Creating a change set resolves and locks one published base timetable version, clones it to one linked draft, and records typed items. Draft item mutations project only into draft offerings, targets, and entries. Preview re-reads authoritative impact/readiness state. Publication locks all resources in stable UUID order, verifies the normalized request and acknowledgement set, then publishes offerings, groups, rosters, the target timetable version, and availability changes in one transaction. Dated student membership remains a separate group-scoped mutation because it does not require a timetable-version change, but it uses interval history and date-correct reads.

**Tech Stack:** PostgreSQL/SQLx migrations, Rust/Axum/Serde/Utoipa, SvelteKit 5/TypeScript, shadcn-svelte, generated OpenAPI contracts, Node static tests, disposable PostgreSQL test runner.

**Spec:** `docs/superpowers/specs/2026-08-30-academic-operational-change-and-timetable-versioning-design.md`

## Global Constraints

- Never edit migrations 001–052; add only `backend-school/migrations/053_academic_operational_change_workflows.sql` if the tested runtime invariants require database support.
- Run all commands serially. Never overlap Rust, Node, frontend, Docker, database, or deployment commands.
- Do not add compatibility readers, dual-write paths, feature flags, fallback DTOs, or tenant-specific branches.
- `subject_versions` remains the owner of official credit, hours, and standard periods. Only `academic_timetable_version_targets` owns the operational weekly-period target.
- Adding or stopping an offering never mutates a published curriculum and never creates an A-to-B replacement relation.
- Stopping applies to the whole offering in Release 2. It preserves groups, memberships, assessment plans, scores, results, observations, and all earlier timetable versions.
- Teacher assignment stays locked after group publication. Release 2 adds no teacher-replacement endpoint or unlock mechanism.
- Dated roster removal uses an inclusive end date: a membership with `left_at = 2027-07-10` is valid through 2027-07-10 and absent from 2027-07-11. Re-adding inserts a new interval.
- An active term accepts an effective date from today through the academic-year end; a planning term accepts any date from term start through academic-year end. Closing, closed, and cancelled terms reject mutations.
- Deficits are blocking. Excess periods require explicit acknowledgement of the current preview finding codes before publication.
- Mutation authorization continues to use generated Learning Offering manage policies. Read-only routes must not load management-only teacher, student, room, or catalog options.
- Rust DTOs plus Utoipa own the wire contract. Regenerate OpenAPI and TypeScript; never hand-edit generated contracts or use `unknown`, `Record`, or response casts.
- Audit metadata may contain IDs, counts, action names, dates, row versions, and hashes, but no roster identities, national IDs, contacts, credentials, raw requests, or database URLs. Realtime payloads are invalidation signals only.
- Release 3 owns the richer timetable-version workspace. Release 2 may deep-link to the existing explicit draft-version timetable editor and must not build drag-and-drop or an autoscheduler.
- Run every `.rules` verification gate required by the change-type matrix and report any unavailable credentialed external gate honestly.

---

### Task 1: Prove and add the migration 053 workflow invariants

**Files:**
- Create: `backend-school/migrations/053_academic_operational_change_workflows.sql`
- Modify: `backend-school/src/modules/academic/core/schema_tests.rs`
- Test: `backend-school/src/modules/academic/core/schema_tests.rs`

**Interfaces:**
- Extends `academic_term_change_sets` with normalized creation/publication request hashes, a publication idempotency binding, and a warning acknowledgement snapshot.
- Enforces non-overlapping inclusive membership intervals for one `(learning_group_id, student_id)` pair.
- Preserves every migration-052 row and never rewrites migration 052.

- [ ] **Step 1: Add failing migration tests**

Add `migration_053_adds_operational_publication_and_roster_guards` using a fixture applied through 052. Assert 053 adds:

```sql
academic_term_change_sets.publication_idempotency_key uuid
academic_term_change_sets.publication_request_hash text
academic_term_change_sets.acknowledged_warning_codes text[]
academic_term_change_sets.creation_request_hash text
```

Assert a unique partial index reserves a non-null publication idempotency key and a status-shape check requires the publication key/hash only for `published`. Assert existing rows, offerings, targets, versions, timetable entries, and memberships retain exact counts.

Add `migration_053_rejects_overlapping_roster_intervals_atomically`. Insert an ended membership, apply 053, then prove that direct insert/update attempts whose inclusive `daterange(joined_at, left_at, '[]')` overlaps another interval for the same group/student fail with bounded code `ACADEMIC_ROSTER_MEMBERSHIP_INTERVAL_OVERLAP`. A strictly later interval must succeed. An interval for another group or student must succeed.

Add `migration_053_rejects_malformed_published_change_set_metadata`. Prove direct updates cannot publish without a UUID publication key, a 64-character lowercase SHA-256 request hash, and a non-null acknowledgement array. Prove draft/cancelled rows cannot retain publication metadata.

- [ ] **Step 2: Run the exact tests and observe failure**

Run each test separately:

```bash
./scripts/test_backend_school.sh \
  modules::academic::core::schema_tests::migration_053_adds_operational_publication_and_roster_guards \
  -- --exact --nocapture --test-threads=1
```

Expected: FAIL because migration 053 does not exist. Repeat serially for the two rejection tests.

- [ ] **Step 3: Implement migration 053**

Add the three publication columns plus `creation_request_hash TEXT`. Backfill existing rows with the bounded legacy marker `repeat('0', 64)`, make the creation hash non-null, and require every creation/publication hash to match `^[0-9a-f]{64}$`. Backfill the warning array to `{}` only; leave publication key/hash null for all existing draft rows. Add:

```sql
CREATE UNIQUE INDEX academic_term_change_sets_publication_idempotency_key
    ON academic_term_change_sets(publication_idempotency_key)
    WHERE publication_idempotency_key IS NOT NULL;
```

Replace the migration-052 status metadata check with the final shape:

- draft/cancelled: publication key/hash null and warning codes empty;
- published: key/hash non-null, hash matches `^[0-9a-f]{64}$`, warning codes non-null and normalized by the service.

Create a `BEFORE INSERT OR UPDATE` membership trigger that first takes a transaction-scoped PostgreSQL advisory lock derived from the exact `(learning_group_id, student_id)` UUID pair, then locks existing rows for that pair in stable ID order. Reject any inclusive interval overlap with `ACADEMIC_ROSTER_MEMBERSHIP_INTERVAL_OVERLAP`. Treat null `left_at` as infinity. Exclude `OLD.id` on update. Do not include row data in the exception. The advisory lock closes the race where two concurrent inserts both begin with no existing row.

End the migration with bounded assertions for the new columns, index, enabled trigger, and unchanged row counts captured in a temporary PL/pgSQL block.

- [ ] **Step 4: Run the three exact tests**

Expected: PASS.

- [ ] **Step 5: Commit the database boundary**

```bash
git add backend-school/migrations/053_academic_operational_change_workflows.sql \
  backend-school/src/modules/academic/core/schema_tests.rs
git commit -m "feat(academic): guard operational change publication"
```

---

### Task 2: Define typed change-set contracts and draft lifecycle

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Modify: `backend-school/src/modules/academic/delivery/services.rs`
- Create: `backend-school/src/modules/academic/delivery/services/change_sets.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`

**Interfaces:**
- `list_change_sets(pool, term_id) -> Vec<AcademicTermChangeSet>`
- `get_change_set(pool, id) -> AcademicTermChangeSet`
- `create_change_set(pool, actor_id, CreateAcademicTermChangeSetRequest) -> AcademicTermChangeSet`
- `update_change_set(pool, actor_id, id, UpdateAcademicTermChangeSetRequest) -> AcademicTermChangeSet`
- `cancel_change_set(pool, actor_id, id, CancelAcademicTermChangeSetRequest) -> AcademicTermChangeSet`

- [ ] **Step 1: Add failing lifecycle tests**

Cover:

- creation resolves the latest published version valid on `effectiveFrom`, clones its targets/entries/instructors, links base and target IDs in both directions, and stores a trimmed reason plus creation-request hash;
- active-term past dates, dates before term start/outside academic year, and closing/closed/cancelled terms fail;
- create retry with the same term `idempotencyKey` and identical normalized input returns the existing change set; the same key with different input conflicts;
- update changes only effective date/reason while draft and increments `rowVersion`; stale row versions conflict;
- changing the date updates the linked draft version only when no items or timetable mutations exist, otherwise returns an actionable conflict;
- cancelling marks both change set and linked draft version cancelled without touching the published base;
- published/cancelled sets are immutable.

- [ ] **Step 2: Run the focused lifecycle tests**

Use one exact `services_tests` test at a time. Expected: FAIL because typed models/service do not exist.

- [ ] **Step 3: Add DTOs and hydration**

Add enums `AcademicTermChangeSetStatus` and `AcademicTermChangeActionKind`, tagged `AcademicTermChangeItem`, and resources:

```rust
pub struct AcademicTermChangeSet {
    pub id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub effective_from: NaiveDate,
    pub reason: String,
    pub status: AcademicTermChangeSetStatus,
    pub base_timetable_version_id: Uuid,
    pub target_timetable_version_id: Uuid,
    pub row_version: i64,
    pub created_by: Uuid,
    pub published_by: Option<Uuid>,
    pub published_at: Option<DateTime<Utc>>,
    pub cancelled_by: Option<Uuid>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub items: Vec<AcademicTermChangeItem>,
}
```

Requests use camelCase plus `deny_unknown_fields`:

```rust
CreateAcademicTermChangeSetRequest {
    academic_term_id: Uuid,
    effective_from: NaiveDate,
    reason: String,
    idempotency_key: Uuid,
}
UpdateAcademicTermChangeSetRequest { row_version: i64, effective_from: NaiveDate, reason: String }
CancelAcademicTermChangeSetRequest { row_version: i64 }
AcademicTermChangeSetQuery { academic_term_id: Uuid }
```

- [ ] **Step 4: Implement the lifecycle service**

Factor transaction-local helpers in `change_sets.rs`: `require_draft_change_set_for_update`, `resolve_base_version_for_date`, `clone_version_in_tx`, `hydrate_change_set`, and `normalized_change_set_request_hash`. Clone entries and instructor rows with the same deterministic batch remapping used by `timetable_version_service::clone_draft`; move the shared SQL/helper into `timetable_version_service` rather than duplicating clone semantics.

Use SHA-256 over a serializable normalized input structure for create-retry comparison and store that hash in audit metadata, not a raw request. Lock term, then base version, then change set/target in that order.

- [ ] **Step 5: Run focused tests and commit**

Expected: PASS.

```bash
git add backend-school/src/modules/academic/delivery/models.rs \
  backend-school/src/modules/academic/delivery/services.rs \
  backend-school/src/modules/academic/delivery/services/change_sets.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs \
  backend-school/src/modules/academic/services/timetable_version_service.rs
git commit -m "feat(academic): add operational change drafts"
```

---

### Task 3: Project typed add, stop, and target-adjustment items into the draft

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/change_sets.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/offerings.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/groups.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`

**Interfaces:**
- `upsert_change_item(pool, actor_id, change_set_id, UpsertAcademicTermChangeItemRequest) -> AcademicTermChangeSet`
- `delete_change_item(pool, actor_id, change_set_id, item_id, DeleteAcademicTermChangeItemRequest) -> AcademicTermChangeSet`
- Tagged request variants: `add_course`, `add_activity`, `stop_offering`, `adjust_weekly_period_target`.

- [ ] **Step 1: Add failing item-projection tests**

Prove:

- add-course uses a published/effective `subject_version_id`, copies official snapshot values, defaults the draft target from `subject_versions.periods_per_week`, creates draft target(s)/group(s), and labels absence from the published curriculum as extra without editing curriculum;
- add-activity uses a published/effective `activity_version_id` and requires an explicit positive weekly target; it never converts clock hours;
- add requests reuse the existing owner and offering-target shapes, then derive initial draft groups/homeroom coverage from those targets. Missing teachers, rooms, or roster completion become readiness findings rather than making the draft impossible to create; invalid catalog/target context still rejects before writes;
- all added offerings start on the change-set effective date and remain draft until atomic publication;
- stop accepts only a published offering available on the effective date, removes its target and active entries from the linked draft version, and leaves the source version and downstream rows unchanged;
- target adjustment changes only the target draft and stores an `adjust_weekly_period_target` item;
- the same action/offering upserts by item `rowVersion`; stale item/change-set versions conflict;
- deleting an add item hard-deletes only its draft-only offering graph after a zero-downstream-reference guard; deleting a stop/adjust item restores entries/target from the immutable base version;
- published/cancelled sets reject item writes; teacher mutation remains impossible after group publication.

- [ ] **Step 2: Run the item tests and observe failure**

Run exact tests serially. Expected: FAIL.

- [ ] **Step 3: Add named request variants**

Reuse existing typed course/activity configuration structures through composition; do not add JSON payload columns. Add:

```rust
#[serde(tag = "action", rename_all = "snake_case")]
pub enum UpsertAcademicTermChangeItemRequest {
    AddCourse(AddCourseChangeItemRequest),
    AddActivity(AddActivityChangeItemRequest),
    StopOffering(StopOfferingChangeItemRequest),
    AdjustWeeklyPeriodTarget(AdjustWeeklyPeriodTargetChangeItemRequest),
}
```

Each variant carries `changeSetRowVersion`; existing-item updates also carry `itemRowVersion`. Add variants compose the existing typed course/activity offering request and carry an explicit operational target only for activities. The service derives the initial draft group/homeroom coverage with the same target-to-group proposal rules used by curriculum preparation; staff then use the existing draft-group controls to assign teachers, rooms, and initial roster. Stop/adjust variants carry `learningOfferingId`; adjust carries `weeklyPeriodTarget`.

- [ ] **Step 4: Implement transactional projection**

Extract transaction-local `insert_course`, `insert_activity`, target validation, group creation, teacher, room, homeroom, and roster insertion helpers from existing services so ordinary setup and change-set setup share validation. Do not call pool-level public functions inside a transaction.

For stop restoration/deletion, use immutable base-version entries/targets as source of truth. Never copy entries that reference the newly added offering into a base version. Update change-set/item row versions after every accepted mutation and append one PII-free audit event after commit.

- [ ] **Step 5: Run focused tests and commit**

```bash
git add backend-school/src/modules/academic/delivery/models.rs \
  backend-school/src/modules/academic/delivery/services/change_sets.rs \
  backend-school/src/modules/academic/delivery/services/offerings.rs \
  backend-school/src/modules/academic/delivery/services/groups.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs
git commit -m "feat(delivery): project operational add and stop changes"
```

---

### Task 4: Add dated roster membership mutations and date-correct student reads

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Create: `backend-school/src/modules/academic/delivery/services/roster_memberships.rs`
- Modify: `backend-school/src/modules/academic/delivery/services.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/groups.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service_tests.rs`
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`
- Modify: `backend-school/src/modules/parents/services.rs`

**Interfaces:**
- `list_memberships(pool, group_id) -> Vec<LearningGroupStudent>`
- `add_membership(pool, actor_id, group_id, AddDatedRosterMembershipRequest) -> LearningGroupStudent`
- `remove_membership(pool, actor_id, group_id, membership_id, RemoveDatedRosterMembershipRequest) -> LearningGroupStudent`

- [ ] **Step 1: Add failing dated-membership tests**

Cover:

- add stores the chosen `joinedAt`, publishes the interval immediately for a published roster, and increments group `rowVersion`;
- remove stores inclusive `leftAt`, changes status to ended, keeps the original `joinedAt`, and never deletes scores/results;
- re-add after the inclusive end creates a new row and preserves the earlier row byte-for-byte;
- same-day re-add overlaps and fails; stale group/membership row versions fail;
- closed term/group, date outside student academic year, date before offering `starts_on`, date after offering `ends_on`, wrong academic-year context, and inactive student year fail with Thai guidance;
- list returns complete interval history ordered by student then joined date;
- the bulk initial-roster workflow keeps using term start only before first roster publication and cannot overwrite dated history afterward;
- student/parent timetable requested on a date includes the group exactly when `joined_at <= date AND (left_at IS NULL OR left_at >= date)` and resolves the timetable version for the same date.

- [ ] **Step 2: Run focused tests and observe failure**

Expected: FAIL.

- [ ] **Step 3: Add DTOs and interval service**

Add:

```rust
AddDatedRosterMembershipRequest {
    group_row_version: i64,
    student_academic_year_id: Uuid,
    joined_at: NaiveDate,
}
RemoveDatedRosterMembershipRequest {
    group_row_version: i64,
    membership_row_version: i64,
    left_at: NaiveDate,
}
```

Use `roster_source = 'operational_change'`. Lock group, offering, student-year, then memberships in stable ID order. A future-dated add remains an interval row but timetable readers use dates, never `membership_status` alone. Removal requires `leftAt >= joinedAt`; re-add requires `joinedAt` strictly later than every earlier inclusive end.

- [ ] **Step 4: Correct all student timetable membership predicates**

Change `timetable_service::list_student_entries` to accept `on_date: NaiveDate`. Pass the already-requested date from both `academic/handlers/timetable.rs` and `parents/services.rs`, then centralize the interval predicate used by both paths. Current/current-status labels may use status, but historical visibility must use the requested date. Add a static architecture assertion that date-based student timetable SQL contains both membership bounds and does not filter solely on `membership_status = 'active'`.

- [ ] **Step 5: Run focused tests and commit**

```bash
git add backend-school/src/modules/academic/delivery/models.rs \
  backend-school/src/modules/academic/delivery/services.rs \
  backend-school/src/modules/academic/delivery/services/roster_memberships.rs \
  backend-school/src/modules/academic/delivery/services/groups.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs \
  backend-school/src/modules/academic/handlers/timetable.rs \
  backend-school/src/modules/academic/services/timetable_service.rs \
  backend-school/src/modules/academic/services/timetable_service_tests.rs \
  backend-school/src/modules/parents/services.rs \
  backend-school/tests/static_architecture.rs
git commit -m "feat(delivery): preserve dated roster membership history"
```

---

### Task 5: Build typed impact preview, readiness, and atomic publication

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/models.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/change_sets.rs`
- Modify: `backend-school/src/modules/academic/delivery/services_tests.rs`
- Modify: `backend-school/src/modules/academic/services/timetable_service.rs`
- Modify: `backend-school/src/modules/academic/services/assessment_service.rs`
- Modify: relevant assessment service tests.

**Interfaces:**
- `preview_change_set(pool, id) -> AcademicTermChangeSetPreview`
- `publish_change_set(pool, actor_id, id, PublishAcademicTermChangeSetRequest) -> AcademicTermChangeSet`
- Stable typed finding codes and severities; no untyped payload maps.

- [ ] **Step 1: Add failing preview/readiness tests**

For stop impact, assert exact counts for groups, homerooms, membership intervals, teacher assignments, target-version timetable entries, course assessment plans/categories/items, learning results, exam schedule items, and supervision observations. The current schema has no student score-entry table, so do not invent one; `learning_results` remains the result count until a future grade-entry owner is introduced. Assert DTOs expose only counts/labels/IDs needed to navigate, not student identities.

For readiness, assert stable findings for:

- stale base/target/change-set/item/resource row versions;
- closed term or invalid effective date;
- draft group, missing active primary teacher, unpublished roster;
- offering unavailable on the effective date;
- missing/non-positive target;
- each active group's scheduled `actual/target` count;
- target deficit as `blocking`;
- target excess as `warning`;
- homeroom, group, teacher, and room conflicts;
- stopped offering target/entry still present;
- no-op change set.

- [ ] **Step 2: Add failing atomic publication tests**

Prove one successful transaction:

- publishes added offerings/groups/rosters;
- keeps teacher rows locked;
- sets added offering `starts_on = effective_from`;
- sets stopped offering `ends_on = effective_from - 1` plus reason/actor/time/change-set ID;
- publishes the linked target timetable version with publisher/time;
- publishes the change set with publication idempotency/hash/acknowledgements;
- leaves base timetable version immutable;
- appends bounded audit events.

Prove rollback for every blocking finding and injected final-write failure. Compare pre/post snapshots of change set, offerings, groups, rosters, version, targets, and entries.

Prove warning acknowledgement behavior: an excess warning blocks when its current stable code is absent; exact acknowledgement succeeds; stale/unknown codes fail. Same publication UUID plus identical normalized request returns the published resource; reuse with different acknowledgements or a different change set conflicts.

- [ ] **Step 3: Implement named preview DTOs**

Add `AcademicChangeFindingSeverity`, `AcademicChangeFindingCode`, `AcademicChangeFinding`, `AcademicChangeImpactCounts`, `AcademicOfferingScheduleCount`, and `AcademicTermChangeSetPreview`. Finding fields are `code`, `severity`, `title`, `guidance`, `affectedCount`, `route`, and typed `resourceId`/`learningGroupId`/`learningOfferingId` optionals. Do not add arbitrary `details` JSON.

Publish request:

```rust
PublishAcademicTermChangeSetRequest {
    row_version: i64,
    target_timetable_version_row_version: i64,
    preview_hash: String,
    acknowledged_warning_codes: Vec<AcademicChangeFindingCode>,
    idempotency_key: Uuid,
}
```

- [ ] **Step 4: Implement preview from authoritative state**

Build preview inside a read-only transaction and hash the canonical sorted tuple of change-set context, row versions, item projection, targets, schedule counts, conflicts, and findings. Reuse timetable conflict comparison helpers from `timetable_service`; make only the narrow transaction-local validators `pub(crate)`.

Stopped-offering counts use left joins and preserve historical rows. Update assessment authorization/read predicates so an otherwise authorized teacher may finish existing assessment/result work for a stopped published offering; availability boundaries control teaching visibility, not historical grade ownership.

- [ ] **Step 5: Implement stable-lock atomic publication**

Lock in this order:

1. academic term;
2. change set;
3. base then target timetable version;
4. affected offerings by UUID;
5. affected groups by UUID;
6. memberships/teachers/targets/entries by stable compound keys.

Rebuild preview inside the same transaction. Reject row/preview/hash/acknowledgement drift. Apply all status and availability writes, then audit rows, then commit. Convert unique/trigger failures to bounded domain conflicts. Never write status incrementally outside this transaction.

- [ ] **Step 6: Run focused tests and commit**

```bash
git add backend-school/src/modules/academic/delivery/models.rs \
  backend-school/src/modules/academic/delivery/services/change_sets.rs \
  backend-school/src/modules/academic/delivery/services_tests.rs \
  backend-school/src/modules/academic/services/timetable_service.rs \
  backend-school/src/modules/academic/services/assessment_service.rs \
  backend-school/src/modules/academic/services/assessment_service_tests.rs
git commit -m "feat(academic): publish operational changes atomically"
```

---

### Task 6: Expose authorized handlers, routes, OpenAPI, realtime, and generated contracts

**Files:**
- Modify: `backend-school/src/modules/academic/delivery/handlers.rs`
- Modify: `backend-school/src/modules/academic/delivery.rs`
- Modify: `backend-school/src/api_contract.rs`
- Modify: `backend-school/src/modules/academic/delivery/services/offerings.rs`
- Modify: API contract and static architecture tests.
- Generate: `contracts/openapi/school-api.json`
- Generate: `frontend-school/src/lib/api/generated/school-api.ts`

**Endpoints:**

```text
GET    /api/academic/term-change-sets?academicTermId=
POST   /api/academic/term-change-sets
GET    /api/academic/term-change-sets/{id}
PATCH  /api/academic/term-change-sets/{id}
POST   /api/academic/term-change-sets/{id}/cancel
PUT    /api/academic/term-change-sets/{id}/items
DELETE /api/academic/term-change-sets/{id}/items/{itemId}
GET    /api/academic/term-change-sets/{id}/preview
POST   /api/academic/term-change-sets/{id}/publish
GET    /api/academic/learning-groups/{id}/memberships
POST   /api/academic/learning-groups/{id}/memberships
POST   /api/academic/learning-groups/{id}/memberships/{membershipId}/end
```

- [ ] **Step 1: Add failing handler/contract assertions**

Assert every endpoint, operation ID, camelCase query/body field, named success schema, 400/403/404/409 error response, and absence of additional-properties escape hatches. Assert read endpoints use Learning Offering read access, mutations use the existing resource-aware manage policy, and cross-resource publication checks every affected scope.

- [ ] **Step 2: Implement handlers and routes**

Keep handler logic to extraction, policy checks, service call, invalidation signal, and typed response. List/get/preview never load management options. Emit one school/organization/resource invalidation descriptor set after commit; do not include resource data in SSE.

- [ ] **Step 3: Register Utoipa schemas and regenerate**

Update `api_contract.rs`, then run serially:

```bash
cd frontend-school && npm run generate:api-contracts
cd frontend-school && npm run check:api-contracts
cd frontend-school && npm run test:api-contracts
```

- [ ] **Step 4: Run API tests and commit**

```bash
git add backend-school/src/modules/academic/delivery/handlers.rs \
  backend-school/src/modules/academic/delivery.rs \
  backend-school/src/api_contract.rs \
  backend-school/src/modules/academic/delivery/services/offerings.rs \
  backend-school/tests/static_architecture.rs \
  contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts
git commit -m "feat(api): expose operational academic changes"
```

---

### Task 7: Add the Release 2 Delivery change-set and dated-roster UI

**Skills:** Apply `frontend-design`, `svelte:svelte-code-writer`, and `svelte:svelte-core-bestpractices` before editing Svelte files. Run the Svelte autofixer on every changed `.svelte` file.

**Files:**
- Modify: `frontend-school/src/lib/api/learning-delivery.ts`
- Modify: `frontend-school/src/routes/(app)/staff/academic/delivery/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte`
- Create: `frontend-school/src/lib/components/learning-delivery/AcademicChangeSetDialog.svelte`
- Create: `frontend-school/src/lib/components/learning-delivery/AcademicChangeSetPanel.svelte`
- Create: `frontend-school/src/lib/components/learning-delivery/DatedRosterMemberships.svelte`
- Modify: `frontend-school/src/lib/components/learning-delivery/HomeroomDeliveryWorkspace.svelte`
- Modify: `frontend-school/src/lib/components/learning-delivery/OfferingOverviewTable.svelte`
- Create: `frontend-school/tests/static/academic-operational-change.test.mjs`
- Modify: `frontend-school/tests/static/learning-delivery-workspace.test.mjs`
- Create: `frontend-school/tests/e2e/academic-operational-change.spec.ts`

**UX boundary:** The normal term setup remains the primary flow. Operational changes appear as a clearly labelled exceptional workflow, not as ordinary editing of published rows.

- [ ] **Step 1: Add failing static/UI tests**

Assert:

- generated types are consumed directly and every query key matches OpenAPI camelCase;
- read-only users never call management options or mutation endpoints;
- published offerings show date-derived upcoming/active/ended labels and an authorized “เพิ่ม/ปรับ/หยุดกลางภาค” action;
- change creation requires effective date and reason and explains that curriculum is unchanged;
- add course/activity inputs use existing shadcn combobox/select/date controls, activity requires a period target, and course shows catalog standard beside operational target;
- stop preview shows all impact categories and historical-data preservation guidance;
- readiness separates blocking findings from warnings and publish stays disabled until current excess warnings are acknowledged;
- draft panel deep-links to `/staff/academic/timetable?timetableVersionId=...` for entry editing;
- published teachers remain visibly locked with no replacement action;
- roster mutation asks for an explicit join/end date and explains inclusive end semantics.

- [ ] **Step 2: Add typed API wrappers**

Export generated schema types and wrappers for all Task 6 endpoints. Query objects must use `satisfies operations[...]`. Keep the existing 409 stale-data guidance. Do not add hand-written response mirrors.

- [ ] **Step 3: Implement the change-set dialog/panel**

Use one compact sheet/dialog workflow:

1. choose effective date and reason;
2. choose add course, add activity, stop offering, or adjust periods;
3. configure the typed item and draft groups;
4. show preview impact/readiness;
5. navigate to explicit draft timetable version when scheduling is incomplete;
6. acknowledge excess warnings and publish.

Keep the panel usable on notebook widths: one main column plus a sticky summary only at large breakpoints. Use semantic status color, not decoration-heavy cards. Use `PageState`/`AcademicPrerequisiteNotice` for missing context and actionable failures.

- [ ] **Step 4: Implement dated roster history**

Replace the post-publication bulk roster control with interval history and explicit add/end actions. Keep bulk preview/apply/publish for initial draft roster setup only. Show joined date, inclusive end date, derived current/upcoming/ended state, and history rows. Do not display sensitive identifiers beyond the existing permitted student code/name context.

- [ ] **Step 5: Run Svelte autofixer serially**

Run these commands separately:

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/delivery/+page.svelte' --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte' --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/lib/components/learning-delivery/AcademicChangeSetDialog.svelte' --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/lib/components/learning-delivery/AcademicChangeSetPanel.svelte' --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/lib/components/learning-delivery/DatedRosterMemberships.svelte' --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/lib/components/learning-delivery/HomeroomDeliveryWorkspace.svelte' --svelte-version 5
npx @sveltejs/mcp svelte-autofixer 'src/lib/components/learning-delivery/OfferingOverviewTable.svelte' --svelte-version 5
```

Apply every safe correction and rerun until no issue remains.

- [ ] **Step 6: Run focused frontend gates and commit**

Run serially:

```bash
cd frontend-school && npm run lint
cd frontend-school && npm run check
cd frontend-school && npm run test:static -- --test-concurrency=1
```

```bash
git add frontend-school/src/lib/api/learning-delivery.ts \
  frontend-school/src/lib/components/learning-delivery \
  'frontend-school/src/routes/(app)/staff/academic/delivery' \
  frontend-school/tests/static/academic-operational-change.test.mjs \
  frontend-school/tests/static/learning-delivery-workspace.test.mjs \
  frontend-school/tests/e2e/academic-operational-change.spec.ts
git commit -m "feat(delivery): add midterm change workspace"
```

---

### Task 8: Verify Release 2 end to end, retire its plan, push, deploy, and smoke-test

**Files:**
- Modify only if a verification test exposes a real defect.
- Delete after all gates pass: `docs/superpowers/plans/2026-08-30-academic-operational-change-release-2.md`

- [ ] **Step 1: Run focused regression tests**

Run all new schema, lifecycle, item, roster, preview/publication, assessment-history, timetable-membership, handler, contract, and frontend static tests one command at a time with one Rust test thread / one Node test-concurrency.

- [ ] **Step 2: Run the complete repository gates serially**

At minimum:

```bash
./scripts/test_backend_school.sh -- --test-threads=1
cd backend-school && cargo fmt --all -- --check
cd backend-school && cargo check
cd frontend-school && npm run check:api-contracts
cd frontend-school && npm run test:api-contracts
cd frontend-school && npm run lint
cd frontend-school && npm run check
cd frontend-school && npm run test:static -- --test-concurrency=1
```

Also run permission-contract, static-architecture, menu-sync, and Playwright discovery commands named by `.rules`/`docs/TESTING.md`, serially. Do not claim an unavailable credentialed browser test passed.

- [ ] **Step 3: Review database and privacy invariants**

Verify:

- `_sqlx_migrations` reaches 53 on a disposable database;
- no applied migration was edited;
- no published version/resource can be mutated directly;
- a failed publish leaves every authoritative table unchanged;
- stopped offerings remain readable to assessment/result/history consumers;
- logs, audit metadata, realtime events, API DTOs, and previews contain no plaintext national ID or unnecessary roster identity;
- generated permission/API contracts are clean.

- [ ] **Step 4: Retire the completed implementation plan**

Delete this plan only after every required gate passes. Keep the approved design spec because Releases 3–4 still depend on it.

- [ ] **Step 5: Commit verification/plan retirement**

```bash
git add -A
git commit -m "test(academic): verify operational change release 2"
```

- [ ] **Step 6: Push main and monitor automatic deployment**

```bash
git push origin main
```

Monitor the API-contract, permission-contract, backend, frontend, and documentation workflows one at a time. Confirm migration 053 applies and maintenance reopens.

- [ ] **Step 7: Run authenticated sandbox smoke checks**

Using the configured `sandbox.schoolorbit.app` test account without printing credentials, verify:

- existing academic context and delivery pages load;
- create/cancel a harmless draft change set;
- preview an add/stop/adjust draft and confirm typed impact/readiness output;
- dated roster validation returns an actionable response on a safe non-writing or disposable target;
- historical timetable/version and stopped-offering assessment reads remain accessible;
- browser Network contains no repeated/batch regression or camelCase query deserialization errors.

Do not publish a real operational change in smoke testing unless the sandbox fixture was deliberately created for it.
