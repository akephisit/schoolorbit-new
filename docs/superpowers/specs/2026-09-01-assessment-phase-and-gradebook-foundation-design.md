# Assessment Phase and Gradebook Foundation Design

**Date:** 2026-09-01

**Status:** Approved in chat; awaiting written-spec review

**Scope:** fixed four-phase assessment plans, one assessment coordinator per course offering,
per-phase teacher controls, readable assessment-plan workspace, clean exam-schedule integration,
per-learning-group score-item ownership, forward-only tenant migration, generated API contracts,
and serial verification

## Context

The current assessment workspace exposes assessment categories as free-form rows. A user may add,
delete, rename, reorder, or mark any category as `none`, `in_timetable`, `outside_timetable`, or
`practical`. The backend already creates four virtual defaults—before midterm, midterm, after
midterm, and final—and the exam scheduler already imports only ready midterm/final categories
marked `in_timetable`. The UI and storage contract nevertheless still describe these phases as
arbitrary categories.

The current `course_assessment_items` boundary also belongs to a course assessment plan. One plan
belongs to one learning offering and can cover several learning groups. If score items remain under
that plan, teachers of every covered homeroom would be forced to share the same worksheets, tests,
projects, and exam sections. The desired rule is different:

- every learning group under the same offered subject uses the same four phase maxima;
- the midterm/final exam arrangement is shared by the offered subject;
- each learning group may define its own score items; and
- future student score entry consumes those group-owned items.

Assessment-plan readiness must not depend on score-item creation. Academic staff need the shared
phase allocations and exam intentions early enough to prepare the central exam timetable, while a
teacher may decide or revise worksheets and exam sections later during teaching. There is no
explicit submit step: valid edits auto-save, and readiness is derived from the current saved values.

The existing school-wide `academic_assessment_teacher_access` feature toggle is too coarse for that
future workflow. Academic staff need independent controls for score-item editing and student-score
entry for each of the four phases.

## Current Tenant Evidence

A read-only inspection of `schoolorbit_snwsb_v2` on 2026-09-01 found:

- 131 persisted course assessment plans in the selected term;
- all 131 plans have exactly the four canonical phase codes;
- all 131 plans total exactly 100.00 points;
- all 131 plans are currently `saved`;
- zero `course_assessment_items` rows under those plans;
- no assessment phase uses the `practical` exam mode;
- 79 midterm and 79 final phases are marked `in_timetable`;
- 47 midterm and 47 final phases are marked `outside_timetable`;
- 5 midterm and 5 final phases are marked `none`;
- 13 final `in_timetable` phases do not yet have a duration;
- the published midterm exam round contains 178 imported schedule items; and
- the draft final exam round contains 157 imported schedule items.

Existing exam schedule items must survive the cutover. The current status column is removed from
the new workflow; fresh import eligibility is derived from the current coordinator, four-phase
total, exam arrangement, duration, and delivery context rather than a remembered submit action.

## Relationship to Existing Approved Designs

This design extends these existing boundaries:

- `2026-08-23-academic-core-lifecycle-redesign-design.md` remains authoritative for explicit
  academic context, normalized Academic Core ownership, no compatibility runtime, and the future
  Gradebook boundary.
- `2026-08-28-curriculum-structure-and-homeroom-delivery-design.md` remains authoritative for the
  catalog -> curriculum -> offering -> learning-group chain and homeroom coverage.
- `2026-08-30-timetable-drag-drop-and-effective-teacher-assignment-design.md` remains authoritative
  for effective-dated learning-group teacher responsibility. A group `primary` teacher remains a
  group-level role and is not redefined as the course-offering assessment coordinator.

This design supersedes only the current free-form assessment-category UI and the ownership of
course-level assessment items. It does not change curriculum credit, workload, delivery membership,
timetable placement, or the published exam-schedule lifecycle.

## Goals

- Make the assessment overview readable across all offered subjects in a term.
- Enforce exactly four assessment phases in the canonical order:
  `before_midterm`, `midterm`, `after_midterm`, and `final`.
- Keep the four phase labels and order system-owned and non-editable.
- Let one shared course-offering plan own phase maxima and midterm/final exam arrangement.
- Derive plan readiness only when phase maxima equal the offering grading policy total, normally
  100 points.
- Let one explicit assessment coordinator prepare the shared plan.
- Suggest the common primary teacher automatically when every learning group has the same active
  primary teacher.
- Preserve explicit academic-manager override of that suggestion.
- Let future score items belong to one learning group and one fixed phase.
- Allow different learning groups to use different score items while enforcing the same phase
  maxima.
- Do not require score items before an assessment plan becomes ready.
- Add separate, per-term, per-phase controls for score-item editing and student-score entry.
- Feed only ready, in-timetable midterm/final phases with a duration into central exam
  scheduling.
- Keep imported exam items as explicit snapshots and surface later source changes without silently
  deleting, moving, or resizing an arranged exam.
- Let a draft exam round preview and explicitly synchronize safe source changes with authoritative
  conflict revalidation.
- Preserve current tenant data and existing exam schedule identities through a forward migration.
- Remove free-form/custom phase behavior and the `practical` phase mode without compatibility reads
  or writes.

## Non-Goals

- Do not build the complete student Gradebook or score-entry screen in this release.
- Do not implement weighted averages, normalization, bonus-score semantics, or scores above a
  phase maximum.
- Do not create a special `bonus` item kind. A teacher may name an ordinary score item however they
  need.
- Do not require all learning groups to use the same worksheets, assignments, projects, or exam
  sections.
- Do not create separate timetable sessions for multiple-choice, written-response, or other
  sections of one midterm/final exam.
- Do not add automatic exam scheduling or change existing exam conflict behavior.
- Do not silently synchronize a published exam round or add published-round revision/versioning in
  this release.
- Do not infer an assessment coordinator when primary teachers differ across learning groups.
- Do not silently replace an explicitly selected coordinator after a teacher assignment changes.
- Do not retain a deleted score item, its future student scores, a restore workflow, or a domain
  history table after an authorized hard delete.
- Do not retain dual schemas, legacy request fields, fallback free-form categories, or runtime
  compatibility branches after cutover.

## Considered Approaches

### 1. Keep course-level items and change only the page

This is the smallest code change, but it keeps score items shared across every learning group under
the offering. It cannot represent different worksheets or exam sections for different rooms and is
rejected.

### 2. Split course phase policy from group score items

One course-offering plan owns the four shared phase maxima, coordinator, and exam intentions. Score
items belong to learning groups. This matches the school rule, supports future Gradebook work, and
keeps the exam scheduler independent from classroom score-entry details. This is the selected
approach.

### 3. Duplicate the complete assessment plan per learning group

This gives maximum local flexibility but allows phase maxima and exam intention to drift between
rooms with the same offered subject. It also duplicates review and exam-schedule input and is
rejected.

## Selected Domain Model

### Shared assessment plan

One course assessment plan continues to belong to one `learning_offering_id`, one subject version,
one academic term, and one academic year. The offering/subject-version identity is authoritative;
the displayed subject code is not used as a free-form join key.

The plan owns:

- optional assessment coordinator staff ID;
- optimistic row version and update metadata; and
- exactly four child assessment phases.

The coordinator is assessment-specific. It does not replace any learning-group teacher role and
does not imply that the coordinator teaches every covered group.

### Fixed assessment phases

The current category boundary becomes an explicit phase boundary. Each plan must own exactly one
row for each canonical code:

| Phase code | Thai label | Order | Exam arrangement |
|---|---|---:|---|
| `before_midterm` | ก่อนกลางภาค | 1 | Always `none` |
| `midterm` | กลางภาค | 2 | `none`, `outside_timetable`, or `in_timetable` |
| `after_midterm` | หลังกลางภาค | 3 | Always `none` |
| `final` | ปลายภาค | 4 | `none`, `outside_timetable`, or `in_timetable` |

Phase labels and ordering are derived from the code and are not stored as editable business data.
`custom` and `practical` are removed from the phase contract.

Each phase stores its exact `NUMERIC(10,2)` maximum. An `in_timetable` midterm/final phase must have
a positive duration before the plan is ready. Duration is cleared for `none` and may be omitted for
`outside_timetable`, because an outside-timetable exam is not imported into the central scheduler.

An auto-saved plan may be incomplete or temporarily over/under its grading-policy total while the
coordinator reallocates points between phases. Each individual maximum must be non-negative and
representable exactly. Readiness requires a coordinator, all four phases, a total equal to the
grading-policy total, and complete in-timetable exam metadata. Score items are not part of this
calculation.

### Assessment coordinator

The coordinator candidate set is every currently active teacher assigned to at least one learning
group under the offering. Academic managers select the coordinator; the selected coordinator may
edit the auto-saved shared plan under the exact assessment permissions.

When no coordinator has been persisted, the system evaluates current group-primary assignments:

1. every active learning group must have exactly one active primary teacher;
2. the distinct teacher-ID set across those groups must contain exactly one teacher; and
3. that teacher becomes the suggested coordinator.

The assessment page preselects the suggestion, and the first authorized plan save persists it. A
read request does not mutate tenant data. If the primary teachers differ, a group lacks a valid
primary teacher, or the offering has no active group, no suggestion is made.

Once persisted, an explicit coordinator is never silently overwritten. If that teacher is no
longer active on any group in the offering, the workspace shows a blocking attention state for the
next plan mutation and offers the current common primary teacher as a replacement when one exists.
Academic managers may select any current candidate manually.

### Per-phase access controls

Each academic term owns exactly four phase-control rows. Each row stores two independent Boolean
controls:

- `item_editing_enabled`: group teachers may create, rename, reorder, resize, or delete score items
  in this phase; and
- `score_entry_enabled`: group teachers may enter or change student scores in this phase.

The controls are school-wide for the selected academic term in the first release. Per-offering,
per-group, and per-teacher overrides are intentionally omitted.

Academic managers can change the controls from the assessment workspace. The API updates the
requested phase atomically with row-version checking. The future Gradebook must enforce both the
exact group assignment and the relevant phase control on the server; hiding a frontend control is
not authorization.

The current global teacher-access feature toggle is removed from the assessment workflow rather
than retained as an additional master switch. Shared-plan edit authority comes from coordinator and
school-management permission. Group score-item and score-entry authority comes from group teacher
assignment, exact permission, and the two phase controls.

### Group-owned score items

The clean future boundary is `learning_group_score_items`, keyed to:

- one learning group;
- one canonical assessment phase under the group's course assessment plan;
- free-form item name;
- exact maximum score;
- display order; and
- row version and normal creator/updater timestamps.

Examples such as `ชีท 1`, `ชิ้นงาน`, `กิจกรรมเพิ่มเติม`, `ปรนัย`, and `อัตนัย` are ordinary item
names. No special bonus or exam-section type is required for the first release.

Items may be absent when the shared plan becomes ready. During future score entry, a teacher may
temporarily have item maxima that do not yet equal the phase maximum. Before submitting/finalizing
that learning group's phase scores, active item maxima must equal the shared phase maximum.

Multiple exam sections remain score-entry columns only. A 60-minute in-timetable final containing
`ปรนัย 12` and `อัตนัย 8` produces one 60-minute exam-schedule item, not two sessions.

The current release establishes and migrates this ownership boundary but does not build student
score rows or the complete Gradebook UI.

### Score-item deletion contract

The future Gradebook uses a guarded hard-delete operation:

- an item without student scores can be deleted after ordinary confirmation;
- an item with scores first returns an impact count;
- the UI names the item and number of affected student-score rows;
- explicit confirmation deletes the item and all its student-score rows in one transaction; and
- no archive row, restore workflow, or domain score-item history is retained.

The delete service must require an unchanged item row version and unchanged impact count so a
concurrent score entry cannot be deleted under a stale confirmation. This behavior is documented
now but implemented with the future student-score boundary.

## Assessment Workspace Design

### Visual direction

The workspace is a compact academic ledger rather than a dashboard of oversized generic cards. Its
signature element is a four-phase score rail repeated consistently in the overview and editor. The
rail uses structure, labels, and status text rather than color alone.

The page remains aligned with the existing SchoolOrbit typography, spacing, shadcn-svelte controls,
permission states, and academic-context top bar.

### Term control strip

The top of the page shows a compact term summary:

- number of offered subjects;
- number with a persisted coordinator;
- number whose phase total matches policy; and
- number ready for central midterm/final import.

Below it, academic managers see the four-row phase-control matrix:

| Phase | Score-item editing | Student-score entry |
|---|---|---|
| ก่อนกลางภาค | switch | switch |
| กลางภาค | switch | switch |
| หลังกลางภาค | switch | switch |
| ปลายภาค | switch | switch |

Non-managers see the same state read-only so the reason a teacher action is unavailable remains
visible.

### Course overview table

The primary overview is a searchable table rather than a vertical list of large buttons. Columns
are:

- subject code and name;
- subject-version label and learning-group count;
- assessment coordinator;
- before-midterm maximum;
- midterm maximum and exam badge;
- after-midterm maximum;
- final maximum and exam badge;
- total versus expected total; and
- derived readiness status.

Filters cover missing coordinator, incomplete total, missing in-timetable duration, readiness, and
exam arrangement. The table remains read-first and loads through one set-based plan-summary request
for the selected academic term.

On narrow screens the four phase columns become one four-cell phase rail inside each row; the
subject identity and readiness remain visible without horizontal control overflow.

### Focused plan editor

Selecting one row opens a focused editor in the existing split workspace on wide screens and a
stacked detail region on small screens. The header contains:

- offered subject identity;
- derived readiness;
- coordinator selector or read-only coordinator;
- coordinator suggestion and attention message when relevant; and
- persistent auto-save status: `กำลังบันทึก`, `บันทึกแล้ว`, or `บันทึกไม่สำเร็จ`.

The body always renders the four phases in canonical order. Users cannot add, remove, rename, or
reorder them. Each phase shows maximum score and allocation status. Midterm and final additionally
show the three-value exam-arrangement selector. Duration appears and becomes required only for
`in_timetable`.

A sticky or persistent total line shows `current / expected` and identifies the exact remaining or
excess amount. Readiness messages are local to the affected phase instead of one large undirected
error list. There is no save or submit button.

Text/numeric changes save after a short debounce once the current field parses, and flush on blur.
Select and switch changes save immediately. Saving sends the complete four-phase snapshot with the
current row version so one partially typed field never becomes an authoritative request. A failed
save remains visibly unsaved and offers retry; it is never presented as `บันทึกแล้ว`.

There is no score-item editor on this page. A future Gradebook link may show group score-item
readiness without making this course-level workspace own group data.

### Dirty state and concurrency

The current academic-context dirty-source guard remains only while an auto-save is pending or has
failed. Switching term, year, or offered subject waits for a pending flush or asks the user to stay
and retry after a failed save; there is no manual save/discard workflow in the normal path.

Plan and phase-control mutations use row versions. A stale update returns `409 Conflict`, preserves
the user's local values, and asks them to reload the authoritative version. Coordinator eligibility
and exam metadata are revalidated in the same transaction as each auto-save.

## Workflow

1. Delivery publishes a course offering, its learning groups, homeroom coverage, and group teachers.
2. The assessment workspace lists one shared plan boundary for that offering.
3. If all groups share one primary teacher and no coordinator exists, the UI preselects that teacher.
4. An academic manager may accept or replace the suggested coordinator.
5. The coordinator or academic manager sets the four maxima and midterm/final exam arrangement.
6. Every valid edit auto-saves; score items are not required.
7. Readiness becomes true automatically when the coordinator, total, and required exam metadata are
   complete.
8. The central exam scheduler imports ready midterm/final phases marked `in_timetable`.
9. Academic managers independently open score-item editing and student-score entry per phase.
10. The future Gradebook lets each group teacher define ordinary score items for their own group.
11. Before a group's phase scores are finalized, its active item maxima must equal the shared phase
    maximum.

## Exam-Schedule Integration

The central exam source remains phase-level. Import eligibility is:

```text
plan readiness = ready
phase.code = exam_round.exam_kind
phase.exam_arrangement = in_timetable
phase.duration_minutes is present and positive
learning group and active homeroom coverage exist in the same academic context
```

Only `midterm` and `final` exam rounds exist in this contract. `outside_timetable` phases never
appear in the central scheduler and do not require a duration for readiness. Score items are never
joined by the exam-schedule import.

Existing imported exam schedule items preserve their assessment-phase IDs through the migration.
Changing a plan after an import does not silently delete schedule rows; the existing explicit
mismatch/readiness workflow remains responsible for review.

### Source changes after import

An imported exam schedule item is a snapshot. It retains the duration and source identities used
when it was imported. A later assessment auto-save never deletes the item, changes its duration, or
moves its session automatically.

The exam workspace compares each imported snapshot with the current fixed phase and reports four
source states:

- `unchanged`: the imported snapshot still matches a ready in-timetable source;
- `newly_eligible`: a current ready source has no imported item in this round;
- `changed`: the source remains eligible but scheduling data such as duration differs; and
- `no_longer_eligible`: the source is incomplete, outside timetable, no-exam, or otherwise no
  longer importable.

Changing only phase points does not make an exam item stale when exam arrangement and duration are
unchanged.

A draft round provides `ตรวจสอบการเปลี่ยนแปลง` before mutation. The preview names new, changed, and
no-longer-eligible items and calculates the effect on existing sessions:

- a new eligible source can be imported explicitly;
- an unplaced item with a changed duration can be synchronized explicitly;
- a placed item with a changed duration is revalidated against the exam day, blocked windows,
  room/homeroom occupancy, and all existing session conflicts before update;
- a valid placed duration update changes the snapshot only after confirmation;
- a conflicting placed update remains unchanged and links the user to rearrange it; and
- a no-longer-eligible item remains in the draft until the user explicitly confirms removal or
  restores the assessment source.

The preview and apply operations use one authoritative server-side change set and row versions so
a source or schedule change between preview and confirmation returns `409 Conflict` rather than
applying a stale decision.

A published round is immutable with respect to assessment-source synchronization. It continues to
show the exact schedule already published to teachers, students, and parents. The management
workspace displays the old snapshot beside the current source and explains that the published
round did not change. Creating a separately controlled published-round revision is future
exam-schedule work; this release does not unpublish or mutate a published round merely because the
assessment source changed.

## API Contract

The Rust/OpenAPI contract exposes typed phase codes and exam arrangements. Frontend types continue
to come only from the generated school API.

Required assessment endpoints support:

- set-based plan summaries by exact `academicTermId`;
- one plan detail with four fixed phases, coordinator, candidates, and suggestion;
- auto-save the complete plan snapshot with row version, coordinator ID, and four phase values;
- return derived readiness and exact readiness findings from list/detail/save;
- read all four phase controls for the selected term; and
- update one phase control with row version.

Required exam-schedule endpoints additionally support:

- preview source changes for one draft or published round without mutation; and
- apply an unchanged preview to a draft round only, with per-item conflict results and explicit
  removal choices.

The old free-form category request, category add/delete behavior, `custom` code, `practical` mode,
and global teacher-access settings endpoint are removed from the generated contract. No snake-case
query aliases or legacy payload fields remain.

## Authorization

- School assessment managers can read all plans, appoint or replace coordinators, edit any plan,
  and edit term phase controls.
- The persisted assessment coordinator can read and edit only the assigned offering plan.
- Group teachers can read the shared plan for their assigned groups.
- Future group score-item mutations require an active teacher assignment to the exact learning
  group, the exact score-item permission, and `item_editing_enabled` for the phase.
- Future student-score mutations require an active teacher assignment to the exact learning group,
  the exact score-entry permission, and `score_entry_enabled` for the phase.
- Exam-schedule permissions remain separate from assessment-plan permissions.

All authorization is enforced before data load and mutation on the backend. Frontend visibility is
not treated as enforcement.

## Forward Migration and Cutover

A new migration is added; no applied migration is edited.

The migration performs a clean cutover:

1. preflight every tenant for canonical assessment data that can be represented safely;
2. rename/rebuild the course category boundary as fixed course assessment phases while preserving
   phase IDs referenced by exam schedule items;
3. make phase code non-null and unique per plan;
4. remove editable name/order, `custom`, and `practical` phase behavior;
5. rename exam-schedule foreign-key semantics from assessment category to assessment phase while
   preserving IDs and rows;
6. remove submit/status/lock workflow columns and add the nullable assessment coordinator to course
   assessment plans;
7. infer existing coordinators only where every active learning group has the same valid primary
   teacher;
8. create exactly four term phase-control rows with conservative disabled defaults;
9. create the group-owned score-item boundary;
10. for any tenant with existing legacy course-level score items, copy each item deterministically
    to every active learning group under that offering, preserving name, score, order, and source
    provenance; and
11. remove the legacy course-level item boundary after count and identity reconciliation.

The migration does not silently delete a custom/missing/duplicate phase or guess how to normalize
one. Preflight stops the tenant migration with a deterministic safe error if its current plan cannot
be represented as exactly four canonical phases. Operations must inspect and resolve that tenant
before retrying; maintenance remains enabled on migration failure.

For `snwsb`, the observed 131 plans already satisfy the four-phase and total constraints, and there
are no legacy item rows to duplicate. Existing scores, exam arrangement, durations, phase IDs, and
exam schedule item links are preserved. Legacy plan status is intentionally not carried into the
runtime contract. The 13 missing final durations remain explicit readiness findings rather than
receiving guessed values.

## Error and Empty States

- No academic term selected: route the user to the top-bar academic context selector.
- No course offerings: show the existing prerequisite notice linking to Delivery.
- Missing coordinator: show an exact row/detail attention state and suggested common primary when
  available.
- Incomplete total: show remaining/excess points next to the total and affected phase rail.
- In-timetable exam without duration: mark the exact phase and keep readiness false.
- No common primary teacher: require manual coordinator selection; do not guess.
- Coordinator no longer teaches any covered group: require replacement before the next plan
  mutation.
- Stale row version: return conflict and reload after preserving unsaved local values.
- Auto-save failure: retain the pending local snapshot, display persistent failure state, and offer
  retry before context navigation.
- Exam import finds no eligible phases: explain coordinator, total, exam arrangement, duration, and
  group coverage separately.
- Draft exam source differs: retain every arranged item until an explicit, valid synchronization.
- Published exam source differs: keep the published snapshot unchanged and show the current source
  difference read-only.

## Verification

Verification follows `.rules` and runs serially to avoid the resource contention observed in this
workspace.

Required coverage includes:

- pure validation tests for four canonical phases, exact decimal totals, allowed exam arrangements,
  and in-timetable duration requirements;
- coordinator inference tests for zero groups, one group, a common primary, different primaries,
  missing primary, inactive assignments, and explicit override preservation;
- authorization tests for school manager, assigned coordinator, unrelated teacher, and exact group
  teacher boundaries;
- service/database tests for list/detail/auto-save/readiness and phase-control row-version conflicts;
- exam import tests proving only ready in-timetable matching phases are imported;
- exam source-preview tests for new, duration-changed, no-longer-eligible, unchanged, and
  points-only changes;
- draft synchronization tests for unplaced duration update, placed valid update, placed conflict,
  explicit removal, and stale preview conflict;
- published-round tests proving preview is read-only and source changes never mutate the published
  snapshot;
- migration schema tests proving phase and exam-schedule identities survive and legacy items are
  deterministically expanded per group;
- migration rejection tests for custom, missing, or duplicate phase codes;
- generated OpenAPI/schema and frontend-contract checks;
- Svelte autofixer analysis for every changed Svelte file;
- frontend typecheck and focused UI tests for table filtering, fixed phase order, coordinator
  suggestion, phase controls, validation messages, auto-save states, pending navigation, and
  responsive presentation; and
- live/read-only post-deploy inspection of safe counts and readiness state without student PII.

No claim of successful migration, deployment, or tenant readiness is made without the relevant
serial command output and post-deploy verification.

## Release Boundary

This release delivers:

- the clean fixed-phase assessment schema and API;
- assessment coordinator inference and selection;
- the readable assessment overview and fixed-phase editor;
- per-term, per-phase item-editing and score-entry controls;
- clean central exam-schedule sourcing;
- draft exam-source preview/synchronization with published snapshot protection; and
- the per-learning-group score-item foundation and data cutover.

The next Gradebook release delivers:

- student score rows;
- the group score-item manager inside the score-entry workspace;
- per-student entry, calculation, and group-phase finalization;
- guarded hard deletion with an exact score impact count; and
- consumption of the two phase controls already established here.
