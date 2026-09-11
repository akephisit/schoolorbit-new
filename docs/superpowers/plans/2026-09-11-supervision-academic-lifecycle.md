# Supervision Academic Lifecycle Implementation Plan

> Execute inline using `superpowers:executing-plans`. Run one test/build job at a time.

**Goal:** Make supervision source mutations respect academic closure in the same transaction as their records, evaluator responses and action history.

**Architecture:** Core owns transition/year/term locking. A private supervision helper resolves context before locking cycles and observations. Supervision keeps its existing permissions, workflow statuses, rubric rules and read projections; no second workflow or compatibility API is introduced.

**Tech Stack:** Rust, SQLx/PostgreSQL, generated OpenAPI and TypeScript artifacts.

**Spec:** `docs/superpowers/specs/2026-09-10-academic-term-lifecycle-design.md`.

## Constraints

- `closing` remains writable. Closed/cancelled terms and closed/archived years reject ordinary writes even with school management permission.
- Cycles intentionally support an optional term (migration 044). Observations always belong to an exact term and the cycle's year.
- Keep owner/assignment authorization before exposing mutation outcomes, and recheck mutable eligibility under the retained entity lock.
- Keep historical reports readable, global templates separate, submitted evaluators protected, and explicit action history atomic with the change it records.
- A writer must never acquire another pooled connection while retaining its transaction. Share connection-backed private readers between transactional writes and public read wrappers; keep the baseline flow tests on a one-connection pool so pool starvation cannot hide.
- Do not alter applied migrations, live records, roles, grants, grading behavior, or observation date rules unrelated to closure.

## Task 1: Cycle and observation context locks

Files: create `backend-school/src/modules/supervision/services/lifecycle.rs`; modify `services.rs`, `services/cycles.rs`, `services_tests.rs`.

Interfaces:

```rust
type SupervisionAcademicContext = (Uuid, Option<Uuid>);
async fn require_context_writes(tx: &mut Transaction<'_, Postgres>, contexts: &[SupervisionAcademicContext]) -> Result<(), AppError>;
async fn require_cycle_write(tx: &mut Transaction<'_, Postgres>, cycle: Uuid, target: Option<SupervisionAcademicContext>) -> Result<SupervisionAcademicContext, AppError>;
async fn require_observation_write(tx: &mut Transaction<'_, Postgres>, observation: Uuid) -> Result<(), AppError>;
```

- [x] Upgrade the disposable supervision fixture through migration 67 and use a writable canonical term rather than whichever historical term sorts first. Keep ordinary flow fixtures on one connection; use a separate multi-connection fixture only for lock-contention probes.
- [x] Add failing tests for cycle create/update under closed/archived years and closed/cancelled terms, including removing the term or moving to an open context. Assert unchanged cycle/targets. Preserve successful year-wide and closing-term cycles.
- [x] Resolve context without entity locks, acquire shared Core transition coordination and sorted/deduplicated year-exclusive locks before sorted term guards, then lock the cycle and recheck mutable context. A concurrent move returns conflict, never acquires a third context after the cycle lock.
- [x] `require_observation_write` resolves immutable observation year/term/cycle, guards both the cycle context and exact observation context through `require_cycle_write`, verifies year ownership, then locks the observation. Add focused pure ordering/deduplication tests in the new helper module and DB lock-order tests in service tests.
- [x] Cycle create acquires the requested context before insert/targets. Cycle update acquires both old/new contexts before merging against the current locked cycle. Reject moving an existing observation outside its cycle's year or selected term with a contextual conflict instead of a database failure. Whole-year cycles can still contain observations from multiple terms.
- [x] Run the focused cycle lifecycle tests before proceeding.

## Task 2: Observation mutations and status actions

Files: `services/observations.rs`, `services/reviews_and_reports.rs`, `services_tests.rs`.

- [x] Add closed-context tests for request, teacher update/cancel, manager update/cancel, approve-request and return-request. Use valid workflow states and requests so the lifecycle guard, not unrelated validation, is exercised; retain action/observation counts on denial.
- [x] New requests guard the cycle and requested term before loading the current booking/target/lesson context. Insert the observation and its action using the same transaction, commit, then hydrate the response.
- [x] Existing mutations authorize the observed teacher where required, acquire observation context locks, reload current state while locked, validate eligibility and lesson/evaluator references, write and commit before response hydration. Approval keeps evaluator replacement in that same transaction.
- [x] Add connection-backed cycle/observation detail readers and lesson/target resolution helpers. Public read functions acquire a connection and delegate; mutation callers pass their own transaction connection. Single-query helpers shared with pool readers may accept SQLx `Executor`; do not duplicate SQL or call a pool-backed detail reader while holding a write transaction.
- [x] Change `insert_observation_action` to accept the owner's `&mut Transaction<'_, Postgres>`; migrate every caller and remove post-commit action writes. No pool-writing compatibility overload remains.
- [x] Make `set_observation_status` own one guarded transaction. Introduce a private `ObservationTransitionPolicy` (`Manager`, `RequestedTeacher`, `Certification`, `ObservedTeacher`) so teacher ownership/request eligibility and complete required-evaluator checks run under the same lock as the status change. Callers keep their existing action labels and target statuses; no public DTO changes.
- [x] Test teacher cancellation racing approval cannot cancel a now-planned observation, and failure writing an action rolls back the observation/status mutation. Use disposable test-only failure injection; never runtime fault switches.

## Task 3: Evaluation submission and reviews

Files: `services/evaluations.rs`, `services/reviews_and_reports.rs`, `services_tests.rs`.

- [x] Add lifecycle denials for evaluator replacement/submission and certify/approve/acknowledge at valid workflow stages. Assert responses, submission state, observation state and actions remain unchanged; reject unassigned users independently of closure.
- [x] Evaluator replacement acquires the observation boundary before reading current submitted assignments. Preserve submitted evaluators and existing availability validation, and write its action before commit.
- [x] Submission verifies assignment, starts one guarded transaction, rechecks the current assignment, validates/deduplicates responses, writes responses and submitted status, calculates required-evaluator completion from that transaction, writes the observation status/action, then commits. Remove the private pool-writing save helper; it has no independent public endpoint. Keep existing empty-resubmission behavior and nonempty submitted-response rejection.
- [x] Use transaction-backed private evaluation readers/writers; where `load_evaluator_submission_states` is also used by read-only consumers, accept a SQLx executor so callers use either `&PgPool` or `&mut PgConnection` without duplicating SQL.
- [x] Extend the existing completed evaluation flow with closed-context checks between stages and successful continuation in `closing`. Add an action-insert failure regression proving no partial response/submission survives.

## Task 4: Contracts and integration

Files: `backend-school/src/modules/supervision/handlers.rs`, `backend-school/src/api_contract.rs`, generated contract artifacts.

- [x] Assert every term/year-owned supervision mutation documents a 409 `ApiErrorResponse`. Add only missing response declarations; preserve paths, request/response shapes, policy enforcement and result redaction.
- [x] Generate and check API artifacts and run generator tests. Run `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh supervision -- --test-threads=1` and the combined lifecycle regression suite serially.
- [x] Run backend formatting/architecture/check and API tests, then generated-frontend lint/check/static tests. Review lock order, same-transaction action history, cross-context denials, and all read-only paths. Commit on the current feature branch without deploying the unfinished release.

## Remaining release boundary

Year-level enrollment/deactivation writers still need their scoped audit. Lifecycle readiness, aggregate UI, term preparation/close/reopen/activation and Release 4 annual results/reviewed promotion remain separate work under the approved specs; supervision guards alone do not complete either release.
