# Academic assignment IDs implementation

**Status:** Contract approved in chat; source work awaits Senior review of this written plan.

**Design:** [Academic responsibility from published allocation IDs](../specs/2026-10-31-academic-assignment-ids-design.md)

## Ownership and order

The Senior owns Git, shared contracts, checkpoints, PR/integration, independent acceptance and
production. The Backend writer owns Delivery and Assessment below. A separate Results writer may
begin only after the provider contract is implemented and handed over. Frontend confirmation-context
work has separate owners and must finish its writing phase before the two Backend writers start.
One designated owner runs Cargo at a time on the shared target and disposable Docker fixture.

Backend Delivery/Assessment writer paths:

- `backend-school/crates/school-academic-delivery/src/models/versions.rs`
- `backend-school/crates/school-academic-delivery/src/services/versions.rs`
- Delivery provider tests in that owner, or its focused crate-owned persistence test
- `backend-school/crates/school-academic-assessment/src/gradebook/services.rs`
- `backend-school/crates/school-academic-assessment/src/gradebook/services/workspace.rs`
- `backend-school/crates/school-academic-assessment/src/learner_evaluation/services.rs`
- `backend-school/crates/school-academic-assessment/src/learner_evaluation/services/entry.rs`
- `backend-school/crates/school-academic-assessment/src/learner_evaluation/services/confirmation.rs`
- `backend-school/crates/school-academic-assessment/src/learner_evaluation/services/locking.rs`
- `backend-school/crates/school-academic-assessment/src/policy/learner_evaluation.rs`
- `backend-school/crates/school-academic-assessment/src/learner_evaluation/services/summary.rs` if its authorization/snapshot composition requires adjustment
- `backend-school/crates/school-academic-assessment/src/policy/assessment.rs`
- `backend-school/crates/school-academic-assessment/src/assessment/services.rs` only assigned/instructor filtering and shared provider consumption; preserve automatic suggestion
- `backend-school/crates/school-academic-assessment/tests/persistence.rs`
- `backend-school/src/integration_tests/academic/gradebook/services_tests.rs`
- `backend-school/src/integration_tests/academic/learner_evaluation/services_tests.rs`
- `backend-school/src/integration_tests/academic/learner_evaluation/current_obligation_tests.rs`
- `backend-school/src/integration_tests/academic/services/assessment_service_tests.rs`
- `backend-school/tests/static_architecture.rs` for the durable provider boundary
- `.rules`, `docs/TESTING.md`, and only a bounded `docs/OPERATIONS.md` policy/recovery note if needed
- These two temporary workflow documents until their recorded-outcome cleanup

Results writer paths:

- `backend-school/crates/school-academic-results/src/services/course_preparation.rs`
- `backend-school/crates/school-academic-results/src/services/activities.rs`
- `backend-school/crates/school-academic-results/src/services/readiness.rs`
- `backend-school/crates/school-academic-results/src/services/locking.rs`
- `backend-school/crates/school-academic-results/src/services/effective_results.rs`
- `backend-school/crates/school-academic-results/src/policy/results.rs` only responsibility wording or matching role tests
- `backend-school/crates/school-academic-results/tests/persistence.rs`
- `backend-school/src/integration_tests/academic/results/services_tests.rs`
- `backend-school/src/integration_tests/academic/results/result_obligations_tests.rs`
- Additional existing Results fixture consumers only after reporting the exact dependent path to Senior

Protect all applied migrations, manifests/lockfiles, public DTOs, generated contracts, unrelated
catalog/organization/Supervision/date owners and the other writer's files. Result obligation provider
and application adapter signatures remain date-bearing and unchanged unless a separately evidenced
contract need is approved.

## Implementation

1. Add focused red regressions demonstrating date-expired/future teachers present in the latest
   graph and current live teachers absent from it. Record actual before/after evidence.
2. Implement and test the two narrow Delivery operations and typed availability/role sets. Cover
   active users, all exact IDs, future/draft/no-source/empty/omitted graphs and two-primary sets.
   Hand this stable interface to the Senior before the Results writer starts dependent work.
3. Replace Assessment/Gradebook/Learner teacher episode queries with provider IDs, preserving list
   scope filters before limits, existing coordinator grants and all locks. Distinguish absent source
   from a known negative before read-time confirmation invalidation. Add private Gradebook confirmer
   hydration without changing its public DTO.
4. In parallel after handoff, replace Results course/activity/history/readiness/lock teacher checks,
   including operation-date-based checks, with the same provider. Keep dated obligation selection,
   authoritative student rosters and official snapshots unchanged. Validate all confirmation layers
   against primary ID membership, including Gradebook phase confirmations.
5. Correct only fixtures whose direct-episode or today-based assumptions conflict with the approved
   graph policy. Retain equivalent tests for permissions, coordinator-only access, source staleness,
   missing-source read preservation, already-invalid state, dated audiences, history and locks.
6. Add a focused architecture guard against restoring consumer episode/date/snapshot selection,
   update canonical policy/test references, reconcile both owners, and freeze for independent Reviewer
   and QA. Remove these workflow inputs after the PR records the implemented outcome.

## Verification

Use pinned Rust, the existing shared target and one disposable PostgreSQL server with a fresh owned
database per invocation. Record actual commands and candidate inputs. Focused cases precede broader
owner runs; avoid repeating still-valid evidence merely to checkpoint.

```bash
./scripts/test_backend_school.sh --package school-academic-delivery teacher_assignments -- --nocapture
./scripts/test_backend_school.sh --package school-academic-assessment -- --test-threads=8
./scripts/test_backend_school.sh --package school-academic-results -- --test-threads=8
./scripts/test_backend_school.sh integration_tests::academic::gradebook -- --test-threads=4
./scripts/test_backend_school.sh integration_tests::academic::learner_evaluation -- --test-threads=4
./scripts/test_backend_school.sh integration_tests::academic::results -- --test-threads=4
./scripts/test_backend_school.sh integration_tests::academic::services::assessment_ -- --test-threads=4
```

From `backend-school`, run formatter verification, scoped production panic Clippy for the affected
owners/consumers in default and `api-docs` configurations, compile affected owners and HTTP/Lifecycle
consumers, and run `static_architecture` plus `database_execution_architecture`. Broaden workspace
coverage only if final dependency analysis or failures establish a need. Keep all chosen checks
required, including denied/history/transaction paths. Run the documentation-policy check for the
canonical updates. Senior reviews the final diff, `git diff --check` and `git status --short`.

Frontend's owner/QA selects its helper/type/browser checks. No API/permission regeneration is needed
while public contracts remain unchanged. Data analysis remains read-only except for explicitly
assigned disposable test fixtures; no production repair, migration edit or historical backfill is
part of this change.
