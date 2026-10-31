# Assessment coordinator allocation version implementation

**Status:** Contract approved in chat; source work waits for Senior review of this plan.

**Design:** [Assessment coordinator allocation version](../specs/2026-10-31-assessment-coordinator-version-design.md)

## Ownership

The Senior owns Git, integration, candidate freezes, PR delivery and production. The Backend writer
owns the paths below and is the sole Cargo owner after source authorization. Frontend files already
owned by its writer remain protected. Independent Reviewer and QA assess the frozen candidate.

Backend implementation paths:

- `backend-school/migrations/150_assessment_coordinator_delivery_versions.sql`
- `backend-school/crates/school-academic-delivery/src/models/versions.rs`
- `backend-school/crates/school-academic-delivery/src/services/versions.rs`
- `backend-school/crates/school-academic-delivery/src/services/change_sets.rs`
- `backend-school/crates/school-academic-assessment/Cargo.toml`
- `backend-school/Cargo.lock` only for the existing workspace dependency edge
- `backend-school/crates/school-academic-assessment/src/assessment/services.rs`
- `backend-school/crates/school-academic-assessment/tests/persistence.rs`
- `backend-school/src/integration_tests/academic/services/assessment_publication_tests.rs`
- `backend-school/src/integration_tests/academic/services/assessment_service_tests.rs`
- `backend-school/src/integration_tests/academic/delivery/services_tests.rs`
- `backend-school/tests/static_architecture.rs` only for the Assessment dependency allowlist
- A focused Delivery provider or migration test under its existing test owner if needed
- `.rules` and `docs/TESTING.md` only to record the durable provider policy and focused verification

The Data specialist remains advisory unless the Senior explicitly reassigns migration ownership.
Keep the publication port, application adapter, HTTP state and public DTOs unchanged. Do not edit
applied migrations, generated contracts, dependencies unrelated to the workspace edge, or production.

## Steps

1. Add a regression fixture with a canonical published allocation whose dated teacher episode is
   outside today. Assert its stored owner remains a candidate across list/detail, save and exam
   eligibility; run the focused failing case before fixing runtime selection.
2. Add migration150's shared Delivery projection and replace exam coordinator eligibility with it.
   Assert no stored assessment, interval, publication or imported exam snapshot data changes.
3. Add the typed Delivery provider and Assessment dependency. Replace both current date-based
   candidate queries with provider consumption, preserving canonical names, active users, distinct
   group/assignment counts and the all-nonclosed-registry-group denominator. Update the save error.
4. Reorder initialization after both publication transitions in the same transaction. Verify first
   and later publications see their own selected graph, and an injected initialization failure
   rolls back statuses, episode changes and newly initialized assessment state.
5. Update only fixtures/expectations whose old today-based or no-allocation assumption contradicts
   the approved policy. Keep equivalent coverage for locks, permissions, concurrency and replay.
   Add latest replacement/omission/draft/no-version, incomplete groups, assignment identity and
   unique-full-coverage cases without tightening the existing subset-primary heuristic.
6. Record the durable provider policy and focused command in the existing canonical owners only if
   required. Review every changed path, then hand the candidate to Senior for independent review
   and QA. Remove these temporary workflow artifacts when the implementation PR records completion.

## Verification

Run one Cargo process at a time using the pinned toolchain and shared target. Database invocations
use the owning disposable Docker server/database runner; no shared tenant or production fixtures.
Select the final exact test filters after fixtures exist, preserving these coverage owners:

```bash
./scripts/test_backend_school.sh --package school-academic-assessment -- --test-threads=8
./scripts/test_backend_school.sh --package school-academic-delivery teacher_allocations -- --nocapture
./scripts/test_backend_school.sh integration_tests::academic::services::assessment_ -- --nocapture
./scripts/test_backend_school.sh integration_tests::academic::delivery::services_tests:: -- --nocapture
./scripts/test_backend_school.sh integration_tests::academic::services::exam_schedule -- --nocapture
```

Compile the changed owners and direct consumers without a workspace-wide rebuild unless the final
dependency inspection reveals wider uncertainty:

```bash
cd backend-school
cargo fmt --all -- --check
cargo check -p school-academic-assessment -p school-academic-delivery -p school-academic-http -p school-academic-lifecycle --all-targets
cargo test --test static_architecture
```

Run the repository documentation-policy check when `.rules` or canonical documentation changes.
The Senior reviews `git diff --check`, the final diff and `git status --short`. API and permission
generation are unnecessary unless the approved unchanged public contracts actually change. QA may
refine the selected checks from the final diff; do not repeat valid checks without changed inputs.
