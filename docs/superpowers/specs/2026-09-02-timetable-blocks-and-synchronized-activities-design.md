# Timetable Blocks and Synchronized Activities Design

**Date:** 2026-09-02

**Status:** Approved in chat; awaiting written-spec review

**Scope:** canonical timetable blocks, synchronized curriculum activities before group creation,
structural school blocks, per-period teacher selection for ordinary delivery, drag-and-drop
interaction, per-target removal, conflict and sync semantics, dedicated timetable permissions,
generated API contracts, clean data migration, and coordinated deployment verification

## Context

SchoolOrbit already supports timetable versions, exact per-entry instructors, homeroom and teacher
views, placement preview, conflict checks, and term delivery through learning offerings and learning
groups. The remaining design problem is that the scheduled row is still group-first. A course or
activity timetable entry requires a learning group, while `batch_id` only groups entries that
already exist.

That shape cannot represent the normal preparation order for synchronized student-development
activities. Schools commonly reserve one weekly period for scout, club, guidance, or another
curriculum activity before they know the final group names, teachers, rooms, or rosters. Groups are
created later in Delivery and must join the already-reserved period without inventing a second
schedule.

The current batch shape is also insufficient for school-wide structural periods such as flag
ceremony, homeroom, lunch, or teacher meeting. Staff need to create one logical event for many
homerooms or teachers, move it as one event, inspect it without repeated cards, and still remove one
homeroom or teacher without deleting the whole set.

Finally, the current unscheduled tray prevents direct dragging when a learning group has multiple
eligible teachers. It falls back to a dialog and click-to-place workflow. The desired primary
desktop interaction is direct drag-and-drop after selecting the exact teacher set on the tray card,
with compact scheduled cards and precise collision feedback.

## Relationship to Existing Designs

This design extends these existing approved boundaries:

- `2026-08-30-academic-operational-change-and-timetable-versioning-design.md` remains authoritative
  for immutable published timetable versions, effective-from resolution, cloning, atomic
  publication, and historical reads.
- `2026-08-30-timetable-drag-drop-and-effective-teacher-assignment-design.md` remains authoritative
  for exact per-period instructors, editable homeroom/teacher/group views, one bell period per drag,
  teacher handoff, and read-only whole-school inspection.
- the catalog -> curriculum -> term offering -> learning group chain remains authoritative for
  academic ownership. A timetable block never becomes a second curriculum or delivery catalog.

This design supersedes the following narrower decisions:

- `academic_timetable_entries` is no longer the top-level scheduled fact. A canonical block owns
  the version, day, period, source, and logical identity; group, homeroom, and teacher allocations
  are children.
- `batch_id` is removed as a runtime owner. Optional block series replace it only for repeated
  create/delete operations across multiple days or periods.
- synchronized curriculum activities may be placed before learning groups exist.
- a multi-teacher tray selection is reset after every successful placement or explicit cancel,
  rather than remaining selected for later placements.
- timetable access no longer reuses Learning Offering permissions. Dedicated generated timetable
  permissions own reading, draft mutation, and publication.

A draft effective date alone has no operational effect. Once a future timetable version is
published, the existing version resolver activates it automatically on its effective-from date.
This design does not change that versioning rule.

## Goals

- Represent one scheduled period independently from whether its delivery groups exist yet.
- Schedule synchronized curriculum activities before final groups, teachers, rooms, or rosters.
- Synchronize later Delivery groups into the reserved period without moving the period
  automatically.
- Allow non-conflicting groups to synchronize even when another group is blocked.
- Represent structural school events without fabricating curriculum offerings or learning groups.
- Create a structural event for many homerooms or teachers while allowing individual target removal.
- Keep ordinary course and independent-activity teacher selection exact per period.
- Make drag-and-drop the primary desktop interaction while retaining complete keyboard, touch, and
  mobile placement alternatives.
- Detect homeroom, group, teacher, and room conflicts before a drop and revalidate atomically on the
  server.
- Render one logical synchronized or structural event as one aggregate card where repetition would
  obscure the timetable.
- Preserve immutable published history, downstream timetable consumers, auditability, and generated
  contracts through a clean cutover without compatibility reads or writes.

## Non-Goals

- Do not build automatic timetable generation, optimization, or conflict auto-resolution.
- Do not auto-move a synchronized activity when a later group or teacher conflicts.
- Do not invent final scout/club group names, teachers, rooms, or student rosters from the timetable.
- Do not add linked double-period blocks. Staff place each bell period separately.
- Do not add alternating-week, date-specific substitution, delivered-lesson, payroll, or attendance
  behavior.
- Do not make the whole-school overview an editable dense grid.
- Do not add a special teacher-choice rule for synchronized activities; every teacher assigned to a
  synchronized group in Delivery participates in that group's occurrence.
- Do not change curriculum credit, official hours, or catalog-standard periods when a timetable
  block changes.
- Do not retain the old entry/batch API, old schema, dual reads, dual writes, fallback derivation, or
  tenant-specific compatibility branches after cutover.

## Selected Architecture

### Canonical timetable block

One row in `academic_timetable_blocks` is the canonical scheduled fact for one timetable version,
one day, and one bell period. It owns:

- timetable version, academic year, academic term, day, and period;
- block kind `DELIVERY` or `STRUCTURAL`;
- optional learning offering for delivery blocks;
- structural kind for structural blocks;
- display title and optional note;
- optional series ID for blocks created together across multiple days or periods;
- row version, active state, creator/updater, and timestamps.

A block is the unit moved by drag-and-drop. Moving a block moves every attached allocation in one
transaction. Distinct blocks are always distinct simultaneous events and conflict normally. Child
allocations within one block are participating in the same event and do not conflict with each
other merely because they share a teacher, homeroom, or physical room.

The clean child tables are:

- `academic_timetable_block_groups`: a concrete learning-group allocation, optional room,
  row version, and sync provenance;
- `academic_timetable_block_group_instructors`: exact instructors for one group allocation;
- `academic_timetable_block_homerooms`: explicit homeroom reservation or structural target,
  optional room, and active/excluded state;
- `academic_timetable_block_teachers`: explicit teacher target for a structural block; and
- `academic_timetable_block_group_sync`: per block/group state, conflict code/details,
  attempted revision, and linked group-allocation ID when synchronization succeeds.

Foreign keys and shape checks make invalid combinations impossible. A delivery block references a
term learning offering. A structural block has no offering. A course or independently scheduled
activity block has exactly one learning-group allocation. A synchronized activity block may have
zero or many group allocations and retains explicit homeroom reservations before groups exist.

The old `academic_timetable_entries` and `timetable_entry_instructors` tables are migrated into the
new child model and removed. Downstream foreign keys that refer to a lesson occurrence, including
supervision, move to the concrete block-group allocation. Existing IDs are preserved where a prior
entry maps one-to-one; preflight blocks any ambiguous downstream reference instead of guessing.

### Block categories

#### Course and independently scheduled activity

These remain group-first delivery. Each placement creates one delivery block with one group
allocation and one exact instructor set. The selected instructors must be active eligible teachers
for that learning group at the target version's effective date.

Different periods for the same group may select different teacher subsets. Selecting A+B means
co-teaching that period. Moving the block preserves the complete instructor set. Removing only B is
an inspector edit, not a partial drag or a deletion from B's timetable.

#### Synchronized curriculum activity

The block references the synchronized term learning offering and explicit intended homerooms. It
may be created with no learning groups. The homeroom rows reserve the period immediately, so other
blocks cannot occupy those homerooms while final scout, club, guidance, or similar groups are still
being prepared.

When Delivery later creates or changes a group, the sync service compares the group's covered
homerooms with the block scope, resolves every active group teacher, and attempts to create one
group allocation with that exact teacher set and optional room. The timetable page never presents a
teacher selector for this category.

#### Structural school block

A structural block represents flag ceremony, homeroom, break/lunch, teacher meeting, or another
named school event. It does not create a curriculum requirement, offering, or learning group.

The create dialog accepts one or more days/periods plus explicit audiences:

- all, grade-filtered, or selected homerooms;
- all or selected teachers; or
- both homerooms and teachers.

One block is created per selected day/period. Blocks from one operation share a series ID. Each
homeroom and teacher remains an individual child target, so removing one does not remove the rest.
If the final target is removed, the now-empty draft block deactivates in the same transaction.

## Synchronization Semantics

Group synchronization is deterministic and per group:

1. Resolve the synchronized activity block for the same offering and draft timetable version.
2. Validate that the group's homeroom coverage is inside the reserved block scope.
3. Resolve every teacher assigned to that group through Delivery for the version date.
4. Validate teacher and physical-room occupancy against distinct blocks in the same version/slot.
5. On success, create or update the group allocation and exact instructor children atomically.
6. On failure, leave the timetable placement unchanged and record a typed sync finding.

User-facing states are:

- `LINKED`: the group is attached to the block;
- `WAITING_FOR_DATA`: the group lacks required teacher, room when required, or another delivery
  dependency;
- `CONFLICT`: a distinct block occupies a required teacher, homeroom, or room;
- `OUTSIDE_SCOPE`: group coverage is not part of the reserved block audience; and
- `EXCLUDED`: an authorized user intentionally removed the group from this block.

Sync attempts are independent. One conflict never rolls back other valid groups. `EXCLUDED` is
sticky and is not re-added by automatic retry; an explicit `Restore to block` action clears it.
Conflict details use stable codes plus school-facing labels and never contain unrelated personal
data.

For a draft version, Delivery changes may synchronize directly. If only a published version exists,
the operational-change workflow creates or reuses a draft cloned from the effective published
version; it never mutates published block children. Staff review and publish that draft before the
new allocation becomes operational.

Moving a synchronized block is atomic for every linked allocation. Placement preview checks all
linked teachers, homerooms, and rooms. Pending groups are re-evaluated after a successful move. The
service never moves only the conflict-free children to a new slot.

## Conflict and Publication Rules

Every create, move, target edit, instructor edit, sync, and publication checks:

| Resource | Blocking rule across distinct active blocks in one version/slot |
|---|---|
| Learning group | A group cannot occupy two blocks |
| Homeroom | Covered or explicitly reserved homerooms cannot occupy two blocks |
| Teacher | An exact instructor or explicit structural teacher target cannot occupy two blocks |
| Physical room | A room cannot be allocated to two blocks |

Targets inside the same block may overlap because they participate in one synchronized event. This
permits one teacher to supervise multiple activity groups in the same synchronized block and permits
multiple targets to share an auditorium or activity space intentionally.

The frontend occupancy index provides immediate green/red feedback. The backend locks the version,
block, target resources, and slot in stable order, then repeats the authoritative checks. A stale or
server-discovered conflict returns typed `409 Conflict`; no partial mutation remains.

Publication retains the existing version readiness rules and adds:

- every ordinary course/independent activity block has one group and at least one eligible exact
  instructor;
- synchronized blocks may publish before groups exist only when their intended homeroom reservation
  is explicit;
- every linked synchronized group has the complete current Delivery teacher set;
- waiting/conflict/outside-scope group findings are visible publication warnings, not hidden data;
- an excluded group remains an explicit acknowledged exception; and
- no hard cross-block group, homeroom, teacher, or room conflict exists.

An intentionally group-free synchronized block is valid planning data; it does not claim that
Delivery is complete. Existing target-count and school readiness policy decides whether a warning
requires acknowledgement or blocks the broader academic publication workflow.

## Version and Mutation Lifecycle

Only draft blocks and targets accept mutation. Published versions remain immutable snapshots.

- Opening a published version is read-only.
- The first attempted edit prompts once, then creates or reuses a draft clone and redirects the
  workspace to that version.
- An effective-from date on an unpublished draft does not activate it.
- Publishing a future version makes it resolve automatically on and after effective-from.
- Historical versions and their block/target/instructor children never change.

Individual removal semantics are explicit:

- deleting a course or independent-activity card removes the complete block and all co-teachers;
- removing one co-teacher uses the inspector's teacher-set edit;
- deleting a synchronized group occurrence marks that group `EXCLUDED` without deleting the parent
  block;
- deleting from a homeroom structural view removes only that homeroom target;
- deleting from a teacher structural view removes only that teacher target;
- deleting a complete block or series is available only from its detail surface with explicit
  confirmation.

## Timetable Workspace UX

### Page structure

The existing PageShell and explicit version selector remain. Editable views use a split workspace:

```text
┌────────────────────────────┬────────────────────────────────────────┐
│ Items waiting to schedule  │ Homeroom / teacher / group timetable  │
│ - courses/independent      │                                        │
│ - synchronized activities  │ Drag one period into the board         │
│ - add structural block     │                                        │
└────────────────────────────┴────────────────────────────────────────┘
```

The homeroom view remains the default. Homeroom and teacher views are editable. The learning-group
view remains available for combined/elective groups. The whole-school view remains read-only and
links each issue to an exact editable context.

### Tray cards and teacher selection

An ordinary delivery card shows code, title, group/homeroom, scheduled/target count, and exact
teacher selector for the next placement.

- one eligible teacher is shown by full name and selected automatically;
- multiple eligible teachers use a shadcn-svelte multi-select, not precomputed A/B/A+B combinations;
- the compact summary renders the selected names, then `+ N` when needed;
- dragging is disabled until at least one teacher is selected;
- successful placement and explicit cancel clear the selection; and
- failed placement preserves the selection so the user can choose another cell without reselecting.

A synchronized activity card has no teacher selector. It shows reserved audience, target periods,
and aggregate state such as `8 groups · 7 linked · 1 conflict` or `groups not created yet`.

### Drag behavior and compact scheduled cards

Desktop drag uses a dedicated handle so selecting teachers, opening details, and deleting cannot
start an accidental drag. The drag ghost names the code/title, group or reserved audience, and
selected teacher set. Cells expose `valid`, `blocked`, and occupied/swap states through text,
accessible descriptions, and semantic color.

Scheduled cards remove the large `move`, `edit details`, and `remove` action row:

- drag the handle to move the complete block;
- click the card to open its inspector;
- use a small trash icon with a context-specific confirmation for individual removal; and
- use the detail surface for full block or series deletion.

Homeroom and whole-school views aggregate synchronized/structural blocks instead of rendering one
duplicate card per child. Teacher and group views resolve exact participation. Student and parent
views resolve the student's rostered group within the block.

Keyboard, touch, and narrow mobile screens retain a complete select-item then select-day/period
workflow. Escape cancels an active selection. Live regions announce target validity and mutation
outcomes. Horizontal scrolling, sticky headers, focus retention, reduced motion, and non-color-only
states remain required.

## Typed API and Realtime Contract

Rust DTOs plus `utoipa` remain authoritative. The frontend consumes only generated TypeScript wire
DTOs through concrete API wrappers.

The block contract provides bounded, set-based operations for:

- workspace read by explicit academic term and timetable version;
- placement preview for new blocks and existing block moves;
- create ordinary delivery block with group and exact instructors;
- create synchronized offering block with homeroom reservations;
- create structural block series with normalized/deduplicated targets;
- update block metadata, room allocation, or ordinary instructor set;
- remove one group/homeroom/teacher target;
- restore an excluded synchronized group;
- remove one block or an explicitly confirmed series;
- sync or retry synchronized activity groups; and
- publish/read readiness through the existing timetable-version boundary.

Workspace reads include block identity, series identity, source, targets, exact instructors, sync
summaries, conflict labels, row versions, and only the management options permitted to the caller.
They must not cause one request per group, teacher, block, or cell. Successful mutations return the
changed block/targets and summary revision for local patching.

Realtime messages remain invalidation signals. Block/target changes emit a timetable invalidation
for the affected tenant, term, and version. A remote mutation during drag marks the preview stale;
the client refreshes authoritative HTTP state before retrying.

## Authorization

The permission contract gains a dedicated `academic_timetable` module with generated codes:

- `academic_timetable.read.assigned`
- `academic_timetable.read.organization_unit`
- `academic_timetable.read.organization_tree`
- `academic_timetable.read.school`
- `academic_timetable.manage.assigned`
- `academic_timetable.manage.organization_unit`
- `academic_timetable.manage.organization_tree`
- `academic_timetable.manage.school`
- `academic_timetable.publish.school`

Independent scopes combine as a union. Every mutation checks every affected offering, group,
homeroom, teacher, room, and version through a reusable timetable resource policy. A series or
cross-target mutation requires authority for the complete affected set. Structural blocks targeting
the whole school require school manage authority.

Self-service teacher/student/parent timetable endpoints retain their ownership policies and do not
require management permissions. `academic_timetable_today.read.school` remains the separate
read-only daily overview capability. Learning Offering permissions continue to own Delivery and no
longer implicitly grant timetable mutation.

Permission definitions change only through `contracts/permissions.json`, a new tenant migration,
and generated backend/frontend registries. Frontend visibility is convenience UX; backend policy is
authoritative.

## Migration and Clean Cutover

Implementation reads the complete applied migration timeline and adds only the next sequential
forward migration. No applied migration or `_sqlx_migrations` row is edited.

### Preflight

Preflight inventories and validates:

- timetable versions, entries, exact instructors, active state, source kind, and batch membership;
- one-to-one and grouped mappings from legacy entries to canonical blocks;
- duplicate or inconsistent day/period/source values inside one batch;
- downstream timetable-entry references, including supervision;
- course/activity entries missing groups, offerings, exact instructors, or academic context;
- structural entries with ambiguous homeroom/teacher ownership;
- cross-entry conflicts that would become cross-block conflicts; and
- expected block, group-target, homeroom-target, teacher-target, and instructor cardinalities.

Any ambiguous batch, unmappable downstream reference, missing context, or non-deterministic target
blocks the destructive stage and reports school-facing labels for source repair. Migration never
guesses an offering, group, teacher, or intended audience.

### Forward migration

The coordinated cutover:

1. creates block, explicit child target, sync-state, constraint, and index tables;
2. maps each ordinary standalone entry to one block and one group/structural target;
3. maps a consistent existing batch at the same version/day/period/source to one block;
4. preserves old entry IDs for one-to-one group-target mappings and records deterministic migration
   provenance for all other mappings;
5. migrates exact instructors to block-group instructors and structural teacher targets;
6. rewires downstream foreign keys to block/group-target identity;
7. verifies source/target cardinalities, conflicts, published immutability, and downstream links;
8. cuts all timetable, self-service, parent, daily teaching, supervision, PDF, export, clone,
   readiness, and occupancy consumers to the new model; and
9. removes `academic_timetable_entries`, `timetable_entry_instructors`, `batch_id`, old triggers,
   old endpoints, and old runtime services after verification inside the coordinated release.

There is no compatibility API or runtime fallback after deployment.

## Release and Deployment Strategy

Implementation is divided into serially verified work packages but deployed as one coordinated
cutover release:

1. schema, migration, conflict engine, and block services;
2. typed APIs, OpenAPI generation, permissions, resource policy, and realtime invalidation;
3. drag workspace, teacher multi-select, synchronized activity state, structural dialog, compact
   cards, and accessible fallback;
4. all downstream consumers, preflight, cleanup, and end-to-end verification.

The release uses the existing maintenance and all-tenant migration workflow:

```text
protected Neon snapshot
-> maintenance enabled
-> new backend migration and reconciliation
-> backend/frontend cutover
-> all-tenant preflight and readiness checks
-> authenticated sandbox smoke tests
-> maintenance disabled
```

If migration or tenant verification fails, the transaction rolls back and maintenance remains
enabled. The prior application is not a supported runtime after the destructive cutover accepts new
writes. Before writes are accepted, recovery may restore the protected snapshot and prior release;
after acceptance, recovery proceeds through a reviewed forward migration unless the school accepts
loss/reconciliation of later writes.

Local Rust, frontend, contract, migration, and browser commands run serially because the working
environment is known to stall under concurrent builds.

## Testing and Verification

Database and backend coverage proves:

- exact standalone/batch migration cardinality and preserved downstream references;
- valid and invalid shape constraints for every block category;
- ordinary solo, split-period, and co-teacher allocations;
- synchronized block creation before groups exist;
- independent linked/waiting/conflict/outside-scope/excluded sync outcomes;
- sticky exclusion and explicit restore;
- atomic whole-block move and no partial synchronized move;
- individual structural homeroom/teacher removal and explicit full-series removal;
- cross-block group, homeroom, teacher, and room conflicts with same-block participation allowed;
- published immutability, draft clone behavior, and effective-date resolution;
- allowed, denied, and multi-scope authorization paths; and
- typed OpenAPI registration for every endpoint and error variant.

Frontend and browser coverage proves:

- one-teacher auto-selection and multi-teacher one/many selection;
- selection reset after success/cancel and preservation after failure;
- drag activation only after a valid teacher set;
- exact valid/blocked/swap feedback with accessible text;
- synchronized activity placement without groups and later partial sync;
- structural create-many followed by remove-one in homeroom and teacher views;
- compact click/drag/delete card behavior without accidental drag;
- one logical block renders consistently across homeroom, teacher, group, whole-school, staff,
  student, and parent views;
- read-only users never request management-only data;
- no request-per-target fan-out and no untyped API payload; and
- mobile, keyboard, touch, focus, reduced-motion, dark-mode, and horizontal-scroll behavior.

Verification follows `.rules`: focused service/database/static/Playwright tests, the disposable
migration gate, permission generation/check/tests, API contract generation/check/tests, backend
format/static-architecture/check, frontend lint/Svelte check/static tests, `git diff --check`, final
diff review, and `git status --short`. Deployment verification uses the existing readiness and
authenticated proxy smoke paths documented in `docs/TESTING.md` and `docs/OPERATIONS.md`.

## Success Criteria

- Staff can reserve a scout, club, guidance, or other synchronized curriculum period before its
  final groups exist.
- Later groups synchronize independently into that block, and conflicts remain visible without
  moving or deleting the block.
- Ordinary courses choose an exact teacher subset before each drag; synchronized groups use every
  Delivery teacher without a timetable selector.
- Structural school events can target many homerooms/teachers and still remove one target safely.
- Drag-and-drop is direct and compact on desktop, with complete keyboard/touch/mobile alternatives.
- Every view and downstream consumer resolves the same canonical block data without duplication or
  request fan-out.
- Published history remains immutable and future published versions activate only through the
  existing effective-from resolver.
- The migration reconciles every source row and downstream reference, then removes legacy entry,
  instructor, batch, API, service, and timetable authorization paths without compatibility code;
  Learning Offering permissions remain available for Delivery itself.
- Generated contracts, permission checks, backend/frontend tests, migration gates, deployment, and
  authenticated sandbox smoke tests pass serially before maintenance is disabled.
