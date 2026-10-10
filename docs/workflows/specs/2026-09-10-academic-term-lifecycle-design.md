# Academic Term Lifecycle — Release 3

**Status:** Approved on 2026-09-10 for inline implementation.

**Parent:** [Approved academic lifecycle architecture](2026-08-23-academic-core-lifecycle-redesign-design.md).

## Scope and current foundation

Implement term result aggregation, readiness, preparation, closure, reopening, and
activation on the current Academic Core and locked-result system. The current
`academic_course_results`, `academic_activity_results`, learner-evaluation locks,
and effective correction views are authoritative. Do not recreate the older
score-sheet submission/approval model described in the parent architecture.

Preserve the agreed Gradebook behavior: four phases, numeric or blank score cells,
teacher-owned subordinate items, phase confirmation, independent course and
learner-evaluation domain locks, and academic-office result corrections. No
norm-referenced grading is introduced. Closing a term is a separate operation from
closing an entry window or locking a subject.

Release 4 consumes the locked aggregate revisions produced here. Thai official
documents, attendance products, and unrelated portal functionality are outside
these two releases.

## User workflow and navigation

Keep `ตั้งค่าปีและภาคเรียน` for planning year/term records and bell schedules. Add
`ปิดและเปลี่ยนภาคเรียน` under the academic administration work section, guarded by
explicit school-scoped lifecycle permissions. Do not add a mandatory global
academic setup center or require every operational module to be ready at school
opening.

The lifecycle workspace shows the selected term, status, and available actions:

1. Review readiness, with counts, specific reasons, and deep links to the owning
   pages in the same academic context.
2. Calculate and inspect aggregate results for each student; lock a current
   complete aggregate revision.
3. Begin closing, finish closure, or cancel closing before closure completes.
4. Prepare a future term through preview/apply without altering the active term.
5. Mark the target ready and activate explicitly. The Topbar only changes the
   viewed context; dates never activate or close a term automatically.

An unknown planned end date is allowed. Final closure requires an actual closure
date within the owning year and not before the term start. Existing historical
closed terms remain readable without fabricated results.

## Aggregate results and exact arithmetic

Add versioned student-term result snapshots owned by Results, not Lifecycle.
Each revision records expected course/activity coverage, locked source IDs and
effective versions, credits, numeric grades or exceptional outcomes, learner
evaluation summaries, policy version, checksum, actor, and timestamps.

Use `NUMERIC`/`BigDecimal`, and decimal strings on the wire. A numeric zero is a
numeric result, not a missing value. Missing locks, missing expected roster
results, and zero covered courses must not accidentally produce a complete
aggregate. A genuine zero-credit course remains visible and does not contribute
to a credit-weighted average. A zero denominator produces no GPA, never zero GPA.

For numeric outcomes, weighted grade points equal the sum of grade multiplied by
credits. Show attempted, graded, earned, and unresolved credits separately. A
provisional numeric average may be shown with an explicit incomplete label when
exceptional results remain. Do not silently convert ร or มส into zero or claim
that a provisional average is the official GPA. School-approved policy determines
pass thresholds and whether unresolved outcomes permit an aggregate lock with an
explicit academic hold. Policy versions are immutable once used by a lock.

Expected coverage is derived from authoritative delivery/enrollment records and
retained locked results. Closing a group must not make its historical learners or
results disappear. Snapshot credit values alongside results so later offering
changes cannot silently rewrite a previously locked aggregate.

## Readiness and state transitions

Core owns transactional state changes. Lifecycle coordinates typed readiness
providers from Results, Delivery, timetable, exams, and existing supervision
services. Do not introduce a second module that directly updates all their tables.

Every finding has a stable code, severity (`ready`, `warning`, `blocking`), count,
Thai explanation, and authorized resolution link. Readiness checks existing module
data; it does not require a module that has no configured work to be invented.

Closing readiness blocks on missing required course/activity locks, missing
required learner-evaluation domain locks, incomplete/stale aggregate coverage,
and unresolved delivery changes that affect expected results. Recorded exceptional
outcomes are distinct from missing data; a reviewed hold is visible, not discarded.
Warnings require explicit acknowledgement bound to the current readiness checksum.

Term states remain `planning -> ready -> active -> closing -> closed` and
`planning -> cancelled`. Source and target context versions, checksums, and
idempotency keys accompany consequential writes. The transaction rechecks readiness
and uses one tenant-level transition lock so concurrent activation requests cannot
create two active terms. A stale preview returns a conflict without partial writes.

`closing` allows teachers to finish outstanding work before final closure. `closed`
rejects ordinary term-owned mutations in backend services, including elevated
ordinary manage permissions. Existing audited academic-office corrections remain
available. Enforce the guard inside the mutation transaction so a write racing
closure cannot commit after readiness has been accepted. Do not rely on disabled
frontend buttons.

Reopening requires a separate school permission and reason. It is allowed only if
no successor term has activated, the year is not closed, and no dependent locked
annual result or executed promotion exists. It returns the term to `closing`, not
automatically `active`, and does not unlock subject results or reopen score-entry
windows. Corrections after closure create stale aggregate/readiness indicators;
recalculation produces a new revision, preserving the previous closure snapshot.

## Future-term preparation and activation

Presets create ordinary planning terms: two regular terms, two plus summer, three
regular terms, or custom rows. Term count comes from rows, never a second count
field. Terms included in annual results must also block annual closure. A summer
term is not treated as an automatic replacement for an earlier result; corrections
use the existing correction workflow.

Preparation is explicit preview/apply with selected modules and conflict reporting.
Create target offerings from the target curriculum requirements. Reuse applicable
group/teacher configuration as drafts with source-to-target mappings; do not copy
an inapplicable term-1 subject into term 2. Reset actual teaching periods to the
curriculum standard instead of carrying a source-term workload override.

Assessment structures may be copied only for matching targets as independent four-
phase drafts. Timetable assignments may be copied only after explicit group,
teacher, room, and bell-slot mapping and collision validation, and remain drafts.
Exam rounds, supervision cycles, and registration configuration require compatible
target references and explicit target dates. Missing mappings appear in the preview;
there is no silent fallback to a source-term ID. Never copy scores, attendance,
exam outcomes, teaching logs, or supervision observation records. Copying settings
does not open entry windows or publish data automatically.

Activation checks target year ownership, term configuration, source closure, and
only the target readiness gates configured by the school. Score structures, exam
schedules, and later activity grouping are not mandatory opening gates by default.
First-ever activation requires no fabricated predecessor. First-term activation of
a future year delegates to the Release 4 atomic year/term activation operation.

## Authorization, contracts, and audit

Introduce explicit school-scoped lifecycle read/manage/close/reopen/activate
capabilities in the generated permission contract. Only verified system admin
roles receive appropriate defaults; no teacher or group-head read scope is upgraded
to a transition capability. Result access continues to use existing resource
policies, with dedicated school-level aggregate access where necessary.

Use typed camel-case APIs and generated frontend DTOs, optimistic versions, and
append-only transition/correction audit metadata without plaintext national IDs or
unnecessary PII. Realtime sends change signals and clients reload authoritative
context/options after successful transitions. New forward migrations only.

## Verification and sandbox boundary

Local disposable PostgreSQL tests cover every allowed/denied transition, concurrent
closure and mutation, duplicate retries, stale aggregate inputs, empty coverage,
decimal arithmetic, exceptional outcomes, term inclusion, and cross-context IDs.
Browser tests cover navigation, actionable blockers, permissions, preview/apply,
and Topbar refresh; mocked UI tests are reported separately from live flow tests.

Live E2E uses only `sandbox.schoolorbit.app`, runtime credentials, and named
synthetic fixtures prefixed `E2E-LIFECYCLE-` with a per-run ownership manifest.
No real identities or modifications to unrelated fixtures. Verify blank scores,
explicit zero, corrections, and retained source data after target preparation.
No wholesale tenant reset. Retain closed/audited test records for inspection;
remove only owned disposable drafts through supported APIs when appropriate.

The current deployment workflow replaces a shared backend and migrates all tenants.
Selecting sandbox for smoke does not isolate deployment. Do not deploy an unfinished
branch via that workflow. Live testing of new endpoints requires either a reviewed
isolated sandbox runtime or an explicitly approved shared release after local gates.
