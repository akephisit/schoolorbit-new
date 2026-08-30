# Timetable Drag-and-Drop and Effective Teacher Assignment Design

**Date:** 2026-08-30

**Status:** Approved in chat; awaiting written-spec review

**Scope:** effective-dated learning-group teacher responsibility, exact per-period instructors,
mid-term teacher changes, editable homeroom and teacher timetable boards, read-only whole-school
overview, conflict preview and atomic placement, generated API contracts, forward-only tenant
migration, and serial release verification

## Context

SchoolOrbit now owns immutable published timetable versions, effective-from operational change sets,
version-owned weekly-period targets, learning offerings, learning groups, homeroom coverage, and
teacher assignments. The current timetable page is still a 1,240-line form-driven workspace: staff
select one learning group or homeroom, click one cell, and edit the period through a side form. The
backend already exposes version-aware create, update, swap, move-validation, and occupancy
boundaries, but the page does not provide a direct drag-and-drop workflow.

The current teacher rule is also too coarse for real scheduling. For a learning-group timetable
entry, the runtime derives every teacher assigned to the group and treats every one of them as
teaching every period. This cannot distinguish:

- teacher A teaching one period and teacher B teaching another period for the same subject and
  learning group;
- teachers A and B co-teaching one period;
- a teacher joining or stopping during an active term without rewriting old timetable history; or
- the exact workload and personal timetable of each teacher.

The existing `timetable_entry_instructors` table already represents the right boundary—teachers
actually scheduled for one timetable entry—but current course/activity group entries do not use it
as the sole runtime source. A clean cutover is required so conflict checking, personal timetables,
daily teaching, exports, and future workload consumers all read the same exact per-period teacher
set.

Academic staff also need to resolve one schedule change from more than one working perspective.
They commonly build a complete timetable room by room, then repair teacher workload and conflicts
from a teacher timetable. A dense whole-school matrix is useful for inspection but unsafe as a
primary mutation surface. These are views of one timetable version, not separate schedules.

## Relationship to Existing Approved Designs

This design extends, and does not replace, these approved boundaries:

- `2026-08-23-academic-core-lifecycle-redesign-design.md` remains authoritative for the normalized
  Academic Core, explicit context, published snapshots, and future Gradebook/lifecycle ownership.
- `2026-08-28-curriculum-structure-and-homeroom-delivery-design.md` remains authoritative for the
  catalog -> curriculum -> offering -> learning-group chain, homeroom coverage, and combined groups.
- `2026-08-29-academic-workload-and-term-delivery-design.md` remains authoritative for official
  catalog workload, timetable-version weekly-period targets, and later-term reset to catalog
  standards.
- `2026-08-30-academic-operational-change-and-timetable-versioning-design.md` remains authoritative
  for effective-from change sets, immutable published timetable versions, offering add/stop,
  historical resolution by date, atomic publication, and no compatibility runtime.

This design supersedes only the previous first-release decision that teachers on a published
learning group can never change. Published historical teacher responsibility remains immutable, but
a later effective-dated teacher episode may now be added, adjusted, or stopped through the existing
mid-term operational change-set workflow. Direct in-place editing of historical assignments remains
forbidden.

It also supersedes the current runtime behavior that every group teacher teaches every entry. After
cutover, `timetable_entry_instructors` is the only owner of the teachers who actually teach a
specific timetable period.

## Goals

- Let staff arrange a draft timetable by dragging one period at a time.
- Make homeroom and teacher views both fully editable against the same timetable version.
- Keep a whole-school overview read-only and route each issue to an exact editable context.
- Let one learning-group period have one teacher or multiple co-teachers.
- Let different periods of the same subject and learning group use different teachers.
- Add, adjust, or stop teacher responsibility mid-term through the existing effective-from change
  set and timetable-version workflow.
- Preserve old teacher and timetable history while a future published version waits for its
  effective date.
- Clone the prior timetable as the starting point; never require a full rebuild merely because one
  teacher changes.
- Offer explicit bulk handoff choices without guessing how two or more incoming teachers divide
  periods.
- Detect learning-group, covered-homeroom, teacher, and physical-room conflicts before a drop and
  revalidate them atomically on the server.
- Support a valid move into an empty cell and an atomic swap with an occupied cell; never overwrite
  or silently remove the existing period.
- Preserve version-owned target counts and publication readiness.
- Use set-based reads, typed Rust/OpenAPI DTOs, generated frontend contracts, optimistic
  concurrency, audit, and exact migration reconciliation.
- Decompose the oversized Svelte route into focused controller, data-access, and presentation
  units with keyboard and touch alternatives to drag-and-drop.

## Non-Goals

- Do not build automatic timetable generation or optimization.
- Do not let the system guess which incoming teacher replaces which outgoing teacher.
- Do not add a separate teacher timetable data store.
- Do not allow mutation from the dense whole-school overview.
- Do not support alternating weeks, A/B weeks, date-specific substitutions, delivered-lesson
  attendance, or emergency same-day substitute workflows in this release.
- Do not introduce linked double-period or multi-period blocks. One drag creates or moves one bell
  period. Staff place two adjacent periods twice when they want a double period.
- Do not infer co-teaching from adjacent entries or from multiple teachers on a learning group.
- Do not allow an instructor to teach two distinct entries in the same slot. Legitimate combined
  teaching is one learning group and one timetable entry with all co-teachers attached.
- Do not change curriculum credit, official hours, catalog standard periods, or timetable-version
  weekly-period targets merely because periods are moved or teachers change.
- Do not change a learning group's student roster from the timetable page.
- Do not build teacher payroll weighting, fractional workload credit, or compensation rules.
- Do not implement Gradebook, term closure, annual closure, or promotion in this work.
- Do not retain dual reads, dual writes, fallback teacher derivation, legacy request parsing, or a
  per-tenant compatibility branch after cutover.

## Selected Approach

### One timetable version with multiple working views

One `academic_timetable_entry` remains the scheduled unit for one bell period. Homeroom, teacher,
learning-group, and whole-school presentations index the same entry collection. Moving an entry in
one editable view changes the same row seen by every other view.

The selected views are:

| View | Mutation | Primary job |
|---|---:|---|
| Homeroom | Yes | Complete and repair the weekly timetable of one homeroom |
| Teacher | Yes | Complete and repair the periods actually taught by one teacher |
| Learning group | Yes | Retain the existing focused group editor and support combined groups |
| Whole school | No | Inspect completeness, gaps, conflicts, and workload at school scale |

The homeroom view is the default because it gives academic staff the clearest completion boundary.
The teacher view is equally capable of create, move, swap, room change, and per-period instructor
change. The learning-group view remains available so a combined or elective group can be managed
without pretending it belongs to only one homeroom.

The whole-school overview selects one day and renders homerooms by bell period plus issue summaries.
Clicking a room, teacher, or issue opens the exact timetable version and editable view with the
relevant entity and period focused. It does not accept drops.

### One period per drag

The unscheduled tray shows each learning group's target, scheduled count, and remaining count. A
drag always creates one period. To schedule `2+1`, staff place the same group twice in adjacent
periods and once elsewhere. The system does not store or infer a linked block.

This keeps create, move, swap, deletion, target counting, keyboard interaction, and conflict
semantics uniform. An excess over the target remains a reviewable warning at publication; a deficit
remains blocking under the existing timetable-version policy.

### Exact teachers per timetable entry

Learning-group teacher responsibility and timetable-entry instruction have different meanings:

- a learning-group teacher assignment says that a teacher may be responsible for the group during
  an effective date interval; and
- a timetable-entry instructor says that the teacher actually teaches this one scheduled period in
  this timetable version.

A course or activity entry may select one or more teachers active for its learning group at the
target timetable version's effective-from date. One selected teacher means solo teaching. Multiple
selected teachers on the same entry mean co-teaching and are checked together for conflicts.

Different entries of the same learning group may select different subsets. For example:

```text
ค21101 · ม.1/1 · target 3 periods
Monday period 1    -> teacher A
Wednesday period 2 -> teacher B
Friday period 3    -> teachers A + B
```

The learning-group responsibility role and the per-entry display order do not determine workload
weight. For the first release, the UI selects a teacher set, not a per-period lead role. The backend
orders the selected teachers deterministically using active group role, assignment start, and ID for
stable display. A sole selected teacher is the entry's effective primary instructor even if their
group-responsibility role is secondary.

Draft timetable entries may temporarily have no instructor while a teacher stop is being resolved.
Published course and activity entries must have at least one eligible active instructor. Structural
entries such as homeroom or academic events retain their existing optional direct instructor
selection.

### Effective teacher changes reuse the existing mid-term workflow

The Delivery page remains the owner of the button `เพิ่ม / ปรับ / หยุดกลางภาค`. The dialog gains a
teacher category alongside offering/activity and weekly-period changes. Initial typed teacher
actions are:

- add a teacher responsibility episode for a learning group;
- adjust the responsibility role from the change set's effective date; and
- stop an existing responsibility episode from the change set's effective date.

The change set continues to own one effective-from date, reason, base timetable version, and target
draft timetable version. A draft has no operational effect even when its planned date arrives. Only
publication creates future effective state. A future published version remains upcoming until its
date; a draft whose date has passed must move to a valid current or future date before publication
under the existing active-term rule.

Publishing an effective stop on 1 August derives the old episode's final day as 31 July. Users do
not enter that derived date separately. Published historical assignment episodes are never deleted
or edited into a different identity.

Adding a teacher does not silently change any cloned timetable entry. Stopping a teacher makes every
target-version entry that still references that teacher a blocking handoff finding. The target
version keeps the same day, period, group, homeroom coverage, and room until staff explicitly choose
new instructors or move the entry.

### Explicit teacher handoff, not automatic replacement

When a change set stops teacher A and adds teachers B and C, the Delivery flow links to a handoff
step over the cloned target timetable. The system previews the periods affected and offers explicit
choices:

- assign B to every affected period;
- assign C to every affected period;
- assign B and C as co-teachers to every affected period; or
- assign each period manually.

These choices stage a proposal first. They do not partially or silently apply. The preview reports
which rows are valid, which proposed teachers conflict, and which entries still lack an eligible
teacher. Staff may change individual rows, choose another eligible teacher, or open the drag board
to move a period.

If a stopped teacher remains on any target-version entry, publication is blocked. If a newly added
teacher is not assigned to any entry, publication is allowed; responsibility does not imply a
mandatory personal quota.

### Move, swap, and collision behavior

During a drag, cells have four semantic states:

- neutral: not evaluated or outside the active board;
- valid/green: the entry can move or be created there;
- swap/purple: an occupied cell can exchange its entry atomically with the dragged entry; and
- blocked/red: the operation would violate a hard constraint.

The state is never communicated by color alone. Each target has text and accessible status such as
`วางได้`, `สลับคาบ`, or `ครู B มีสอน ค22101 · ม.2/1`.

Dropping into an empty valid cell performs one create or move. Dropping onto a valid occupied cell
performs one swap after previewing both sides. Dropping onto a blocked cell performs no mutation.
There is no overwrite action.

Dragging a co-taught entry from either teacher view moves the complete entry: the learning group,
all covered homerooms, physical room, and every co-teacher. The preview names every affected teacher
and homeroom. Removing one co-teacher is a teacher-set edit in the entry inspector, not a drag of a
partial entry.

The frontend uses set-based occupancy data for immediate target highlighting. The backend locks the
relevant version, entries, and slots in stable order and rechecks authoritative state on drop. A
concurrent edit returns `409 Conflict`, preserves the draft, and reloads the affected version for
review.

Version-aware database guards remain the final invariant. Entry insert/move/reactivation checks the
learning group, complete covered-homeroom set, and physical room under the same slot lock. Exact
instructor child insert/update checks the teacher under that version/day/period. The service maps
named database violations back to the same typed conflict codes returned by placement preview.

## Authoritative Data Model

### Effective learning-group teacher responsibility

The existing `learning_group_teachers` boundary is extended through a new forward migration. Each
assignment episode owns:

- learning group and academic context;
- teacher and responsibility role;
- `starts_on` and optional inclusive `ends_on`;
- starting and ending change-set provenance where applicable;
- row version, creator/updater, and timestamps; and
- migration provenance for deterministic backfill.

Episodes for the same group and teacher may not overlap. Re-adding a teacher later creates a new
episode rather than reopening the old row. Role adjustment ends the old role episode on the day
before effective-from and creates a new episode beginning on effective-from.

For a published or closed learning group:

- identity, start date, and historical role cannot be rewritten;
- an open episode may be closed only once with a valid stop/adjust change-set item;
- a later episode may be inserted only with valid add/adjust change-set provenance; and
- rows may not be hard-deleted.

Draft learning groups retain direct teacher setup before initial publication. Initial publication
requires an active primary-responsibility teacher and turns those rows into the first immutable
episodes. The current broad `teachers_locked` presentation is replaced with explicit guidance:
historical assignments are locked, while future changes use the mid-term workflow.

### Exact timetable-entry instructors

`timetable_entry_instructors` becomes the only runtime owner of actual instruction for every entry,
including course and activity entries attached to a learning group.

Updating an entry's teacher set:

1. locks the draft timetable version and entry;
2. checks the entry row version;
3. resolves teacher eligibility including pending add/stop items for the target change set;
4. checks every proposed instructor against the target slot;
5. replaces the entry's instructor set in one transaction;
6. bumps the parent entry row version; and
7. appends an audit event without raw request data.

The parent entry row version guards the complete instructor set; child rows do not expose an
independent public mutation race. Published-version child immutability is extended to
`timetable_entry_instructors`, so direct insert, update, or delete against a published timetable
version fails at the database boundary.

Cloning a timetable version copies the exact entry instructor set. Later changes to group teacher
responsibility never rewrite a prior version's instructors.

### Learning groups and shared subject codes

The draggable resource is a learning group, not only a subject/activity code. Separate learning
groups using the same code may have independent schedules and instructor subsets.

A combined learning group may cover multiple homerooms. Its one timetable entry appears in each
covered homeroom and each selected teacher view, but remains one entry ID. Moving it from any view
moves it for every covered homeroom. Conflict checks use the complete homeroom coverage set.

The same code alone never causes a conflict. Conflicts arise from shared group identity, overlapping
homeroom coverage, shared selected teacher, or shared physical room in one slot.

### Timetable versions and effective resolution

Only a draft timetable version accepts mutation. Published versions and all entry/instructor child
rows remain immutable. Date-based readers continue to resolve exactly one published version:

1. the newest published version whose effective-from is on or before the requested date;
2. bounded by the next published version or actual term closure; and
3. never a draft, even if the draft's proposed effective date has arrived.

Teacher responsibility eligibility for a target draft uses that version's effective-from date plus
the owning change set's pending teacher actions. Historical/personal timetable readers use the
instructor snapshots already stored on the resolved published entries.

## Conflict Model

Every candidate create, move, teacher-set change, and swap checks:

| Conflict | Blocking condition |
|---|---|
| Learning group | Same learning group occupies two distinct entries in one slot |
| Homeroom | Any covered homeroom overlaps another active entry in one slot |
| Teacher | Any selected instructor teaches another distinct entry in one slot |
| Physical room | The selected room is used by another active entry in one slot |

Multiple teachers on one entry are not conflicts with each other. One teacher attached twice to the
same entry is normalized to one child row. A teacher appearing on two distinct entries is blocked,
even when the subject code matches. If the teaching is genuinely combined, staff must use one
combined learning group and one entry.

Changing teachers without moving a period normally introduces only teacher conflicts because the
cloned group, homeroom, and room placement was already valid. Once staff move or swap an affected
entry, all four conflict classes are rechecked for both sides.

Hard conflicts are rejected during draft mutations and again at publication. Drafts may temporarily
contain missing instructors or references to a teacher pending an effective stop; those are explicit
blocking readiness findings rather than hidden conflicts.

## Workflows

### Build a timetable by homeroom

1. Staff open a draft timetable version and select a homeroom such as `ม.1/1`.
2. The tray lists applicable learning groups with target, scheduled, and remaining periods.
3. For a group with one eligible teacher, that teacher is selected automatically.
4. For a group with multiple eligible teachers, staff select one or more teacher chips for the next
   placement. The current selection remains visibly associated with that tray card until changed.
5. Staff drag the group into one day/period or use the accessible `เลือกวันและคาบ` action.
6. Validity is shown locally; the server revalidates and creates the entry with the selected exact
   instructors.
7. Clicking an entry opens the inspector for room, teacher set, title/note, move, and deactivate.

### Build or repair from a teacher view

1. Staff select a teacher and see only entries where that teacher is an exact entry instructor.
2. The tray shows learning groups for which the teacher is eligible at the version date, together
   with each group's overall target progress. It does not invent a mandatory per-teacher quota.
3. Placing a new entry from this view includes the selected teacher by default; additional eligible
   co-teachers may be selected.
4. Moving an existing entry moves the whole entry and every co-teacher.
5. A teacher-set edit may transfer one period to another eligible teacher without moving the period.

### Add and stop teachers mid-term

1. Staff open Delivery and click `เพิ่ม / ปรับ / หยุดกลางภาค`.
2. They enter one effective-from date and reason.
3. They add teacher responsibility actions under exact learning groups.
4. The workflow clones the effective base timetable into a target draft.
5. A handoff panel lists every target entry still using a teacher who will be inactive.
6. Staff apply an explicit bulk proposal or assign periods individually.
7. Conflicting proposals identify the existing teacher entry and link to the editable teacher or
   homeroom board.
8. Staff may rearrange any authorized period in the target draft, not only affected teacher rows,
   because chain moves may be required.
9. Publication validates and applies teacher episodes, timetable instructors, placements, and
   version state in one transaction.
10. The old version and old teacher episodes remain authoritative before effective-from; the new
    version resolves automatically on and after that date.

### Inspect the whole school

1. Staff select a day in the whole-school tab.
2. The page shows a read-only matrix of homerooms by period plus totals for missing target periods,
   unresolved teacher handoffs, teacher conflicts, room conflicts, and unscheduled groups.
3. A clicked cell opens the exact homeroom editor. A clicked teacher finding opens the exact teacher
   editor. URLs retain academic year, term, timetable version, entity, day, and period context.

## Frontend Workspace

### Visual direction

The page is a calm Thai academic operations desk, not a promotional dashboard. It retains the
SchoolOrbit shell and Kanit typography. The compact palette is School Blue `#0B63B6`, Canvas
`#F4F7FA`, Ink `#0F172A`, Valid `#16A34A`, Swap `#7C3AED`, and Blocked `#DC2626`, implemented through
existing semantic tokens where available. Dark mode uses token equivalents rather than fixed light
surfaces.

The signature element is a live constraint map during drag: cells communicate valid, swap, and
blocked state while the inspector names the exact resource causing a block. This is the one strong
visual device; surrounding controls remain quiet and dense enough for real school timetables.

### Page composition

The route keeps PageShell and the explicit timetable-version selector. Its working area becomes:

```text
Version/status and publication readiness
View tabs: Homeroom | Teacher | Learning group | Whole school
Filters and selected-context summary

Editable views:
┌──────────────────┬──────────────────────────────┬────────────────────┐
│ Unscheduled tray │ Weekly timetable board       │ Entry/issues panel │
└──────────────────┴──────────────────────────────┴────────────────────┘

Whole-school view:
┌─────────────────────────────────────────────────────────────────────┐
│ Read-only day matrix + issue summaries + links to editable context │
└─────────────────────────────────────────────────────────────────────┘
```

Controls use local shadcn-svelte Button, Select/Combobox, Dialog/Sheet, Badge, Tooltip, and Alert
primitives. The timetable grid and draggable cards remain semantic custom components because
shadcn-svelte does not own a timetable board.

### Component boundaries

The existing route is decomposed into focused units rather than growing further:

- route controller for academic context, version selection, permissions, dirty state, and realtime
  invalidation;
- typed timetable-workspace data access;
- derived workspace indexes and local drag preview state in a route-scoped Svelte state class;
- version/readiness header;
- view selector and entity filters;
- unscheduled learning-group tray;
- shared timetable board, cell, and lesson-card components;
- homeroom, teacher, and learning-group view adapters;
- read-only whole-school overview;
- entry inspector for teacher set, room, note, move, and deactivate;
- teacher-handoff panel; and
- teacher-load export kept as a lazy action-owned utility.

Large API collections use `$state.raw` and `$derived` indexes. Effects are reserved for external
socket or drag-library synchronization; event transitions own mutation state. No stateful shared
module may leak timetable data across SSR users.

### Drag, touch, keyboard, and feedback

The route opts into the existing package-local mobile drag/drop support only on this page. Dragging
is not the only interaction:

- keyboard and touch users can select a tray or existing card, choose `ย้าย`, then choose day and
  period through shadcn controls;
- occupied valid targets present an explicit `สลับคาบ` action;
- live-region guidance announces the selected item, target state, successful move, and exact block;
- focus remains on the moved entry or failed target after a mutation;
- reduced-motion preference disables nonessential transitions; and
- horizontal scrolling, sticky period/day headers, and minimum cell widths preserve Thai labels.

Read-only, missing-dependency, permission, stale, conflict, request-failure, and empty states remain
distinct. A stale mutation never discards the selected context or silently reloads the entire app.

## API and Contract Design

Typed Rust DTOs and `utoipa` remain the wire-contract authority. Generated TypeScript types remain
the only frontend API boundary.

### Set-based timetable workspace

A timetable workspace read returns, in one bounded response for an explicit academic term and
timetable version:

- version and bell-schedule context;
- periods and active days;
- timetable entries with exact instructors;
- learning groups, covered homerooms, offering labels, target counts, and preferred rooms;
- effective eligible teacher assignments, including pending target change-set actions where
  authorized;
- physical-room lookup data needed for editing;
- target completion and unresolved handoff summaries; and
- change-set/readiness identifiers needed by the selected draft.

Read-only callers receive only labels and relationships required to render the selected view. They
do not load management-only teacher or room options. The response is set-based and must not issue or
encourage one request per group, teacher, cell, or entry.

The whole-school overview has its own day-bounded set-based read model rather than rendering every
editable control for the entire term.

### Mutations

The coordinated contract cutover:

- extends create/update timetable-entry requests to accept an exact `instructorIds` set for
  learning-group entries;
- lets update replace the complete instructor set while guarding the parent `rowVersion`;
- retains explicit create, move/update, swap, and deactivate semantics inside one draft version;
- replaces the old entry-only move validation shape with a typed placement preview that supports a
  new tray item, an existing move, a proposed room/instructor set, and an occupied swap target;
- adds typed preview/apply operations for bulk teacher handoff proposals;
- extends operational change-item DTOs with named add/adjust/stop teacher variants; and
- returns stable conflict codes, affected entry/resource labels, severity, and exact recovery
  context.

Known request/response variants are tagged DTOs. They do not use `unknown`, generic records, manual
frontend casts, or untyped JSON business state. Successful mutation responses return the changed
entries and updated summary revision needed for a local state patch; they do not require a broad
workspace reload.

### Permissions

The initial authorization boundary reuses generated Learning Offering manage/read permissions and
resource policies:

- viewing an entry requires read access to its offering/group scope;
- editing a group entry requires manage access to that learning group;
- school-wide structural entries and whole-school teacher changes require school manage authority;
- a cross-group move/swap/change set requires the union of access for every affected group; and
- the server checks each resource again during publication.

No broad convenience permission is introduced. Read-only users never receive teacher-selection or
room-management options merely because the page can render a timetable.

## Publication Readiness

In addition to existing change-set and timetable-version checks, publication requires:

- every course/activity entry has at least one exact instructor;
- every exact instructor is an eligible active group teacher at effective-from after pending teacher
  actions are applied;
- no target entry references a teacher ending before effective-from;
- no learning-group, homeroom, teacher, or physical-room conflict exists;
- every scheduled group/offering remains available;
- target period deficits are zero;
- target excess warnings are explicitly acknowledged;
- stopped offerings remain absent from the target version; and
- source row versions, target version revision, and normalized proposal hashes remain current.

Publication locks resources in stable order, re-reads authoritative state, applies teacher episode
changes and the target timetable state, writes audit events, and commits once. Any failure leaves
the prior published version and teacher assignments authoritative.

## Error, Concurrency, Audit, and Realtime Semantics

- A blocked drag never calls a mutation endpoint.
- A server-discovered conflict returns `409 Conflict` with a stable code and exact school-facing
  labels, not unrelated personal data.
- A stale entry/version returns `409`, preserves the user's selected view, and asks them to reload
  the affected draft.
- A teacher bulk handoff preview never applies a partial mapping. Staff review valid and invalid
  rows before apply.
- Reusing a successful idempotency key with the same normalized publication input returns the same
  result; changed input is rejected.
- Audit records teacher responsibility episode changes, entry instructor before/after sets, moves,
  swaps, effective date, reason, actor, change-set/version IDs, and row versions.
- Audit, logs, errors, and realtime payloads contain no plaintext national IDs, credentials,
  database URLs, tokens, cookies, or raw request bodies.
- Realtime events are invalidation signals only. A remote change while the user is dragging marks
  the draft stale; the client re-reads typed state and never trusts an event as data or permission
  truth.

## Migration and Clean Cutover

Implementation reads the complete applied migration timeline and adds only the next sequential
forward migration. No applied migration is edited.

### Preflight

Preflight inventories:

- learning-group teacher rows by group, role, teacher status, and academic context;
- every active/inactive timetable entry by version and learning-group ownership;
- existing timetable-entry instructor rows, including rows already attached to group entries;
- course/activity entries with no active group teacher;
- duplicate or overlapping teacher responsibility candidates;
- teacher conflicts implied by the current runtime rule that every group teacher teaches every group
  entry;
- published and draft timetable-version child cardinalities;
- personal timetable, daily teaching, export, supervision, and other instructor consumers; and
- rows whose offering start or term context cannot deterministically seed an assignment start.

Any unmappable group entry, ambiguous context, invalid interval, or implied historical teacher
conflict blocks the destructive stage and reports school-facing group/teacher labels for repair. The
migration never selects one teacher arbitrarily.

### Forward migration

The cutover:

1. adds effective interval, provenance, and concurrency ownership to learning-group teacher
   assignments;
2. backfills each current assignment from the owning offering start date, which matches the current
   runtime's undated responsibility semantics;
3. replaces the old unique/immutability rules with non-overlap and append/one-way-close history
   constraints;
4. defines the authoritative instructor set for every group timetable entry as every teacher the
   current runtime derives for that group;
5. reconciles existing child rows to exactly that deterministic set and retains direct instructor
   rows for structural entries;
6. installs version-aware database conflict guards for group, covered homeroom, physical room, and
   exact entry instructor, plus published-version child immutability for entry instructors;
7. cuts list, conflict, personal timetable, daily teaching, export, readiness, and clone consumers to
   exact entry instructors;
8. adds teacher change-set shapes and database constraints;
9. verifies entry, assignment, instructor, version, target, and downstream-reference cardinalities;
   and
10. removes the old fallback derivation and hard teacher-lock runtime branch.

There is no compatibility read or write after deployment. The backend, OpenAPI artifact, generated
frontend contract, and frontend deploy as one coordinated release unit for each cutover milestone.
A protected Neon snapshot is required immediately before applying a destructive real-tenant stage.

## Release Sequence

### Release 1 — Exact instructor and effective teacher foundation

- add and reconcile the forward migration;
- cut all instructor consumers to exact timetable-entry instructors;
- extend typed contracts for exact instructor create/update and effective assignments;
- add a minimal instructor multi-select to the current form-driven draft editor so the release is
  operational before drag-and-drop; and
- verify migration, conflicts, personal timetables, daily teaching, exports, clone, and published
  immutability on `sandbox`.

### Release 2 — Editable homeroom and learning-group drag board

- decompose the current Svelte route;
- add the set-based workspace read model and route-scoped derived indexes;
- implement the unscheduled tray, one-period drag, valid/blocked highlighting, atomic move/swap,
  entry inspector, and keyboard/touch alternative;
- preserve version/readiness/publication controls; and
- smoke-test real homeroom, combined-group, room, target, and concurrent-edit scenarios.

### Release 3 — Editable teacher board and whole-school overview

- add the teacher editor with solo/co-teacher placement and exact workload counts;
- make moving from a teacher view move the complete shared entry;
- add the day-bounded read-only whole-school matrix and exact recovery links; and
- verify no data duplication or request-per-row fan-out across views.

### Release 4 — Mid-term teacher change and handoff

- add teacher add/adjust/stop actions to the existing Delivery change-set dialog;
- add effective interval preview/apply, bulk handoff proposals, individual allocation, and conflict
  recovery links;
- integrate teacher readiness into atomic change-set publication; and
- verify future activation, late-draft behavior, history preservation, idempotent retry, and full
  Delivery-to-timetable Playwright workflows.

Each release receives its own implementation plan, focused verification checkpoint, commit, push,
automated deployment, and authenticated `sandbox` smoke test. Local Rust and frontend commands run
serially because the environment is known to stall under concurrent builds.

## Testing and Verification

Implementation follows test-driven development and the change-type matrix in `.rules`.

Database/backend coverage proves:

- deterministic teacher-episode and exact-entry-instructor backfill with exact cardinality;
- non-overlapping teacher episodes, re-addition after a stop, and one-way historical closure;
- draft group direct setup and published group change-set-only mutation;
- solo, split-period, and co-teaching instructor sets;
- eligibility using target effective-from plus pending add/stop actions;
- conflict checks for group, covered homeroom, exact teacher set, and physical room;
- valid empty moves, valid swaps, invalid swaps, stable lock order, and stale row versions;
- combined-group entries appear across homerooms without duplicate storage;
- cloned versions preserve exact instructors;
- old and future date resolution returns the correct instructor snapshots;
- bulk handoff preview is non-mutating and apply is atomic;
- publication blocks missing/inactive/stopped teachers and every hard conflict;
- allowed and denied resource-policy paths; and
- OpenAPI registration for every changed typed DTO.

Frontend coverage proves:

- one-teacher groups default visibly and multi-teacher groups allow one or many selections;
- one drag creates exactly one period;
- homeroom, teacher, and learning-group edits update the same local entry identity;
- co-taught entries render on both teacher views and move as one complete entry;
- valid, swap, and blocked cells have text and accessible announcements, not color alone;
- keyboard/touch placement can complete every drag action;
- whole-school overview is read-only and routes to an exact editable context;
- target remaining counts, excess warnings, and unresolved handoffs update without a broad reload;
- read-only users do not request management-only options;
- no request-per-row/cell fan-out or untyped payload is introduced; and
- mobile, dark mode, sticky headers, horizontal scrolling, focus, and reduced motion remain usable.

Focused Playwright workflows cover:

- build a homeroom timetable from the tray;
- split one subject's periods between two teachers;
- assign two teachers to one co-taught period;
- move and swap from homeroom and teacher views;
- block each conflict class with exact recovery guidance;
- move one combined-group entry and observe every covered homeroom;
- add B/C, stop A, allocate periods, publish a future version, and verify old/new date resolution;
- reject a late unpublished effective date; and
- recover from a concurrent stale mutation without losing selected context.

Applicable gates include focused database/service/component tests, disposable migration/preflight
tests, `cargo fmt --all -- --check`, backend static architecture tests, `cargo check`, API contract
generation/check/tests, frontend lint, Svelte check and analyzer/autofixer, frontend static tests,
focused Playwright, `git diff --check`, final diff review, and `git status --short`. Neon compatibility
uses only the explicit disposable-branch gate. Deployment and snapshot handling follow
`docs/OPERATIONS.md`.

## Deployment and Recovery

- Keep a protected Neon snapshot until migration reconciliation, authenticated read-only checks,
  exact instructor checks, and selected workflow smoke tests pass.
- Deploy through the existing maintenance/all-tenant migration workflow; do not run ad-hoc live SQL
  or edit `_sqlx_migrations`.
- If preflight or migration verification fails, the transaction rolls back and maintenance remains
  enabled. Repair uses a reviewed forward migration or explicit source-data correction.
- The previous application is not a supported rollback after the incompatible exact-instructor
  cutover accepts new writes. Before accepting writes, recovery may restore the snapshot and prior
  release. After acceptance, recovery moves forward unless the school explicitly accepts snapshot
  restoration and reconciliation of later writes.

## Success Criteria

- Staff can arrange one draft version by homeroom or teacher and see the same entry update in every
  view.
- The whole-school view reveals issues without allowing unsafe dense-grid mutation.
- One subject/group may split periods among teachers or use multiple co-teachers in one period.
- Personal timetables, daily teaching, exports, workload, and conflicts use teachers who actually
  teach each period rather than every teacher assigned to the group.
- Mid-term teacher add/adjust/stop uses the existing effective-from workflow and preserves history.
- A teacher change clones the prior schedule; staff reassign only affected periods unless they
  intentionally rearrange more of the draft.
- No system guess assigns B/C when A stops, and unresolved or conflicting handoffs cannot publish.
- Empty moves and valid swaps are atomic; overwrites and hard conflicts are impossible through UI,
  API, or database paths.
- Combined groups remain one entry shared across covered homerooms and co-teacher views.
- Published versions and exact instructor snapshots remain immutable and resolve correctly by date.
- Migration reconciles exact teacher/instructor cardinality and removes the old fallback runtime
  without compatibility code.
- Generated contracts, permissions, backend/frontend checks, migration gates, deployments, and
  authenticated `sandbox` smoke tests pass serially for every release.
