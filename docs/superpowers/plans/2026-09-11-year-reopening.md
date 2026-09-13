# Year Reopening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline. The user has already selected inline execution; do not dispatch agents or create another worktree. Run tests/builds serially.

**Goal:** Reopen a closed year to controlled review only when no executed promotion or activated successor depends on it, retaining all historical results and placements.

**Architecture:** Core owns year state and immutable command receipts. Lifecycle combines Core recovery evidence with its own promotion receipts. A dedicated reopening request carries a required reason; the existing three-action closure request remains unchanged. Both commands share an actor/action/checksum-aware receipt reader so a cross-action request UUID produces a conflict before decoding an incompatible outcome.

**Tech Stack:** Rust/Axum/SQLx/PostgreSQL, generated OpenAPI and permissions, Svelte 5/shadcn dialogs.

**Spec:** `../specs/2026-09-10-academic-year-promotion-design.md`, especially Year activation and recovery. Atomic new-year activation and post-execution impact resolution are separate downstream deliverables, not completed by this plan.

## Global constraints

- Reopening means `closed -> closing`, never `active`.
- Require `ACADEMIC_LIFECYCLE_READ_SCHOOL` plus `ACADEMIC_LIFECYCLE_REOPEN_SCHOOL`; ordinary manage/close grants cannot reopen.
- Preserve course/activity/evaluation locks, annual revisions, score-entry controls, enrollment and placements.
- An execution receipt, including a held decision, blocks reopening of its source year.
- A promotion run in `executing` also blocks reopening before its first item receipt commits; failed attempts with no receipts do not fabricate a completed dependency.
- Block if another year/term is running or a successor year has activated, including a successor now closed/archived.
- Use the tenant transition lock before entity locks and recheck evidence in the mutation transaction.
- Migration 070 already permits reopening receipts; do not edit it or any applied migration. New endpoint fixtures apply through 076 before invoking promotion-dependent readiness.
- Do not touch the other session's exam export files, apply live migrations, or deploy the unfinished release.

## Task 1: Read-only recovery evidence

**Files:**
- Create `backend-school/src/modules/academic/core/models/year_reopening.rs`; register in Core `models.rs`.
- Create `backend-school/src/modules/academic/core/services/year_reopening.rs`; register in Core `services.rs`.
- Create `backend-school/src/modules/academic/lifecycle/models/year_reopening.rs` and `services/year_reopening.rs`; register in sibling module owners.
- Create `backend-school/src/modules/academic/lifecycle/services/year_reopening_tests.rs`.

**Interfaces:**

```rust
// Core-owned read model, internal rather than an HTTP response.
struct YearRecoveryState {
    context: YearLifecycleContext,
    running_years: i64,
    successor_years: i64,
    running_terms: i64,
}
async fn recovery_state(tx: &mut Transaction<'_, Postgres>, year: Uuid)
    -> Result<YearRecoveryState, AppError>;

// Lifecycle-owned HTTP workspace.
struct YearReopeningWorkspace {
    context: YearLifecycleContext,
    findings: Vec<LifecycleFinding>,
    can_reopen: bool,
    source_checksum: String,
}
async fn get_year_reopening_workspace(pool: &PgPool, actor: &ActorContext, year: Uuid)
    -> Result<YearReopeningWorkspace, AppError>;
pub(crate) async fn reopening_workspace_in_transaction(
    tx: &mut Transaction<'_, Postgres>, actor: &ActorContext, year: Uuid,
) -> Result<YearReopeningWorkspace, AppError>;
```

- [x] Add a disposable fixture through migration 076 and reader/manager/reopener actors. Test the following states before implementing the read owner:

```rust
assert!(!get_year_reopening_workspace(&pool, &reader, active_year).await.unwrap().can_reopen);
assert!(get_year_reopening_workspace(&pool, &reader, closed_year).await.unwrap().can_reopen);
assert!(matches!(get_year_reopening_workspace(&pool, &no_read, closed_year).await, Err(AppError::Forbidden(_))));
assert!(matches!(get_year_reopening_workspace(&pool, &reader, Uuid::new_v4()).await, Err(AppError::NotFound(_))));
```

- [x] Run `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh -- year_reopening --test-threads=1` and observe RED.
- [x] Implement Core evidence with source context and set-based counts. A successor has a later start date and status active/closing/closed/archived, or an immutable `activate` receipt. Running counts include another active/closing year and any active/closing term. Do not query or mutate promotion tables from Core.
- [x] Lifecycle reads `count(*)` from its immutable `academic_promotion_execution_receipts WHERE source_year_id=$1` and counts runs in `executing` for that source year. Emit stable blocking codes `year.reopen_state`, `year.reopen_running`, `year.reopen_successor`, `year.reopen_promotion`, `year.reopen_execution_pending`. Hash Core evidence plus both execution counts; permission-filtered links are excluded from the hash.
- [x] Test an actual executed promotion blocks the source year; test different readers receive the same source checksum. Rerun the focused tests GREEN.

## Task 2: Atomic, reasoned reopening and typed replay

**Files:** Core `models/year_reopening.rs`, `services/year_reopening.rs`, new `services/year_commands.rs`, existing `services/year_transitions.rs`, Lifecycle `services/year_reopening_tests.rs`.

**Interfaces:**

```rust
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct YearReopeningRequest {
    request_id: Uuid,
    expected_year_version: i64,
    source_checksum: String,
    reason: String,
}
struct YearReopeningOutcome {
    request_id: Uuid,
    context: YearLifecycleContext,
    completed_at: DateTime<Utc>,
}
async fn reopen_year(pool: &PgPool, actor: &ActorContext, year: Uuid,
    input: YearReopeningRequest) -> Result<YearReopeningOutcome, AppError>;
async fn replay_year_command<T: DeserializeOwned + Send + Unpin + 'static>(
    tx: &mut Transaction<'_, Postgres>, request: Uuid, actor: Uuid,
    action: &str, checksum: &str,
) -> Result<Option<T>, AppError>;
```

- [x] Tests require distinct reopen authority, reject nil IDs/nonpositive versions/non-hex checksums, blank/overlong reasons and national-ID-like text. An identical actor/input replay returns the original outcome; changed actor/action/input conflicts.
- [x] The shared receipt reader first selects actor/action/request checksum. Only after matching all three does it decode `outcome` as the caller's typed response. Update existing closure replay to use this reader, preserving its request/outcome types.
- [x] Implement reopening: validate -> begin transaction -> tenant exclusive lock -> replay check -> source year lock -> recompute recovery workspace -> exact version/checksum and no blockers -> update only year to closing/version+1 -> immutable receipt and Core audit -> commit. The audit stores the validated reason. Use current canonical context in the response.
- [x] Inject audit failure and assert year state/receipt rollback. Race two reopening commands at the same version and assert only one succeeds. Insert a consequential dependency after preview and assert the stale command cannot reopen.
- [x] Hash official results, score controls, enrollment and placements before/after reopening and assert equality. Run `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh -- year_reopening year_lifecycle --test-threads=1` GREEN, then format/check.

## Task 3: Explicit API and lazy office dialog

**Files:** new `lifecycle/handlers/year_reopening.rs`, `lifecycle/handlers.rs`, `lifecycle.rs`, `api_contract.rs`, generated artifacts, `frontend-school/src/lib/api/academic-lifecycle.ts`, new `frontend-school/src/lib/components/academic/lifecycle/YearReopeningDialog.svelte`, existing `staff/academic/year-lifecycle/+page.svelte`, `frontend-school/tests/e2e/academic-year-lifecycle.spec.ts`.

```text
GET  /api/academic/lifecycle/years/{year_id}/reopening
POST /api/academic/lifecycle/years/{year_id}/reopening
```

- [x] Add typed OpenAPI paths (`getYearReopeningWorkspace`, `reopenAcademicYear`), request/response schemas and 200/400/401/403/404/409/422/500 envelopes. Thin handlers use the authenticated tenant context; broadcast academic context change only after commit.
- [x] Add contract tests for required reason/version/checksum, camelCase and unknown-field rejection. Generate artifacts; add concrete generated TypeScript wrappers.
- [x] The year workspace offers a compact `ตรวจการเปิดปีเก่ากลับ` action only on closed years. Load the dedicated read endpoint when that dialog opens, not for every active-year page load. Readers can inspect blockers; only exact reopen-capable users see the consequential button. No extra global setup center.
- [x] Use the existing shadcn dialog, reason textarea, inline blockers, visible X/footer close and action-specific loading. Keep the original request ID for uncertain identical retry. A conflict retains the reason and requires explicit readiness refresh before a new command. After success patch year context and refresh the shared academic-context store; do not open any score-entry window.
- [x] Browser tests cover reader versus reopener, blockers, reason requirement, uncertain retry, stale preview and mobile dismissal. Run Svelte tooling, frontend lint/check/static/menu tests, generated permission/API checks and Rust architecture/check serially. Retain separate mock versus live sandbox evidence.

Local evidence: 17 focused backend tests, 163 architecture tests, 635 frontend static tests,
11 mocked browser tests, API/permission artifact checks, and frontend lint/type checks pass.
Desktop/mobile recovery screenshots were inspected. No live migration, push, deployment,
or exact-build sandbox flow was performed. Remaining Release 3–4 work is unchanged:
future-term preparation, atomic activation, and post-execution impact resolution.
