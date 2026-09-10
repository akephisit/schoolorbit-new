# Term Result Aggregation Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline, as requested by the user. Steps use checkbox syntax for tracking.

**Goal:** Provide a permission-protected, correction-aware per-student preview of term credits and weighted GPA before adding durable term result locks.

**Architecture:** Results owns exact arithmetic and a repeatable-read loader of authoritative expected course coverage plus locked, corrected results. A read-only API exposes explicitly provisional totals; it never closes a term or creates an official result. Later aggregate locks must resolve an immutable school policy on the server, not accept this preview's caller-selected passing grade as authority.

**Tech Stack:** Rust, Axum, SQLx/PostgreSQL, BigDecimal, utoipa, generated TypeScript.

**Spec:** `docs/superpowers/specs/2026-09-10-academic-term-lifecycle-design.md`.

## Global Constraints

- Inline at repository root on `feature/academic-lifecycle-release-3-4`; keep image-engine stash untouched.
- Run tests sequentially. Database tests use `scripts/test_backend_school.sh` and disposable local PostgreSQL.
- Preserve numeric zero, exceptional outcomes, missing results, closed-group history, and the existing correction workflow.
- No new dependency, migration, production write, auto-deploy, or official GPA/promotion decision in this foundation slice.
- Runtime credentials only. Live baseline smoke uses sandbox; it does not verify new branch endpoints.

## Interfaces and files

- `results/models/aggregation.rs`: `CourseAggregateInput`, `CourseCreditTotals`, `TermResultPreviewQuery`, `TermResultPreview` with decimal strings.
- `results/services/aggregation.rs`: `aggregate_course_credits(courses: &[CourseAggregateInput], passing_grade: &str) -> Result<CourseCreditTotals, AppError>`.
- `results/services/term_preview.rs`: `preview_student_term(pool: &PgPool, actor: &ActorContext, student_year_id: Uuid, query: &TermResultPreviewQuery) -> Result<TermResultPreview, AppError>`.
- `results/models.rs` and `results/services.rs`: expose the new types and service through existing facades.
- `policies/academic_result_access_policy.rs`: `can_read_student_aggregate(actor: &ActorContext) -> bool`, school read/manage/lock/correct only. Assigned/unit readers must not acquire whole-student access.
- `results/handlers.rs`, `results.rs`, `api_contract.rs`: typed `GET /api/academic/results/students/{student_year_id}/term-preview`.
- `frontend-school/src/lib/api/academicResults.ts` plus generated API artifacts: typed wrapper; no new UI until durable aggregation and lifecycle are ready.
- `results/services_tests.rs`: real DB integration tests using the existing fixture and lock/correction helpers.

## Task 1: Exact course-credit aggregation

- [x] Write tests before implementation. Hand-derived expectations include:

```rust
// 1.5 credits * grade 4 + 0.5 credits * grade 2 = 7 points / 2 credits.
assert_eq!(totals.weighted_grade_points, "7.0000");
assert_eq!(totals.provisional_gpa.as_deref(), Some("3.50"));
// A grade 0 contributes attempted/graded credits but no earned credits.
// Missing, ร and มส remain unresolved; they are never converted into grade 0.
```

- [x] Run `CARGO_BUILD_JOBS=1 cargo test --bin backend-school aggregation -- --test-threads=1` in backend-school; confirm the new tests fail because the feature is absent.
- [x] Implement accumulation with `BigDecimal`, reject duplicate subject IDs, invalid grades/credits, mismatched outcome/grade, and malformed decimals. Preserve all multiplication precision; round only the display ratio with explicit half-up rounding. Zero denominator produces `None`.
- [x] Include missing-result count, exceptional-result count, unresolved credit total, and coverage completeness. Empty courses are not complete. Complete coverage with an exceptional result is not a resolved numeric aggregate.
- [x] Rerun focused tests, including input-order independence, zero-credit exceptional outcomes, exact fractional credits, custom passing threshold, and no binary floating point.

## Task 2: Authoritative read-only loader and authorization

- [x] Add DB tests in `results/services_tests.rs` using `fixture`, `prepare_phases`, existing confirmation/locking, and `correct_result`. A permissionless or assigned-only actor must receive `Forbidden`; a mismatched student/year must fail before data is exposed.
- [x] Implement a repeatable-read read-only transaction. Validate year/term and student-year identity. Read coverage from active term course memberships UNION retained locked course results without filtering closed groups. One subject contributes once; conflicting duplicate offering coverage fails instead of double-counting.
- [x] Load effective outcomes using the latest correction's expected effective version, not original numeric grade. Return source result IDs, effective versions, credits, canonical decimal strings, explicit passing grade, and a stable checksum over ordered inputs and context.
- [x] Test before locks (missing coverage), after numeric locks, after exceptional/numeric corrections, and after group closure. Verify canonical preview-policy formatting preserves the checksum; change an effective result and verify checksum changes. Input ordering is fixed by the loader and aggregate arithmetic is order-independent.
- [x] Run `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh results_term_preview -- --test-threads=1` from root; integration tests exercise school access and deny partial scopes through the real service.

## Task 3: Wire contract and verification

- [x] Register the thin handler and route; require explicit `academicYearId`, `academicTermId`, and `passingGrade` query parameters. Passing grade is a preview parameter, returned in the response, and never applied to official stored results.
- [x] Add `ToSchema`/`IntoParams`, deny unknown query fields, and use `ApiResponse<TermResultPreview>`. Include the new handler/schema registration in `api_contract.rs`.
- [x] Add a typed `previewStudentTermResults` export in `academicResults.ts` using generated DTOs and camel-case query names.
- [x] Generate API artifacts with `npm run generate:api-contracts`; run `npm run check:api-contracts` and `npm run test:api-contracts` sequentially in frontend-school.
- [x] Run backend formatting, focused Results tests, `cargo test --test static_architecture`, `cargo check`, then frontend lint/check/static tests for the wrapper change, all sequentially. Results: 37 Results tests, 163 backend architecture tests, 621 frontend static tests; frontend check reports zero errors/warnings. Existing backend dead-code warnings remain.
- [x] Review the full diff and `git diff --check`, then commit only this foundation and its tests. Do not mark Release 3 or 4 complete.

## Following dependent work

After this independently testable foundation, expand the next implementation plan for immutable aggregate policies, durable aggregate revisions and holds, transactional lifecycle guards/readiness/term transitions, and preparation preview/apply. Release 4 then adds annual aggregates and reviewed promotion runs. The approved specs remain the acceptance boundary; this foundation does not stand in for either completed release.
