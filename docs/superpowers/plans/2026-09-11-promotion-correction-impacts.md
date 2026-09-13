# Promotion Correction Impact Evidence Plan

> **For agentic workers:** Use superpowers:executing-plans inline. The user selected inline execution. Tests/builds run serially; do not modify the other session's exam files or deploy this unfinished branch.

**Goal:** Identify corrections to exact results used by executed promotion decisions, without moving students or duplicating correction history.

**Architecture:** Results reads immutable annual/term snapshots and the append-only correction ledger. Lifecycle projects that evidence onto immutable execution receipts. Effective-version comparison, not wall-clock timestamps, determines whether a correction occurred after the result version used by a decision. Pending impacts are derived from these authoritative records; reads do not create database records. A later explicit resolution command will record the academic office's decision and any Core-owned adjustment separately.

**Tech Stack:** Rust, SQLx, typed snapshot JSON, generated OpenAPI/TypeScript, existing promotion workspace.

**Approved design:** `../specs/2026-09-10-academic-year-promotion-design.md`, Persistent runs and safe retries.

## Boundaries

- Do not modify Results' correction command or add a callback/trigger that writes promotion data. The existing correction ledger already records each accepted change atomically.
- Do not alter migrations 001–076. This read-only evidence slice needs no migration.
- Results owns queries against annual snapshots and correction rows; Lifecycle owns execution/run queries. Neither writes Core enrollment or placements.
- A correction before the annual snapshot's effective version is not a new impact. A correction and a later reversal are two retained impacts, not an erased history.
- Compare exact result identity and effective version for courses, activities, and every learner-evaluation criterion. A term excluded from the annual source is not a dependency.
- Unexecuted run items remain stale/reviewable through the existing calculation workflow; they do not become executed impacts.
- Current review/approval already requires a locked annual source even for a hold. An executed item without that evidence is inconsistent data, not an empty impact list; fail closed rather than fabricate or silently skip its dependency.
- Use stable `(execution item, correction)` identities and read-first `ACADEMIC_PROMOTION_READ_SCHOOL` authorization. Do not upgrade readers to correction authority.
- This plan does not complete the separately required authorized impact-resolution/Core-adjustment workflow or atomic year activation. Those remain downstream and must be implemented before claiming Release 4 complete.

## Task 1: Results-owned correction evidence

**Files:** new `backend-school/src/modules/academic/results/models/annual_corrections.rs`, new `services/annual_corrections.rs`, new `services/annual_correction_tests.rs`, sibling registration in `models.rs` and `services.rs`.

```rust
struct AnnualCorrectionEvidence {
    annual_revision_id: Uuid,
    academic_term_id: Uuid,
    term_name: String,
    offering_code: String,
    offering_name: String,
    criterion_name: Option<String>,
    evaluation_domain: Option<LearnerEvaluationDomain>,
    result_id: Uuid,
    source_effective_version: i64,
    correction: ResultCorrectionRecord,
}
pub(crate) async fn corrections_after_annuals(
    tx: &mut Transaction<'_, Postgres>, year: Uuid, annual_ids: &[Uuid],
) -> Result<Vec<AnnualCorrectionEvidence>, AppError>;
```

- [x] Add tests against a real locked annual fixture. Correct one course after the snapshot and assert exactly that correction is returned, with the original version and before/after value. Observe RED before implementing.
- [x] Read 1–500 distinct year-scoped annual IDs in one query using `Json<AnnualResultPreview>`. Reject missing/cross-year IDs rather than silently omitting them.
- [x] Flatten typed course/activity/criterion snapshots into unique result/version references. Reject inconsistent stored references; explicitly bound the batch rather than truncate.
- [x] Join correction rows set-wise via typed JSON record input, matching result kind and ID, with `expected_effective_version >= source_version`. Return named typed rows and existing typed correction values; no ad-hoc JSON API payload.
- [x] Tests cover correction before a newly locked annual revision, multiple corrections including reversal, activities, both evaluation domains, missing/duplicate/cross-year inputs, and unchanged student-year/placement/run records.
- [x] Run `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh -- annual_correction --test-threads=1` GREEN.

## Task 2: Lifecycle-owned executed impact projection

**Files:** new Lifecycle `models/promotion_impacts.rs`, `services/promotion_impacts.rs`, focused tests and sibling registration.

```rust
struct PromotionCorrectionImpact {
    id: Uuid,
    item_id: Uuid,
    student_academic_year_id: Uuid,
    evidence: AnnualCorrectionEvidence,
}
struct PromotionImpactWorkspace {
    run_id: Uuid,
    source_year_id: Uuid,
    target_year_id: Uuid,
    impacts: Vec<PromotionCorrectionImpact>,
    total_count: usize,
    next_cursor: Option<Uuid>,
    source_checksum: String,
}
```

- [x] Start from actual execution receipts joined to their immutable executed item state. Load annual evidence through Results in bounded batches, with no per-student SQL.
- [x] Expose a consistent read-only transaction and exact run ownership; derive stable impact IDs from execution item and correction UUID. Hash typed evidence for later optimistic resolution.
- [x] Test executed hold/promote/terminal outcomes with actual fixtures; corrections must not move students, mutate approved intent, or alter completed receipts. An unexecuted stale item produces no executed impact.
- [x] Add count/pagination bounds appropriate to the existing 10,000-student run limit; never silently drop impacts.

## Task 3: Read-only HTTP and promotion UI

**Files:** Lifecycle handler/router/OpenAPI, generated contracts, promotion API wrapper/detail component, browser tests.

```text
GET /api/academic/lifecycle/promotion-runs/{run_id}/impacts
```

- [x] Add concrete generated DTOs and read-first handler; contract test required context and error envelopes, then generate artifacts.
- [x] Show a compact notice and an explicit inspection action in the existing run detail. Fetch evidence only on inspection. Match student labels from the current run workspace, and show result kind, original versus corrected result, and correction date without exposing irrelevant identifiers.
- [x] Keep completed decisions immutable and visible. No button claims a correction is resolved until the separately authorized resolution command exists.
- [x] Browser tests cover an executed changed result, ordinary reader, empty impact list, and reload persistence. Report mocked UI separately from exact-build live sandbox coverage.
- [ ] Run Svelte tooling, relevant frontend static/lint/type/API tests, focused backend tests and the `.rules` matrix. Preserve the other session's uncommitted files.
