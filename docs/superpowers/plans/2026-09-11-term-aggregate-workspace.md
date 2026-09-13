# Term Aggregate Workspace Implementation Plan

> Execute inline with `superpowers:executing-plans`, no agents, one verification process at a time.

**Goal:** Let authorized academic staff inspect expected learners, calculate complete term aggregates, review holds and lock immutable revisions from a readable workspace.

**Architecture:** Results owns the roster and aggregate services. A school-authorized roster joins the existing closure coverage to minimal Core student identity in one read transaction. The page loads only the selected student's preview/history, never one request per row. Existing aggregate policy/revision APIs remain authoritative.

**Tech Stack:** Rust/Axum/SQLx, generated OpenAPI, Svelte 5, local shadcn-svelte.

**Spec:** `../specs/2026-09-10-academic-term-lifecycle-design.md`.

## Constraints and design

- Require both school-level result and learner-evaluation access for the combined view; separate manage and lock capabilities. No teacher scope escalation and no unnecessary national ID/contact data.
- Empty/missing/stale coverage cannot be locked. A hold requires a reviewed policy and reason; it never hides missing locks. Do not calculate official arithmetic in JavaScript.
- Keep existing immutable revisions and original idempotency keys on identical retries. Stale preview requires reloading, not resubmitting automatically.
- Use existing Kanit headings/body and tabular numerals for credit/GPA values. Theme palette: blue `#005baa`, paper `#f8fafc`, ink `#172b3a`, border `#dbe4eb`, amber `#92400e`, red `#b91c1c` via existing tokens.
- Signature: a student ledger with explicit missing/current/stale statuses and an adjacent selected-student result sheet. No redundant score cards or repeated identity headers. On mobile stack the inspection sheet below the searchable ledger, with dialogs retaining visible close controls.

```text
สรุปผลรายภาค          selected policy / review policy
student search + status ledger | selected student
code / name / grade / status   | credits, GPA, missing inputs
                              | revision history / lock confirmation
```

Critique: a grid of GPA cards would obscure missing coverage. The ledger instead keeps learners without results visible and separates provisional averages from official locked GPA.

## Task 1: Results-owned roster API

Files: `results/models/aggregate_revision.rs`, new `results/services/aggregate_roster.rs`, `results/services.rs`, `results/handlers.rs`, `academic/results.rs`, `api_contract.rs`.

Interface:
```rust
pub async fn list_aggregate_students(pool: &PgPool, actor: &ActorContext, context: &ResultContext)
    -> Result<Vec<AggregateStudent>, AppError>;
// AggregateStudent: student_academic_year_id, student_code, student_name,
// grade_level_name, study_program_name, closure: TermClosureStudent.
```

- [ ] Write disposable service tests: missing revisions remain visible, exact context enforced, one-domain/teacher access denied, unrelated year students excluded. Observe RED with `scripts/test_backend_school.sh aggregate_roster -- --test-threads=1`.
- [ ] In repeatable-read read-only transaction, call `term_closure_coverage`, select minimal identities with `WHERE student_year.id=ANY($1)` and merge by ID. Return ordered code/name/ID rows. Never return missing-name fallback or issue a query per learner.
- [ ] Register `GET /api/academic/results/aggregate-students` with `ResultContext` query and typed envelope. Generate contracts; test API registration and focused service suite GREEN.

## Task 2: Generated wrappers and presentation

Files: new `frontend-school/src/lib/api/academicAggregates.ts`, `src/lib/academic/results/aggregate-presentation.ts`, `tests/static/academic-aggregate-presentation.test.mjs`.

- [ ] Consume generated operations for roster, policies, preview, history and lock. Use existing `apiClient` query/body handling and `requireApiData`.
- [ ] Test real helper behavior before implementation: missing revision versus stale/current; reason required only for holds; numeric zero display retained; lock denied for blockers and stale preview. Keep decimal strings untouched.
- [ ] Implement Thai blocker/hold labels and exact capability helpers matching backend policies. Run focused static tests.

## Task 3: Read-first route and explicit locks

Files: `staff/academic/results/aggregates/+page.ts`, `+page.svelte`, a focused selected-student component if needed, lifecycle readiness link, `tests/e2e/academic-term-aggregates.spec.ts`.

- [ ] Write browser tests against absent route, verify RED. Reader sees missing students without mutation controls; manager selects one student and preview policy; lock requires confirmation and preserves source checksum/revision/request ID; 409 retains intent and demands fresh preview.
- [ ] Add capability-guarded route metadata under existing Results. Load roster and policy list once per context; selection requests only preview/history for that student with abort/stale response protection. Do not require student-year administration permission to inspect authorized aggregates.
- [ ] Add reviewed policy dialog using existing policy API, named thresholds and explicit acknowledgement. No auto-created or auto-approved default policy.
- [ ] Show selected snapshot credits/grade points/provisional and official GPA, explicit blockers and holds, immutable history. Lock dialog requires reason when policy permits holds. Patch only selected student's current revision after success; refresh authoritative coverage for that row through roster, not all previews.
- [ ] Link lifecycle incomplete/hold findings to the aggregate page only when combined aggregate-read capabilities pass. Keep ordinary result navigation unchanged for teachers.
- [ ] Run Svelte autofixer, serial browser GREEN, lint/check/static, backend fmt/architecture/check/API tests and generated contract checks. Inline review before committing a coherent verified change.

Annual aggregation, promotion and future-term preparation remain separate coordinated tasks under the approved Release 3–4 specs; this page alone does not complete either release.
