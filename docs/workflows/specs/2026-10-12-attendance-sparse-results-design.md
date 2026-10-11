# Attendance explicit results and scoped reset

Approved by the user in this chat: three kinds (arrival, lesson, special), no inferred absence,
read-only previews, partial saves and later corrections, no unchecked result rows or background
materialization, delayed special results, efficient coverage reports, and a complete Attendance-only
reset of `snwsb`, including evidence and face templates. Academic data, accounts, other modules and
other tenants are outside deletion scope. Earlier preservation of snwsb attendance was superseded.

## Owners and contracts

Keep the existing school-attendance crate for typed models, policies, persistence, summaries and
notifications. The application attendance adapter composes date-effective Academic Core, Delivery
and Timetable provider ports. No new crate is justified. Frontend attendance components and the
generated School OpenAPI contract remain the consumers; permissions retain their existing owners.

An immutable compact roster belongs to each committed session. Missing result rows mean unchecked.
Preview sessions and rosters are computed, not persisted. A revision and roster token bind mutations
to the displayed roster, session context and module generation. First save creates metadata and only
explicit non-unchecked results in one transaction. Later saves patch only changed students;
corrections require a reason, additions do not. Unchanged saves do not send duplicate notices.
Unselected students are never absent. Capture retries use event identity and verify committed events
before fresh-capture validation. Manual results retain priority over conflicting scans.

Saved rosters preserve historical participants. Unsaved expected rounds use provider-owned published
timetable versions and effective membership intervals; no result facts or daily placeholder sessions
are written by reports, calendar navigation, digest delivery, or background catch-up. Coverage counts
missing facts separately from absence. A module coverage-start date prevents a reset from
reconstructing virtual unchecked history before the reset day; explicitly saved older sessions
remain visible until deliberately removed. Range projections must batch provider reads, not query once
per date/student. Persisted cancellation and special schedule metadata are legitimate explicit writes.

## Migration and deletion

Read the complete current migration timeline before adding sequential forward migrations. Expand
roster metadata, deterministically backfill from existing records, reconcile IDs/participants and all
explicit result fields, convert prior ceremony events into named historical special rounds with
stable session IDs, and remove unchecked fact rows only after reconciliation. Archived summaries
retain valid totals and ceremony meaning through canonical special categories. Do not infer arrival
from ceremony. The coordinated backend/contracts/frontend cutover prohibits old binaries after
cleanup; recovery is repair-forward under maintenance, not old-runtime rollback.

The user-authorized operational reset is separate from universal schema migration. Require school
manage and delete authorization, exact impact/revision checks and a tenant-scoped transaction.
Delete attendance facts, schedules, calendar overrides/settings, groups, devices, face enrollments,
term archives/summaries and attendance-owned notifications. Request removal of every attendance
evidence file, including uncommitted uploads, through File Platform ownership APIs. Never directly
delete object keys or other purposes. Advance the module generation, invalidate gallery/runtime
caches and prevent stale previews from recreating data. Preserve the central reset audit. Report
completion only after durable data and physical evidence cleanup are verified. Only snwsb is reset.

## Verification and delivery

Synthetic PostgreSQL fixtures cover sparse saves, partial additions/corrections, stale preview and
concurrent mutations, unchanged-save notices, cached-gallery withdrawal, scan duplicate reconciliation,
holiday/cancellation coverage, legacy migration preservation/refusal, verified term archival, and
module reset isolation/cleanup. Browser fixtures cover read-only opening, exact delta payloads,
partial status preservation, conflicts, all three kinds, family coverage and mobile/light/dark.
Measure representative report query plans and stored rows before asserting efficiency.

Run generated-contract checks, focused backend/browser suites and `./scripts/pipeline verify --scope
auto` on the exact final tree. Integrate by PR, retain the branch through deployment, use coordinated
maintenance/migrations/readiness/acceptance, then perform the scoped reset and bounded live reads.
