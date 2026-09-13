# Annual Result Snapshots Implementation Plan

> Execute inline with `superpowers:executing-plans`; no subagents and serial verification.

**Goal:** Provide current, immutable annual result revisions built from the applicable locked term aggregates, ready for reviewed year closure and promotion.

**Architecture:** Results owns annual calculation, source validation and revisions. Core supplies year/term identity and the existing transition lock. No score-cell reads or writes are introduced. Lifecycle consumes the annual readiness, not duplicated arithmetic.

**Tech Stack:** Rust/Axum/SQLx/PostgreSQL, BigDecimal, generated OpenAPI, Svelte 5.

**Spec:** `../specs/2026-09-10-academic-year-promotion-design.md` and its Release 3 dependency.

## Constraints

- Included non-cancelled terms determine the annual source; partial-enrollment summer/remedial terms apply only to their participants. Missing or stale expected revisions block a lock.
- Compute GPA from summed weighted grade points and graded credits. Preserve explicit zero, unresolved credits and reviewed holds. No denominator means no GPA. Annual GPA is not GPAX.
- Current inputs and immutable history remain distinct. Source corrections create stale annual revisions; they never edit old snapshots or move students automatically.
- Combined school result/evaluation read and lock permissions remain required. Handlers use shared context and envelopes, no SQL. No plaintext national IDs.
- Add the next sequential migration; never modify migration 066 or any applied migration.
- Use existing tenant shared transition lock for snapshot locks and exclusive lock for Core transitions. Store the actor-bound request checksum and result atomically; duplicate requests cannot create another revision.

## Task 1: Exact annual calculation

Files: `backend-school/src/modules/academic/results/models/annual.rs`, `results/models.rs`, `results/services/annual_calculation.rs`, `results/services.rs`.

Interfaces:
```rust
// AnnualTermSource carries term ID/name/sequence, selected revision and current flag.
// Sum only exact credit totals; current/missing source and hold decisions remain
// explicit fields on AnnualResultPreview, outside the arithmetic helper.
fn sum_annual_credit_totals(sources: &[CourseCreditTotals]) -> Result<CourseCreditTotals, AppError>;
```

- [x] RED arithmetic tests with term points/credits `4/1` and `3/3`: annual `7/4 = 1.75`, not mean of term GPAs `2.50`; zero-credit, numeric zero and unresolved totals included. Source revision and hold checks are covered in Task 2.
- [x] Parse stored canonical decimal strings through Results helpers. Sum exact values, derive a provisional average only with a positive denominator. Missing/stale sources never report lockable, and a held source prevents official GPA.
- [x] Run focused pure tests GREEN before connecting database writes: `CARGO_BUILD_JOBS=1 cargo test --bin backend-school annual_calculation -- --test-threads=1` (3 passed).

## Task 2: Versioned annual preview and locks

Files: next forward migration `069_annual_result_revisions.sql`; new `results/services/annual_revisions.rs`; `results/annual_revision_tests.rs`.

Interfaces:
```rust
pub async fn preview_annual(pool: &PgPool, actor: &ActorContext, year: Uuid, student: Uuid)
    -> Result<AnnualResultPreview, AppError>;
pub async fn lock_annual(pool: &PgPool, actor: &ActorContext, year: Uuid, student: Uuid, input: AnnualLockInput)
    -> Result<AnnualResultRevision, AppError>;
pub async fn list_annual_revisions(pool: &PgPool, actor: &ActorContext, year: Uuid, student: Uuid)
    -> Result<Vec<AnnualResultRevision>, AppError>;
```

- [ ] Disposable fixture tests before implementation: wrong-year learner denied, missing term lock blocks, excluded/cancelled term ignored, participating summer required, nonparticipating summer skipped, correction makes old revision stale, same request replays, changed actor/input conflicts, unauthorized lock denied.
- [x] Add immutable annual table with student/year/revision uniqueness, request UUID uniqueness, snapshot/checksum, official GPA or held reason, actor/time and exact foreign keys. Include original term revision identities in the typed snapshot. Migration 069 verified on disposable PostgreSQL; no live application.
- [x] Implement caller-transaction preview using current term coverage and pinned-policy validation. Never call a public per-student pool reader from an open transaction. Lock serializably under the shared transition lock, check expected annual revision and source checksum, then append one revision. Reject absent hold reason when sources contain reviewed holds.
- [x] Provide Results-owned `annual_closure_coverage` for the expected year cohort and retained annual revisions. Missing and stale learners remain explicit; empty coverage cannot imply readiness. Recalculate current annual inputs in batches of at most 500 students.
- [x] Run full relevant Results tests with local disposable PostgreSQL.

## Task 3: Typed API and current/history inspection

Files: `results/handlers.rs`, `academic/results.rs`, `api_contract.rs`, generated API artifacts, `frontend-school/src/lib/api/academicAggregates.ts`, annual route under Results, serial browser tests.

- [x] API contract tests for year-scoped annual roster/preview/history/lock, camel-case fields, typed envelopes and actual 400/401/403/404/409/422 outcomes. Generate DTOs and consume them without response casts. Frontend check: 0 errors and 0 warnings.
- [x] Read-first annual ledger follows the term aggregate layout. Show included term sources and individual current/stale/missing status; separate annual GPA from provisional numeric average and unavailable historical GPAX.
- [x] Lock confirmation displays the selected source revisions and requires a hold reason when needed. Keep immutable history readable and preserve request ID for uncertain retries. A stale conflict requires a fresh review.
- [x] Browser tests verify teacher denial, held/missing/zero outcomes, explicit confirmation, no automatic source edits, and stale retry behavior.
- [x] Connect the Results-owned annual-source guard to the actual Core term-reopening command; disposable integration test rejects reopening a referenced term. No source data or existing result locks are removed.
- [ ] Run `.rules` verification matrix and inline review. Connect promotion/year dependency checks before exposing the Release 4 year/promotion commands.

Year transition commands, reviewed promotion policy/runs, owned target enrollment and atomic activation form the next coordinated implementation plan. No automatic promotion or deployment is authorized by a successful annual calculation alone.
