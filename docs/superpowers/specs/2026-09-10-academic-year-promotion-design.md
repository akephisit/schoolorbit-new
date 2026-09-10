# Academic Year Lifecycle and Promotion — Release 4

**Status:** Proposed for review; implementation has not started.

**Dependencies:** [Term lifecycle](2026-09-10-academic-term-lifecycle-design.md)
and the [approved lifecycle architecture](2026-08-23-academic-core-lifecycle-redesign-design.md).

## Scope

Close academic years, review locked annual results, prepare future student-year
records, and execute reviewed promotion decisions. Preserve the current Gradebook
and result correction workflows. Never infer promotion from mutable score cells.
Official transcripts, ปพ. documents, and government exchange formats are separate
work; these releases do not claim regulatory certification of a school policy.

## Annual results and closure

Results owns versioned annual snapshots built from current locked term aggregates
for non-cancelled included terms. Annual snapshots preserve term revisions,
effective correction versions, policy snapshots, and exact decimal credit/grade
totals. Combine weighted grade points and credits, not rounded per-term GPAs.
Annual GPA is not GPAX: a cumulative display includes only complete eligible
historical locked inputs and identifies unavailable historical coverage.

Retain exceptional outcomes, failed activities, and learner-evaluation findings
separately. Do not assign a fabricated numeric grade or silently drop unresolved
credits. The school reviews and locks annual results or explicitly approves an
individual hold. Historical imported years without valid locked inputs are not
eligible promotion sources.

Year closure requires all configured blocking terms to be closed and every
required student to have a current locked annual revision or a reviewed hold.
Summer inclusion affects this gate; closing term 2 never closes the year itself.
Concurrent corrections invalidate the affected readiness checksum before closure
or promotion can proceed. Archive and locked snapshot history are retained.

## School policy and student decisions

Create versioned promotion policy configuration. Schools explicitly review the
grade/program progression rules, required credits, unresolved outcomes, activity
and learner-evaluation requirements, and graduation criteria. There is no
unreviewed default that automatically passes all students. Ambiguous or missing
rules produce a review finding, not a guessed destination.

Policy calculation yields recommendations only. Each student has one reviewed
decision: `promote`, `repeat`, `graduate`, `transfer_out`, `hold`, or `conditional`.
Promote/repeat/conditional specify a valid target grade/program and may specify a
target homeroom; conditional requires a recorded condition. Overrides and holds
require a reason. Graduation and transfer create no target student-year row.

The `เลื่อนชั้นและเตรียมปีใหม่` workspace selects source and target years, shows
annual readiness, creates a recommendation run, lets academic staff review
individual exceptions, and requires explicit approval before execution. Review
and execution permissions are distinct; the same user may perform both only if
granted both capabilities. Selecting a target year in the Topbar is not execution.

## Persistent runs and safe retries

Runs progress through `draft -> calculated -> reviewed -> approved -> executing ->
completed`, with `failed` for recoverable interrupted execution. Store policy and
source snapshot versions, creator/reviewer/approver/executor, and per-item status.
Changes after review invalidate approval before the affected item can execute.

Each decision transaction validates the approved source, creates or reconciles
exactly its owned target student-year/placement, and records completion atomically.
Unique student/year and run/student constraints prevent duplicates. Existing
future records owned by another workflow cause a review conflict; do not overwrite
or silently adopt them. Retry skips completed decisions, resumes safe unfinished
ones, and preserves the initial approved intent. Hold decisions remain visible
without creating target enrollment; a run reports completed-with-holds explicitly.

Do not move or delete source-year placements or change current-year students while
preparing the target. Created target student-year rows remain `planned`. Explicit
execution of graduate/transfer decisions updates only the intended source-year
status through Core and preserves historical placements and results.

After an executed decision, a result correction never moves a student automatically.
It creates an actionable impact for a separately authorized placement/promotion
adjustment with reason and audit. Unexecuted affected decisions become stale and
must be recalculated and reviewed. Corrections cannot erase an executed run.

## Year activation and recovery

Activate the target year and first eligible ready term in one Core transaction,
with the same tenant-level transition lock used by Release 3. Verify source-year
closure, reviewed promotion/hold outcomes, valid planned enrollment/placements,
and the school's configured opening gates. Do not require score structures or exam
schedules merely because they will be needed later in the term.

Set only eligible planned target records active; do not regenerate offerings,
scores, or placements. Never allow two active years or terms. Initial school
activation without a predecessor remains supported. Failed activation rolls back
all status/default-context changes. Automated retries use the original idempotency
key and return the recorded outcome instead of repeating side effects.

Year reopening requires an explicit permission and reason and is blocked after
target-year activation or promotion execution. It preserves existing locks and
returns to a controlled closing/review state. Otherwise use audited corrections
and impact resolution, not destructive rollback of historical data.

## Ownership and implementation boundaries

Results owns annual aggregation and source revision validation. Core owns year,
student-year, progression, and placement mutations. Lifecycle owns policies,
run/decision orchestration, readiness, and audit. Cross-module commands participate
in the caller's transaction; handlers contain no SQL. Use existing domain folders
and focused service files, not a giant lifecycle service with cross-module writes.

Add new sequential migrations, typed camel-case APIs/OpenAPI DTOs, generated
school-scoped permissions for lifecycle and promotion operations, and permission-
filtered menus. No compatibility aliases or modification of applied migrations.
Operational mutations enforce closed-year guards in backend transactions, including
admin ordinary writes; dedicated corrections/impact adjustments are explicit paths.

## Acceptance and live workflow coverage

Use serial local service/integration tests and browser tests for:

- Two regular terms plus summer, included/excluded terms, and annual blockers.
- Every decision outcome, invalid grade/program transitions, explicit override,
  missing source results, no GPA denominator, and unavailable historical GPAX.
- Repeated requests, interruption between batches, stale corrections before and
  during execution, and no duplicate target student-years or placements.
- Teacher denial and academic-office permissions, cross-year ID rejection, and
  no mutation of historical source records during future preparation.
- Atomic target year/term activation, rejection of parallel activation, and
  reopening restrictions after consequential downstream work.

Live sandbox flow creates synthetic students and academic fixtures, configures
four-phase scoring, confirms and locks course/activity and both evaluation domains,
closes term 1, prepares/activates term 2, tests summer, locks annual results, reviews
all promotion outcomes, retries execution, and activates the next year. Refresh
and log back in to verify persistence and permission-filtered views. Compare source
scores and placements before and after preparation/execution.

Apply the Release 3 sandbox ownership and shared-deployment safeguards. Live tests
must use the exact build under test without API interception; current-production
smoke and mocked browser tests do not prove a feature-branch flow works. Report
code checks, local DB tests, browser mocks, and live sandbox evidence separately.
