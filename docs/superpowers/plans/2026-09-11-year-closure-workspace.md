# Year Closure Workspace Implementation Plan

> Execute inline using `superpowers:executing-plans`. Keep tests/builds serial; do not create a worktree or dispatch agents.

**Goal:** Let academic staff review annual readiness, begin year closure, cancel that review, and explicitly close a ready year without changing grades or student placements.

**Architecture:** Results owns current annual coverage. Lifecycle combines that coverage with Core term states. Core owns the exclusive, idempotent year transition and its atomic audit. Promotion execution, year reopening after downstream checks, and target-year/first-term activation are coordinated in the promotion implementation, not guessed by this closure slice.

**Tech Stack:** Rust/Axum/SQLx/PostgreSQL; generated permission/API contracts; Svelte 5 and local shadcn primitives.

**Spec:** `../specs/2026-09-10-academic-year-promotion-design.md`.

## Constraints

- A closed term never closes its year automatically. Selecting a year or crossing a date never performs a transition.
- All non-cancelled blocking terms must be closed. An active/closing nonblocking term also prevents closure so the current term cannot remain running under a closed year.
- Nonblocking planning/ready terms may remain historical unstarted records only after the office acknowledges that warning. Do not silently delete or cancel them.
- Every expected student needs a current annual revision or reviewed hold. Missing/stale or empty coverage blocks closure. Reviewed annual holds require acknowledgement, not conversion to a numeric GPA.
- Close changes the year state only. It never unlocks results, opens score windows, creates target enrollment, or changes historical placement.
- Use the existing school lifecycle read/manage/close capabilities. A reader cannot mutate; school management does not imply close permission.
- Migrations are forward-only. Runtime commands must not check whether a future schema exists to emulate compatibility.

## Task 1: Readiness and immutable transition receipts

**Files:**
- Create `backend-school/migrations/070_year_closure_transitions.sql`.
- Create `backend-school/src/modules/academic/core/models/year_lifecycle.rs` and export through `core/models.rs`.
- Create `backend-school/src/modules/academic/lifecycle/services/year_readiness.rs`; export through `lifecycle/services.rs`.
- Add year DTOs to `lifecycle/models.rs`.

**Interfaces:**
```rust
// Action wire values: begin_closing, cancel_closing, close.
// Request: requestId, action, expectedYearVersion, readinessChecksum,
// acknowledgedWarningCodes. Reject unknown fields and malformed IDs/checksums.
pub async fn get_year_workspace(pool: &PgPool, actor: &ActorContext, year: Uuid)
    -> Result<YearLifecycleWorkspace, AppError>;
pub(crate) async fn year_workspace_in_transaction(
    tx: &mut Transaction<'_, Postgres>, actor: &ActorContext, year: Uuid,
) -> Result<YearLifecycleWorkspace, AppError>;
// Context: ID, year number, name, start/end date, status, rowVersion.
// Terms: ID, name, sequence, status, includedInYearResult,
// blocksYearClosure, rowVersion. Coverage: Results AnnualClosureCoverage.
// Findings: code, blocking/warning severity, count, authorized resolution URL.
// Workspace also returns availableActions and the stable sourceChecksum.
```

- [x] Write tests proving missing annual results block close, an optional unstarted summer only warns, an active optional summer blocks, and foreign year/read permission is rejected. Exercise the real Results coverage and actual Core term rows.
- [x] Run `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh year_lifecycle -- --test-threads=1` and observe the missing implementation fail.
- [x] Add an immutable receipt table with unique request UUID, year FK, actor FK, request checksum, accepted readiness JSON, typed outcome JSON, and completion timestamp. Add a unique running-year index covering `active` and `closing`; do not edit migration 041.
- [x] Read year and ordered terms once, call `Results::annual_closure_coverage` in the same repeatable-read transaction, and hash the full owned context, ordered terms, coverage checksum, and findings. Resolution links require their owning read capability.
- [x] Run the focused readiness tests GREEN. Do not expose partial year activation or reopening actions.

## Task 2: Explicit year closure commands

**Files:** Create `core/services/year_transitions.rs`, export from `core/services.rs`; test in `lifecycle/services/year_transition_tests.rs` and the Results annual revision integration suite.

**Interfaces:**
```rust
pub async fn transition_year(pool: &PgPool, actor: &ActorContext, year: Uuid,
    input: YearTransitionRequest) -> Result<YearTransitionOutcome, AppError>;
// Outcome contains original requestId/action, updated year context, completedAt.
// Allowed states: active -> closing; closing -> active; closing -> closed.
```

- [ ] Test the state table directly, including rejection for planning, ready, closed and archived years. Test exact actor-bound replay, changed payload/actor conflicts, stale year/source checksums and all-warning acknowledgement.
- [ ] Test a real held annual revision: begin closing, reject unacknowledged reviewed holds, acknowledge, close; original annual revisions, score cells and placements remain unchanged. Test correction invalidation before close.
- [x] Test audit failure rolls back year state and receipt, and simultaneous transition requests cannot both accept the same version.
- [x] Implement capability checks before receipt replay; acquire tenant-exclusive transition lock before year/term locks. Recompute workspace in that transaction, validate expected version/checksum/state/acknowledgements, update year version/state, and insert immutable receipt plus Core audit before commit.
- [x] Keep the current-context year visible during `closing` by updating all staff/student/parent current-year selection predicates consistently. A closed year is not a running default.
- [x] Clarify the existing school lifecycle read/manage/close labels for both years and terms in migration 071 and the generated permission contract; do not add grants.
- [ ] Run focused year and existing term lifecycle/result tests serially. Review that no score or enrollment table is mutated by the command.

## Task 3: Typed API and office UI

**Files:** `lifecycle/handlers.rs`, `academic/lifecycle.rs`, `api_contract.rs`, generated API artifacts, `frontend-school/src/lib/api/academic-lifecycle.ts`, new `staff/academic/year-lifecycle/+page.ts` and `+page.svelte`, `tests/e2e/academic-year-lifecycle.spec.ts`.

```text
GET  /api/academic/lifecycle/years/{year_id}
POST /api/academic/lifecycle/years/{year_id}/transitions
```

- [x] Add contract tests for typed envelopes, camel-case request fields, explicit expected version/request UUID/checksum, and actual 400/401/403/404/409/422 responses. Generate artifacts; consume generated DTOs.
- [x] Build a year-required, read-first office ledger: year state, configured terms and their closure roles, annual-ready/missing/stale/held counts, and authorized links to annual results and term closure. Show only allowed actions.
- [x] Confirm close explicitly with current warnings. Preserve the same request UUID for an uncertain identical retry; a 409 requires fresh readiness. Patch the returned year and refresh authoritative academic context after success.
- [x] Browser tests: reader has no mutations, missing annual readiness disables close, optional summer warning requires acknowledgement, explicit close refreshes context, stale conflict cannot blindly retry, and mobile confirmation can be dismissed before saving.
- [ ] Run Svelte tooling and the applicable `.rules` matrix. Keep shared deployment pending until promotion/activation and Release 3 preparation acceptance are complete.
