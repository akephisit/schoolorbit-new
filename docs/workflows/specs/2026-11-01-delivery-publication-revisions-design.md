# Delivery and timetable publication revisions

Approved scope: separate publication identity and order from deliberate business dates, including
the same-day teacher/offering changes and timetable publication needed to make that separation work.
The user has approved implementation. This artifact records that decision; it adds no approval gate.

## Problem and required outcome

Delivery currently creates a reference date at `max(Bangkok today, latest published start + 1 day)`.
Preview then requires the next start to be strictly later. Repeated edits can therefore manufacture
future dates, and a timetable for an earlier date cannot use the resulting source. The same-day
uniqueness rule also exists independently on timetable publication. Relaxing only the UI or one
index would leave teacher interval, offering interval, source-selection and publication failures.

A UUID identifies a version. A positive publication number orders accepted publications within
one term and domain. `publishedAt` and `publishedBy` record the actual acceptance. `effectiveFrom`
is the deliberate civil date from which that plan replaces earlier plans. These values have
separate meanings and must be presented separately.

## Canonical model and selection

- Keep existing version UUIDs, source UUIDs, statuses, row revisions, snapshots and receipts.
  New Delivery and timetable version identities continue to use UUIDv7.
- Add nullable `publication_no BIGINT` to Delivery versions and timetable versions. Each domain
  has its own positive, unique sequence per term. Drafts and cancelled unpublished versions have
  no number. Published versions have a number, effective date and publication evidence.
- Allocate `max(publication_no) + 1` only when accepting publication while holding the existing
  term serialization lock. Failed transactions consume no number. Replays return the original
  UUIDs, number and evidence without repeating side effects.
- Latest planning publication means highest publication number, including a future plan.
  Effective publication on date D means highest publication number among published versions
  whose `effective_from <= D`. A future publication is never activated before its chosen date.
- A publication's derived `effective_until` is the minimum start date of **any** higher-numbered
  publication, minus one day. It can precede that publication's own start, expressing a plan that
  has been entirely superseded. Do not clamp it or reactivate that plan later.
- Multiple publications may share a start date. Remove date uniqueness, automatic date increment,
  strictly increasing date validation and timetable's `date_already_published` rejection.
  New Delivery drafts use Bangkok today as their honest reference date. Academic terms are
  date-free identities; no term date range is inferred.

For example, Delivery publication 1 starting November 1 followed by publication 2 starting
October 30 selects publication 2 on October 30 and November 1. Publication 1 never returns on
November 1. A later publication 3 starting November 5 remains upcoming before November 5.
Timetable revisions apply the same rule on their independent sequence.

## Ownership and public contracts

`school-academic-delivery` remains the owner of opening graphs, publication chronology, teacher
and offering effectivity, audiences and source eligibility. Extend its existing typed providers
in `services/versions.rs`: latest-publication operations order by publication number; a dated
published-source operation selects the highest eligible number on the caller's connection or
transaction. `contains_date` and derived intervals use the same rule. The shared SQL allocation
projection remains Delivery-owned and ranks by publication number without a today cutoff for
Assessment and assigned discovery. Dated audience and operational consumers use the dated owner
projection. Missing publications/resources provide no fallback to unrelated live assignments.

`school-academic-timetable` owns its independent sequence, date resolution and published spans.
Its existing `resolve_version_id_for_date_on_connection`, version hydration and Attendance span
provider use publication order. Lifecycle obtains current planning sources through the owning
provider rather than retaining independent raw-date ranking. Keep existing domain crates; this
change expands their established responsibilities and does not justify another crate.

The HTTP contract keeps current create/update/preview/publish request shapes and authorization.
Add `publicationNo: integer | null` to `DeliveryVersion`, `DeliveryVersionSummary`,
`AcademicTermChangeSet`, `AcademicTermChangeSetSummary` and `TimetableVersion`. The change-set
number is projected from its exact target version, not independently persisted or allocated.
Expose `publishedAt` and `publishedBy` on list summaries as well as existing detail DTOs.
Use required nullable schema fields where absence is a valid unpublished state. Keep
`effectiveFrom`, `referenceDate`, `effectiveUntil`, UUID pins and `rowVersion` distinct.
Delivery version/detail summaries also expose `isSuperseded` for an empty logical interval.
`PublicationDateRules { today, minimumEffectiveFrom }` is Delivery-owned and appears on change-set
detail and the timetable workspace; the minimum is today only for an active term, otherwise null.
`DeliveryPublicationContext { id, publicationNo, effectiveFrom, effectiveUntil, publishedAt,
isSuperseded }` describes a published source without resource counts or personal data. Workspace
adds `deliveryPublication` and `latestDeliveryPublication`; timetable preview adds both and nullable
`applicableDeliveryPublication` for its chosen date. Keep existing FK fields. Read these through
the same owning connection/transaction, with no frontend catalog/admin fetch or parallel state.
The date picker does not narrow dates to a future latest source: a user can choose an earlier date
and then explicitly adopt its applicable source. Cloning and resuming preserve the selected exact
Delivery pin; only the explicit source-update operation changes it.
Change-set HTTP detail also includes required `baseTeacherAssignments`, containing the exact
before-command publication/construction source `assignmentId`, `learningGroupId`, `teacherId`, `displayName` and `role`. Hydrate this array
once through the route's existing read/manage resource visibility projection on every detail
response. Internal unscoped DTOs do not supply authoritative membership evidence.
Extend `TimetableVersionDisplayState` with `superseded` for a published logical interval whose
end precedes its start. Classify this before upcoming/current states and exclude it from default
selection. Existing explicit UUID history remains available.

Preview findings distinguish a stale base from a date restriction. Add a typed
`consumed_history_conflict` refusal where changing unpinned consumed history cannot be proven
safe; retain `effective_date_invalid` for the active-term no-retroactive default. Remove obsolete
date-collision findings from the executable timetable contract. Rust DTOs and annotated handlers
own the contract; regenerate OpenAPI and TypeScript through the existing generator.

The existing route/session middleware and offering/timetable resource policies remain
authoritative. Keep manage scope, stale revision checks, collaboration commit permits, audit
requirements and realtime invalidation. No new permissions, runtime grants or runtime DDL.

## Immutable facts and effectivity projection

A new revision starts from the latest immutable base graph. Inherited teachers come from that
graph, with explicit draft resource edits and teacher commands applied afterward. Candidate-date
preview must not recapture an earlier live teacher merely because the selected date moved backward.
Groups present in the immutable base retain that exact selected teacher graph. For a newly selected
published group absent from the base, construction filters registry episodes through canonical
effectivity at the candidate date before applying explicit commands, so a stopped episode cannot
return through its unchanged raw end. Genuinely draft groups instead retain their explicit staged
teacher selection; an earlier candidate materializes a carried identity when a tentative start is
later, rather than discarding that selection.

The first plan has no publication base. Persist its initial offering UUIDs once on the existing
change-set owner as `initial_offering_ids`; ordinary construction captures its initial selection,
while term preparation passes its exact applied output IDs, including future-start resources.
Candidate/reference dates never rediscover this membership. Existing Add/Stop/adjust/exclusion
commands remain separate; do not fabricate Add commands for initial members.
For any draft, selected groups absent from the publication base can also have identity-only
`initial_teacher_group_ids` and `initial_teacher_assignment_ids`. Group presence records an
explicit empty selection. Capture published selections on construction/inclusion; draft choices
stay live until the existing standalone offering publisher freezes the last staged selection under
an exclusive term lock before publishing groups. Delivery publication already fixes its final
graph before that transition. Existing marked selections remain immutable when commands are
removed; projection and command validation share this source. These arrays are provenance, not
another mutable graph or HTTP request field.
Newly added resources retain their explicit draft construction and inclusion rules.

Preserve original teacher episode dates, roles, UUIDs, provenance and all accepted snapshots.
Represent later operational effectivity through append-only supersession/cutoff evidence owned by
Delivery. Effective teacher ranges are half-open ranges bounded by the original end and accepted
cutoffs; a cutoff at or before the start produces an empty logical range. Preserve valid earlier
history. A global superseded boolean, physical deletion or an impossible `ends_on < starts_on`
does not represent this meaning.

For a publication at date D, reconcile the complete selected graph against prior episode
effectivity. Cut off incompatible assignments at D, including future episodes and episodes already
ended by another publication when their remaining slice would otherwise conflict. Carry an
existing exact assignment only when its projected interval covers D and has no finite end. Otherwise materialize a
revision-owned assignment with the retained teacher and role, and reference that exact ID from
the candidate/published snapshot. This includes carried future-start and already-ended assignments.
Same-day stop/role replacement therefore retains the old identity and gives it an empty later
effective slice without rewriting its original facts.
An obsolete finite/future cutoff must not truncate a retained replacement plan later: materialize
a carried episode when necessary to escape such a boundary, even if the old episode covers D.

Existing deterministic teacher-command episode IDs remain unchanged. Stable carried assignment
IDs use UUIDv5 of the change-set UUID and `carried-teacher-episode:<source assignment UUID>`.
Snapshot projection and publication share that exact derivation and tests. This bounded use of
derived IDs follows the existing deterministic UUID policy; it does not replace UUIDv7 for new
version identities or add a mapping table solely to force UUIDv7. Preview remains read-only and
repeated candidate previews produce stable IDs and hashes.

Replace raw-start uniqueness and overlap interpretation with provenance-aware identity and
effective-range validation while retaining group/teacher serialization and context foreign keys.
Append-only effectivity evidence must be valid only for the owning publication and protected
against later mutation. Existing historical episode readers retain every original row; current
membership/readiness readers use the canonical graph or effective projection. Classify consumers
before switching them rather than applying a blanket historical-row filter.

Offering effectivity needs activation as well as cutoff evidence. A carried offering originally
starting November 1 can be selected by an October 30 replacement without rewriting its original
start. Stopping it at/before that start must record an empty effective slice without violating the
original interval check. Dated offering providers, lifecycle readiness and guards consume the
Delivery-owned activation/cutoff projection. Raw registry dates remain preserved provenance.
The canonical offering projection is the union of nonempty publication spans in whose immutable
graph the offering appears. On each date, the highest eligible publication number owns membership.
The append-only activation/cutoff ledger records accepted transitions; global minimum activation
and cutoff dates are not availability truth. For example, X in A, a future stop in B on November 5,
and newer C retaining X on November 1 must keep X available after November 5. Recurring gaps remain
distinct spans. Roster validators/tracking use dated membership rather than singular raw bounds;
preserve accepted joined dates and exact historical pins. A roster departure selects the last
actual available slice before its placement boundary, including when that boundary falls within
a gap. No valid slice marks a removed membership without inventing an inverted stored interval.
Only genuinely draft resource preparation uses raw draft bounds.
An explicit inclusion into a new draft can reactivate a published registry offering after a gap.
It validates exact context, allowed registry status and pinned catalog eligibility; prior dated
availability is not a prerequisite for this deliberate plan mutation. Closed/cancelled resources
remain ineligible, and absence remains authoritative for operational reads and roster validators.
Earlier activation still validates the pinned catalog version's start against the newly chosen
date; preserving a source UUID does not authorize using that catalog before its own eligibility.

## Historical consumption and publication transactions

Published timetables retain their exact Delivery UUID, block/instructor identities and accepted
facts. Attendance and grade/result evidence retain their existing exact source references.
Historical reads do not revalidate old pins against a newly shortened current source interval.
Delivery publication never republishes or retargets a timetable. A timetable draft upgrades its
source only through its existing explicit operation and reconciles placements before publishing.
New timetable publication validates that its pinned Delivery publication is applicable on its
chosen date. A future latest planning source does not become eligible early.

Keep the active-term no-retroactive rule. Earlier dates are supported for unconsumed planning;
do not invent a permissive historical correction workflow. If accepted unpinned business records
would be reinterpreted, refuse with the typed consumed-history finding. Frozen source facts remain
readable regardless of the latest planning graph.

After acquiring the term lock, both publication owners recheck accepted idempotency receipts
before draft/stale validation. Matching replay returns the original result, including when the
first request committed while a concurrent request waited. Same key with a different payload
conflicts. Allocate the number, apply effectivity evidence and business transitions, initialize
Assessment as currently required, persist terminal state and audit within one transaction.
Failure at any point rolls back the entire decision. Preserve collaboration and uncertain-outcome
handling; no automatic mutation retry is added.

## Migration, reconciliation and recovery

Add the next sequential migration after the Senior verifies current main and reserves its number
(151 on the inspected base). Never edit applied migrations. Lock affected owners, capture bounded
preservation fingerprints, expand and deterministically backfill, reconcile, replace projections
and constraints, then switch runtime consumers as one coordinated release.

Backfill each domain's existing publications by `row_number()` partitioned by term and ordered
by original effective date then UUID. Existing per-day uniqueness makes this deterministic and
preserves the old winner for every date. Imported timestamp ties are legitimate. Validate
published shape, same-context source identity/status and exact graph assignment ownership; preserve
valid source links even when legacy date order differs from that lineage, and refuse conflicting
or unmappable evidence with bounded aggregate codes. Preserve every original effective date,
timestamp, UUID, snapshot, journal, receipt, source link, attendance result and grade/result fact.
No migration-created supersession evidence guesses which legacy dates were accidental.
Legacy draft construction metadata comes only from cached exact identities and original command
sources, excluding prospective Add/Adjust episode IDs. Preserve empty group markers. Recover
omitted term-preparation outputs only from a unique exact receipt, revision/target context,
actor, creation timestamp and creation-audit mapping already supported by the 148 proof. A hidden
stopped group's complete teacher selection cannot be inferred from a group ID or partial teacher
commands; missing proof refuses atomically with `PUBLICATION_151_INITIAL_TEACHERS_UNMAPPABLE`.
Contradictory initial-resource/receipt mapping refuses with `PUBLICATION_151_INITIAL_SOURCE_UNMAPPABLE`.
No initial baseline is inferred for old published or cancelled records. Before/after comparison
preserves all original change-set columns, command rows, preparation receipts and audit bytes,
excluding only the three new provenance columns and publication numbers.

Reconciliation compares preserved columns, relationships and exact source pins before and after,
excluding only intentionally added sequence/projection metadata. Persist an atomic preservation
audit and test the refusal/retry path. Use the canonical migration runner and existing all-tenant
readiness gates; add no parallel runner or runtime owner privileges.

The November 1–5 dates have not been inspected against live consumption or user intent. Their
meaning is unknown and they remain unchanged. A deliberate new planning revision can correct
the plan before consumption under the normal guarded workflow. Any correction of consumed
legacy evidence requires a separately reviewed recovery with actual evidence.
For example, a deliberate publication 6 based on the existing November 5 snapshot can start on
November 1 and supersede the old future plans while retaining every original November 1–5 record.
An unchanged graph is publishable when the chosen start is strictly earlier than its immutable
base's start; it needs no artificial teacher or weekly-target edit. The same rule applies to
unchanged timetable placements with an exact retained Delivery pin. Same-date and later-date
unchanged plans remain no-ops. All normal source, lifecycle and consumed-history guards still apply.

Deploy matching backend, generated contracts and frontend under existing maintenance and exact
migration/readiness gates. Once new sequence/effectivity semantics are accepted, recover forward
with a compatible runtime; a date-ordered older writer is unsafe. Keep recovery and verification
recipes in `docs/OPERATIONS.md` and `docs/TESTING.md` rather than adding another permanent guide.

## Acceptance

Verify repeated same-day publication; earlier replacement without later reactivation; future-plan
exclusion; source-independent timetable numbering; stable read-only previews; carried future and
ended teacher assignments; same-day teacher role/stop and offering activation/stop; stale bases;
active no-retroactive and consumed-history denials; idempotent and competing concurrent writers;
Assessment/audit/final-write rollback; unchanged historical pins; migration preservation,
malformation refusal and retry; real DML-only runtime-role execution without TEMP or DDL grants;
and the generated API plus affected desktop/mobile opening and timetable workflows.
