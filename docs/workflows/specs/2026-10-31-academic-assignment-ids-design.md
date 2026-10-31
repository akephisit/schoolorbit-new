# Academic responsibility from published allocation IDs

**Status:** Contract approved in chat; written design awaiting Senior review.

## Behavior and ownership

The user requested ID-based academic responsibility throughout the selected context, extending the
previous choice of the latest published Delivery allocation version. Gradebook, Learner Evaluation,
Results preparation/history/readiness/locking and Assessment assigned discovery must use that graph
in the exact academic year and term. They must not authorize or validate a teacher from today,
another operation date, an older published graph or an arbitrary historical teaching episode.

Reuse migration150's Delivery-owned projection and its exact offering/group/assignment identity
checks. Its latest ordering remains effective start, publication timestamp, then ID; published
future-effective versions participate and drafts do not. The snapshot role remains authoritative.
No published version or an omitted resource supplies no teacher assignment. Active accounts remain
required. Do not synthesize an allocation or backfill production teaching facts.

Delivery remains the authoritative provider. Assessment and Results already depend on it, so this
extension needs no new crate, dependency edge, schema migration or change to applied migration150.
Consumers must not parse Delivery snapshots or query Delivery episode/version internals themselves.

## Typed provider extension

Extend the existing Delivery versions model/service owner with internal Rust types:

```rust
pub struct GroupTeacherAssignments {
    pub learning_group_id: Uuid,
    pub teacher_ids: Vec<Uuid>,
    pub primary_teacher_ids: Vec<Uuid>,
}
pub struct PublishedTeacherAssignments {
    pub delivery_version_id: Option<Uuid>,
    pub groups: Vec<GroupTeacherAssignments>,
}
pub struct PublishedTeacherGroupIds {
    pub delivery_version_id: Option<Uuid>,
    pub learning_group_ids: Vec<Uuid>,
}

pub async fn group_teacher_assignments_on_connection(
    connection: &mut sqlx::PgConnection,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    group_ids: &[Uuid],
) -> Result<PublishedTeacherAssignments, AppError>;

pub async fn assigned_group_ids_on_connection(
    connection: &mut sqlx::PgConnection,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    teacher_id: Uuid,
) -> Result<PublishedTeacherGroupIds, AppError>;
```

Each operation reads latest-version availability together with migration150 membership and current
active accounts on the caller's connection. Preserve an available version ID even when the graph
is empty or omits every requested resource. A nonempty request returning no source is distinct from
a known published graph that excludes a teacher. Keep group/teacher IDs sorted and deduplicated.
Queries remain bounded by requested groups or one actor in one context; respect existing caller
scan limits and never silently truncate an authorization set.

List/history queries bind the provider's assigned group IDs before resource filtering, pagination
or limits. Group workspaces and readiness use batched group assignments, including all primary IDs.
Complex SQL may join a named, typed serialization of those provider results; it must not reconstruct
snapshot selection. Keep the existing offering-scoped raw allocation provider and Assessment's
automatic coordinator heuristic unchanged.

## Primary roles and confirmation identity

Delivery permits several primary teachers in a group, and Gradebook already permits any primary to
confirm. Use a set of distinct active primary teacher IDs consistently. Any member may explicitly
confirm; a retained confirmation is current only while its `confirmed_by` ID remains in that set
and all existing source, roster, revision and invalidation checks pass. Remove arbitrary first-row
selection from Learner Evaluation and Results. A school management permission never substitutes for
a primary role. Secondary/assistant teachers retain their existing entry rights.

Gradebook must read its confirmer through a private persistence row to apply the same membership
check in workspace status and Results phase readiness. Keep public confirmation DTOs, checksum
algorithms and stored snapshot shapes unchanged. This group-role rule does not tighten Assessment's
separate unique-full-coverage automatic coordinator suggestion.

## Missing source and preservation

When no published source is available, deny new teacher confirmations and report confirmation or
readiness as not current. Missing membership knowledge alone must not persist Learner Evaluation
invalidation or advance its retained confirmation revision. Independent real source/roster staleness
guards remain in force. A known published graph removing a confirmer may use the existing writable,
unlocked invalidation policy. Never clear a previous invalidation automatically.

Preserve stored coordinators, scores, responses, criteria, confirmation identities/checksums,
official results, corrections, locks and historical snapshots. Preserve existing explicit
coordinator/own relationships where their current policies grant authority; do not invent a new
coordinator grant or erase one because an allocation changed. Keep school, exact organization and
assigned scope unions and whole-student/all-contributing-subject authorization intact.

Actual student memberships, dated audiences, attendance, lessons, events, publication chronology,
offering dates and official operation timestamps retain their owners and semantics. In particular,
Results obligation selection and its application-bound Assessment port still use the actual
operation date to classify roster/audience obligations and retain established history. Remove that
date only from teacher responsibility. Catalog revision chronology and current organization
security remain separate policies. Delivery registry/draft management access is outside this change.

## Affected flows and public impact

Existing authenticated Academic HTTP routes already receive year/term/resource IDs. Changes remain
behind the current Gradebook, Learner Evaluation, Assessment and Results handlers/services/policies;
no endpoint, permission code, public DTO or generated API change is anticipated.

Update list/detail/write/confirmation/readiness/history paths together, including Results locking
and Learner Evaluation whole-term summary authorization. Reuse the caller's transaction/connection
and existing lifecycle/offering/group locks. Keep scope predicates before limits and repeatable-read
snapshots where already used. Any required summary authorization/snapshot composition adjustment
stays within its owning service; do not introduce a parallel access policy.

## Acceptance

Reproduce expired/future episodes retained in the selected graph without changing their dates.
Verify latest replacement, missing/empty/omitted graphs, drafts, live-only teachers, inactive users,
closed groups and wrong year/term/offering/group/assignment IDs. Verify both distinct published
primaries can confirm, a secondary or manager without that role cannot, and a removed confirmer
becomes stale without rewriting immutable history. Duplicate snapshot identities cannot create
additional authority.

Verify list/detail/write/readiness/lock/history parity, school/organization/assigned unions,
coordinator-only configuration and all-subject summary denial. Missing-source reads preserve the
stored confirmation and revision; previously invalidated confirmations never revive. Keep real
dated obligation and retained-history tests, lifecycle/result locks, rollback, stale writes and
official result snapshot preservation. Correct fixtures by publishing explicit canonical allocation
graphs or seeding genuine historical facts appropriate to each test, never by weakening assertions.
