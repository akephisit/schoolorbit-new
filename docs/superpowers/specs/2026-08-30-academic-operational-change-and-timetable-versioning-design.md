# Academic Operational Change and Timetable Versioning Design

**Date:** 2026-08-30

**Status:** Approved in conversation; pending written-spec review

**Scope:** academic-term dates, term learning offerings and groups, teacher and roster mutation
rules, timetable versions, curriculum-versus-delivery alignment, typed API contracts, frontend
workspaces, forward-only tenant migration, and serial deployment verification

## Context

SchoolOrbit now has a clean separation between published catalog versions, published curriculum
requirements, term learning offerings, learning groups, teachers, rosters, and timetable entries.
The current workflow is safe while staff prepare a term, but its publication boundary is too coarse
for ordinary operational changes after teaching begins. A school may add a course or activity, stop
one that is no longer taught, change the number of timetable periods allocated in the term, or move
students into or out of a learning group. Updating published rows in place would make historical
timetables and downstream records appear as though the new setup had always existed.

The existing timetable is one recurring weekly pattern for an entire academic term. It cannot
represent “use this pattern until 31 July, then use a revised pattern from 1 August” without
overwriting history. It also requires callers to infer whether the current term-level weekly-period
target should be compared with the old or revised pattern.

Academic staff also do not necessarily know a term's final date when they first configure it.
Requiring a guessed end date for every term and every timetable revision adds false precision and
makes later corrections look exceptional when they are normal planning work.

This design adds reviewable operational change sets and effective-from timetable versions. It keeps
published curriculum versions immutable, preserves historical schedules, locks teachers after group
publication as explicitly chosen for the first release, and avoids a special “replace course A with
course B” concept. Adding and stopping remain independent operations.

## Relationship to Existing Approved Designs

This design extends the following approved boundaries rather than replacing the normalized Academic
Core model:

- `2026-08-23-academic-core-lifecycle-redesign-design.md` remains authoritative for explicit
  academic context, immutable published snapshots, term closure, Gradebook/result ownership, and
  forward-only lifecycle history.
- `2026-08-28-curriculum-structure-and-homeroom-delivery-design.md` remains authoritative for the
  catalog -> curriculum -> offering -> group ownership chain and for the homeroom-first delivery
  workspace.
- `2026-08-29-academic-workload-and-term-delivery-design.md` remains authoritative for catalog-owned
  official credit, official hours, standard periods per week, and the rule that a later term starts
  from the catalog standard.

The new design supersedes only the earlier assumption that one
`course_offering_details.weekly_period_target` is the authoritative allocation target for the whole
term. After this cutover, a published timetable version owns the operational weekly-period target for
its effective interval. Official catalog and curriculum metrics do not move into timetable data and
do not change when the operational target changes.

## Goals

- Let authorized staff add a course or student-development activity during an open term.
- Let authorized staff stop an offering from an effective date without
  deleting prior schedules, scores, results, or audit history.
- Let the term-specific weekly-period target change with a timetable version while retaining the
  prior target for historical validation.
- Represent recurring timetable changes as immutable published versions selected by date.
- Require only an effective-from date for a timetable version; derive the preceding version's end
  from the next published version or the term's actual closure date.
- Make a term's expected end optional and record its actual end only when the term closes.
- Lock learning-group teachers after group publication in both services and database constraints.
- Allow dated student roster additions and removals while the term is writable.
- Compare curriculum expectations with what is actually offered in a selected term and timetable
  version without mutating the published curriculum.
- Use preview, validation, atomic publication, optimistic concurrency, audit, typed OpenAPI, and
  generated frontend contracts throughout.
- Migrate existing timetable entries and weekly targets into one initial version with verified
  cardinality and no compatibility runtime.

## Non-Goals

- Do not support changing teachers after a learning group is published.
- Do not keep effective-dated teacher assignment history in this release.
- Do not introduce a “replace A with B” action or transfer scores between offerings.
- Do not stop only selected groups under one offering in the first release; stopping applies to the
  whole offering and every group under it.
- Do not mutate a published curriculum version when operational delivery changes.
- Do not infer that an extra offering satisfies a missing curriculum requirement.
- Do not build full drag-and-drop timetable interaction, automatic scheduling, alternating-week
  patterns, date-specific substitutions, or delivered-lesson attendance in the first release.
- Do not copy scores, attendance, results, or teaching logs into a new offering.
- Do not build term closure, result corrections, or promotion in this work; this design only
  preserves the boundaries those later workflows consume.
- Do not add event sourcing or a general-purpose polymorphic workflow engine.

## Selected Approach

### Effective-from operational change sets and timetable versions

One term change set groups changes that must become operational together. It has one effective-from
date, a required reason, a draft timetable version cloned from a published base version, and typed
  change items such as adding an offering, stopping an offering, and adjusting an operational
weekly-period target.

Publishing the change set validates and applies all changes in one transaction. Existing published
versions are never edited. Date-based readers resolve the most recent published version whose
effective-from date is on or before the requested date. The final version remains open-ended until a
later version begins or the term closes.

This is preferred to two rejected alternatives:

- **Mutate the current timetable and offering rows in place:** smaller initially, but destroys the
  historical meaning used by daily teaching, supervision, attendance, and future reporting.
- **Lock all delivery after teaching starts:** safest mechanically, but cannot support normal
  mid-term additions, early stops, or schedule changes.

The selected model preserves history without effective-dating every academic relationship.
Teachers are intentionally locked after group publication; only the relationships that require
mid-term operational change receive interval semantics.

## Authoritative Data Model

### Academic-term dates

The clean academic-term contract owns three different meanings:

- `start_date`: required instructional-context start;
- `planned_end_date`: optional planning estimate, editable while the term remains writable; and
- `closed_on`: actual final date, populated only by the future term-closing transition.

The old required `end_date` is not retained as a second runtime owner. The cutover migrates its value
to `planned_end_date`, updates every current consumer, and removes the obsolete column only after
preflight and consumer verification pass. A planning or active term is valid without a planned end.

Features that genuinely need a bounded range must validate their own dependency. For example, an
exam or calendar workflow may require an explicit date inside the owning academic year; term
creation itself does not fabricate an end date merely to satisfy that later feature.

### Operational change sets

`academic_term_change_sets` owns:

- academic year and term context;
- effective-from date;
- required human reason;
- status `draft`, `published`, or `cancelled`;
- base and target timetable-version IDs;
- row version;
- creator, publisher/canceller, and timestamps.

`academic_term_change_items` uses an explicit action-kind check and shape constraints rather than an
untyped business-state JSON payload. Initial actions are:

- `add_offering`;
- `stop_offering`;
- `adjust_weekly_period_target`.

Items reference the affected offering and store only fields meaningful to their action.
Audit details may use named JSON payloads because the audit event is historical metadata, not the
authoritative operational state.

A draft change set may be edited or cancelled. A published change set is immutable. Cancelling a
published change requires a new forward change set; it never rewrites the earlier event.

### Offering availability

Published `learning_offerings` gain availability boundaries:

- `starts_on`: first date the resource may appear in operational delivery;
- `ends_on`: optional final date; and
- stop metadata containing reason, actor, time, and change-set ID.

Existing term-start offerings are backfilled with the owning term's `start_date`.
Stopping from an effective date sets `ends_on` to the preceding calendar date. The database rejects
an end before the start or availability outside the owning academic-year bounds.

Stored workflow state remains small:

- offering: `draft`, `published`, `cancelled`, `closed`;
- group: `draft`, `published`, `closed`.

User-facing states such as upcoming, active, ended, or historical are derived from workflow state,
availability dates, the selected date, and the term state. No background job is required merely to
flip a date-derived label.

An offering stop applies to every group under that offering. It does not delete group rosters,
assessment plans, scores, results, or timetable history. A later separately designed workflow may
support ending selected groups without changing the offering.

### Timetable versions

`academic_timetable_versions` owns:

- academic year and term;
- effective-from date;
- status `draft`, `published`, or `cancelled`;
- source version when cloned;
- optional owning change-set ID;
- selected bell schedule snapshot/reference;
- row version, creator/publisher, and timestamps.

Published versions do not store a user-entered end date. For any published version, its effective
end is derived as:

1. the day before the next published version begins; otherwise
2. the term's `closed_on`; otherwise
3. open-ended.

There may be only one published version for a term and effective-from date. A draft is always edited
through an explicit version ID. A published version and its entries are immutable. Display states
`current`, `upcoming`, and `historical` are derived from date and term context rather than stored.

Every `academic_timetable_entry` belongs to exactly one timetable version. Conflict uniqueness and
slot-locking include the version ID, so the same homeroom, group, teacher, or room may occupy a
different slot in a later version without colliding with history. Read APIs that represent a
specific date resolve one version first and never merge entries from multiple versions.

### Version-owned operational period targets

A timetable-version target table owns a positive integer weekly-period target for each scheduled
course or activity offering in that version.

- For a newly prepared course, the first version defaults from
  `subject_versions.periods_per_week`.
- For an activity, staff enter an explicit timetable-period target. The system does not convert
  official clock hours to bell periods implicitly.
- Creating a new timetable version clones targets from the base version.
- Adjusting a target changes only the draft target version.
- Stopping an offering removes it from the new version's targets and entries while prior versions
  remain unchanged.
- Every active group under one offering uses the same target; no per-group target override is
  introduced.
- A later academic term again starts from the catalog standard or explicit activity setup and never
  copies a prior term's override automatically.

After migration and consumer cutover, `course_offering_details.weekly_period_target` is removed so
there is no dual owner or compatibility branch. Official `standardPeriodsPerWeek`, credit, and total
hours remain catalog-owned.

### Teacher assignments

Teacher assignments may be created, removed, or role-updated only while their learning group is
`draft`. Publishing a group locks its teacher rows. The service returns a domain conflict for later
replacement, and a database trigger prevents direct insert, update, or delete against teacher rows
for published or closed groups.

This release does not provide a mid-term teacher-change path. A new mid-term offering or group must
have its teacher selected before publication, after which the same lock applies.

### Student memberships

Student membership already carries joined and left dates. The clean roster mutation API exposes
the actual effective date instead of silently using the term start for every change.

- Adding creates an active interval beginning on the selected date.
- Removing ends the active interval on the selected date according to one documented inclusive-end
  convention.
- Re-adding later creates a new interval; it does not reopen or overwrite the prior interval.
- Historical membership and any existing score/result rows remain intact.
- Mutations are rejected for closed terms/groups or dates outside the student's academic-year and
  offering availability.

The timetable shown to a student for a requested date uses the membership interval valid on that
date rather than only the current membership flag.

## Workflows

### Add a course or activity during a term

1. Staff create a change set and choose the effective-from date and reason.
2. They select a published catalog version. A new catalog resource is created through the existing
   catalog workflow first when necessary.
3. They choose applicable grade/program or homeroom targets, term weekly-period target, and owning
   organization unit.
4. The system creates a draft offering and draft groups without changing the published curriculum.
5. Staff assign teachers, preferred rooms, and roster members. Teachers become locked when each
   group is published.
6. The target timetable version is cloned from the effective base version. Staff place the new
   groups into its weekly pattern using the existing timetable editing interactions.
7. Readiness reports missing teachers, unpublished rosters, target deficits/excesses, room/teacher/
   student conflicts, and stale source versions.
8. Atomic publication publishes the operational resources and target timetable version together.

An offering absent from the selected published curriculum is explicitly labelled an extra offering.
It is not carried automatically into the next term or treated as satisfying a curriculum
requirement.

### Stop an offering

1. Staff choose the offering, effective-from date, and reason.
2. Preview reports affected groups, homerooms, students, teacher assignments, future timetable
   entries, assessment structures, score/result counts, and downstream references.
3. The draft target timetable version removes the affected entries and targets while allowing staff
   to finish other scheduling changes.
4. Publication sets the appropriate availability end and publishes the new timetable version.
5. Historical timetable, roster, scores, results, supervision observations, and audit data remain
   readable.

A draft offering with no published group, roster, timetable entry, assessment plan, score, result,
or downstream reference may be hard-deleted through a guarded draft-only delete path. Anything
published uses stop/cancel semantics instead.

Teachers assigned to a stopped offering may continue completing already-authorized assessment work
until the term's result workflow later locks it. Stopping future teaching does not delete or migrate
scores.

### Adjust weekly periods

Staff edit the target in a draft timetable version. Validation compares each active group under the
offering with that version's target and displays counts such as `1/2 คาบ`. The new value takes effect
only when the version is published. Earlier versions retain the earlier target.

### Change the curriculum permanently

Operational add/stop actions never mutate a published curriculum. The delivery and curriculum
workspaces show alignment states:

- matches curriculum;
- curriculum requirement not offered;
- extra offering;
- ended early; and
- operational periods differ from the catalog standard.

If staff decide the change is permanent, `สร้างหลักสูตรรุ่นใหม่แบบร่าง` clones the selected
published curriculum version through the existing curriculum versioning boundary. Staff explicitly
add or remove requirements, validate totals and official metrics, select the future effective
cohort/year, and publish the new version. There is no automatic A-to-B substitution relationship.

## Publication Transaction and Readiness

Publishing a change set locks the term, change set, base/target timetable versions, affected
offerings, and groups in stable order. The service then re-reads authoritative state and verifies:

- the term is not closing, closed, or cancelled;
- effective-from is not before the term start or the current date for an active term;
- no published timetable version already owns the same term/effective-from key;
- the base version and every mutable resource still have the expected row version;
- every scheduled group is published, has a locked teacher assignment, and has a published roster;
- every scheduled offering is available on the target version's effective-from date;
- every required timetable-version target is positive;
- per-group scheduled counts meet the chosen target policy;
- homeroom, group, teacher, and physical-room slots do not conflict within the target version; and
- stopped resources are absent from the target version while historical versions remain unchanged.

The initial policy treats a target deficit as blocking publication and an excess as a reviewable
warning that requires explicit acknowledgement. The implementation plan may split this into focused
milestones, but it must not silently publish an incomplete timetable.

All state changes, timetable publication, availability boundaries, and audit inserts occur in one
transaction. Any validation or database failure leaves the prior published schedule and all
operational resources unchanged.

## API and Contract Design

Typed Rust DTOs and `utoipa` remain the wire-contract authority. Generated TypeScript DTOs are the
only frontend API boundary.

The contract adds typed resources and mutations for:

- listing, getting, creating, updating, cancelling, previewing, and publishing term change sets;
- listing timetable versions and resolving the version for a requested date;
- cloning a version into a new draft;
- reading and mutating entries and weekly-period targets within an explicit draft version;
- adding/stopping offerings through typed change items;
- dated roster additions/removals;
- curriculum-versus-delivery alignment; and
- impact/readiness findings with stable codes, severity, affected count, Thai guidance, and recovery
  route/context.

Existing timetable entry routes are cut over to require an explicit version ID for editing. Date-
based read routes accept a date and return the one resolved published version. Term-only reads use a
documented default suitable for the route, never an implicit merge across versions.

Known change-item and readiness shapes use named tagged DTOs. They do not use `unknown`, generic
records, response casts, or ad-hoc JSON. Mutations use `rowVersion`; preview/apply also binds to the
source version and normalized input so stale work fails before writes.

## Authorization, Privacy, Audit, and Realtime

Existing generated permissions remain the initial authorization boundary:

- curriculum reads/changes use the existing Academic Curriculum read/manage scopes;
- offering, group, roster, and timetable reads/changes use Learning Offering read/manage resource
  policies; and
- term-date configuration uses Academic Term manage school.

Publishing one cross-domain change set requires the union of every capability needed by its items.
The service checks each affected offering/group resource and requires school-level authority for
school-wide non-group timetable entries. No new broad role or permission is introduced merely for
convenience.

Read-only pages do not load management-only catalog, teacher, student, or room options. Impact and
alignment summaries expose labels and counts, not national IDs, blind indexes, contacts, medical
data, or unnecessary roster identities. Audit and logs never contain plaintext national IDs,
credentials, tokens, cookies, database URLs, or raw request bodies.

Publication appends academic audit events with change-set ID, action, before/after state, effective
date, actor, reason, row versions, and idempotency/request identifier. Realtime events contain only
invalidation signals. Clients re-read the affected typed resources and never treat an event payload
as permission or data truth.

## Frontend Workspaces

### Academic term setup

The term form requires the start date, makes the planned end explicitly optional, and shows the
actual closure date read-only when present. Pages that need an end explain their own missing
dependency instead of making the setup form guess.

### Homeroom-first delivery and offering detail

The delivery workspace shows curriculum alignment and date-derived offering/group state. Authorized
staff can start an add/stop change set from the relevant homeroom or offering row. The offering
detail distinguishes:

- catalog standard periods;
- target periods in the selected timetable version;
- scheduled periods per active group; and
- current/upcoming/historical availability.

Teacher controls are editable only for draft groups. Published groups show a concise locked state;
the frontend does not offer a replacement workflow. Roster controls collect an effective date for
each addition or removal.

### Timetable workspace

The timetable page shows a version selector with current, upcoming, historical, and draft labels.
Editing is possible only inside a draft version. Creating a revision asks for effective-from and
reason, clones the selected published base, and links the new draft to its change set.

The existing create, move, swap, and deactivate interactions operate inside the selected draft. A
side panel shows additions, stops, target counts, conflicts, warnings, and readiness. Publishing is
one explicit reviewed action. Published versions are read-only.

This release prepares the correct data and workflow boundary for a later drag-and-drop board. Drag
and drop will not be the only accessible interaction when added; keyboard and explicit move controls
remain supported.

### Curriculum workspace

The curriculum document remains context-independent for definition. When opened from delivery, an
alignment view uses the explicit year, term, and optional timetable-version context to show missing,
extra, ended-early, and period-difference states. Permanent changes create a new curriculum draft;
they never edit the published source version.

All pages use existing PageShell/app-state patterns and local shadcn-svelte primitives. Dense tables
retain readable fixed widths and horizontal scrolling. Empty, missing-dependency, permission, stale,
conflict, and request-failure states remain distinct and actionable.

## Migration and Contract Cutover

Implementation reads the complete applied migration timeline and adds only the next sequential
forward migration. No applied migration, including migration 051, is edited.

Preflight inventories:

- every academic term and every current `end_date` consumer;
- timetable entries by term, active state, group, homeroom, teacher, room, and bell period;
- offering/group publication state and term context;
- current course weekly-period targets and groups requiring each target;
- duplicate/conflicting timetable slots that would become invalid inside one version;
- teacher assignments under published groups;
- student memberships with invalid or ambiguous date intervals; and
- assessment, result, supervision, and other downstream timetable references.

The forward cutover then:

1. adds optional planned and actual term-end ownership and migrates existing end dates to the
   planned field;
2. creates change-set, timetable-version, change-item, and version-target structures;
3. creates one deterministic initial published timetable version per term that has timetable or
   weekly-target data, effective from the term start;
4. assigns every existing timetable entry to its deterministic initial version;
5. copies every current course weekly-period target into that version and creates explicit activity
   targets only where deterministic source data exists;
6. adds offering availability, backfilled from term start;
7. installs teacher-lock and published-version immutability constraints;
8. replaces timetable conflict indexes with version-aware equivalents;
9. updates backend, generated contract, and frontend consumers in one clean cutover;
10. verifies exact entry, group, teacher, target, and downstream-reference cardinalities; and
11. removes obsolete `academic_terms.end_date` and
    `course_offering_details.weekly_period_target` only after all verification passes.

If any activity target, date interval, conflict, or relationship cannot be mapped deterministically,
preflight blocks the destructive stage and reports school-facing identifiers for explicit repair. It
never invents a timetable period count from clock hours or guesses an operational end date.

There is no dual read, dual write, legacy request parser, fallback response, or per-tenant
compatibility path. The backend, OpenAPI artifact, generated frontend DTOs, and frontend deploy as a
coordinated release unit. A protected Neon snapshot is required immediately before applying the
cutover to a real tenant.

## Error and Concurrency Semantics

- Stale row versions or a changed base timetable return `409 Conflict` and preserve the draft for
  review; they never overwrite another user's work.
- Editing a published timetable version, published group teacher, closed term, or closed group
  returns a domain conflict even when called outside the UI.
- An invalid effective-from date identifies the term start/current-date rule that failed.
- A timetable conflict names the resource type and school-facing label without leaking unrelated
  personal data.
- A target deficit is blocking; an excess is a warning requiring acknowledgement.
- A stopped resource with historical scores/results remains readable and is never hard-deleted.
- A missing curriculum requirement and an extra offering are alignment states, not API failures.
- A failed change-set publication rolls back the complete transaction and leaves the prior
  published version current.
- Retrying a successful publication with the same idempotency key returns the same result; reusing
  the key with changed normalized input is rejected.

## Release Sequence

### Release 1 — Versioning foundation and migration

- add optional planned/actual term-end ownership;
- add change-set, timetable-version, availability, and version-target schema;
- migrate current timetable and target data into deterministic initial versions;
- cut current timetable readers/writers over to explicit versions;
- enforce published-version and published-group teacher immutability; and
- regenerate contracts and update affected frontend reads without introducing the new workflow UI.

Release 1 is deployed and smoke-tested on `sandbox` before Release 2 begins.

### Release 2 — Operational add/stop and dated roster workflows

- add typed change-set preview/apply/publication services;
- add course/activity offering add/stop actions;
- add dated roster membership mutations;
- add version-owned target adjustment and readiness;
- integrate impact previews and transactional publication into Delivery; and
- preserve assessment/result access for stopped historical delivery.

### Release 3 — Timetable version workspace

- add version selection, cloning, draft editing, readiness, warning acknowledgement, and publication;
- make existing create/move/swap/deactivate operations version-aware;
- expose per-group target completion and conflict summaries; and
- keep full drag-and-drop and automatic scheduling deferred.

### Release 4 — Curriculum alignment and permanent-change handoff

- add set-based curriculum-versus-delivery alignment;
- show missing, extra, ended-early, and period-difference states by homeroom/program;
- add the explicit clone-to-new-curriculum-draft handoff; and
- retain immutable published curriculum versions and explicit future effectiveness.

Each release has its own implementation plan, focused verification checkpoint, commit, push,
automated deployment, and authenticated `sandbox` smoke test. Local commands run one at a time
because the environment is known to stall when Rust and frontend work run concurrently.

## Testing and Verification

Implementation follows test-driven development and the change-type matrix in `.rules`, with all
commands executed serially.

Database/backend coverage proves:

- optional planned term end and actual closure-date semantics;
- deterministic initial timetable-version and target backfill with exact cardinality;
- published version immutability and version-aware conflict uniqueness;
- date resolution before, on, and after a later version's effective-from date;
- teacher writes accepted for draft groups and rejected by service and database for published groups;
- dated membership add, remove, re-add, and student timetable resolution;
- add/stop offering workflows for courses and activities;
- target cloning, adjustment, deficit, excess acknowledgement, and later-term default reset;
- atomic publication, rollback, stale row versions, source changes, and idempotent retry;
- historical assessment/result/supervision references remain intact after a stop;
- curriculum alignment states do not mutate curriculum requirements;
- allowed and denied resource-policy paths; and
- OpenAPI registration for every changed typed DTO.

Frontend coverage proves:

- term setup does not require a planned end;
- current/upcoming/historical/draft timetable versions resolve and render correctly;
- only draft versions expose timetable mutation controls;
- teacher controls disappear or show locked state after group publication;
- dated roster controls preserve entered dates and actionable validation;
- add/stop impact previews and readiness distinguish blocking findings from warnings;
- standard, target, and scheduled period values have unambiguous labels;
- curriculum alignment shows missing, extra, ended-early, and period-difference states;
- read-only users do not request management-only options; and
- no request-per-row fan-out, untyped payload, or broad reload is introduced.

Applicable gates include disposable database migration/preflight tests, focused Rust service and
policy tests, `cargo fmt --all -- --check`, static architecture checks, `cargo check`, API contract
generation/check/tests, frontend lint, Svelte check/tooling, static tests, focused Playwright
workflows, `git diff --check`, final diff review, and `git status --short`. Neon compatibility uses
only the explicit disposable-branch gate. Deployment verification follows `docs/OPERATIONS.md`.

## Deployment and Recovery

- Keep a protected Neon snapshot until migration, authenticated read-only verification, and all
  selected workflow smoke tests pass.
- Deploy through the existing maintenance/all-tenant migration workflow; do not run ad-hoc live SQL
  or edit `_sqlx_migrations`.
- If preflight or migration verification fails, the transaction rolls back and maintenance remains
  enabled. Repair uses a reviewed forward migration or explicit source-data correction.
- The previous application is not a supported rollback after the incompatible cutover accepts new
  writes. Before accepting writes, rollback may restore the snapshot and previous release. After
  acceptance, recovery moves forward unless the school explicitly accepts snapshot restoration and
  reconciliation of later writes.

## Success Criteria

- Staff can add or stop a course/activity during an open term through one previewed, audited,
  atomic change set.
- Historical schedules remain unchanged and a requested date resolves exactly one published
  timetable version.
- No timetable version or term setup requires a guessed end date.
- Operational targets can differ by timetable version while catalog credit, official hours, and
  standard periods remain unchanged.
- Published-group teachers cannot be changed through UI, API, or direct database writes.
- Student roster changes retain valid join/leave history and date-correct timetable visibility.
- Curriculum pages identify missing, extra, ended-early, and period-difference delivery without
  mutating published curriculum versions or inventing A-to-B replacement semantics.
- Existing timetable entries, targets, groups, teachers, rosters, and downstream references migrate
  with verified cardinality and no runtime compatibility branch.
- Generated contracts, permissions, backend/frontend checks, migration gates, deployment, and
  authenticated `sandbox` smoke tests pass serially for every release.
