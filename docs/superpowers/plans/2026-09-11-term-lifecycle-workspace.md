# Term Lifecycle Workspace Implementation Plan

> Execute inline with `superpowers:executing-plans`; no subagents and one verification job at a time.

**Goal:** Deliver a permission-filtered term lifecycle workspace with authoritative readiness, explicit transitions, immutable receipts and safe retries.

**Architecture:** Results supplies current aggregate coverage in the caller's transaction. Existing domain services supply operational findings. Lifecycle coordinates read models and checksums; Core alone changes term/year state. No copying of score cells and no automatic transitions by date.

**Tech Stack:** Rust/Axum/SQLx/PostgreSQL, generated permissions/OpenAPI, Svelte 5 and local shadcn-svelte.

**Spec:** `../specs/2026-09-10-academic-term-lifecycle-design.md`; year activation and downstream reopening dependencies follow `../specs/2026-09-10-academic-year-promotion-design.md`.

## Constraints

- `closing` remains writable; `closed` rejects ordinary writers, including admin management. Dedicated corrections remain possible and make a previous readiness checksum stale.
- Readiness and transition writes coordinate on the existing tenant transition advisory lock before resolving entity locks. Transition receipts and audit commit with state changes.
- A reviewed hold never conceals missing results or absent coverage. Latest revisions are checked against their pinned policies and current source inputs.
- Actual close date is required only for final closure and must lie within the owning year, at or after term start. Planned end remains optional.
- Ready/activation checks do not require score structures, exams or later club groups simply because those modules exist.
- No applied migration edits, compatibility aliases, default teacher transition grants, live tenant writes or shared deployment of an unfinished branch.

## Task 1: Results-owned closure coverage

**Files:** create `backend-school/src/modules/academic/results/services/closure_coverage.rs`; extend `results/models/aggregate_revision.rs`, `results/services.rs`, `results/services_tests.rs`.

**Interface:** `term_closure_coverage(tx: &mut Transaction<'_, Postgres>, context: &ResultContext) -> Result<TermClosureCoverage, AppError>`. Returns ordered expected student-year IDs, latest revision identities, missing/stale revision flags, reviewed holds and checksum. Internal caller owns school-level authorization and transaction isolation.

- [x] Add disposable tests proving an empty aggregate table cannot imply readiness, withdrawn students with retained results remain covered, latest source correction makes coverage stale, and a batch matches pinned-policy recalculation.
- [x] Observe RED through `scripts/test_backend_school.sh closure_coverage -- --test-threads=1`.
- [x] Select expected students from active/completed year enrollment plus term membership/results/revisions. Bound total students, read latest revisions in one query, group by pinned policy, calculate in chunks of at most 500 through `aggregate_students_in_transaction`. Never call the public per-student pool reader inside a transaction.
- [x] Preserve explicit missing/stale/hold identity in the checksum; incomplete or empty coverage cannot report ready. Run focused Results tests.

## Task 2: Readiness providers and orchestration

**Files:** new `academic/lifecycle.rs`, `academic/lifecycle/models.rs`, `academic/lifecycle/services.rs` and focused child services/tests; narrow provider functions in Delivery, timetable, exams and Supervision owners.

- [ ] Add tests for no configured module work, unresolved draft delivery changes, existing timetable/exam drafts and incomplete supervision observations, deterministic finding order and checksum changes when any consequential input changes.
- [ ] Providers return typed counts/version fingerprints in the supplied transaction. Delivery changes affecting results block closure; unfinished timetable/exam/supervision work produces explained warnings, not mandatory invented setup tasks.
- [ ] Assemble `TermLifecycleWorkspace` with context versions, capabilities, stable finding codes, counts and authorized context-preserving links. Use Results coverage unchanged; no duplicated result arithmetic.

## Task 3: Core state transitions and persistent receipts

**Files:** new sequential lifecycle migration; `core/services/term_transitions.rs`; typed lifecycle request/outcome models; integration tests using disposable migrated fixtures.

- [ ] RED tests cover active→closing, closing→active cancellation, planning→ready, planning→cancelled, closing→closed and closed→closing reopening; every invalid predecessor returns conflict without changing rows.
- [ ] Validate positive expected versions, UUID request identity, exact request checksum, close date and required reopening reason. Acquire exclusive transition lock first, then sorted year/term locks and recheck readiness in that transaction.
- [ ] Store immutable actor-bound request receipt, input checksum, before/after context versions and accepted readiness snapshot with the Core audit in one transaction. Same request replays the original outcome; changed input conflicts. Failure rolls back receipt/audit/state together.
- [ ] Final closure blocks absent/stale aggregate coverage and requires acknowledgement of exactly the current warning codes/checksum. Reopening rejects closed years and activated successors; annual/promotion dependency checks connect to the Release 4 owners before those operations become available.
- [ ] Same-year term activation requires ready target, closed non-cancelled predecessors and no other active/closing term. First-ever and future-year activation use the atomic Core year/term command from the Release 4 plan, not a fallback status UPDATE.

## Task 4: Permissions, API and workspace

**UI direction:** A compact Thai academic-office checklist, not a dashboard of decorative metric cards. Reuse the existing Kanit typography (semibold headings, regular explanation) and tabular/monospace numerals only for counts. Existing theme tokens remain authoritative: school blue (approximately `#005baa`), paper `#f8fafc`, ink `#172b3a`, quiet border `#dbe4eb`, warning amber `#92400e`, blocking red `#b91c1c`. The signature is the real term-state sequence above a single readiness ledger; actions sit beside it on desktop and below it on mobile. Confirmation dialogs retain the visible standard close control. No new fonts, gradients, mandatory setup progress meter, or repeated teacher/subject identity cards.

```text
selected year / term                         refresh
planning → ready → active → closing → closed
readiness ledger                    allowed actions
reason / count / resolution link     explicit confirmation
```

Critique: a generic statistics-card header would hide the actual work. Replace it with the state strip and readable reasons; keep the selected term and current running term distinct. Any stale conflict retains the confirmation context and requires a fresh review, not an automatic resubmission.

**Files:** `contracts/permissions.json`, forward permission seed, generated registries; lifecycle handlers/routes and `api_contract.rs`; `frontend-school/src/lib/api/academic-lifecycle.ts`; staff lifecycle route metadata and Svelte components.

- [ ] Add generated school capabilities read/manage/close/reopen/activate; grant defaults only to verified system administrators. Backend permission denial tests must show teachers and subject-group readers cannot transition or read school-wide learner summaries.
- [ ] Register typed camel-case workspace and transition APIs with 400/401/403/404/409 responses and shared envelopes. Generate and consume DTOs; no transport casts.
- [ ] Build the approved `ปิดและเปลี่ยนภาคเรียน` workspace with status/header, readiness counts, result links and capability-gated confirmation dialogs. Keep view-context selection separate from activation. Patch successful response then refresh authoritative context choices; show conflicts without discarding the user's intent.
- [ ] Add browser flow tests for reader versus manager, blockers, warning acknowledgement, stale conflict, close date validation and explicit activation; mocked browser tests are not live sandbox evidence.

## Verification and remaining release work

- [ ] Run focused DB tests RED→GREEN, concurrent writer/transition tests, full relevant Results/Core/Lifecycle suites, Rust fmt/architecture/check/API tests, generated permission/API checks and tests, frontend lint/type/static checks, and serial Playwright tests.
- [ ] Inline review final diff, authorization, lock order, immutable receipts and cross-context rejection. Commit only verified coherent changes on `feature/academic-lifecycle-release-3-4`.

Future-term preview/apply with explicit mappings, the aggregate UI, annual snapshots, reviewed promotion runs and atomic year activation are subsequent coordinated tasks under the approved Release 3–4 specs. Neither release is complete until those tasks and their workflow coverage are finished. Live verification additionally requires an authorized isolated runtime or an approved shared release of the complete build.
