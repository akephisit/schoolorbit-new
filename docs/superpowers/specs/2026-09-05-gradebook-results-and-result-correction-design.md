# Gradebook, Academic Results, and Result Correction Design

**Date:** 2026-09-05

**Status:** Approved in chat; awaiting written-spec review

**Scope:** Gradebook score entry, per-phase controls and confirmations, criterion- and
group-referenced grading, initial course and activity results, academic-affairs locking, result
correction, clean forward-only migration, generated permissions and API contracts, and responsive
staff workspaces

## Context

The assessment foundation already gives every offered subject one plan with exactly four canonical
phases: before midterm, midterm, after midterm, and final. Every learning group under the offering
shares the phase maxima, while `learning_group_score_items` allows each room to define different
worksheets, tasks, or exam sections. The foundation does not yet store student scores, confirm a
room's phase, calculate grades, lock initial results, or support formal corrections.

The current assessment phase-control row also contains both plan editing and future score-entry
controls. That makes Assessment appear to own a workflow that belongs to the Gradebook. Release 2
separates those responsibilities: Assessment owns the shared four-phase plan, Gradebook owns
classroom items and student scores, and Results owns grading, locking, and correction.

This release also completes activity evaluation. Guidance, scouts, clubs, and social/public-benefit
activities do not use numeric course grades; their result is pass or fail (`ผ/มผ`).

## Relationship to Existing Designs

- `2026-08-23-academic-core-lifecycle-redesign-design.md` remains authoritative for explicit
  academic context and the Release 2 -> term closure -> annual promotion sequence.
- `2026-08-28-curriculum-structure-and-homeroom-delivery-design.md` remains authoritative for the
  catalog -> curriculum -> offering -> learning-group chain.
- `2026-08-30-timetable-drag-drop-and-effective-teacher-assignment-design.md` remains authoritative
  for effective-dated teacher assignments and the `primary` group-teacher role.
- `2026-09-01-assessment-phase-and-gradebook-foundation-design.md` remains authoritative for fixed
  phase maxima, assessment coordinators, and group-owned score items except where this design
  explicitly supersedes it.

This design supersedes two foundation decisions:

1. `score_entry_enabled` moves out of the Assessment-owned control boundary into a Gradebook-owned
   boundary. `plan_editing_enabled` remains owned by Assessment.
2. A score item that already has student scores is cancelled rather than hard-deleted. Its scores
   remain stored but stop contributing to calculations. There is no restore workflow or separate
   score-item history ledger in this release.

## Goals

- Give assigned teachers a fast spreadsheet-style Gradebook for their learning groups.
- Save numeric scores automatically without conflating blank values with zero.
- Let each learning group define different score items while preserving shared phase maxima.
- Control score-item editing and score entry independently for each canonical phase of a term.
- Confirm scores per learning group and phase and invalidate only affected confirmations when
  source data changes.
- Let each offered subject use either the school's criterion-referenced policy or one
  group-referenced boundary set across all rooms of that subject in the term.
- Derive numeric course grades and allow the group primary teacher to select explicit `0`, `ร`, or
  `มส` outcomes per student.
- Record activity results as `ผ/มผ` and require a complete group before confirmation.
- Let academic affairs lock initial results and correct locked results from separate workspaces.
- Preserve the initial locked result and every subsequent correction without overwriting history.
- Establish an immutable result boundary that later releases can consume for term closure and
  promotion.
- Remove overlapping legacy runtime storage and APIs after verified migration; do not retain
  compatibility branches.

## Non-Goals

- Do not close an academic term, calculate cumulative GPA/GPAX, create transcripts, or promote
  students in this release.
- Do not schedule exams or change the existing exam-round snapshot workflow.
- Do not add absent, missing-work, exempt, or not-applicable score-cell states.
- Do not infer `ร` or `มส` automatically from blank score cells, attendance, or exam records.
- Do not let teachers manually select numeric grades `1` through `4`; they are derived from the
  confirmed score and grading policy.
- Do not add bonus-score semantics or permit scores beyond an item's maximum.
- Do not add fixed grade quotas for group-referenced grading.
- Do not add a result-correction remark field or a full score-edit audit ledger.
- Do not move or duplicate Assessment plan configuration into the Gradebook.
- Do not keep dual schemas, fallback legacy endpoints, or compatibility request fields after the
  cutover.

## Considered Approaches

### 1. Separate Gradebook and Results from Assessment

Assessment owns shared phase policy, Gradebook owns classroom evidence and scores, and Results owns
grading and official outcomes. These modules exchange typed IDs and snapshots. This creates clear
authorization, API, and data boundaries and is the selected approach.

### 2. Extend the Assessment module in place

This initially requires fewer routes, but combines plan preparation, daily score entry, result
approval, and academic-affairs correction in one module. Authorization and UI state would become
increasingly tangled. This approach is rejected.

### 3. Store every score and result change in an event-sourced ledger

This gives the strongest reconstruction capability, but introduces event replay, projections, and
more difficult operational recovery than this workflow needs. Immutable initial locks plus append-
only result corrections provide the required traceability without event-sourcing complexity. This
approach is rejected.

## Selected Architecture

The workflow is a one-way dependency chain:

```text
Assessment plan
(four shared phase maxima)
        |
        v
Gradebook
(group items, student scores, phase confirmations)
        |
        v
Result preparation
(criterion/group grading, 0/R/MS, activity pass/fail)
        |
        v
Academic-affairs initial lock
        |
        v
Academic-affairs result correction
        |
        v
Future term closure and promotion
```

Later stages read stable outputs from the previous stage. They do not mutate the preceding
domain. Once an initial result is locked, Gradebook and result-preparation data become read-only for
that locked scope. A formal correction changes only the effective official result.

## Academic Identity and Scope

Every request carries explicit `academicYearId` and `academicTermId`. Backend services validate that
the term belongs to the year and that every referenced offering, plan, group, student enrollment,
and teacher assignment belongs to the same context.

The stable course-result scope is `subject_id + academic_term_id`, reached through the selected
learning offering and subject version. Displayed subject codes are labels and are never used as
free-form join keys. All active course learning groups for that subject in the term contribute to
one grading method and one initial lock.

Activity results remain scoped to one activity learning group because clubs and other activity
groups may have different teachers and participants. Academic affairs receives bulk actions but
each activity group has an independent confirmation and lock.

## Gradebook Workflow

### Entry control ownership

Each academic term has one Gradebook entry-control row for each of the four canonical phase codes.
`score_entry_enabled` governs both score-item changes and numeric student-score changes by assigned
teachers. It does not govern Assessment plan changes.

The Assessment control retains only `plan_editing_enabled`. Changing either switch does not
invalidate saved data or an existing confirmation by itself. A school-management permission can
bypass a closed teacher window; the backend enforces that bypass independently of the UI.

### Score items

An active score item belongs to exactly one learning group and one assessment phase. It has a
non-empty name, non-negative exact maximum, display order, and row version. Names such as `ชีท 1`,
`คะแนนพิเศษ`, `ปรนัย`, or `อัตนัย` remain ordinary teacher-defined items with no special arithmetic.

Before a group phase can be confirmed, active item maxima must equal the shared phase maximum
exactly. Items may be temporarily incomplete while a teacher is arranging columns.

- An item without student scores can be deleted.
- An item with at least one student score can only be cancelled.
- Cancelling retains the item and its stored scores, excludes it from active-column totals and
  grade calculation, and invalidates that group-phase confirmation.
- A cancelled item has no restore workflow in Release 2. A teacher creates a new active item when
  a replacement is required.

### Student score semantics

Only two cell states exist:

- a numeric value from zero through the item's maximum; or
- no stored value, displayed as a blank cell meaning “not entered.”

Clearing a numeric cell deletes the current score value and returns it to blank. It never writes
zero. A teacher must explicitly type `0` to record zero.

Scores use exact decimal storage. The backend validates range and item status and derives phase and
course totals; it never trusts totals submitted by the browser. A multi-cell paste is normalized,
validated, and persisted as one bounded batch operation.

### Autosave and concurrency

An edited cell saves after 750 milliseconds of idle time and also on blur,
Enter, Tab, sheet close, or navigation. The page keeps a sticky status: unsaved changes, saving,
saved, or failed with retry. Closing or navigating waits for pending requests or presents a clear
retry/discard choice; it never silently loses an unresolved edit.

Items, score values, controls, and confirmations use optimistic row versions. If another teacher
has changed a row, the server rejects the stale mutation. The UI retains the local value, explains
the conflict, and offers a refresh before another save. It does not overwrite newer data
automatically.

### Phase confirmation

The group primary teacher confirms one `learning_group + phase_code` at a time. Confirmation
requires:

- an unchanged Assessment phase maximum;
- active item maxima exactly equal to that maximum;
- no invalid or pending score mutation; and
- an unchanged group roster revision.

Blank score cells do not block confirmation. The confirmation dialog reports their count and
states that each blank contributes zero to result calculation while remaining blank in storage.

Confirmation stores the source revisions/checksum used for the decision. Creating, resizing, or
cancelling an item, changing/clearing a score, changing the shared phase maximum, or changing the
roster invalidates only affected group-phase confirmations. Renaming or reordering an item does not
change a calculation and therefore does not invalidate confirmation. Opening or closing the global
entry window does not invalidate it.

## Course Grading

### School criterion policy

The school owns versioned criterion policies and has exactly one active default version at a time.
A policy defines the inclusive lower boundary for
the eight standard numeric outcomes `0`, `1`, `1.5`, `2`, `2.5`, `3`, `3.5`, and `4`, against the
Assessment plan's total score. Boundaries must be ordered, non-overlapping, and within the total.

An activated version is immutable. Changing the school criterion creates and activates a new
version while retaining the prior version. Every unlocked subject using criterion grading follows
the active default version and recalculates its preview after activation. An initial lock embeds a
complete policy snapshot; later school-policy changes cannot change locked results.

### Subject-term grading setting

One subject-term setting selects either:

- `criterion`: use the school's current active default criterion-policy version; or
- `group_referenced`: use one confirmed boundary set calculated from all included students in all
  active rooms of the same `subject_id + academic_term_id`.

The assessment coordinator becomes the initial suggested subject coordinator. An academic manager
may explicitly choose another currently assigned teacher. The persisted choice is not silently
replaced when teacher assignments later change.

### Group-referenced grading

The result workspace shows the combined cohort count, mean, median, standard deviation,
percentiles, score distribution, and the student count under each proposed grade. Its optional
starting suggestion uses lower boundaries of mean plus `1.5`, `1.0`, `0.5`, and `0.0` standard
deviations for grades `4`, `3.5`, `3`, and `2.5`, then mean minus `0.5`, `1.0`, and `1.5` standard
deviations for grades `2`, `1.5`, and `1`; grade `0` starts at zero. Suggested values are clamped to
the plan total and rounded to two decimals. Clamping or rounding can make a suggestion invalid, so
strictly increasing editable boundaries are required before confirmation. A suggestion is never
official until the subject coordinator reviews and confirms it.

The coordinator can adjust every numeric lower boundary. The preview shows affected counts and
student names before confirmation. The system does not impose grade quotas or force a fixed
percentage into each grade.

A cohort with fewer than 30 included students remains eligible for group-referenced grading. The UI
warns that its distribution may be unstable and requires explicit confirmation; it does not
silently switch to criterion grading.

Group boundaries may be confirmed only after every contributing learning group has confirmed all
four phases. The boundary snapshot records all source confirmation revisions. Any later score,
item, phase-plan, or roster change that invalidates a contributing confirmation also makes the
group-boundary snapshot stale and blocks initial result locking until it is reviewed again.

### Student course outcomes

After all four phase confirmations exist, the backend calculates a numeric total and derived grade
for each student. The group primary teacher sees four choices:

- `ตามคะแนน`: retain the derived numeric grade;
- `0`: explicitly set the initial numeric result to zero;
- `ร`: incomplete result; or
- `มส`: insufficient-attendance result.

The system does not infer `ร` or `มส` from blank scores. Teachers cannot manually choose numeric
grades `1` through `4`. An explicit `0` is stored with its manual source even when the calculated
grade would also be zero.

The primary teacher confirms the prepared results for one group. There is no additional “submit to
academic affairs” button: when every group under the subject is confirmed and the grading snapshot
is current, the subject automatically appears as ready in the academic-affairs queue.

The confirmation snapshots the four group-phase confirmation revisions, the criterion-policy or
group-boundary revision, and the current explicit student selections. A changed phase confirmation,
new active criterion policy, reconfirmed group boundary, or changed explicit selection makes only
the affected group-result confirmation stale. The primary teacher reviews and confirms it again;
the system never silently carries a confirmation onto different calculated outcomes.

## Activity Evaluation

Guidance, scouts, clubs, and social/public-benefit activity groups use no numeric score items and no
course grading policy. An assigned activity teacher records exactly one of `ผ` (`pass`) or `มผ`
(`fail`) per participating student.

Blank means “not evaluated” and never means fail. The group primary teacher cannot confirm until
every current participant has a result. Changing membership or any participant result invalidates
the group confirmation.

Academic affairs locks each activity group independently and has a “lock all ready groups”
bulk operation. The operation skips non-ready groups and returns typed, actionable reasons for each
skip. It does not weaken the per-group completeness rule.

## Initial Result Locking

Initial course locking belongs only to academic affairs. One lock transaction covers every active
learning group under `subject_id + academic_term_id`. The service revalidates authorization and all
source revisions inside the transaction. It requires:

- a ready four-phase Assessment plan;
- every group-phase confirmation;
- a current criterion-policy or confirmed group-boundary snapshot;
- every group-primary result confirmation; and
- no pending, invalid, or stale source revision.

If any room fails validation, no room is locked. The response names the exact room, phase, and
reason. A successful lock writes immutable initial course-result rows plus a lock snapshot of the
grading method, boundaries, source confirmation revisions, calculated totals, explicit outcome
selections, actor, and timestamp.

Locked result scopes make Assessment plan fields, relevant Gradebook items and scores, grading
settings, and teacher-prepared outcomes read-only through their normal APIs. Frontend disabling is
only explanatory; backend services are authoritative.

## Result Correction

Locked results are corrected only in a separate academic-affairs workspace. Teachers cannot reopen
or overwrite them through Assessment, Gradebook, or Results.

For a course result, academic affairs may select any valid numeric grade from `0` through `4` in
half-grade increments, `ร`, or `มส`. For an activity result, it may select `ผ` or `มผ`. A correction
does not change score rows, calculated totals, or the initial result snapshot.

Every correction appends one row containing the previous effective result, new result, actor,
timestamp, and optimistic source version. No free-text remark is required. The effective official
result is the most recent correction, or the immutable initial result when no correction exists.
Concurrent corrections against a stale effective version are rejected.

## Data Model

The following physical tables own the Release 2 data. The implementation plan defines their exact
columns and constraints without renaming these ownership boundaries.

### Gradebook-owned storage

- `academic_gradebook_phase_controls`: one term/phase row with `score_entry_enabled`, row version,
  and update metadata.
- `learning_group_score_items`: the existing group/phase item table, extended with an active or
  cancelled lifecycle and cancellation metadata.
- `learning_group_student_scores`: one numeric value per active or retained
  `score_item + student_academic_year`, with exact value and row version. Absence is blank.
- `learning_group_phase_confirmations`: one current confirmation per group/phase with source
  revisions/checksum, confirmer, and timestamp.

### Grading-owned storage

- `academic_grading_policy_versions`: named school criterion-policy versions and lifecycle.
- `academic_grading_policy_bands`: ordered numeric grade lower boundaries per policy version.
- `subject_term_grading_settings`: one grading method and responsible teacher per
  subject/term.
- `subject_term_group_boundaries`: the coordinator-confirmed group-referenced boundary snapshot,
  statistics, cohort count, and source revisions.
- `learning_group_result_overrides`: the current explicit `0`, `ร`, or `มส` selection per student;
  absence means `ตามคะแนน`.
- `learning_group_result_confirmations`: primary-teacher confirmation of prepared course outcomes
  for one group.

### Official-result storage

- `academic_course_result_locks`: immutable subject-term initial-lock header and complete source
  snapshot.
- `academic_course_results`: immutable initial course outcome per included student, including the
  calculated score/grade and explicit selection source.
- `academic_activity_evaluations`: mutable pre-lock `pass/fail` value per activity participant.
- `academic_activity_result_confirmations`: current teacher confirmation per activity group.
- `academic_activity_result_locks`: immutable initial activity-group lock.
- `academic_activity_results`: immutable initial `pass/fail` result per participant.
- `academic_result_corrections`: append-only correction rows pointing to exactly one course or
  activity result.

Contextual composite foreign keys enforce matching year, term, offering, group, subject, and
student academic-year identity. Uniqueness prevents more than one initial result for the same
locked scope and student. Partial/check constraints enforce mutually exclusive course/activity
correction targets and valid outcome families.

The legacy `learning_results` and `activity_result_details` tables are migrated into the new
activity-result boundary and then removed. Runtime code, destructive-change impact summaries, and
tests move to the new tables in the same cutover; no dual writes or compatibility views remain.

## Authorization

New permissions follow generated `module.action.scope` contracts and existing resource scopes:

- `academic_gradebook.read.assigned`, `academic_gradebook.read.organization_unit`, and
  `academic_gradebook.read.school` cover Gradebook discovery and authorized reads;
- `academic_gradebook.manage.assigned` covers assigned-teacher item and score mutations;
- `academic_gradebook.manage.school` covers school-wide management, entry controls, and the
  explicit closed-window override;
- `academic_result.read.assigned`, `academic_result.read.organization_unit`, and
  `academic_result.read.school` cover result preparation and official-result reads;
- `academic_result.manage.assigned` covers primary-teacher confirmation and the persisted subject
  coordinator's grading work;
- `academic_result.manage.school` covers school policy versions and school-wide result
  preparation;
- `academic_result.lock.school` covers immutable initial course/activity locking; and
- `academic_result.correct.school` covers append-only post-lock corrections.

The resource capability boundary is:

- assigned teachers can read and edit Gradebook data only for currently authorized learning
  groups and open phases;
- a group's active primary teacher can confirm its phase scores and prepared results;
- the persisted subject coordinator can read every contributing group and manage the subject-term
  grading setting/boundaries;
- assigned activity teachers can enter their group results, while its primary teacher confirms;
- school-level academic-result managers can bypass teacher windows, view all scopes, lock initial
  results, and append corrections; and
- ordinary readers may inspect authorized locked results but cannot load mutation-only data.

List policies union independent assigned, organization-unit/tree, and school scopes. School scope
is the only all-record short circuit. Backend resource policies resolve group assignment,
coordinator ownership, and school-management override; route guards and hidden controls are only
UX.

No Gradebook, result, export, error, log, audit metadata, or realtime signal exposes plaintext
national IDs or unnecessary student-sensitive fields.

## API and Module Boundaries

Backend feature code is split into focused services:

- Assessment exposes plan editing and `plan_editing_enabled` only.
- Gradebook exposes entry controls, group workspace reads, score-item mutations, validated batch
  score mutations, and group-phase confirmation.
- Grading exposes school policy versions, subject-term method selection, statistics/boundary
  preview, boundary confirmation, calculated results, and group-result confirmation.
- Official Results exposes academic-affairs readiness queues, initial locks, activity locks, and
  corrections.

Handlers remain limited to authenticated context, policy, typed service call, response envelope,
and a realtime invalidation signal where needed. SQL and calculation logic live in focused
services/pure helpers with tests.

Every endpoint uses camel-case typed Rust DTOs, `ApiResponse`, `utoipa` registration, generated
OpenAPI output, and generated frontend DTOs. Mutations return the updated resource/revision so the
client patches only affected state. There are no `unknown` response casts or manual query-name
aliases.

Realtime events are change signals, not result truth. A client receiving a relevant signal refetches
the authoritative affected workspace or row. Events do not include score values or student PII.

## Frontend Workspaces

### Assessment

`/staff/academic/assessments` continues to show subjects owned by the current account first. It
edits shared four-phase plans and shows only the manager's “allow course score-plan editing”
switches. It shows the Gradebook entry state read-only with a link, but does not mutate that state.

### Gradebook

`/staff/academic/gradebook` is a deep-linkable workspace with year/term context and subjects owned
by the current account first. The user chooses subject, learning group, and one of four phase tabs.
The manager-only control surface owns the four “allow student-score entry” switches.

The desktop entry surface is a horizontally scrollable academic ledger with frozen student
identity columns and readable fixed-width score columns. It supports keyboard entry and bounded
multi-cell paste. The mobile editor uses a full-screen sheet with a sticky header, explicit back/X
action, current student/item context, and sticky save status; it never depend on an off-screen close
button.

### Result preparation

`/staff/academic/results` shows subject readiness, criterion/group selection, combined-room
statistics, grading-boundary preview, per-room result confirmation, and activity evaluation. It
does not expose academic-affairs correction actions.

### Academic-affairs workspaces

Initial locking and result correction are separate deep-linkable routes and separate menu services.
The lock queue groups course results by subject and activity results by group, with readiness
reasons visible before mutation. The correction page searches locked results and shows the immutable
initial result next to the current effective result and correction history.

Standard controls use local shadcn-svelte primitives and shared `PageShell`, loading, empty, error,
and permission-aware states. Read-only users never trigger action-only requests.

## Validation and Error Handling

Business errors identify the object and remediation, for example:

- “งานย่อยรวม 18 จาก 20 คะแนน”;
- “ม.1/2 ยังไม่ยืนยันช่วงปลายภาค”;
- “มีคะแนน 14 ช่องที่ยังว่างและจะคิดเป็นศูนย์”;
- “เกณฑ์อิงกลุ่มหมดอายุเพราะคะแนน ม.2/3 เปลี่ยนแปลง”; or
- “ผลกิจกรรมยังขาดนักเรียน 3 คน.”

Expected validation, conflict, and permission failures return the standard JSON error envelope
without internal SQL, stack, or sensitive identity data. An autosave failure remains visible until
resolved. Bulk operations report per-scope outcomes but remain transactionally atomic wherever the
design promises all-or-nothing behavior.

## Forward-Only Cutover

The cutover uses new sequential tenant migrations; no applied migration is edited.

1. Preflight validates context integrity, canonical phase controls, score-item references, and all
   legacy activity-result outcome values.
2. Create the new Gradebook, grading, lock, and correction tables and constraints.
3. Copy each term/phase `score_entry_enabled` value into
   `academic_gradebook_phase_controls`, then remove that column from the Assessment control.
4. Add the score-item cancellation lifecycle without changing existing active item identity.
5. Convert recognized legacy `pass/fail` activity results into pre-lock
   `academic_activity_evaluations`. They remain populated but unconfirmed so the responsible teacher
   reviews the migrated values before academic affairs creates an initial lock. Verify source/target
   counts and identities.
6. Update every runtime reference, impact-count query, typed API, permission, and test to the new
   ownership boundary.
7. Drop legacy `learning_results` and `activity_result_details` after verification in the same
   reviewed cutover; do not retain compatibility views or fallback reads.

Unknown legacy result values or count/context mismatches abort migration with an actionable error.
Deployment maintenance remains enabled until tenant migration, readiness, and contract verification
succeed. Recovery uses the pre-deployment Neon snapshot and the documented deployment rollback
procedure rather than an improvised reverse migration.

## Delivery Sequence Within Release 2

Release 2 remains one coherent cutover but is implemented in reviewable commits:

1. forward schema, data preflight, permissions, and generated contracts;
2. Gradebook services, autosave, item lifecycle, and phase confirmation;
3. criterion/group grading, course outcomes, and activity evaluation;
4. academic-affairs initial locks and append-only corrections;
5. responsive workspaces, route/menu integration, legacy removal, and full verification.

New user-facing menu entries remain undiscoverable until their complete backend authorization and
workflow are ready. This is rollout sequencing, not a retained compatibility path.

## Verification Strategy

Focused service, policy, contract, schema, and UI tests cover:

- blank versus explicit zero, including clearing a previously stored score;
- minimum/maximum boundaries and decimal values;
- item totals that underfill or overfill a phase;
- hard deletion without scores and cancellation with retained scores;
- autosave batching, retry, and optimistic conflicts between assigned teachers;
- teacher-window closure and school-management bypass;
- confirmation with blanks, targeted invalidation, roster/plan invalidation, and no invalidation
  from a control toggle alone;
- criterion-policy version changes before lock and snapshot stability after lock;
- group grading across multiple rooms, small-cohort warnings, adjusted boundaries, and stale
  boundary detection;
- primary-teacher, coordinator, assigned, organization, and school scope allow/deny/union cases;
- all-or-nothing subject locking and per-group activity locking;
- activity completeness and bulk “lock all ready” skip reasons;
- append-only course/activity corrections and stale-correction rejection;
- migration source/target counts, unknown-value failure, and removal of legacy runtime references;
- generated permission/API contract drift; and
- responsive Gradebook close/save behavior and read-only request discipline.

The implementation runs focused checks throughout and every applicable command in `.rules`,
including permission generation/check/tests, API contract generation/check/tests, backend format,
architecture tests and checks, frontend lint/check/static tests, `git diff --check`, and final status
review. Browser workflow coverage uses disposable test accounts and environment-provided
credentials only.

## Acceptance Criteria

- Assessment no longer mutates the score-entry control.
- Assigned teachers can enter only authorized group scores while their phase is open; school-level
  managers can perform an explicit authorized override.
- Blank and zero remain distinguishable through save, reload, confirmation, calculation, and lock.
- Each room may use different active score items while every confirmed room matches shared phase
  maxima.
- Confirmation and group-boundary invalidation are narrow, deterministic, and visible.
- Criterion grading uses a versioned school policy; group grading uses one confirmed combined-room
  boundary set without quotas.
- Teachers choose only `ตามคะแนน`, explicit `0`, `ร`, or `มส`; numeric grades above zero are
  derived.
- Course readiness is automatic after all room confirmations; no redundant submit button exists.
- Activity groups require complete `ผ/มผ` results and lock independently.
- Academic affairs locks initial results and performs all post-lock corrections from separate
  routes.
- Initial locked results are immutable, corrections append, and the effective result is
  deterministic.
- Legacy overlapping result storage and APIs are removed after verified forward migration, with no
  compatibility runtime.
- The locked result boundary is ready for later term-closure and promotion releases without those
  workflows being implemented here.
