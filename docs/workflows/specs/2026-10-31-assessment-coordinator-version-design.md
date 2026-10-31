# Assessment coordinator allocation version

**Status:** Contract approved in chat; written design awaiting Senior review.

## Problem and chosen behavior

After academic year and term dates were retired, Assessment candidate queries and the exam-source
view changed from a term-clamped date to Bangkok today. A saved coordinator can consequently become
invalid when viewing another academic context even though its allocation has not changed.

The user chose the latest published Delivery allocation version in the selected term. Select the
version by `effective_from DESC, published_at DESC, id DESC`, with exact year and term ownership and
no comparison against today. A published future-effective version participates in that ranking.
Draft versions do not participate. No published version, an omitted offering, or an omitted teacher
means no candidate; preserve the stored coordinator and report its invalidity. Never reconstruct a
version from dates or assignment history, backfill an allocation, or replace a stored coordinator.

## Ownership and contracts

Delivery owns canonical allocation snapshots and their selection. Migration150 adds the
Delivery-owned `academic_delivery_latest_published_teacher_allocations` view with these columns:

| Column | Type |
| --- | --- |
| `delivery_version_id` | UUID |
| `academic_year_id` | UUID |
| `academic_term_id` | UUID |
| `learning_offering_id` | UUID |
| `learning_group_id` | UUID |
| `assignment_id` | UUID |
| `teacher_id` | UUID |
| `role` | TEXT |

The view selects the latest published version per exact academic context, then expands that
version's canonical offering, group and teacher graph. Match offering and group IDs to their exact
year/term context and match the snapshot assignment ID to its actual group, teacher and context.
Exclude closed groups. Preserve the snapshot role and assignment identity: actual episode dates
and current roles do not replace snapshot facts. Identical snapshot identity rows may be
deduplicated; distinct assignment IDs remain distinct. Do not fall back to an older version when
the latest graph omits an offering, group or teacher.

Delivery exposes a narrow typed provider in its existing versions owner:

```rust
pub async fn teacher_allocations_on_connection(
    connection: &mut sqlx::PgConnection,
    offering_ids: &[Uuid],
) -> Result<Vec<DeliveryTeacherAllocation>, AppError>;
```

`DeliveryTeacherAllocation` contains the view columns and uses `LearningTeacherRole` for `role`.
The operation uses the caller's existing connection, queries the view in bulk, and returns an empty
result for empty or uncovered input. Assessment depends on this existing Delivery public operation;
Delivery continues to call Assessment through its existing application-bound publication port, so
the dependency remains acyclic. No new crate is warranted: allocation selection already belongs to
Delivery and the new provider is narrower than that existing owner.

Assessment remains responsible for active-account filtering, current canonical display names,
candidate counts, coordinator suggestion, validation and readiness. Read/list/detail, save and
missing-owner initialization consume the same provider. The exam-source view consumes the same
Delivery projection and preserves the same active-account and exact-context guards.

## Counts and automatic initialization

Preserve all nonclosed offering registry groups as the denominator and displayed group list/count,
including groups absent from the selected graph. Missing groups and groups without teachers prevent
an incomplete allocation from producing a full-coverage suggestion.

Preserve the existing unique-full-coverage suggestion rule: a candidate must be primary in every
denominator group and have exactly one distinct primary assignment identity per denominator group
in total; exactly one candidate may satisfy those counts. A second primary covering only a subset
does not prevent the unique full-coverage teacher from being suggested. Two full-coverage candidates
remain ambiguous. Keep distinct primary assignment IDs even when they share a group and teacher.
Initialization fills only missing coordinators and retains lifecycle/result locks and replay guards.

## Publication transaction

Move the existing Assessment initialization call after both the Delivery version and its change set
have transitioned to published, before the audit insertion and commit. Episode changes remain
before both transitions because their provenance guard requires the draft change set. Initialization
then reads the target through the ordinary latest-published provider on the same transaction.

Any initialization error rolls back episode changes, publication statuses, new plans and owners
together. Keep the existing port and application adapter signatures. Direct offering publication
creates no allocation version; it therefore uses existing latest-version coverage or produces no
new owner. It must not synthesize coverage.

## Preservation, deployment and public impact

Migration150 creates/replaces read projections only. Applied migrations146–149 remain immutable.
Preserve all stored coordinators, plan/phase identities and values, actual teacher intervals,
published allocation graphs and exam schedule snapshots. Reusing a ready plan for new exam imports
must agree with Assessment's coordinator validity.

HTTP DTOs, routes, permissions, generated contracts and realtime contracts remain unchanged. Update
the save-validation message to describe teachers in the latest published allocation. The existing
frontend correction preserves the saved coordinator label when it is no longer eligible.

The schema addition is structurally compatible with the preceding binary, but its today-based
Assessment rule disagrees with the new exam projection. Do not resume the old application binary
after150 activation; normal maintenance and release acceptance must activate the matching binary
before reopening. Recover forward while preserving data.

## Acceptance

Use disposable fixtures to verify latest-version selection independent of wall-clock eligibility,
historical/future dated episodes retained in the chosen graph, replacement and omission, draft
exclusion, no-version coverage, exact offering/year/term/assignment boundaries, inactive accounts,
closed groups, duplicate identity handling and incomplete-group denominators. Verify list/detail,
save, initialization and exam-source parity, unique-full-coverage suggestions including subset
primary coverage, preservation of existing owners/phases/intervals, publication rollback and replay,
and existing lifecycle/result-lock denial. Production assignment facts remain unobserved by this
design; no screenshot is treated as database proof.
