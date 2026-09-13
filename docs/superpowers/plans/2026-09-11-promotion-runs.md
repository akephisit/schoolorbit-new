# Promotion Runs Implementation Plan

> Execute inline with `superpowers:executing-plans`; keep the feature branch in the project root and run test/build jobs serially.

**Goal:** Review, approve and execute student promotion decisions persistently, with safe retries and no unintended source-year changes.

**Architecture:** Lifecycle owns run state, decisions and immutable execution receipts. Results supplies current annual evidence in the caller's transaction. Core owns destination validation and student-year/placement writes; no Lifecycle SQL writes Core student tables.

**Tech Stack:** Rust/Axum/SQLx/PostgreSQL, generated OpenAPI/permissions, Svelte 5/shadcn.

**Spec:** `../specs/2026-09-10-academic-year-promotion-design.md`.

## Constraints

- Keep all six decisions: promote, repeat, graduate, transfer_out, hold, conditional.
- A recommendation is never a reviewed decision. Review requires explicit input and current evidence.
- Overrides and holds require a reason; conditional also requires a condition.
- No source placement deletion or move. Only explicit graduate/transfer execution changes source enrollment status.
- An existing future student-year owned elsewhere is a conflict, never silently adopted.
- Permission read/manage/approve/execute/correct remains separate; no new teacher grants.
- New migrations follow 072; migrations already applied to disposable fixtures remain immutable.
- Do not merge unrelated exam edits, change live tenants, or deploy the incomplete branch.

## Task 1: Decision invariants

**Files:** create `lifecycle/models/promotion_run.rs` and `lifecycle/services/promotion_decision.rs`; register sibling modules under `backend-school/src/modules/academic/`.

```rust
enum PromotionDecisionOutcome { Promote, Repeat, Graduate, TransferOut, Hold, Conditional }
struct PromotionDecisionInput {
    outcome: PromotionDecisionOutcome,
    target_grade_level_id: Option<Uuid>,
    target_study_program_id: Option<Uuid>,
    target_homeroom_id: Option<Uuid>,
    reason: Option<String>,
    condition: Option<String>,
}
fn validate_decision(input: &PromotionDecisionInput, source_grade: Uuid,
    recommendation: &PromotionRecommendation) -> Result<(), AppError>;
```

- [x] Add pure tests: exact recommended promote/graduate accepted without reason; changed outcome or destination requires reason; repeat must keep source grade; promote must change it; terminal/hold cannot carry destination fields; conditional requires target and condition; blank, overlong and national-ID-like reason/condition rejected.
- [x] Run `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh promotion_decision -- --test-threads=1` and observe the missing implementation failure.
- [x] Implement only decision validation. Check all optional UUIDs for nil, trim text for validation, allow at most 1000 Unicode characters per reason/condition; reject 13 numeric digits even when separated by punctuation.
- [x] Run the same tests GREEN. These rules do not authorize a write or bypass current annual readiness.

## Task 2: Durable calculation and source ownership

**Files:** new sequential migrations `backend-school/migrations/073_promotion_runs.sql` and `074_promotion_run_items.sql`; create `lifecycle/services/promotion_runs.rs`, `promotion_run_tests.rs`, `promotion_calculation.rs`, `promotion_calculation_tests.rs`, `core/services/promotion_students.rs`, `results/services/promotion_sources.rs`.

```rust
struct CreatePromotionRunInput {
    request_id: Uuid, source_year_id: Uuid, target_year_id: Uuid, policy_id: Uuid,
}
struct PromotionRun { id: Uuid, source_year_id: Uuid, target_year_id: Uuid,
    policy_id: Uuid, status: PromotionRunStatus, row_version: i64 }
async fn create_run(pool: &PgPool, actor: &ActorContext,
    input: CreatePromotionRunInput) -> Result<PromotionRun, AppError>;
```

- [x] Add isolated DB tests for a teacher denial, missing/reversed/same year IDs, non-planning target, missing policy, and exact actor-bound request replay versus changed-input conflict.
- [x] Persist runs with source/target year FKs, policy FK, immutable creation checksum, optimistic revision, creator/reviewer/approver/executor and timestamps. Persist one item per run/source student-year; keep source annual revision, source enrollment version and typed recommendation JSON. No student writes during calculation.
- [x] Core's read projection includes only student-year identity, code/name, grade/program/status/version and matching year IDs. Results' batch provider returns the latest annual revision plus currentness computed against live locked sources, in batches of at most 500 and cohort at most 10000.
- [x] Calculate against repeatable evidence under the tenant transition lock; match policy by exact source grade/program, retain missing/stale findings without inventing a decision. Source and target must be chronologically distinct, source active/closing/closed and target planning.
- [x] Test existing target data is flagged, missing annual results remain visible, calculation preserves source checksums and placements, and repeated calculation changes only unexecuted items and invalidates their approvals.

## Task 3: Review and approval

**Files:** `lifecycle/services/promotion_run_review.rs`, shared run models and run tests; Core's `promotion_targets.rs` and `promotion_target_tests.rs` own destination reference validation.

Approval uses forward migration `075_promotion_run_approvals.sql`, an immutable intent table and an explicit run-owned approval pointer. Core validates destination references in bounded batches (at most 500) so whole-run approval never performs a query for every student's identical curriculum/room reference. Results evidence uses the same batch boundary. Execution must revalidate against the pinned approval, not whichever decision happens to be visible later.

```rust
struct ReviewPromotionItemInput { row_version: i64, decision: PromotionDecisionInput }
struct ApprovePromotionRunInput { request_id: Uuid, row_version: i64, source_checksum: String }
```

- [x] Tests fail when stale versions, incomplete annual evidence, a missing decision or an invalid destination is accepted; test exact manage versus approve grants.
- [x] Save explicit decisions under manage permission and increment item/run revisions. Core validates grade transitions, published target curriculum applicability and optional same-year/grade/program homeroom. Snapshot source identity and latest annual revision with the decision.
- [x] Approval requires all items reviewed against current evidence, read+approve capability and exact checksum. Record immutable approved intent; subsequent changes invalidate approval rather than modifying its historical meaning.
- [x] Use same transaction for audit and writes; inject audit failure and assert rollback. Store actor-bound request receipts for consequential approval requests.

Review/approval service coverage includes manage/read/approve denial, missing or corrected annual evidence, stale item versions, read-after-write persistence, no student writes, audit rollback, recalculation clearing reviewed decisions, immutable approved intent and action/actor-bound receipt replay. HTTP/UI exposure and execution remain separate pending tasks; do not treat a reviewed run as executable.

## Task 4: Per-item execution and interruption

**Files:** `lifecycle/services/promotion_execution.rs`, Core `promotion_students.rs`, run migration and focused tests.

Core's transactional writer lives in `core/services/promotion_execution.rs`. It rereads and locks the exact source enrollment, rejects a changed source revision or an independently created destination, and validates destination references again. A planned placement starts on the configured target year's start date and counts planned/current placements against homeroom capacity. The writer never commits independently: the Lifecycle item receipt and audit must share its transaction. Test this owner separately before exposing any execution endpoint.

Verification for this slice: `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh -- promotion_core_execution --test-threads=1`, then the existing promotion service suite. The Core writer is not an HTTP command and must not be presented as finished end-to-end execution.

Forward migration 076 owns immutable execution batch intents (actor, approval, bounded item IDs) and immutable per-item receipts with exact run/source/target foreign keys. A completed HTTP command replays its original typed outcome. An interrupted command resumes the same recorded batch, skipping its completed receipts; a returned recoverable failure is retried with a new explicit request ID. Before each item, recheck the pinned approval, unchanged decision and current annual evidence under the tenant lock. Failure recording retains the original approval and completed receipts; recalculation/review explicitly invalidates the approval for unfinished items. Audit failures must roll back Core writes and leave the interrupted batch safely resumable.

```rust
struct ExecutePromotionRunInput { request_id: Uuid, row_version: i64, limit: u16 }
struct PromotionExecutionReceipt { item_id: Uuid, target_student_year_id: Option<Uuid>,
    target_placement_id: Option<Uuid>, executed_by: Uuid }
```

- [x] Fail tests for execute without permission/approval, stale corrected result, target taken by another workflow, invalid room/capacity, duplicate retry, and source placement mutation.
- [x] Execute at most 100 pending items per request, one transaction per item: tenant lock → ordered year locks → run/item → source enrollment → destination. Revalidate approved annual source and target references before Core mutations.
- [x] Core creates planned target enrollment and optional planned placement for promote/repeat/conditional. Graduate/transfer update only the approved source-year status; hold records a receipt without target writes.
- [x] Record receipt, item completion and audit atomically. Existing completed receipt means skip, not repeat. Mark recoverable failure without discarding completed receipts; retry resumes unfinished items.
- [x] Test interrupted batches, correction before and during execution, unique target enrollment, audit failure rollback, completed-with-holds reporting and immutable approved intent.

Core owner tests cover all six outcomes, source-version conflicts, independently created destination enrollment, planned placement dates, homeroom capacity and preserving source placements. Orchestration tests cover interrupted hold batches, actor/action-bound command replay, actual result corrections after approval and between two students' batches, and rolling back a transfer plus receipt when the audit fails. A two-student fixture exercises partial completion followed by correction/recalculation: the completed item stays byte-for-byte equivalent, the unfinished decision is cleared, and approval is invalidated without removing receipts. It then relocks the corrected evidence, reviews and approves again, completes only the remaining item, reports the hold, and preserves the original item's receipt and approval. Shared release acceptance still requires the full live flow and downstream impact/activation coverage.

## Task 5: API and office workspace

**Files:** lifecycle handlers/routes, `api_contract.rs`, generated outputs, `frontend-school/src/lib/api/academic-promotion.ts`, `staff/academic/promotion/+page.ts`, `+page.svelte`, `tests/e2e/academic-promotion-runs.spec.ts`.

The read owner is `lifecycle/services/promotion_workspace.rs`: list by explicit source year and optional target year, with stable creation-time/ID keyset pagination (50 rows); reject a cursor outside that filter. Detail reads a repeatable-read transaction, hydrates minimal student labels through Core in batches of 500, and reads current annual evidence through Results. Return immutable execution receipts separately from recalculation flags for unfinished items; never imply that recalculation rewrites completed outcomes. Approval uses the exact current run/item checksum returned by this workspace.

**UI direction (frontend-design):** the academic office reviews a before/after enrollment ledger. Keep the existing school blue (`#005baa`), paper (`#f8fafc`), ink (`#172b3a`), border (`#dbe4eb`), amber (`#92400e`) and error red (`#b91c1c`) semantic tokens; Kanit semibold headings, regular body and tabular data numerals. The signature is adjacent source-year and destination-year columns, not a dashboard of metric cards. A compact state/action bar separates calculation, review, approval and execution. Run list uses year-filtered keyset pagination; detail filters/paginates the student ledger locally while preserving the full approval scope. Existing shadcn dialogs provide readable scroll bodies and visible close controls on mobile. Critique: a wizard that hides other students would make exceptions and whole-run approval harder to check, so keep the ledger visible and use dialogs only for individual decisions and consequential confirmations.

```text
GET/POST /api/academic/lifecycle/promotion-runs
GET      /api/academic/lifecycle/promotion-runs/{id}
POST     /api/academic/lifecycle/promotion-runs/{id}/calculate
PUT      /api/academic/lifecycle/promotion-runs/{id}/items/{itemId}
POST     /api/academic/lifecycle/promotion-runs/{id}/approve
POST     /api/academic/lifecycle/promotion-runs/{id}/execute
```

- [ ] Add typed contract tests for exact context, envelopes, camelCase, unknown-input rejection and observed error statuses; then regenerate contracts.
- [x] Create read-first run list/detail with explicit source/target year, policy, readiness findings and per-student decisions. Lazy-load action data only with matching permission. All six outcomes use shadcn controls with visible destination/reason/condition fields as applicable.
- [x] Approval and execution have separate confirmations. Patch returned item/run state; preserve edits on conflict and offer an explicit reload. Display holds and failed/retry items distinctly without exposing raw DB errors.
- [x] Browser tests cover read-only, review-only, approve-only, executor, each outcome, reload persistence, stale rejection, retry and mobile close controls.
- [ ] Run generated permission/API checks, Rust static architecture/check, frontend lint/check/static/menu and relevant browser suites serially. Year activation and post-execution impact resolution remain the separate downstream part of the approved spec, not silently completed by this plan.

Browser coverage uses a mocked API on the local frontend, never production interception: read-only, manage-only, approve-only, execute-only, all six decisions, exact source/target creation, persisted execution receipts, conflict-preserved text, explicit refresh after stale versions, network-uncertain retry with the same command ID, and mobile close controls. The full exact-build sandbox flow remains an acceptance gate.
