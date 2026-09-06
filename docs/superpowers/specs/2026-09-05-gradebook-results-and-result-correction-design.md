# Gradebook, Academic Results, and Result Correction Design

**Date:** 2026-09-05

**Status:** Approved in chat; awaiting written-spec review

**Scope:** Gradebook score entry, per-phase controls and confirmations, school-wide criterion
grading, course-linked desirable-characteristic and reading/thinking/writing evaluation, initial
course and activity results, academic-affairs locking, result correction, clean forward-only
migration, generated permissions and API contracts, and responsive staff workspaces

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

This release also completes the other result inputs needed by later term closure. Guidance, scouts,
clubs, and social/public-benefit activities use pass or fail (`ผ/มผ`). Every subject teacher also
evaluates desirable characteristics and reading, analytical thinking, and writing for the students
in each taught learning group using four quality levels.

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
- Use one active, versioned school criterion policy for every unlocked subject, without a
  per-subject grading-method override.
- Derive numeric course grades and allow the group primary teacher to select explicit `0`, `ร`, or
  `มส` outcomes per student.
- Let every subject evaluate every active desirable-characteristic and
  reading/thinking/writing criterion by default while allowing subject-specific additions and
  deactivations.
- Confirm and lock the two learner-evaluation domains independently by group and subject.
- Derive each student's term-level learner-evaluation summary from effective locked subject
  evaluations without requiring a second summary lock.
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
- Do not add group-referenced grading, score-distribution grading, percentile cutoffs, standard-
  deviation cutoffs, or fixed grade quotas.
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
(criterion grading, 0/R/MS, learner evaluation, activity pass/fail)
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
one initial course-result lock and use the same active school criterion policy.

Desirable-characteristic and reading/thinking/writing input is stored by subject, learning group,
student, and criterion. Each learner-evaluation domain is confirmed per group and locked per
`subject_id + academic_term_id + domain`. Term-level summaries are derived per student from the
effective locked subject evaluations.

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
- Renaming, reordering, or changing an item's maximum is allowed while the phase is editable and
  the result scope is not locked. A lower maximum is rejected when any retained student score
  exceeds it; the teacher must reduce or clear those scores first.

### Student score semantics

Only two cell states exist:

- a numeric value from zero through the item's maximum; or
- no stored value, displayed as a blank cell meaning “not entered.”

Clearing a numeric cell deletes the current score value and returns it to blank. It never writes
zero. A teacher must explicitly type `0` to record zero.

Scores use exact decimal storage. The backend validates range and item status and derives phase and
course totals; it never trusts totals submitted by the browser. A multi-cell paste is normalized,
validated, and persisted as one bounded batch operation.

### Entry column selection

Every active score-item header has a checkbox that selects whether its student cells are editable
in the current browser workspace. No item is selected on initial entry. The page also provides
“select all in this phase” and “clear selection.” Adding a new item selects its new column so the
teacher can use it immediately.

Unchecked columns remain visible and read-only. Their existing scores still contribute to phase
and course totals. The selection is browser UI state only: it is not sent to the API, does not
activate/deactivate an item, does not alter calculation, and does not affect another teacher's
workspace.

Keyboard Tab/Enter movement and multi-cell paste traverse only selected columns. Clearing a
selection first resolves any pending autosave in that column, then makes its cells read-only; it
does not clear their values.

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
A policy defines the inclusive lower boundary for the eight standard numeric outcomes `0`, `1`,
`1.5`, `2`, `2.5`, `3`, `3.5`, and `4`, against the Assessment plan's total score. Boundaries must
be ordered, non-overlapping, and within the total.

An activated version is immutable. Changing the school criterion creates and activates a new
version while retaining the prior version. Every unlocked subject using criterion grading follows
the active default version and recalculates its preview after activation. An initial lock embeds a
complete policy snapshot; later school-policy changes cannot change locked results.

Every subject uses the active default school policy. There is no subject-term grading setting,
method selector, group boundary, percentile calculation, distribution-based recommendation, or
per-subject criterion override. The assessment coordinator remains responsible for the shared
four-phase Assessment plan and the subject's learner-evaluation criteria, but does not choose a
grading method.

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

The confirmation snapshots the four group-phase confirmation revisions, active criterion-policy
revision, and current explicit student selections. A changed phase confirmation, newly activated
criterion policy, or changed explicit selection makes only the affected group-result confirmation
stale. The primary teacher reviews and confirms it again; the system never silently carries a
confirmation onto different calculated outcomes.

## Course-Linked Learner Evaluation

### Domains and quality scale

Every course learning group evaluates two independent domains:

- `desirable_characteristic`: คุณลักษณะอันพึงประสงค์; and
- `reading_thinking_writing`: การอ่าน คิดวิเคราะห์ และเขียน.

Both domains use the system-owned values `3` (ดีเยี่ยม), `2` (ดี), `1` (ผ่าน), and `0`
(ไม่ผ่าน). No stored value means “not evaluated” and is never interpreted as zero.

### School criteria and subject-term configuration

The initial school catalog contains the eight standard desirable characteristics: รักชาติ ศาสน์
กษัตริย์; ซื่อสัตย์สุจริต; มีวินัย; ใฝ่เรียนรู้; อยู่อย่างพอเพียง; มุ่งมั่นในการทำงาน;
รักความเป็นไทย; and มีจิตสาธารณะ. The initial reading/thinking/writing catalog contains the three
high-level criteria การอ่าน, การคิดวิเคราะห์, and การเขียน. School managers can add detailed
indicators, reorder, rename, or deactivate catalog criteria. A criterion already referenced by a
subject is deactivated rather than deleted.

The first configuration for `subject_id + academic_term_id + domain` copies every currently active,
applicable school criterion. The assessment coordinator manages that subject-term configuration and
can add a subject-only criterion, reorder criteria, or deactivate a criterion for the subject. The
same configuration applies to every course learning group under the subject.

Later school-catalog changes affect only configurations created afterward. They never silently
change a subject already operating in the term. Within an unlocked subject-domain, a criterion
without responses can be removed; one with responses can only be deactivated, retaining its values
but excluding it from current calculation. A locked subject-domain configuration is immutable.

### Independent entry controls

Each term has two learner-evaluation entry controls, one per domain. Assigned teachers can edit
responses only while that domain is open. School-level managers can change either control and can
perform an explicitly authorized override while it is closed. Opening or closing a control does not
invalidate data or confirmation.

### Entry and group confirmation

Every assigned subject teacher can enter `0` through `3` for students in an authorized learning
group. The group primary teacher confirms `learning_group + domain`. Confirmation requires a value
for every current student and every active subject criterion; blank responses block confirmation
and the error identifies each missing student/criterion.

Confirmation snapshots the subject configuration, roster, and response revisions. A response,
roster, or active-criterion change invalidates only that group-domain confirmation. Renaming or
reordering a criterion does not affect calculation and does not invalidate it.

### Subject-domain lock

Academic affairs locks `subject_id + academic_term_id + domain` after every active course learning
group has a current primary-teacher confirmation. Desirable-characteristic and
reading/thinking/writing locks are independent; one can lock while the other remains editable. The
lock is all-or-nothing across the subject's rooms and writes immutable initial responses plus the
configuration and source-confirmation snapshot.

### Derived term summary

For each locked subject-domain, the system first averages the student's active criterion values so
each subject contributes one equally weighted domain value regardless of how many standard or
subject-only criteria it contains. The student's term-domain average then equally averages those
values across every locked course subject in which the student is enrolled.

The summary also shows each school-catalog criterion averaged across locked subjects that include
it. Subject-only criteria remain visible in the source drill-down and contribute to their subject's
domain value, but do not create misleading same-name cross-subject summaries.

A versioned school aggregation policy converts the exact average to a quality level. The initial
policy uses inclusive lower bounds `2.50` for level `3`, `1.50` for level `2`, `1.00` for level `1`,
and `0.00` for level `0`. School managers change the thresholds by activating a new immutable
version.

The summary is provisional and names missing subjects until every expected subject-domain is
locked. Once all expected locks exist, the summary is complete and deterministic. It is not locked
a second time in Release 2. A post-lock subject response correction recalculates the summary. The
future term-closure release stores the final summary and aggregation-policy snapshot.

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
- the current active school criterion-policy version;
- every group-primary result confirmation; and
- no pending, invalid, or stale source revision.

If any room fails validation, no room is locked. The response names the exact room, phase, and
reason. A successful lock writes immutable initial course-result rows plus a lock snapshot of the
criterion-policy boundaries, source confirmation revisions, calculated totals, explicit outcome
selections, actor, and timestamp.

Locked course-result scopes make Assessment plan fields, relevant Gradebook items and scores, and
teacher-prepared outcomes read-only through their normal APIs. Frontend disabling is only
explanatory; backend services are authoritative.

## Result Correction

Locked results are corrected only in a separate academic-affairs workspace. Teachers cannot reopen
or overwrite them through Assessment, Gradebook, or Results.

For a course result, academic affairs may select any valid numeric grade from `0` through `4` in
half-grade increments, `ร`, or `มส`. For an activity result, it may select `ผ` or `มผ`. For a
course-linked learner evaluation, it may change one student's one criterion from `0` through `3`
under a locked subject-domain. A correction does not change score rows, calculated totals, or any
initial result snapshot.

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
- `learning_group_result_overrides`: the current explicit `0`, `ร`, or `มส` selection per student;
  absence means `ตามคะแนน`.
- `learning_group_result_confirmations`: primary-teacher confirmation of prepared course outcomes
  for one group.

### Learner-evaluation-owned storage

- `academic_learner_evaluation_criteria`: the school criterion catalog with domain, applicability,
  lifecycle, and display metadata.
- `subject_term_evaluation_criteria`: the fixed current criterion configuration for one
  subject/term/domain, including school-linked and subject-only criteria.
- `academic_learner_evaluation_controls`: one term/domain entry-control row.
- `learning_group_student_evaluations`: one `0` through `3` response per
  group/student/subject-term criterion, with row version; absence is blank.
- `learning_group_evaluation_confirmations`: one current group/domain confirmation with source
  revisions/checksum.
- `subject_term_evaluation_locks`: immutable subject/term/domain lock and complete source snapshot.
- `subject_term_student_evaluations`: immutable initial response per locked subject criterion and
  student.
- `academic_learner_evaluation_policy_versions`: immutable school term-summary policy versions.
- `academic_learner_evaluation_policy_bands`: ordered inclusive lower boundaries from exact average
  to quality level.

### Official-result storage

- `academic_course_result_locks`: immutable subject-term initial-lock header and complete source
  snapshot.
- `academic_course_results`: immutable initial course outcome per included student, including the
  calculated score/grade and explicit selection source.
- `academic_activity_evaluations`: mutable pre-lock `pass/fail` value per activity participant.
- `academic_activity_result_confirmations`: current teacher confirmation per activity group.
- `academic_activity_result_locks`: immutable initial activity-group lock.
- `academic_activity_results`: immutable initial `pass/fail` result per participant.
- `academic_result_corrections`: append-only correction rows pointing to exactly one course,
  activity, or course-linked learner-evaluation result.

Contextual composite foreign keys enforce matching year, term, offering, group, subject, and
student academic-year identity. Uniqueness prevents more than one initial result for the same
locked scope, student, and criterion. Partial/check constraints enforce mutually exclusive course,
activity, and learner-evaluation correction targets and valid outcome families.

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
- `academic_result.manage.assigned` covers primary-teacher course-result confirmation and assigned
  activity evaluation;
- `academic_result.manage.school` covers school policy versions and school-wide result
  preparation;
- `academic_result.lock.school` covers immutable initial course/activity locking; and
- `academic_result.correct.school` covers append-only post-lock course/activity corrections;
- `academic_learner_evaluation.read.assigned`,
  `academic_learner_evaluation.read.organization_unit`, and
  `academic_learner_evaluation.read.school` cover authorized learner-evaluation reads;
- `academic_learner_evaluation.manage.assigned` covers assigned-teacher entry, primary-teacher
  confirmation, and assessment-coordinator subject-criteria management;
- `academic_learner_evaluation.manage.school` covers school catalog/policy/entry-control management
  and the explicit closed-window override;
- `academic_learner_evaluation.lock.school` covers immutable subject-domain locking; and
- `academic_learner_evaluation.correct.school` covers append-only post-lock criterion corrections.

The resource capability boundary is:

- assigned teachers can read and edit Gradebook data only for currently authorized learning
  groups and open phases;
- a group's active primary teacher can confirm its phase scores and prepared results;
- assigned subject teachers can enter each open learner-evaluation domain for their groups;
- the group primary teacher confirms each learner-evaluation domain independently;
- the persisted assessment coordinator manages the shared subject-term criteria and can inspect
  every contributing group without gaining school-wide access;
- assigned activity teachers can enter their group results, while its primary teacher confirms;
- school-level academic-result and learner-evaluation managers can bypass their respective teacher
  windows, view all scopes, lock initial results, and append corrections; and
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
- Grading exposes school criterion-policy versions, calculated course results, explicit outcome
  selections, and group-result confirmation.
- Learner Evaluation exposes the school criterion catalog, subject-term configurations, independent
  entry controls, validated batch responses, group confirmation, subject-domain locking, derived
  term summaries, and criterion correction.
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
by the current account first. The user chooses subject and learning group once, then switches among
`คะแนน`, `คุณลักษณะอันพึงประสงค์`, and `อ่าน คิดวิเคราะห์ และเขียน`. The score tab contains the
four canonical phase tabs. Manager-only controls own the four score-entry switches plus independent
desirable-characteristic and reading/thinking/writing entry switches.

The desktop entry surface is a horizontally scrollable academic ledger with frozen student
identity columns and readable fixed-width score columns. Each score-item header shows its entry
checkbox, item name, maximum, and compact item actions. Selected columns use a restrained active
state in addition to the checkbox; unchecked columns stay readable but cannot receive input. The
ledger supports keyboard entry and bounded multi-cell paste across selected columns only. The
mobile editor uses a full-screen sheet with a sticky header, explicit back/X action, current
student/item context, and sticky save status; it never depends on an off-screen close button.

Each learner-evaluation tab is a separate horizontally scrollable matrix with students as rows and
active subject criteria as columns. Every criterion header has the same local entry-column checkbox
semantics as a score item. Selected cells accept keyboard `0` through `3`, display the corresponding
Thai quality label, support Tab/Enter and bounded paste, and autosave after 750 milliseconds.
Unchecked columns remain visible and read-only. Blank cells remain visually distinct from level
zero.

The assessment coordinator receives a compact “ตั้งค่าหัวข้อ” action for adding, ordering, or
deactivating subject criteria; ordinary assigned teachers do not. Mobile evaluation uses the same
full-screen, sticky-close/save-status pattern as mobile score entry.

### Result preparation

`/staff/academic/results` shows criterion-policy state, subject readiness, calculated course
results, per-room result confirmation, activity evaluation, learner-evaluation subject readiness,
and the automatically derived term summary with missing-subject explanations. It has no grading-
method selector or score-distribution grading UI and does not expose academic-affairs correction
actions.

### Academic-affairs workspaces

Initial locking and result correction are separate deep-linkable routes and separate menu services.
The lock queue groups course results by subject, learner-evaluation results by subject and domain,
and activity results by group, with readiness reasons visible before mutation. The correction page
searches locked results and shows the immutable initial result next to the current effective result
and correction history.

Standard controls use local shadcn-svelte primitives and shared `PageShell`, loading, empty, error,
and permission-aware states. Read-only users never trigger action-only requests.

## Validation and Error Handling

Business errors identify the object and remediation, for example:

- “งานย่อยรวม 18 จาก 20 คะแนน”;
- “ม.1/2 ยังไม่ยืนยันช่วงปลายภาค”;
- “มีคะแนน 14 ช่องที่ยังว่างและจะคิดเป็นศูนย์”;
- “ค21101 · ม.1/2 ยังขาดผลคุณลักษณะ 6 ช่อง”;
- “ค21101 ยังไม่สามารถล็อกการอ่านฯ เพราะ ม.1/3 ยังไม่ยืนยัน”; or
- “ผลกิจกรรมยังขาดนักเรียน 3 คน.”

Expected validation, conflict, and permission failures return the standard JSON error envelope
without internal SQL, stack, or sensitive identity data. An autosave failure remains visible until
resolved. Bulk operations report per-scope outcomes but remain transactionally atomic wherever the
design promises all-or-nothing behavior.

## Forward-Only Cutover

The cutover uses new sequential tenant migrations; no applied migration is edited.

1. Preflight validates context integrity, canonical phase controls, score-item references, and all
   legacy activity-result outcome values.
2. Create the new Gradebook, criterion-grading, learner-evaluation, lock, and correction tables and
   constraints. Seed the eight named standard desirable characteristics, the three initial
   reading/thinking/writing criteria, and the initial `0` through `3` aggregation policy defined in
   this design.
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
3. criterion grading, course outcomes, activity evaluation, and learner-evaluation entry;
4. academic-affairs course/activity/subject-domain locks, derived learner summaries, and append-only
   corrections;
5. responsive workspaces, route/menu integration, legacy removal, and full verification.

New user-facing menu entries remain undiscoverable until their complete backend authorization and
workflow are ready. This is rollout sequencing, not a retained compatibility path.

## Verification Strategy

Focused service, policy, contract, schema, and UI tests cover:

- blank versus explicit zero, including clearing a previously stored score;
- minimum/maximum boundaries and decimal values;
- rejection of an item-maximum reduction below a retained student score;
- item totals that underfill or overfill a phase;
- hard deletion without scores and cancellation with retained scores;
- local entry-column selection, select-all/clear, read-only unchecked columns, selected-column
  keyboard traversal/paste, and unchanged calculations;
- autosave batching, retry, and optimistic conflicts between assigned teachers;
- teacher-window closure and school-management bypass;
- confirmation with blanks, targeted invalidation, roster/plan invalidation, and no invalidation
  from a control toggle alone;
- criterion-policy version changes before lock and snapshot stability after lock;
- absence of grading-method selection and group-grading runtime/contracts;
- learner-evaluation school catalogs, per-subject additions/deactivations, independent domain
  controls, blank-versus-zero behavior, keyboard entry, and group-domain confirmation;
- learner-evaluation all-room subject-domain locking, independent domain locks, equal subject
  weighting, versioned average thresholds, missing-subject readiness, and recalculation after a
  correction;
- primary-teacher, assessment-coordinator, assigned, organization, and school scope
  allow/deny/union cases;
- all-or-nothing subject locking and per-group activity locking;
- activity completeness and bulk “lock all ready” skip reasons;
- append-only course/activity/learner-evaluation corrections and stale-correction rejection;
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
- Confirmation invalidation is narrow, deterministic, and visible.
- Every course uses one active versioned school criterion policy; no group-referenced grading or
  per-subject grading-method selector remains.
- Teachers choose only `ตามคะแนน`, explicit `0`, `ร`, or `มส`; numeric grades above zero are
  derived.
- Course readiness is automatic after all room confirmations; no redundant submit button exists.
- Activity groups require complete `ผ/มผ` results and lock independently.
- Every course starts with all active learner-evaluation criteria, supports subject-specific
  additions/deactivations, and keeps blank distinct from explicit level zero.
- Learner-evaluation domains confirm independently per group and lock independently per subject
  across all rooms.
- Complete locked subject evaluations produce an automatic term summary with equal subject
  weighting and no redundant summary lock.
- Academic affairs locks initial results and performs all post-lock corrections from separate
  routes.
- Initial locked results are immutable, corrections append, and the effective result is
  deterministic.
- Legacy overlapping result storage and APIs are removed after verified forward migration, with no
  compatibility runtime.
- The locked result boundary is ready for later term-closure and promotion releases without those
  workflows being implemented here.
