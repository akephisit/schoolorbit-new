# Term Readiness Batch Providers Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline. Run verification jobs serially.

**Goal:** Give lifecycle readiness set-based, transaction-aware result providers without changing single-student API output or immutable aggregate checksums.

**Architecture:** Results loads course/activity source rows for a bounded set of student-year IDs, then groups and calculates using the existing pure functions. Learner Evaluation owns its equivalent batched expected coverage, effective criterion values and policy lookup. Existing single-student readers delegate to these same providers. Lifecycle can use the providers inside its closure transaction; no per-student HTTP requests or competing source-of-truth queries.

**Scope:** Approved Release 3 readiness foundation in `2026-09-10-academic-term-lifecycle-design.md`. No lifecycle transition becomes callable until all transactional write guards and readiness gates are implemented.

## Invariants

- Verify every requested student belongs to the selected year and the term belongs to that year; reject the entire batch on a mismatch.
- Bound batch size and source rows; reject oversized data rather than silently truncate. Preserve empty expected coverage as incomplete, not an official zero.
- Preserve per-student ordering, exact decimals, effective corrections, retained locked results and group-closure history.
- Preserve existing wire responses and checksums byte-for-byte for the same source data.
- Public APIs keep current school/assigned authorization. Internal providers explicitly require the caller to authorize contributing domains and own isolation.

## Tasks

- [x] Add disposable PostgreSQL tests comparing multi-student provider output with existing single-student previews and learner summaries; cover independent source rows and retain the existing same-snapshot correction regression.
- [x] Add rejection tests for foreign-year IDs, duplicate IDs and oversized batches; empty requested batch yields no fabricated students.
- [x] Refactor Results course/activity loaders to set-based bounded queries and a pure per-student assembly function; keep public single-student APIs unchanged.
- [x] Refactor Learner Evaluation expected coverage, locks and effective-value loading to batch queries; load the active aggregation policy once per batch.
- [x] Refactor aggregate preview assembly to consume the common source providers, retaining policy-specific exact totals and source checksums. Add a fixed pre-batch checksum fixture to detect immutable-source serialization drift.
- [x] Run full Results and Learner Evaluation tests serially, backend formatting/architecture/check, and generated API artifact checks. Review immutable hash compatibility before committing.

## Verification

```bash
CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh modules::academic::results -- --test-threads=1
CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh modules::academic::learner_evaluation -- --test-threads=1
```

Run the backend and API verification matrix in `.rules`. This provider work is not proof that closure, annual results or promotion is complete.
