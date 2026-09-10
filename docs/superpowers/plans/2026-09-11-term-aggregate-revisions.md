# Term Aggregate Revisions Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline, as requested. Run test commands serially.

**Goal:** Persist immutable, correction-aware student-term aggregate revisions as the prerequisite for Release 3 closure and Release 4 annual results.

**Architecture:** Results owns immutable policy and aggregate snapshots. Its coordinator consumes transaction-aware Results and Learner Evaluation readers in one database snapshot. Existing school-level permissions must cover both contributing domains; no teacher grants expand.

**Tech Stack:** Rust, SQLx/PostgreSQL, BigDecimal, utoipa/generated TypeScript.

**Spec:** `docs/superpowers/specs/2026-09-10-academic-term-lifecycle-design.md`.

## Constraints

- Never rewrite applied migrations or original locked results.
- Missing locks cannot be reviewed away; explicit policy-authorized holds only cover recorded exceptional outcomes, failed activities, or failed learner evaluations.
- Retain source versions, credit values, policy contents, actor, checksum and request identity. Retries return the original revision; reusing a key for different intent conflicts.
- Freshness compares current sources to the stored snapshot, not timestamps.
- This subsystem does not itself permit term/year closure or promotion. Those commands require all readiness providers and transactional write guards from the approved specs.

## Tasks

- [x] Add a regression proving both term readers can participate in the caller's repeatable-read transaction, including corrections after snapshot creation. Extract `preview_student_term_in_transaction` and `summarize_student_term_in_transaction`; preserve authorization on public entry points.
- [x] Add policy validation and aggregate eligibility tests: exact half-step threshold, empty/missing coverage, both learner domains, failed activities and exceptional outcomes, required hold reason. Implement typed aggregate policy/source/eligibility models and pure evaluator.
- [x] Add next forward migration for immutable aggregate policies and student-term revisions, composite context foreign keys, unique request identity and monotonically numbered revisions. Add disposable PostgreSQL tests for immutability and cross-context rejection.
- [x] Implement policy create/list, combined preview, lock and revision history services with school-domain permission intersection, serializable lock writes, checksum validation and idempotent retries. Tests cover denied readers/writers, missing sources, stale corrections, successful locks and retained history.
- [x] Register typed handlers/OpenAPI and consume generated DTOs in the frontend API wrapper. Regenerate and verify contracts. No new menu until the lifecycle workspace is implemented.
- [x] Review diff and run Results/Learner Evaluation tests, backend architecture/fmt/check, API generation checks, frontend lint/check/static serially.

## Verification

```bash
CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh modules::academic::results -- --test-threads=1
CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh modules::academic::learner_evaluation -- --test-threads=1
```

Run the remaining commands from `.rules` section 11 in each owning application directory. A live baseline smoke is not evidence that these new endpoints or the entire Release 3–4 workflow have passed.
