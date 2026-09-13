# Atomic Academic Activation Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline, as selected by the user. Run verification jobs serially and preserve the other session's exam edits. Do not deploy an unfinished shared backend.

**Goal:** Explicitly activate a ready term, atomically opening its planning year and eligible planned student records when it is the first term of that year.

**Architecture:** Core owns year, term, enrollment and placement state. Lifecycle combines Core opening evidence, executed promotion coverage, and school-selected Delivery/timetable gates. The existing `activate` transition delegates to one activation command; its preview is distinct from closing readiness. Opening policy changes, ordinary writers and activation coordinate through the existing tenant transition lock.

**Tech Stack:** Rust/Axum/SQLx, forward PostgreSQL migration, generated OpenAPI/TypeScript, Svelte 5 and local shadcn dialogs.

**Spec:** `../specs/2026-09-10-academic-term-lifecycle-design.md` and `../specs/2026-09-10-academic-year-promotion-design.md`.

## Global constraints

- Dates and the Topbar never activate anything automatically.
- No two active/closing terms or years; the active year is exempt from the other-year check when activating its next term.
- First-ever activation has no fabricated predecessor, promotion run or result snapshot.
- A planning/ready year's first non-cancelled term must be explicitly ready. Earlier years must be closed/archived; subsequent planned years do not block.
- Opening another term in the already active year requires all its preceding terms closed/cancelled, but does not require closing that year or promoting its students.
- No scores, entry windows, subject locks, offerings, groups or source placements change during activation.
- Only planned target student-years and eligible planned placements become active/current. Invalid grade/program/room/date references block; future-dated placements remain planned.
- Default optional gates are off. Required results, score structures, exam schedules and later activity grouping are not invented as opening requirements.
- Use the next unused migration number (currently 077). Never modify migrations 001–076.
- Existing correction impacts must be explicitly handled before opening the affected target year; the separate impact-resolution implementation must connect its authoritative pending projection before the complete release is accepted.

## Task 1: Versioned school opening policy

**Files:** create `backend-school/migrations/077_academic_opening_policy.sql`; Lifecycle `models/opening.rs`, `services/opening_policy.rs`, `services/opening_policy_tests.rs`; register in sibling model/service files.

**Interfaces:**

```rust
struct OpeningPolicy {
    row_version: i64,
    require_homeroom_placements: bool,
    require_published_offerings: bool,
    require_published_timetable: bool,
}
struct UpdateOpeningPolicyInput {
    row_version: i64,
    require_homeroom_placements: bool,
    require_published_offerings: bool,
    require_published_timetable: bool,
}
async fn get_opening_policy(pool: &PgPool, actor: &ActorContext) -> Result<OpeningPolicy, AppError>;
async fn update_opening_policy(pool: &PgPool, actor: &ActorContext, input: UpdateOpeningPolicyInput) -> Result<OpeningPolicy, AppError>;
```

- [x] RED: a migrated fixture reads false defaults at revision 1; readers cannot update; stale revision rejects without update or audit; an authorized changed policy increments version and is visible on another read.
- [x] Add a singleton row with positive version and three non-null booleans. Read requires Lifecycle Read; update requires Read+Manage. Update under the tenant transition lock, compare row version, append Core's existing academic audit in the same transaction, and return typed policy.
- [x] GREEN: `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh -- opening_policy --test-threads=1`.

## Task 2: Core opening evidence and ownership validation

**Files:** create Core `models/activation.rs`, `services/activation_context.rs`, `services/activation_context_tests.rs`; register in sibling files.

```rust
pub(crate) async fn read_activation_state(tx: &mut Transaction<'_, Postgres>, year: Uuid, term: Uuid) -> Result<ActivationState, AppError>;
```

`ActivationState` contains the exact term lifecycle context, optional preceding year, ordered year/term states, and bounded typed target student-year/placement/bell references. It is an internal checksum source, not a PII-heavy API response. Lifecycle receives counts/findings and the hashed evidence, not cross-module mutation authority.

- [x] RED: initial ready term passes structural evidence with no predecessor; active predecessor, wrong-year term, non-first future term, overlapping running context, invalid planned references and duplicate current placement are rejected/reported. Planned future-dated placements are identified separately, not silently activated.
- [x] Load ordered year/term and target references in set-based queries, explicitly reject over 10,000 student records rather than truncate. Check active student identity, published applicable curriculum program/grade, room year/grade/program, placement date range and capacity; require a bell schedule belonging to the target year with usable slots.
- [x] Hash the actual typed IDs, versions and relevant state, not just counts, so equal-sized room swaps or policy changes invalidate a preview.
- [x] GREEN: `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh -- activation_context --test-threads=1`.

## Task 3: Lifecycle opening readiness providers

**Files:** Lifecycle `services/opening_readiness.rs`, `services/promotion_opening.rs`, focused tests; Delivery `services/opening.rs`; timetable service child provider. Register helpers with their existing owners.

```rust
struct TermActivationWorkspace {
    context: TermLifecycleContext,
    opens_year: bool,
    predecessor: Option<YearLifecycleContext>,
    policy: OpeningPolicy,
    planned_students: usize,
    eligible_placements: usize,
    findings: Vec<LifecycleFinding>,
    can_activate: bool,
    source_checksum: String,
}
async fn get_activation_workspace(pool: &PgPool, actor: &ActorContext, year: Uuid, term: Uuid) -> Result<TermActivationWorkspace, AppError>;
pub(crate) async fn activation_workspace_in_transaction(tx: &mut Transaction<'_, Postgres>, actor: &ActorContext, year: Uuid, term: Uuid) -> Result<TermActivationWorkspace, AppError>;
```

- [x] RED: opening without configured optional work does not demand assessments/exams; enabled placement gate identifies unplaced eligible learners; enabled published-offering/timetable gates report absent publications. Policy changes and publication changes alter checksum.
- [x] Delivery and timetable return their own typed, versioned publication evidence. A published-offering gate means at least one published target offering; the label must not claim that every curriculum requirement is fulfilled. A published-timetable gate requires a published target version effective at term start.
- [x] For first-term year opening only, require all eligible continuing source students to have executed reviewed decisions for this target year or executed holds; refuse executing runs, inconsistent receipt-owned targets and unresolved post-execution correction impacts. New entrants without a source-year record do not require fabricated promotion.
- [x] Use stable finding codes and permission-filtered links. Exclude presentation links from the readiness checksum. Keep read-only inspection side-effect free and use one repeatable-read transaction.
- [x] GREEN: `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh -- opening_readiness promotion_opening --test-threads=1`.

## Task 4: One atomic activation command

**Files:** Core `services/activation.rs`, focused tests; existing `services/term_transitions.rs`, Lifecycle transition tests; no edits to already-applied receipt migrations.

```rust
pub async fn activate_term(pool: &PgPool, actor: &ActorContext, term: Uuid, request: TermTransitionRequest) -> Result<TermTransitionOutcome, AppError>;
```

- [x] RED: real ready target plus planned promotion enrollment opens year+term+eligible placements together; scores/source placements remain byte-for-byte unchanged. Initial activation and same-year term-2 activation pass independently.
- [x] Route `TermTransitionAction::Activate` directly to this command, removing the former separate activation write branch. Keep all other transition semantics intact.
- [x] Validate identifiers/versions/checksum and reject close dates, warning acknowledgements or unrelated reason on an activation request. Require Read+Activate, then acquire tenant exclusive lock before any entity locks. Replay requires exact actor, action and request checksum.
- [x] Lock ordered affected years and terms, recompute opening workspace, compare exact context versions/checksum and all gates. Update planned target enrollment/eligible placements only for year opening, then year status/version and target term status/version. Append immutable term receipt and, for year opening, a year activation receipt; append audit before commit.
- [x] Inject audit/receipt failure and assert full rollback. Concurrent commands yield one winner; exact replay returns the original outcome after later changes. Foreign actor/reused request/stale correction/stale room or policy input yields conflict and no partial changes.
- [x] Existing term activation tests now obtain the opening checksum, not the closing checksum; use fixtures migrated through the new policy before invoking the opening command. Do not add missing-table fallbacks or weaken the test migrator.
- [x] GREEN: `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh -- activation lifecycle_transition --test-threads=1`.

## Task 5: Read-first activation preview and compact policy dialog

**Files:** Lifecycle opening handler, router/OpenAPI/generated contracts; `frontend-school/src/lib/api/academic-lifecycle.ts`; new `TermActivationDialog.svelte`, `OpeningPolicyDialog.svelte`; term lifecycle page; focused Playwright tests.

```text
GET /api/academic/lifecycle/terms/{term_id}/activation?academicYearId=...
GET /api/academic/lifecycle/opening-policy
PUT /api/academic/lifecycle/opening-policy
POST /api/academic/lifecycle/terms/{term_id}/transitions (existing typed activate request)
```

- [x] Contract RED requires concrete camel-case DTOs, context query and 200/400/401/403/404/409/422/500 error envelopes where applicable. Generate API artifacts and concrete frontend wrappers.
- [x] Browser RED: reader can inspect gates but not activate; authorized ready-term action opens a dedicated lazy preview. Planning-year preview explicitly states that year and term open together and shows affected planned counts. No score-entry toggles are changed.
- [x] Keep ordinary closing actions in their current dialog. Activation uses the opening preview checksum and its own visible confirmation. Preserve an uncertain command's request ID; after 409 keep inspection open and require a refresh. Clear stale work on context/permission change; retain mobile close/footer controls.
- [x] Put the three optional opening gates in a compact management-only settings dialog, not permanent cards. Read-only users may see accepted policy in activation inspection without action-only fetches.
- [x] On success patch context, refresh the Topbar's authoritative options, and reload the affected workspace. Handle a failed post-save refresh as saved-with-refresh-needed, not as a rolled-back mutation.
- [ ] Run Svelte autofixer, serial browser tests, frontend lint/check/static, API generator/check/tests, and backend `.rules` gates. Review desktop/mobile captures. Exact-build live sandbox remains a separate whole-release gate.

## Completion boundary

This plan completes atomic activation only. Authorized correction-impact resolution, selected-module future-term preview/apply, whole-release integration and exact-build sandbox flow remain required by the approved release specs. Do not call Releases 3–4 complete based on this plan alone.
