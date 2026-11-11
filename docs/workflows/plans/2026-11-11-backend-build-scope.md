# Backend build-scope implementation plan

Date: 2026-11-11

Design: [Backend build scope](../specs/2026-11-11-backend-build-scope-design.md).

Baseline: `2dd8a55675de48d7a24dfad2419b246238ccf6e2`, shared checkout `/workspace/schoolorbit-new`, branch `perf/academic-http-build-scope`.

## Working agreement and owners

The Senior owns design decisions, shared Git state, integration and final acceptance. Backend owns `backend-school/**`, only affected ownership/path guards under `frontend-school/tests/static/**`, backend ownership references in `.rules`, `backend-school/README.md`, `docs/TESTING.md`, and these two workflow artifacts. Pipeline exclusively owns `scripts/**`, `.github/**` and `docs/OPERATIONS.md`. Backend owns workspace manifests and the lockfile source decision; the designated Cargo owner generates the lockfile only after graph changes freeze. No incidental version upgrades are allowed.

Pipeline initially owns all heavy Cargo/compiler commands and before/after measurements. Backend runs no compiler concurrently and requests explicit handoff before focused Cargo work. Reviewer is read-only and reviews the design before implementation and the frozen integrated candidate afterward. Independent QA selects affected verification, runs it only against stable inputs with the assigned build/fixture ownership, and reports failures to the Senior. Writers acknowledge checkpoint pauses before the Senior stages or commits.

The protocol remains owned by the dedicated narrow Academic realtime contract. Each transport crate owns its handlers and concrete state; Core and Delivery own their resource policies. The application package owns state projections, middleware and OpenAPI composition. Preserve the existing data/error/permission/state flow and one shared collaboration manager; no facade, aggregate Academic state, sibling Academic HTTP edge or runtime compilation workaround is permitted.

## Ordered implementation

1. Write this plan and the concrete design, trace direct consumers and state-field use, and submit them to Senior and Reviewer. Wait for the Senior's pre-code design ACK. Pipeline may collect baseline evidence while source is unchanged.
2. Coordinate package/source probes with Pipeline. Record baseline OpenAPI and generated-contract identity and the before-build inputs. Do not move source before the baseline owner releases the source freeze.
3. First make `QueryObservation::complete` nongeneric with `Option<&sqlx::Error>` and a non-inlined boundary; keep typed results and `observed<T>`. Preserve the existing meaningful success, error, cancellation and safe-static-observation tests. Save this bounded implementation checkpoint before the large ownership move; check status remains explicit if compilation has not yet run.
4. Move current Core policies into `school-academic-core::policies` and offering policy into `school-academic-delivery::policies`. Add Delivery's typed `require_term_change_set_access(pool, actor, id, action)` provider operation. Update consumers, preserving original SQL labels and permission/error behavior. Move colocated policy tests with their owner.
5. Add `school-academic-realtime` with the current protocol and port and only its three production dependencies. Add serialization/round-trip and mutation-classification tests for this bounded protocol-owner admission exception. Move the six cohesive HTTP groups into their new crates; move mutation/socket/manager machinery to Timetable HTTP. Add narrow concrete state modules and nongeneric `#[inline(never)]` route factories. Distribute personal/public routes to their actual owners. Retire the old HTTP crate completely.
6. Update workspace/root manifests and feature forwarding, root `AppState` imports, six state projections, socket projection, facility adapter, concrete router composition and every OpenAPI handler reference. Keep the original route methods, prefixes, question-bank nesting and middleware order. Update fixtures and source-path guards. Ask the Cargo owner to generate the lockfile after graph freeze, then inspect it for version stability.
7. Extend backend architecture guards to inventory every moved factory/route, verify nongeneric concrete signatures and application state binding before middleware, enforce allowed dependency directions and prevent the old owner/aggregate state from returning. Update only affected frontend path/ownership guards with equivalent-or-stronger assertions.
8. Update `.rules`, backend README and Testing references to the new transport, policy and socket owners. Pipeline updates its package/source paths and evidenced release overhead independently. Inspect every owned diff and coordinate a complete source freeze with the Senior.
9. Run focused checks through the designated build owner, repair source failures in the assigned owner, then submit the frozen candidate for independent Reviewer and QA acceptance. Re-run affected checks after repairs; do not repeat valid unrelated checks. Record actual commands, environment, candidate and limitations in chat/PR, not a new status file.

## Verification commands and scope

The following commands are planned checks, not passed evidence. Cargo commands run from `backend-school` unless a repository-root wrapper is shown. The current build owner executes them sequentially. QA may refine filters after assessing actual impact, while preserving all affected contracts.

Workspace, formatting and architecture:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo check --workspace --all-targets --features api-docs --locked
cargo test --test static_architecture --locked
cargo test --test database_execution_architecture --locked
cargo clippy --workspace --lib --bins --exclude school-test-db --locked --no-deps -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic -D clippy::unreachable
cargo clippy --workspace --lib --bins --exclude school-test-db --locked --no-deps --features api-docs -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic -D clippy::unreachable
```

The production panic-policy Clippy commands above match the current owning runner in `scripts/pipeline.mjs`; preserve its production scope and stable toolchain flags. All newly moved transport owners and policy/database owners receive focused unit coverage:

```bash
cargo test -p school-academic-core-http --locked
cargo test -p school-academic-delivery-http --locked
cargo test -p school-academic-assessment-http --locked
cargo test -p school-academic-timetable-http --lib --locked
cargo test -p school-academic-results-http --locked
cargo test -p school-academic-lifecycle-http --locked
cargo test -p school-academic-realtime --locked
cargo test -p school-academic-core policies --locked
cargo test -p school-academic-delivery policies --locked
cargo test -p school-database --locked
cargo test --bin backend-school --features api-docs api_contract::tests --locked -- --nocapture
```

Timetable HTTP's colocated socket/security unit tests use memory and channels, with a lazy invalid pool that performs no database work. Run them once with the raw package unit batch above. Actual application session/native-socket/collaboration integration fixtures use the disposable database wrapper rather than an existing school database. At repository root, the focused contract/lifecycle checks include:

```bash
./scripts/test_backend_school.sh concrete_academic_routes_preserve_session_tenant_csrf_and_permission_boundaries --locked -- --nocapture
./scripts/test_backend_school.sh integration_tests::database --locked -- --test-threads=1
./scripts/test_backend_school.sh integration_tests::academic::services::timetable_block_service_tests --locked -- --test-threads=4
./scripts/test_backend_school.sh --package school-academic-delivery teacher_assignments --locked -- --test-threads=1
./scripts/pipeline verify --scope documentation
git diff --check
git status --short
```

Do not repeat the Timetable HTTP unit batch through a database wrapper or repeat its socket/security subset. Inspect the current transport fixture and add/retain assertions for every distributed management, public/personal and socket boundary. Run additional existing Core, Delivery, Assessment/Gradebook/Learner Evaluation, Timetable, Results and Lifecycle fixtures when the move touches their provider/authorization registration. Use isolated disposable databases and at most the documented thread count. Session/frontend static tests are chosen from the actual changed source guards and the existing auth/session contract suite; browser workflow execution is necessary only if independent assessment finds changed browser behavior.

Export baseline and candidate School OpenAPI using the tracked exporter in the same source path, with `api-docs` enabled outside timed production builds. Compare canonical documents exactly. Run the existing generated API/permission contract checks and verify tracked generated/UI/schema files are unchanged. Do not regenerate or bless changed outputs to hide a contract regression.

Compile evidence is owned by Pipeline: unchanged/no-op reuse; source-changing application case; each old/new Academic HTTP domain case; domain-service fan-out; shared realtime-contract fan-out. Use one toolchain, source path, dependency state, target, production profile and concurrency, with separate graph priming. Record rebuilt package lists, Cargo timing/link evidence and binary identity/size. Preserve `opt-level = 3`, existing codegen units and all production defaults. No performance claim is accepted without this paired evidence and its limits.

Pipeline independently runs the matching script/workflow regression suite and format/lint checks for its release changes, and retains every required build/provenance/deployment gate. Senior reconciles that evidence with backend checks before final acceptance.

## Test-change rationale

Runtime expectations and snapshots remain unchanged. File-path and owner assertions change because the approved design retires one transport owner and creates six real owners. Guards must still inspect executed implementations and complete registered routes, and must add protections for the new dependency/state boundaries. Policy fixtures follow the policies into domain owners, preserving grants, denials, tenant isolation, query labels and database-free paths. Any additional test correction needs a concrete old/new requirement and independent review.
