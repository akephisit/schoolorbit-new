# Testing

Use focused tests while implementing, then run every applicable section below. Commands that need databases, credentials, browsers, or deployed services must be reported explicitly when the required environment is unavailable; do not present an unrun check as passing.

## Reporting Verification

Record:

- the exact command that ran;
- whether it passed, failed, or was skipped;
- the relevant failure when it did not pass;
- the missing environment variable or external dependency when it could not run.

Do not replace a failed check by disabling it or by running a narrower command that misses the failure.

## Local verification and GitHub delivery

All GitHub Node setup steps read the root `.node-version` pin. Keep planning, preparation,
deployment tooling and release preflight on that exact full version: frontend artifact hashes
include `process.versions.node`, so mixing cached patch versions must fail preflight. Verify a pin
change with the deployment workflow-consumer guard and pipeline policy fixtures; retain the full
version in artifact/tooling identities. Do not repair mismatches by dropping it from the hash.

Before final acceptance, readiness and integration, the Senior fetches `origin` and checks
`git merge-base --is-ancestor origin/main HEAD`; updating local `main` alone is insufficient.
If behind/diverged, pause/save writers and require a clean tree before the Senior merges
`origin/main`; a delegated writer resolves semantic conflicts. Inspect the integrated result,
repeat affected checks/review, push and re-establish the latest-main `Pipeline gate`. Repeat
if another feature merges; do not blindly update the branch on GitHub or bypass protection.
Select checks using [the verification scope policy](../.rules#11-verification-matrix) before opening or
updating a ready PR. The commands below are recipes for the affected owners and workflows.
Open a draft after the first meaningful commit and use checkpoint commits to preserve progress
under [the Git workflow](../.rules#git-branches-worktrees-and-integration). Drafts may contain
incomplete or unverified work with accurate status; saving a checkpoint does not require rerunning
the full matrix. Before ready status, complete the applicable checks and independent review.
In a shared checkout, the Senior pauses all writers/tools before staging a checkpoint and resumes
them after the push; the final review/QA candidate remains frozen until both reports finish.
`./scripts/pipeline verify --scope documentation` selects the documentation suite; a comma-separated
list such as `contracts,frontend-school` selects whole owner suites. Use the focused package,
service/filter and browser commands below for bounded work. `--scope auto` conservatively selects
whole application suites from changed paths, including uncommitted/untracked files; `--scope full`
runs every suite. Choose these broad modes only when the policy calls for their coverage.
Install the exact Node version in [`.node-version`](../.node-version), pinned Rust, Docker, and the
owning application's locked npm dependencies. Deployment/static checks need ShellCheck, shfmt,
Bats, envsubst and jq. Local test profiles use no debug information or Cargo incremental data;
production release optimization is unchanged. Keep incomplete or failing work in a draft PR.

School performs workspace checking, architecture/unit checks, generated OpenAPI and clean-environment
export, then database consumers with one Docker PostgreSQL server and a fresh owned database per
invocation. Admin applies migrations only to its disposable Docker fixture before SQLx all-target
compilation and tests. Both backend suites also run production library/binary Clippy with
`--no-deps` and deny only `clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic` and
`clippy::unreachable`. School checks default and `api-docs` workspace targets, excluding only
`school-test-db`; Admin runs against its owned disposable SQLx fixture. Test targets remain
outside this guard so assertions and intentional failure fixtures remain usable. Do not replace
this focused gate with `-D warnings` or suppress it to accept a runtime panic. Startup subprocess
regressions use isolated empty configuration and synthetic invalid values to verify nonzero exits,
actionable sanitized diagnostics and the offline export boundary without external service access.
Run `node --test scripts/tests/pipeline-local.test.mjs` to verify that all three Clippy invocations
execute and their failures stop local verification. Frontend suites run lint, type checks and actual contract/unit tests.
Auth/session Playwright discovery uses synthetic values with `--list` and loopback URLs only;
it performs no browser or network execution. Run affected browser workflows locally as required
by the change-type matrix; discovery alone is not browser coverage.

GitHub runs `plan → selected build → Pipeline gate → selected deployment → deployed acceptance`.
PRs only plan their scope and require an up-to-date base. Main builds selected immutable artifacts
using release caches, then deploys under the production lock. GitHub does not run unit/database/
architecture/lint/type/E2E suites, receive local results or refresh compiled test snapshots.
Record local commands and results in the PR for the team; Pipeline does not certify those results.
If main advances, update the branch yourself and rerun affected local checks before making it ready.
Production readiness, migration audits, browser mount/assets and authenticated smoke still run
while both APIs remain in Maintenance Mode; users are admitted only after acceptance succeeds.

Run `node --test scripts/tests/pipeline*.test.mjs` for planner, artifact, provenance, merge and
maintenance failure cases, plus `node --test frontend-school/tests/static/school-release-deployment.test.mjs`
for workflow ownership and gates. Production acceptance remains a separate deployed-proxy and
browser verification after integration. Caches never replace a required test result.
For a bounded change to the team merge controller, run `node --test scripts/tests/pipeline-github.test.mjs`
and the documentation-policy suite. Its fake GitHub API verifies that the eligible PR is merged
with `merge_method: merge` and the exact gated head, dispatches the main Pipeline, and rejects stale
or behind branches without mutating their source. Repository method/protection settings and the
first-controller transition require the separate checks in [operations](./OPERATIONS.md).

For backend network changes, run `bats scripts/tests/installer/network_remote.bats`
and `node --test scripts/tests/pipeline-network.test.mjs` with the local Docker engine.
The first covers missing/stale aliases, DNS delay and unavailable probes without any
network mutations. The Docker fixture verifies that successful validation and refused
configuration drift preserve the same open TCP connection and container address.
Production acceptance also checks aliases and socket source addresses after startup.

For deployment dependency selection and compiler evidence, run
`node --test scripts/tests/pipeline-deployment-tooling.test.mjs scripts/tests/pipeline-build-evidence.test.mjs`.
Runtime observations use the stdlib Python collector and isolated Node contract fixtures:

```bash
node --test scripts/tests/pipeline-runtime-observations.test.mjs \
  frontend-school/tests/static/deployment-installer.test.mjs \
  frontend-school/tests/static/documentation-policy.test.mjs
actionlint .github/workflows/runtime-diagnostics.yml .github/workflows/operations.yml
```

The fixtures verify malicious-input confidentiality, JSON and actual tracing fmt
shapes, wrong types/nonfinite values, exact nearest-rank edge cases at 1/19/20/99/100
samples, sparse-tail nulls, SQLx seconds-to-milliseconds conversion, separate proxy
and backend stages, missing-data semantics, command/time/byte/line/group bounds,
and Podman 5.4.2 resource fields. Tenant transport fixtures exercise both `snwsb`
and `sandbox`, checking the fixed trusted HTTPS API and matching Origin/tenant
header on every request, including login/logout. Invalid/missing labels,
infrastructure names and wrong API hosts fail before transport construction;
dedicated `RUNTIME_PROBE_*` credentials have no smoke/E2E fallback. Fixtures retain
active staff identity, existing active-year selection, one warmup
plus five reads, response/request bounds, redirect refusal, early CSRF capture,
partial endpoint results and own-session cleanup after malformed/oversized login
bodies. Student-read fixtures defer authorization to the backend response:
wildcard, exact scoped grants and absent/unrecognized identity permission metadata
cannot suppress an API-authorized read. A backend 403 wins even when metadata
claims access, stops after one request and retains own-session cleanup. This
replaces the probe's faulty exact-list preflight without changing backend policy.
Fixtures use synthetic secrets and mocked transport only; they do not measure
production latency or database plans.
Artifact transport fixtures execute the actual workflow shell steps with mock
SSH/keyscan commands and actual producer report objects. They preserve original
numeric JSON bytes, including benign sequences that can collide with log masking;
reject masked/malformed/duplicate/extra/empty/oversized or failed output; check
host/user/port validation, private key permissions and cleanup; and ensure that
only the two explicitly validated aggregate files can be uploaded. The previous
logs-only channel lost numeric evidence through Actions masking, so retaining
validated aggregates is required while raw logs, key material and private stderr
remain excluded. A successful workflow or partial report does not establish
missing dashboard context or successful student API authorization.
Workflow guards require main-only `run_tenant_probe` opt-in and an explicit
`probe_subdomain`; the canonical CLI and validated artifact are `--tenant-probe`
and `tenant-probe.json`. Retired sandbox-only names and identifying artifact
fields are rejected. Operational dispatch and interpretation belong in
[Operations](./OPERATIONS.md). The selected-tenant API sample and process-wide
runtime distributions represent different populations. Frontend timing requires
the deployed browser recipe below and locally supplied E2E credentials.

Compiler-service diagnostics use `node --test scripts/tests/pipeline-compiler-cache-diagnostic.test.mjs`
locally, then Operations `compiler-cache-diagnostic` on the reviewed branch. Inspect its bounded
evidence for a successful new write followed by a hit in a fresh container; raw daemon logs must
never enter Actions logs, artifacts or cache layers.
Dependency diagnostics also require the canonical build's JSON write-settlement metric to be
true in both containers. Local evidence fixtures cover delayed writes, empty and invalid counters,
and successful-write counts missing from the human-readable CLI.
Cache-backend experiment fixtures run with
`node --test scripts/tests/pipeline-cache-benchmark.test.mjs`. They exercise paired scratch
contexts, fresh builder ownership, cleanup on failure, frozen local compiler snapshots, read-only counters, missing metrics
and complete counterbalanced sampling. Dispatch the isolated remote comparison through Operations
`cache-benchmark` with `options={"samples":"2"}` on the reviewed experiment branch. Both variants
must retain the runtime UID/command and canonical offline OpenAPI; inspect full build wall time,
dependency-layer reuse, cache export and error counters before changing production cache policy.
Scope fixtures in `scripts/tests/pipeline-policy.test.mjs` cover recipe/report separation,
component deployment ownership and queued baseline reconciliation. The School release source guards
exercise parallel branch failure joins, backend prerequisite refusal, pinned runner compatibility and
Admin's browser tooling subset. Compiler evidence fixtures verify argument/failure propagation,
executable versus linker duration and API-readable BuildKit phases.
`scripts/tests/pipeline-github.test.mjs` rehearses trusted accepted-state advancement between planning
and preflight, preservation of manual scope, and refusal of changed or unprepared inputs.
The fixtures exercise nested/peer dependencies, stale and partial caches, workspace-link exclusion,
bounded summaries and compiler failure propagation. Rehearse a clean install of the extracted lock
outside the checkout and launch its locked Wrangler/browser tools before shipping cache changes.
School build fixtures exercise both dependency cooking and application compilation with compiler
caching disabled, local caching, GitHub caching and missing cache credentials. Compiler failures
must propagate even when optional cache statistics fail. Dockerfile changes also require the
runtime-target build and image-config inspection described below; cache-hit counts alone do not
prove the release binary or its API export is correct.
Compare repeated installs and restores on one toolchain/host; setup-only measurements are not total
pipeline or production latency measurements.

## Every Change

The repository-owned [development skill](../.agents/skills/schoolorbit-development/SKILL.md)
is discovered from `.agents/skills/` by supported Codex hosts. Ordinary SchoolOrbit work requests
automatically use it through `AGENTS.md` and [the activation policy](../.rules#automatic-workflow-activation);
typing `$schoolorbit-development` is an optional shortcut, not a prerequisite. If a host does not
surface the skill, the agent reads its local file and follows the workflow with available tools.
Bounded delegation follows [the coordination rules](../.rules#agent-roles-and-coordination).
Every implementation, however small, has a Senior, delegated writer, separate read-only Reviewer
and separate QA/Verifier, including text, documentation, workflow and tests. The Senior reads,
analyzes, diagnoses and owns Git without writing tracked source/configuration/tests/docs or taking
over repairs. Scale checks and model effort to impact; trivial text needs no pointless tests.
Unavailable subagents are a concrete blocker, not inline independent roles. Explicit newer user
instructions and higher-priority host requirements still apply. Questions do not authorize
implementation or PR creation, and read-only work needs no writer. Children retain assigned roles.

The primary session is the Senior/Coordinator. A full-stack task normally uses Frontend and
Backend implementation roles, then independent Reviewer and QA/Verifier roles for the integrated
candidate. Use Senior + one relevant writer for a single-owner change, then Senior + Reviewer +
QA; no irrelevant Backend writer or simultaneous full roster is required.
Model/reasoning defaults and escalation belong to the coordination rules above; use
the host's real spawn controls and report requested/confirmed settings or fallbacks. The skill
cannot switch the primary session's model or enable unsupported model overrides.

One coordinated feature team normally shares one feature branch/checkout with non-overlapping
file ownership. The Senior alone manages its Git state and serializes shared generators and
dependency changes. Reuse the existing compatible Rust target with one Cargo owner at a time;
never create per-agent targets/caches merely to parallelize Rust verification. Concurrent distinct
features need distinct branches/checkouts; a single checkout cannot hold two branches simultaneously.
Use worktrees for actual concurrent editing needs, or serialize feature-writing phases when one
checkout is requested. A single feature team shares its branch and assigned files.
Freeze all writers before final Reviewer/QA checks. Isolation exceptions and resource limits belong
to the coordination rules above.

For a skill-instruction-only change, run
`node --test frontend-school/tests/static/documentation-policy.test.mjs`. If executable behavior
also changes, select its checks under `.rules` section 11. Verify the skill appears in the
target host's skill selector after checkout refresh; Codex detects skill changes automatically,
and restarting it is the documented fallback when an update does not appear. File/metadata and
instruction-policy checks prove packaging and the presence/consistency of required instructions;
they do not prove live discovery, model selection, agent isolation or autonomous completion.

For workflow changes, have the independent Reviewer inspect the complete candidate against
[reports, recovery and completion](../.rules#reports-failure-recovery-and-completion), and QA run
the owning documentation-policy suite. Inspect actual task handoffs and delivery results separately.
When exercising a feature workflow, check that every implementation uses a delegated writer and
separate review/QA; child failures reach the Senior for diagnosis and return to the writer without
Senior/QA source edits; a bounded pre-existing required failure
is diagnosed and repaired; and ready status is followed through automatic merge and selected
acceptance. A genuine access/scope blocker should produce an explicit blocked handoff. Verify the
Senior's final chat/PR summary includes the outcome, roles/settings, encountered problems and
resolutions, actual checks, delivery status and limitations. Static checks alone do not establish
that these scenarios ran; do not invent failures or production changes just to rehearse them.

For changes to existing tests, follow [test integrity](../.rules#test-integrity-and-legitimate-corrections).
Record the old/new requirement, reason and reproducer/before-after evidence where feasible; an
approved workflow-policy change can legitimately replace an obsolete guard. Retain equivalent-or-
stronger coverage. The Reviewer independently assesses that rationale; QA checks actual outcomes
and regression detection. An unchanged-behavior regression needs a production-cause fix, while an
evidenced faulty/brittle test or environment/fixture defect may need a justified test correction.
Never weaken coverage, skip required checks or blindly bless snapshots merely to get green.

From the repository root:

```bash
git diff --check
git status --short
```

Review the final diff and run focused tests for the behavior changed before broad checks.

## Backend School

From `backend-school`:

```bash
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check --workspace --all-targets
RUSTFLAGS='-D warnings' cargo check --locked --bin backend-school
```

Run `cargo test -p <package>` for each changed internal crate and focused unit or integration tests
for changed application modules. For API-contract work:

```bash
cargo check --workspace --all-targets --features api-docs --locked
cargo test --bin backend-school --features api-docs api_contract::tests -- --nocapture
cargo test -p school-staff --features api-docs career::schema_tests
cargo test -p school-staff --features api-docs personnel_schema_enum_matches_serialized_codes
```

HTTP ownership can be checked independently with `cargo test -p school-auth-http`,
`cargo test -p school-academic-http`, `cargo test -p school-certificates-http`,
`cargo test -p school-navigation`, and `cargo test -p school-notifications`.
The other domain owners expose their concrete routes under `http`; changed owners also run
through the disposable database wrapper when their tests need PostgreSQL.
The application owns `FromRef` state selection and injected realtime, result-lock,
lifecycle and file-deletion adapters. Architecture guards inspect the actual crate
implementation paths; cross-domain/database fixtures retain application-level ownership in
`src/integration_tests`, and shared migration fixtures use `school-test-db`. Keep the full OpenAPI artifact byte-identical when
moving handlers without changing their contract.

For single-process tenant fairness, run the focused pure/concurrency checks from
`backend-school`:

```bash
cargo test -p school-tenancy directory_cache::tests -- --test-threads=1
cargo test -p school-tenancy background::tests -- --test-threads=1
cargo test -p school-auth retention::tests -- --test-threads=1
cargo test -p school-notifications http::handlers::tests -- --test-threads=1
cargo test -p school-notifications runtime::tests -- --test-threads=1
cargo test -p school-notifications publisher::tests -- --test-threads=1
```

These exercise shared success/failure cohorts without negative caching, independent
retry, leader/waiter cancellation, bounded distinct-key admission including evicted
fills, completed-result mutation fencing, shared all-kind database admission, same-key
exclusion, release after cancellation, Auth rotation after a sweep waits behind
occupied slots, and payload-free SSE reconciliation with retained tenant filtering
and identity fail-close. Notification fake transport cases hold two slow item futures
while every other kind obtains shared database admission, and confirm cancellation
joins delivery work. Slow-first-endpoint coverage verifies later device progress and
no surviving endpoint future after the enclosing deadline. A twelve-endpoint case
with two stalled waves verifies attempt-based rotation reaches the tail on retries.
Managed-batch database fixtures use the actual AdminClient and PoolManager with only
external transport controlled: while blocked, another kind receives the sole permit
and resident-pool pressure evicts the original pool. A mock directory UUID change plus
cache fencing then proves both endpoint deletion and acknowledgment are refused.
Cleaner database fixtures hold a real table lock until its phase deadline releases
admission, and verify a healthy tenant completes while a sibling's provider is blocked.
They do not measure production capacity.

From the repository root, run the database-dependent owners with the disposable
PostgreSQL wrapper:

```bash
./scripts/test_backend_school.sh --package school-calendar delivery::reminders::turn_tests -- --test-threads=1
./scripts/test_backend_school.sh --package school-auth cleanup_ -- --test-threads=1
```

Run the new managed Notification and Cleaner regressions from the repository root:

```bash
./scripts/test_backend_school.sh integration_tests::notification_dispatch::notification_managed_batch_ -- --test-threads=1
./scripts/test_backend_school.sh services::cleaner::phase_tests -- --test-threads=1
```

The Calendar cases use 401 durable reminders, lock the first 200, verify suffix
progress and next-invocation retry, observe a real advisory lock before cancelling
a blocked event load, and verify that its session releases the lock before retry. A one-connection pool
also verifies complete event hydration, two recipient acceptance transactions, durable
queue/inbox rows and confirmed advisory unlock without nested pool acquisition.
A controlled slow tenant and a healthy multi-turn backlog verify completion-driven
fair revisits without a round barrier. Auth cases preserve 100-row batches and
`SKIP LOCKED`, and cancel after an earlier cleanup statement to verify transaction
rollback and retry. These fixtures never use a tenant database. The complete
Calendar package now includes database tests and must use the wrapper; pure
validation can still use `cargo test -p school-calendar services::tests`.

From the repository root, check Compose/example forwarding of the three admission settings:

```bash
node --test --test-name-pattern='single School process' frontend-school/tests/static/deployment-installer.test.mjs
```

This checks executable configuration, not runtime fairness. Run the central
notification store's focused frontend reconciliation tests for reconnect, failure retry and stale identity disposal.
Deployed reverse-proxy SSE delivery and browser/device acceptance remain separate.

For tenant pool lifecycle changes, run the `school-tenancy` pool-manager tests through
`scripts/test_backend_school.sh --package school-tenancy pool_manager::tests`.
Run the complete provider suite with
`scripts/test_backend_school.sh --package school-tenancy`. It additionally covers
cache hits/expiry/outages, coalesced bursts, credential/status invalidation,
in-flight stale fills, bounded fenced entries and stale mutation tokens. The pool
fixtures exercise aggregate physical reservations, logical leases between queries,
detached push retention, bounded admission/cancellation and recovery.
They cover a silent TCP peer, blocked schema reads, cancelled initialization,
client socket closure while the database is still blocked, concurrent tenant isolation,
closed cached handles and outstanding borrowers during
drainage, including recovery without restarting the manager. The fixtures own their
loopback proxy and disposable PostgreSQL database; never fault-inject production.
Run the system migration handler tests for independent runtime verification and
refusal of incomplete/corrupt history. These reproduce failure mechanisms, not a
historical outage's initiating network or server fault.

Foundation ownership can be checked independently:

```bash
cargo test -p school-permissions
cargo test -p school-migrations
cargo test -p school-errors
cargo test -p school-http
cargo test -p school-authorization
cargo test -p school-auth -- --test-threads=4
cargo test -p school-academic-core -- --test-threads=8
cargo test -p school-academic-delivery -- --test-threads=8
cargo test -p school-academic-timetable -- --test-threads=8
cargo test -p school-academic-assessment -- --test-threads=8
cargo test -p school-academic-results -- --test-threads=8
cargo test -p school-academic-lifecycle -- --test-threads=8
cargo test -p school-workflow
cargo test -p school-question-bank -- --test-threads=8
cargo test -p school-admission -- --test-threads=4
cargo test -p school-supervision -- --test-threads=8
cargo test -p school-students -- --test-threads=4
cargo test -p school-staff -- --test-threads=4
cargo test -p school-calendar -- --test-threads=8
cargo test -p school-tenancy
cargo test -p school-file-platform
cargo test -p school-crypto
cargo test -p school-fonts -- --test-threads=8
cargo test -p school-certificates -- --test-threads=8
cargo test -p school-test-db
```

`school-test-db` is a development dependency only. Database-backed root tests import its helpers
directly and still run through `scripts/test_backend_school.sh`; production code and the runtime
dependency graph must never depend on it. Permission-authority tests must preserve the application
ordering of session-identity invalidation, permission-cache invalidation, and only then realtime
notification.

`school-tenancy` owns backend-admin retry behavior and tenant pool/lazy-migration coordination.
Its package tests include disposable-database coverage through its dev-only `school-test-db` edge;
normal release dependency trees must expose neither `test-support` nor `school-test-db`.

`school-auth` owns session credentials, policy, persistence, throttling, audit events, the identity
cache, user-profile domain operations, and the root-independent auth runtime. Its package tests use
dev-only database and test-support edges. `school-auth-http` owns cookie/CSRF, origin, trusted-client-address and tenant/session
request-context adapters. The application package retains login/profile Axum handlers,
File Platform profile-image orchestration, and deliberate
cross-domain tests such as staff soft-delete followed by session invalidation.

The public school HTTP fixture also exercises the composed domain routers: anonymous access
is refused across protected feature owners, unknown paths remain 404, authenticated `/api/auth/me`
returns its CSRF header, and the notification stream accepts its scoped realtime tenant hint
from an allowlisted development Origin while rejecting a request without Origin.
All three public Certificate routes retain malformed-body and 16 KiB rejection plus security
headers, including method rejection. Architecture inventories follow feature route modules and
their application-owned prefixes instead of assuming registrations live in `app.rs`.

Personnel rank milestones share a pure versioned calendar calculator in `school-staff`. Run its state/date and bounded scoped database cases with `./scripts/test_backend_school.sh --package school-staff rank_milestone -- --nocapture`, and the authorized HTTP checks with `./scripts/test_backend_school.sh integration_tests::staff::career_integration_tests -- --nocapture`. The fixture covers 53 canonical histories, chronological 50-row pages, own access, missing staff information, all profile scopes, status filters and denied access. Against a ready production preview, run `E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/personnel-workflow.spec.ts tests/e2e/staff-career-workflow.spec.ts tests/e2e/rank-milestones-workflow.spec.ts --project=chromium --workers=2` from `frontend-school`; milestone coverage includes conditional-review wording, independent errors/retry, superseded status, paging and mobile/desktop light/dark layouts.

Delivery and timetable versions have independent publication lifecycles. Rehearse their canonical
migration and concurrent writers against disposable PostgreSQL using:

```bash
./scripts/test_backend_school.sh --integration delivery_versions -- --nocapture
./scripts/test_backend_school.sh --integration delivery_versions zero_course_periods_preserve_delivery_and_require_removing_existing_lessons -- --exact --nocapture
./scripts/test_backend_school.sh --integration delivery_draft_lifecycle -- --nocapture
./scripts/test_backend_school.sh integration_tests::academic::delivery::services_tests -- --nocapture
./scripts/test_backend_school.sh integration_tests::academic::services::timetable_ -- --nocapture
./scripts/test_backend_school.sh integration_tests::academic::lifecycle::services::term_preparation_tests -- --nocapture
./scripts/test_backend_school.sh system::handlers::migration::tests -- --nocapture
./scripts/test_backend_school.sh --package school-academic-delivery
./scripts/test_backend_school.sh --package school-academic-timetable
```

The migration fixtures cover stable identities, a delivery graph shared by multiple tables,
dated A/B/A source chains, independent placement drafts, mixed opening/placement drafts,
explicit legacy target inclusions, early publication boundaries, unmappable sources/targets/historical
groups, fresh reconciliation gates, and safe retry. Runtime
cases cover separate opening publication, exact instructor handoff, source reconciliation,
semantic content comparisons, publication receipts, deletion rollback, and serialized writers.
Against the ready local production preview, run the opening/table workflows from `frontend-school`:

```bash
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test \
  tests/e2e/homeroom-delivery-workspace.spec.ts \
  tests/e2e/delivery-offering-route-loading.spec.ts \
  tests/e2e/academic-teacher-change-handoff.spec.ts \
  tests/e2e/timetable-version-workspace.spec.ts \
  tests/e2e/timetable-board-templates-region-loading.spec.ts \
  tests/e2e/timetable-drag-board.spec.ts \
  tests/e2e/timetable-teacher-board.spec.ts \
  tests/e2e/timetable-whole-school-overview.spec.ts \
  tests/e2e/timetable-dense-board-layout.spec.ts --project=chromium --workers=2
```

The draft-lifecycle fixtures also verify migration 088→089 with unchanged published facts,
nullable draft dates, DELETE-only cancelled guards, optimistic counts/revisions, protected references,
and whole-batch rollback, including an unknown downstream foreign key. Delivery service tests cover
candidate-date hashes, fresh references, dated instructors, idempotency and concurrent writers.
The opening workspace browser fixture exercises repeated finding keys, eager names and before/after
differences, scoped retry, date-change invalidation, deletion refusal/success, read-only controls,
keyboard confirmation and mobile/desktop in both themes. Use protected tenant copies for operational
rehearsal, and keep production cleanup separate from routine disposable-schema tests.

Fixtures use synthetic staff/resources and generated API shapes. A published opening never
implicitly creates or publishes a timetable or student roster. Opening snapshots retain their
course/group/teacher facts; dated student memberships remain on their separate roster lifecycle.

For application-selected roster and Behavior runtime providers, run the owning database fixtures:

```bash
./scripts/test_backend_school.sh admission::application_service_tests -- --test-threads=4
./scripts/test_backend_school.sh lifecycle_student_deactivation -- --test-threads=4
./scripts/test_backend_school.sh behavior_refresh_runs_after_committed_settings_only
```

These exercise same-transaction roster visibility, rollback of enrollment/account/academic changes,
completed-receipt replay, and Behavior refresh only after a committed, authorized, CSRF-checked
mutation. Compare application, Attendance and Delivery source edits on the same toolchain and target
when evaluating these provider boundaries. Delivery remains transitively used by Students through
Timetable; removing a direct manifest edge does not imply the entire rebuild path disappears.

The six `school-academic-*` crates own Core, Delivery, Timetable, Assessment, Results, and
Lifecycle respectively. Lifecycle and Supervision expose dev-only integration-support features
only for root tests that intentionally span domain owners; the normal release graph must keep
those features disabled. The application supplies external readiness, timetable consequence,
result-lock, notification, cache, and realtime adapters without duplicating crate-owned SQL.
Assessment also owns exam scheduling models and services, including published views and
transaction-scoped lifecycle operations. Its pure tests run with
`cargo test -p school-academic-assessment --lib`; the existing application exam-schedule
database fixtures continue to exercise the HTTP/lifecycle adapters and shared transactions.

The Operations `benchmark` task calls [Backend Build Benchmark](../.github/workflows/backend-build-benchmark.yml) and uses
the pinned Docker builder for both backends. It records repeated source-changing Cargo/link
times, binary size, unchanged persistent-target reuse, and exact School OpenAPI equality.
Compiler options apply only to the final application binary; dependency optimization stays unchanged.
Select `options={"backend":"both","samples":"2","variants":"default,cgu64"}`;
the optional `opt2` and `cgu256` variants are experiments, not production settings.
Each case primes its compiler flags before two to five timed source changes and links exactly once.
School also measures Academic HTTP and attendance domain changes under the default profile.
Artifacts include per-unit Cargo times, actual linker flags, binary sizes, CPU/toolchain identity,
and the original builder's cache evidence separately from warm Cargo measurements. All School
exports must equal the default-profile OpenAPI, including across compiler variants. Documentation
exports compile the same source with `api-docs` in a separate debug target outside timed builds;
they do not enable documentation in measured production artifacts or alter release fingerprints.
Use `options={"backend":"backend-school","samples":"2","variants":"default,api-docs"}`
to compare default production compilation with documentation enabled on the same source and runner.
Both cases measure application and Academic HTTP edits; feature/dependency priming is separate.
The final diagnostic compilation records compiler pass times and monomorphization estimates.
Academic HTTP routes use concrete `Router<AcademicHttpState>` factories in their owning crate.
The application binds its existing narrow state before applying the shared session boundary;
this prevents the executable from regenerating domain handler futures and HTTP encoding.
Route ownership guards and protected-router tests cover this boundary. Compare source-changing
application and Academic HTTP builds before changing the factory's compile scope.
For a paired route-factory comparison against an exact baseline commit, select
`options={"backend":"backend-school","samples":"2","variants":"default,http-routes","baseline":"<40-character SHA>"}`.
This validates a bounded source allowlist, freezes both sources from Git, and measures both
application-only edits and Academic HTTP edits on the same runner without changing release flags.
Use `options={"backend":"backend-school","profile_only":true,"variants":"default"}`
to collect only the compiler profile from the exact production builder. This mode skips warm
comparison samples; it does not establish a speed improvement. Pass timings and monomorphization
size estimates identify expensive generated code. The benchmark imports the canonical backend cache without exporting it.
Actions displays a bounded timing table; full per-unit JSON and compiler evidence stay in the
downloadable artifact, avoiding GitHub's 1 MiB step-summary limit.
It enables diagnostic `-Z` options with `RUSTC_BOOTSTRAP=1` only for the selected binary's
rustc invocation in the disposable benchmark container. Cargo and dependency build scripts keep
their ordinary environment and fingerprints. That compilation is excluded from warm sample means and never produces a release
artifact. Production builds keep the pinned stable compiler and normal release flags.
The container keeps a two-CPU quota and two Cargo jobs throughout. Compare compiler variants
sequentially on one runner before attributing a change to a flag: different hosted machines
can produce substantially different timings. A runtime performance claim requires a separate
representative load test; exporter equality establishes the API contract only.

To compare the bounded application-owned schema registry against its preceding source, select
`options={"backend":"backend-school","samples":"2","variants":"default,api-batches","baseline":"<exact-40-character-SHA>"}`.
The comparison refuses any other backend source, manifest, lockfile or Dockerfile difference.
It restores the baseline production builder, then replaces only `src/api_contract.rs` in the same
container. Flag priming is separate, both cases use unchanged release settings, and every export
must equal the baseline document. This paired mode measures application-source changes; the normal
benchmark mode retains the Academic HTTP and attendance invalidation cases.

Explicit schema registration uses bounded constructors in `src/api_contract.rs` to limit one
generated function's optimiser and stack workload. Keep each group near 64 registrations and
preserve their order when adding types. Later explicit declarations override earlier explicit
declarations; handler-derived schemas retain priority over the combined explicit registry.
The precedence regression and full generated-contract check protect both rules. The application
continues owning the entire OpenAPI composition, and the exporter runs without runtime credentials.

The Operations `http-benchmark` task calls [Backend HTTP Boundary Benchmark](../.github/workflows/backend-http-boundary-experiment.yml) and
compares a selected pre-extraction commit with the workflow's candidate commit. Both use
the production Docker builder, two CPU quota, the same release profile, and one persistent
Cargo target. It makes two source changes in each of the application, Academic HTTP,
Certificates HTTP, Navigation, and Notifications owners. Initial dependency/graph priming,
unchanged target reuse, compiled package names, binary sizes, Cargo timing reports, and
exact OpenAPI equality are recorded separately. The source-changing comparison disables
the compiler wrapper to measure Cargo invalidation; production sccache hits/misses must be
assessed in the actual image-build artifacts.

`school-workflow`, `school-question-bank`, `school-admission`, `school-supervision`,
`school-students`, `school-staff`, and `school-calendar` own their domain models, policies where
applicable, persistence, business rules, and focused tests. Root keeps HTTP/OpenAPI composition,
cross-domain deletion and side-effect orchestration, Calendar notification scheduling, and Parent
aggregate views. Keep database-heavy package suites at no more than eight threads when they share
a disposable PostgreSQL instance.

`school-file-platform` owns provider-neutral file inspection, purpose/object-key policy, repository,
storage and malware-scanner implementations, lifecycle operations, runtime configuration, and
reconciliation. Its package tests exercise canonical migrations through a dev-only
`school-test-db` edge. Axum handlers, wire models, tenant/actor resolution, cross-domain
relationships, and cross-domain file authorization remain in the application package.

`school-crypto` is the only application-field encryption and blind-index implementation.
`school-fonts` owns the school font library and its typed staging relationships.
`school-certificates` owns certificate policy, layout, issuance, rendering, verification, rate
limiting, and purge behavior. `school-certificates-http` owns its handlers and API declarations;
the application composes OpenAPI and supplies the cross-domain file-deletion adapter.
`school-academic-http` owns academic handlers and their request policies, while the six
academic domain crates retain business logic and transactions. Both HTTP owners receive
concrete narrow state through application-owned `FromRef` implementations.
`school-auth-http` owns the shared session-derived request context, tenant-origin resolution,
client-address checks, and cookie/CSRF helpers. It uses the existing `AuthRuntime` and shared
permission/identity caches, with no application dependency. Navigation owns menu persistence,
templates, route synchronization and feature toggles; Notifications owns persistence,
tenant/user events, the publisher and Web Push. Startup, routing, middleware and realtime
connection management remain in the application. Use at most eight test
threads for the database-heavy font and certificate package suites so concurrent migration
fixtures do not exhaust the disposable PostgreSQL instance.

The static architecture suite owns backend-only boundaries such as tenant request context, thin handlers, service/database separation, permission invalidation, structured logging, and realtime identity.

For the Staff directory/Timetable boundary, run the application-owned fixtures through the local
Docker database runner:

```bash
./scripts/test_backend_school.sh migration_060_subject_group_projections_support_timetable_load -- --nocapture
./scripts/test_backend_school.sh integration_tests::academic::services::timetable_block_service_tests -- --nocapture --test-threads=4
./scripts/test_backend_school.sh concrete_academic_routes_preserve_session_tenant_csrf_and_permission_boundaries -- --nocapture
```

The projection fixture traverses the real application adapter and Staff query, covering active,
date-effective, deduplicated affiliations, unknown users and an empty selection. The workspace
fixture checks directory injection, bounded distinct user IDs, retained group data and propagation
of provider failures. The protected-router fixture covers the same concrete Academic state.
Lookup DTO ownership changes also require the canonical API export comparison and the existing
Administration lookup and Certificate transport checks; wire schema names and serialization stay
unchanged. Before-and-after compile comparisons must prime each graph separately, use the same
toolchain/host/release flags and at least two source-changing samples for both the executable and
Staff service. Record rebuilt packages as well as wall time; warm Cargo results do not measure
complete build/push/deployment time.

### School session authentication

Session changes require the schema, repository, service, HTTP/middleware, and realtime
boundaries—not only a login happy path. From the repository root, run the crate-owned schema,
repository, cache, and service tests against disposable PostgreSQL:

```bash
./scripts/test_backend_school.sh --package school-auth session_schema_tests -- --nocapture --test-threads=8
./scripts/test_backend_school.sh --package school-auth session_repository_tests -- --nocapture --test-threads=4
./scripts/test_backend_school.sh --package school-auth session_cache_tests -- --nocapture --test-threads=8
./scripts/test_backend_school.sh --package school-auth session_service_tests -- --nocapture --test-threads=4
```

From the repository root, use the disposable local PostgreSQL runner for application-owned HTTP,
realtime, profile/File Platform, and cross-domain behavior:

```bash
ENCRYPTION_KEY=$(openssl rand -hex 32) ./scripts/test_backend_school.sh integration_tests::auth::session_http_tests -- --test-threads=4 --nocapture
./scripts/test_backend_school.sh integration_tests::auth::profile_integration_tests -- --nocapture
./scripts/test_backend_school.sh integration_tests::auth::staff_integration_tests -- --nocapture
./scripts/test_backend_school.sh --package school-academic-http websockets::security_tests -- --nocapture
```

The session-auth HTTP target also exercises encrypted Student Care data. Generate a fresh
test-only key for that command as shown; never use a production encryption key.

From `frontend-school`, verify the browser security boundary and client state:

```bash
node --test tests/static/session-auth-contract.test.mjs \
  tests/static/account-security.test.mjs \
  tests/static/auth-session-state.test.mjs
E2E_SESSION_USERNAME='dedicated-disposable-account' \
E2E_SESSION_PASSWORD='provided-at-runtime' \
npx playwright test --list tests/e2e/login.spec.ts tests/e2e/session-security.spec.ts
```

The destructive `session-security.spec.ts` account must be dedicated and disposable because the suite intentionally revokes a selected browser and then logs out every session. Never fall back to `SMOKE_*`, normal `E2E_USERNAME`/`E2E_PASSWORD`, or an operator account for that file.

## Backend Admin

From `backend-admin`:

```bash
cargo fmt --all -- --check
cargo test
cargo check
```

Use focused test filters first when changing a handler, client, or service.

## Frontend School

The appearance backend uses the owned disposable database fixture. From the repository root:

```bash
CARGO_BUILD_JOBS=2 ./scripts/test_backend_school.sh --package school-administration appearance -- --nocapture
CARGO_BUILD_JOBS=2 ./scripts/test_backend_school.sh --features api-docs integration_tests::school::public_tests::public_http_reads_need_no_session_and_keep_tenants_and_private_routes_separate -- --nocapture
```

From `backend-school`, run `cargo test --bin backend-school --features api-docs api_contract::tests -- --nocapture`
for closed appearance schemas. These focused cases cover strict validation, revision conflicts,
logo preservation, singleton migration refusal, public tenant isolation, read/update permission
separation, missing rows, session/CSRF checks and JSON error envelopes. Use the pinned Rust
toolchain and one shared target directory; do not point fixture recipes at tenant databases.

School appearance uses a self-hosted frontend and local mock tenant backend; it needs no real
student data or credentials. From `frontend-school`, run:

```bash
node --experimental-strip-types --test tests/runtime/design-appearance.test.ts tests/runtime/school-appearance.test.ts tests/runtime/color-picker.test.ts
node --test tests/runtime/server-appearance-worker.test.mjs
npx playwright test tests/e2e/school-appearance.spec.ts tests/e2e/school-surface-contrast.spec.ts --project=chromium --workers=1
```

The focused suite covers the canonical contrast/CSS serializer, exact selected fills with readable
text/hover/boundaries, keyboard RGB and pale presets, responsive form spacing, per-document tenant reads,
ETag revalidation, bounded failure, stale font preparation, same-revision font retry,
delayed CSS/fonts before first paint, an unused stalled legacy bootstrap URL, independent
CSS fail-open when the inline bootstrap cannot execute, personal mode,
role/public pages, persisted settings, a fresh browser, SPA navigation, denied reads/writes
and explicit conflict review. Install the pinned Playwright Chromium and its required host
libraries before execution. Shared layout guards assert the density token owners; browser
checks retain explicit utility overrides rather than requiring one fixed spacing value. The surface-contrast suite checks actual shared fields, thin quiet control outlines, keyboard focus and persistent selected states, plus the real Attendance select/edit/save workflow and public/staff/student consumers at 390, 614 and 1543 pixels in both modes. It covers the three school palettes and pale, white and dark custom fills, and saves synthetic screenshots in Playwright's per-test output directories. Set `E2E_ARTIFACT_DIR` to collect screenshots in an explicit directory. Public pages retain their existing light appearance under either system preference. Neutral surfaces follow the pre-appearance design independently of school palette; card/table/input outlines remain thin and subtle. Browser checks distinguish persistent selection from hover without inset underlines, retain visible keyboard focus and check readable text and exact primary fills with contrast-safe boundaries.
The Workers fixture runs the real stylesheet handler under the Cloudflare runtime, checks fresh
tenant settings on repeated reads and rejects redirects instead of silently accepting another
origin. Browser font cases inspect actual custom glyph fonts for course codes, numbers, shared
exam controls and notifications across all three school fonts, desktop/mobile layouts and SPA
saves; a hard refresh of the saving document and a fresh browser both retain persisted settings.
Technical monospace and KaTeX typography remain separate.

`npm ci` installs the app toolchain (TypeScript 6) and the private `tools/api-contracts` workspace (TypeScript 5 required by `openapi-typescript`). The API generator resolves its CLI from that workspace; keep its peer dependency isolated when upgrading app tooling.

`runed`, used by the UI primitives, still declares an optional Kit 2 peer for its separate Kit utilities. The scoped override selects Kit 3 for the portable utilities used by Bits UI and Svelte Toolbelt, which do not import `runed/kit`. The Kit-specific utilities still use retired Kit APIs; do not import `runed/kit` into the applications until upstream supports Kit 3. Verify the installed graph with `npm ls @sveltejs/kit svelte vite typescript --all` and run the mocked browser acceptance after dependency changes.

From `frontend-school`:

```bash
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:menu-sync
npm run test:route-loading
npm run test:static
npx tsc --project src/service-worker/tsconfig.json
```

Spreadsheet fixtures cover displayed values, zero-padded IDs, literal formula text, CSV quoting, template widths, and multi-sheet rejection. From `frontend-school`:

```bash
node --test tests/static/spreadsheet.test.mjs tests/static/certificate-importer.test.mjs
PUBLIC_BACKEND_URL=http://127.0.0.1:4173 PUBLIC_VAPID_KEY=test npx playwright test tests/e2e/certificate-import-review.spec.ts --project=chromium --workers=1
```

Against the production preview described below, run `admission-final-region-loading.spec.ts` and `admission-exam-room-region-loading.spec.ts` for student-ID import, template downloads, and room exports. These use disposable mocked data and do not mutate a tenant.

The public school homepage and `/executives`, `/management-groups` and `/subject-groups` directories use disposable API fixtures for streamed loading, regional retries, empty states, standalone mobile/desktop navigation, larger portraits and centering of every member row in both themes. Pure organization tests cover highest-role selection without deputies, tied leaders, same-name colleagues, combined responsibilities, vacant groups and leaf-work membership. Rust tests use isolated tenant schemas and the real router to verify anonymous access, tenant isolation, current enrollment/room movement, staff position counts, current members of every position and directory-only person keys across memberships. Run:

```bash
# Repository root
./scripts/test_backend_school.sh integration_tests::school -- --nocapture
# frontend-school (the browser specs start their own local servers)
node --experimental-strip-types --test tests/runtime/public-school-organization.test.ts
node --experimental-strip-types --test tests/runtime/public-school-seo.test.ts
npx playwright test tests/e2e/landing-page.spec.ts --project=chromium --workers=1
npx playwright test tests/e2e/admin-landing-page.spec.ts --project=chromium --workers=1
```

The central public discovery checks use `cd frontend-admin && npm run test:unit`,
`npm run lint`, `npm run check`, and `npm run build`; from `frontend-school`, run
`npx playwright test tests/e2e/admin-landing-page.spec.ts --project=chromium --workers=1`
and `npx playwright test tests/e2e/landing-page.spec.ts --project=chromium --workers=1`
in separate processes so their SvelteKit environments do not share cached configuration.
The Admin fixture verifies first HTML metadata/structured data and school links with
JavaScript disabled, account-page noindex, sitemap/robots, pagination and genuine
unavailable-directory errors. `./scripts/test_backend_admin.sh` covers anonymous
pagination, reserved/inactive exclusion, credential-free projection, unchanged
school snapshots and preserved protected-route authorization. Deployment checks
verify only exact apex/www routes and retirement of the old admin alias. After
release, verify both www schemes permanently redirect to the healthy apex; inspect
central and tenant canonicals/sitemaps and request Search Console validation only
after the affected URLs pass. A local success cannot prove Google has reindexed.

The homepage spec also checks the first HTML with JavaScript disabled, unique metadata and H1, safe School JSON-LD, an eight-second identity timeout and successful four-second identity read, concurrent sibling reads, retries, and crawler endpoints. The public logo and favicon resolvers cover single-request current-crest-only selection, a successful ten-second cold favicon response within the fifteen-second budget, cookie omission, Admin-brand fallback for missing/deleted crests, PNG delivery and retryable service failures. Installation checks parse the browser manifest, verify actual 192/512 PNG dimensions, Apple/tab links, missing/deleted-crest fallbacks and rejection of unsupported sizes without backend reads. SEO runtime cases cover tenant-specific canonical/sitemap URLs, the `ชื่อโรงเรียน - ระบบจัดการโรงเรียนครบวงจร` title, normalized description prefix, stable public-branding image versions, separate wide social image/square structured logo, logo fallback, the configured base domain, and exclusion of sandbox/local/preview hosts. The Admin fixture also exercises a four-second directory read so normal cross-region latency does not become a false outage. After release, fetch each production homepage without executing JavaScript and verify its school name, one title/description, its own HTTPS canonical, favicon and sitemap; check `noindex` on sandbox and login, and fetch its `/school-logo` and `/favicon.png` without Origin or Referer to verify anonymous crawler delivery.

The homepage browser fixture also checks wide Open Graph dimensions/type/alt and `summary_large_image` in the initial HTML, escaped school names, menu-derived protected-route previews without identity reads, and anonymous `/og-image.png` requests. The social-image cases verify current-host Origin, ignored caller file/URL queries, cookie omission, real 1200×630 PNG bytes, five-minute success cache, wrong MIME/dimensions/bytes, declared and streaming size limits, rejected redirects, service errors and the fifteen-second timeout with uncached retryable 503 recovery. Explicit route image overrides retain their own image contract in `tests/static/route-preview-meta.test.mjs`. After release, fetch each school's advertised social image anonymously and inspect its current name/crest or designed platform fallback; platform-owned card layouts and crawler caches require external recrawling.

For changes to public crest PNG rendering, run `./scripts/test_backend_school.sh --package school-file-platform favicon -- --nocapture`; this checks portrait/landscape/square input, inspected bytes, PNG dimensions, denied private/non-ready/non-crest files and read-only provider access. Compile the School Administration and Files HTTP consumers and regenerate/check/test the API contracts when the public endpoint changes. The real public HTTP fixture checks current-school selection, PNG delivery, missing crests, ignored caller file IDs, private-file refusal before object reads and tenant separation. The square image conversion uses existing File Platform image inspection and storage limits; no migration or replacement of stored crests is required.
Run `./scripts/test_backend_school.sh integration_tests::school::public_tests::public_http_reads_need_no_session_and_keep_tenants_and_private_routes_separate -- --nocapture` for the real router's anonymous boundary, cross-school rejection and preserved private-file visibility.

Login and app-header layout fixtures also start their own local servers. They cover small and short viewports, optional school branding failures, pending/rejected login, and the remaining header controls:

Keep these self-hosted dev-server specs on one worker when combining them: Kit's generated files share one project directory, so concurrently starting independent Vite servers is not a valid test environment. The production-preview mocked route gate below can use two workers.

Run self-hosted browser specs after `test:static` and `check` have finished, and before starting a build. The static academic-context cases also start Vite in the same project; overlapping them can rewrite generated public environment modules and hot-reload a running browser fixture with the wrong API origin.

```bash
npx playwright test tests/e2e/login-layout.spec.ts tests/e2e/header-layout.spec.ts --project=chromium --workers=1
```

During implementation, run the relevant static file directly:

```bash
node --test tests/static/<area>.test.mjs
```

`test:route-loading` requires every authenticated page to have a completed data-owner review and a primary loader when route-owned. It rejects retired migration allowances and primary mount reads, follows local mount helper calls, and records browser subscription reasons and focused test owners in the inventory. It also guards focused invalidation, safe preload and the absence of generic page-view endpoints.

The staff tools QR workflow uses synthetic staff/student/parent sessions and public branding fixtures. Against the local production preview below, run `E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/staff-tools-qr.spec.ts --project=chromium --workers=2` from `frontend-school`. It decodes downloaded PNGs with the test-only `jsQR` dependency, verifies exact 1024-pixel output and preview/download equality, checks logo validation/retry/supersession, and captures mobile/desktop light/dark states. Pure input validation and catalog menu registration run in `node --test tests/static/qr-code.test.mjs`. Run `npx playwright test tests/e2e/file-download-cors-cache.spec.ts --project=chromium --workers=1` separately for the self-hosted HTTP-cache regression: it first reproduces a CORS failure from a cached branding image, then verifies a fresh byte download succeeds without request interception.

Against the local production preview below, run `E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/reference-page-layouts.spec.ts tests/e2e/staff-home-region-loading.spec.ts tests/e2e/staff-student-region-loading.spec.ts --project=chromium --workers=2` from `frontend-school` for staff dashboard and student details. Synthetic fixtures cover unequal service-card heights without vertical row gaps or clipped links, separation of detail cards, long contact wrapping, mobile/desktop light/dark screenshots, permission visibility, independent retries, retained refresh data, and rejection of superseded person/year responses. These fixtures do not exercise a deployed backend or real student data.

For the Academic Delivery region-loading browser gate, build and preview the frontend with a local API origin in one terminal, then run the mocked Chromium spec in another:

```bash
PUBLIC_BACKEND_URL=http://127.0.0.1:4173 PUBLIC_VAPID_KEY=test npm run build
PUBLIC_BACKEND_URL=http://127.0.0.1:4173 PUBLIC_VAPID_KEY=test npm run preview -- --host 127.0.0.1 --port 4173
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/route-region-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/academic-catalog-route-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/subject-catalog-workspace.spec.ts tests/e2e/facility-workspace.spec.ts tests/e2e/facility-region-loading.spec.ts tests/e2e/table-layout.spec.ts --project=chromium --workers=1
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/academic-foundation-route-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/curriculum-delivery-alignment.spec.ts tests/e2e/curriculum-publications.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/academic-year-collections-route-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/delivery-offering-route-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/personal-daily-timetable-region-loading.spec.ts tests/e2e/timetable-daily-overview-layout.spec.ts tests/e2e/staff-own-timetable-grid.spec.ts tests/e2e/staff-own-timetable-pdf.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/timetable-board-templates-region-loading.spec.ts tests/e2e/timetable-teacher-board.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/exam-schedule-lists-region-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/staff-exams-workspace.spec.ts tests/e2e/staff-own-timetable-grid.spec.ts --project=chromium --workers=1
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/exam-schedule-detail-region-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/question-bank-region-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/gradebook-workflow.spec.ts tests/e2e/result-preparation-region-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/academic-result-locking.spec.ts tests/e2e/result-corrections-region-loading.spec.ts --project=chromium
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/academic-term-aggregates.spec.ts tests/e2e/academic-annual-results.spec.ts --project=chromium --workers=2
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/academic-promotion-policies.spec.ts tests/e2e/academic-promotion-runs.spec.ts --project=chromium --workers=2
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/academic-term-lifecycle.spec.ts tests/e2e/academic-year-lifecycle.spec.ts --project=chromium --workers=2
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/admission-round-region-loading.spec.ts --project=chromium --workers=2
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/admission-application-region-loading.spec.ts --project=chromium --workers=2
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/admission-exam-room-region-loading.spec.ts --project=chromium --workers=2
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/admission-score-region-loading.spec.ts --project=chromium --workers=2
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/staff-home-region-loading.spec.ts tests/e2e/staff-role-region-loading.spec.ts --project=chromium --workers=2
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/supervision-management-region-loading.spec.ts tests/e2e/supervision-queues-region-loading.spec.ts tests/e2e/supervision-detail-region-loading.spec.ts --project=chromium --workers=2
```

The facility fixtures cover building-to-room navigation, deep links/history and retained academic
context, lazy and cancelled region reads, separate unassigned rooms, name-only filtering before
client pagination, exact mutation permissions, failure/retry and stale-draft ownership. Room and
building edit fixtures assert omitted hidden fields preserve stored metadata, including zero room
capacity. The shared-table geometry cases verify matching logical header/body edge spacing against
the root font size, readable local overflow, and existing PersonalTimetable `p-0`/responsive padding
overrides. Facility CRUD and geometry run at mobile/desktop sizes in light/dark themes; intercepted
APIs establish browser behavior, not database authorization or production acceptance. Run the
focused source guards with `node --test tests/static/facility-region-loading.test.mjs
tests/static/frontend-layout-components.test.mjs tests/static/documentation-policy.test.mjs`.

Physical room-name projections share the existing timetable contracts and display helpers. From
`frontend-school`, run `npm run generate:api-contracts`, `npm run check:api-contracts` and
`npm run test:api-contracts`, followed by:

```bash
node --test tests/static/timetable-block-display.test.mjs tests/static/daily-teaching-display.test.mjs tests/static/academic-timetable-pdf.test.mjs tests/static/timetable-teacher-load-export.test.mjs tests/static/staff-own-timetable-pdf.test.mjs
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/timetable-drag-board.spec.ts tests/e2e/timetable-whole-school-overview.spec.ts tests/e2e/timetable-daily-overview-layout.spec.ts tests/e2e/academic-timetable-pdf.spec.ts tests/e2e/staff-own-timetable-grid.spec.ts tests/e2e/staff-own-timetable-pdf.spec.ts tests/e2e/academic-teacher-change-handoff.spec.ts --project=chromium --workers=1
```

From the repository root, use `./scripts/test_backend_school.sh timetable_block_service_tests --
--nocapture` for the disposable database projection fixtures and `cargo test --manifest-path
backend-school/Cargo.toml -p school-academic-lifecycle room_label_tests` for unique name matching.
The projection fixtures distinguish physical room names from technical codes across group,
homeroom and daily timetable responses while retaining physical IDs; the matching cases reject
blank or ambiguous names. Keep the repository's existing toolchain and database-runner setup.

Run the cross-domain mocked acceptance together against a ready local production preview:

```bash
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test \
  '.*(region-loading|route-loading)\.spec\.ts$' \
  tests/e2e/gradebook-workflow.spec.ts \
  tests/e2e/curriculum-delivery-alignment.spec.ts \
  tests/e2e/academic-term-aggregates.spec.ts tests/e2e/academic-annual-results.spec.ts \
  tests/e2e/academic-promotion-policies.spec.ts tests/e2e/academic-promotion-runs.spec.ts \
  tests/e2e/academic-term-lifecycle.spec.ts tests/e2e/academic-year-lifecycle.spec.ts \
  --project=chromium --workers=2
```

The staff exam and own-timetable workspace fixtures use synthetic read-only staff data. They cover compact context controls, responsive results in light/dark themes, filters and empty states, keyboard navigation, school appearance, and the absence of management requests or writes. Exam cases also verify all four tabs, round changes, mobile assessment phases, and actual personal duty duration.

For staff self-service timetable readability, run the focused display/PDF helpers and browser
workflow from `frontend-school`:

```bash
node --test tests/static/staff-own-timetable-display.test.mjs tests/static/timetable-pdf-class-label.test.mjs tests/static/timetable-block-display.test.mjs tests/static/staff-own-timetable-pdf.test.mjs
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/staff-own-timetable-grid.spec.ts tests/e2e/staff-own-timetable-pdf.spec.ts --project=chromium --workers=1
```

The grid fixture covers 390/614/1029/1533px in light/dark themes: equal content-safe daily
period rows, full multiline titles, consistent card tones, actual class labels without group
aliases, current-instructor-only synchronized targets, no weekly detail controls, and contained
scrolling with native wheel/touch and keyboard navigation. It checks main/document geometry
at scroll boundaries and keeps view/day choices without additional reads. The pure helper
fixtures preserve student/parent shared-label hiding and existing PDF shared-period rules;
PDF class names use the real projection when no external class directory is supplied.
Screenshots use `/workspace/artifacts/staff-timetable-refinement/`.

Subject catalog and facility workspace fixtures exercise compact filters and bounded results, create/detail forms, scoped capabilities, error and pending draft retention, and late-response ownership on mobile/desktop in both themes. Subject version fixtures also check decimal-string payloads and publication revisions; the two subject-version cases in `academic-core-cutover.spec.ts` cover published-history preservation and conflict draft retention with the current overview, Sheet, grade selector and DatePicker.

Mocked route suites block service workers so same-origin requests reach Playwright interception. Confirm preview readiness and warm the server before starting the command; do not edit product files or run another Playwright invocation concurrently.

For deployed readonly route acceptance, use the ignored `frontend-school/.env.e2e.local` (mode `0600`) with HTTPS `E2E_BASE_URL`, `E2E_API_URL`, and pairs `E2E_STAFF_USERNAME/PASSWORD`, `E2E_STUDENT_USERNAME/PASSWORD`, `E2E_PARENT_USERNAME/PASSWORD`. The parent account must have a linked child with academic history in the same tenant. Supply values locally, never in a chat, command argument or Git. Node 24 loads this file explicitly; Playwright does not load it automatically.

```bash
E2E_ROUTE_LOADING_LIVE=1 node --env-file=.env.e2e.local \
  node_modules/@playwright/test/cli.js test tests/e2e/route-loading-live-acceptance.spec.ts \
  --project=chromium --workers=1
```

For staff-only browser measurements, add `--grep 'deployed staff:'` to the command
above and supply local `E2E_STAFF_USERNAME/PASSWORD` for the selected HTTPS tenant.
The [existing spec](../frontend-school/tests/e2e/route-loading-live-acceptance.spec.ts)
measures its full staff route list, including dashboard and students; the account
needs the relevant read permissions and existing academic context for those routes.
The [timing helper](../frontend-school/tests/e2e/helpers/route-performance.ts)
measures request timing, bytes and first useful paint separately. GitHub's
`RUNTIME_PROBE_*` Actions secrets are not local E2E inputs; missing local credentials
leave browser measurements unrun.

The readonly live spec also accepts `SMOKE_USERNAME/PASSWORD` for its staff login when deliberately exported from the trusted Bash-owned `.env.smoke.local`. Do not load a Bash env file containing shell escapes with Node `--env-file`; encode literal values in the dedicated dotenv file instead. It navigates only, then logs out its own newly created session; it never opens mutation workflows or revokes other sessions. It discards one warm-up and records medians from exactly five warm runs with fixed role, permissions, academic context, Chromium and default network, using client navigation with preload disabled. Output contains route templates, region names, durations, bytes and available server phases only. Browser artifacts are disabled. Missing credentials or academic context are explicitly unrun. A missing baseline or query plan prevents claims of production speedup or SQL improvement.

The Delivery spec uses five warm mocked navigations to verify sanitized timing and size summaries. Use the same helper for credentialed, representative environment measurements; keep those results in release evidence rather than committing tenant-specific output.
The catalog spec checks the three route-owned list regions, focused retry, local group patching, and lazy history after create.
The foundation spec checks route-first setup/curriculum regions, focused retry, lazy period reads, and edition creation without owner/grade lookups surviving a stale overview response.
The publication spec covers a published edition opening an amendment, adding term two, publishing the whole edition, reading the baseline unchanged, stale-edition refusal, read-only permissions, independent history retry and superseded selection. It also checks permanent amendment discard, keeping a draft from the confirmation dialog, unchanged publication history and a fresh preview after a conflict. History and discard confirmation layouts run at desktop/mobile sizes in light/dark themes. Native Academic Core service tests additionally verify baseline preservation, live requirement references, concurrent draft reuse, stale tokens, atomic validation failure, immutable snapshots and deferred completeness. Discard cases restore the complete graph and removed live requirement IDs, remove new resources, restore the latest publication, refuse a changed preview or a never-published edition and roll back when an external reference prevents deletion. The delivery apply test verifies source metadata in previews, applied runs and snapshots while retained legacy offerings keep unknown provenance.

Run the pure publication token/source and semantic comparison checks with `cargo test -p school-academic-core curriculum_publication --lib` from `backend-school`. Run the native database cases with `./scripts/test_backend_school.sh curriculum_publication -- --test-threads=4` from the repository root; the CI Academic Core consumer gate includes them in the full service suite. The same workflow runs `BACKEND_SCHOOL_TEST_BIN=seed_sandbox ./scripts/test_backend_school.sh canonical_seed_is_idempotent_across_student_year_and_placement -- --test-threads=1` to verify one visible publication, stable placement identities and preservation of an open staff amendment during repeat seeding.

The curriculum detail spec checks independent edition/level reads, regional retry, invalid or mismatched level refusal, read-only alignment, covered-grade level creation, plan creation and copying a selected published program with conflict handling. Copy and later-year room-selection scenarios cover desktop/mobile in light/dark themes. Database tests additionally verify whole-edition atomic publication, source preservation, semantic term-slot copying, uncovered-grade and stale-revision refusal, migration retry and permission-grant metadata preservation.

For coordinated school release behavior, run:

```bash
node --test tests/runtime/maintenance-controller.test.mjs \
  tests/static/school-release-deployment.test.mjs
node --test ../scripts/tests/pipeline*.test.mjs
node --test ../scripts/tests/worker-release-candidates.test.mjs
```

The push worker browser harness reads the production-built `service-worker.js` unchanged and serves it on an isolated local origin. Build `frontend-school` first, install Playwright Chromium and WebKit with their Linux dependencies, then run:

```bash
npx playwright test tests/e2e/service-worker-production.spec.ts --project=chromium --workers=1
```

This harness launches both browser engines for fresh direct navigation, reload, offline recovery and replacement of an old intercepting worker without losing entered data. Its Push test checks the installed-app icon URL and uses Chromium's full browser channel; Linux WebKit does not expose Playwright's worker evaluation API. Opening activity fixtures in `homeroom-delivery-workspace.spec.ts` block workers and cover central/grouped activation separately. Do not build while a production preview or browser suite is running; restart the preview after changing its build output.

## Frontend Admin

From `frontend-admin`:

```bash
npm run lint
npm run check
npm run test:unit
npm run build
```

## Container Runtime, Installer, and Production Topology

When the VPS installer, canonical Compose runtime, Nginx templates, deployment workflows, or their durable documentation changes, run from the repository root:

```bash
./scripts/pipeline verify --scope deployment
```

This runs ShellCheck/shfmt against the real scripts, installer Bats, pipeline/database-runner and
migration-completion fixtures, deployment source guards, R2 CORS checks, Docker Compose config
resolution and actionlint in Docker. Tests inspect moved implementations rather than workflow
wrappers. Verification pulls digest-pinned PostgreSQL/Nginx from the Docker Official Images mirror
on ECR Public and actionlint from Google's Docker Hub cache, using the same upstream image digests.
Production itself retains rootless Podman; local fixtures and verification use Docker.
Report unavailable dependencies explicitly instead of narrowing the checks.

For focused deployment-runtime work, run:

```bash
bats scripts/tests/installer/runtime_image_retention.bats \
  scripts/tests/installer/deployment_timing.bats \
  scripts/tests/installer/clamd_runtime_matches.bats
node --test scripts/tests/prune-ghcr-versions.test.mjs scripts/tests/pipeline-build-cache.test.mjs
node --test frontend-school/tests/static/deployment-installer.test.mjs
node --test frontend-school/tests/static/documentation-policy.test.mjs
```

The retention tests use only fake Podman and a loopback HTTP server. Cache fixtures cover protected
tags, newest-version retention, the 24-hour grace period, missing packages, denied inventories and
retagging during revalidation. Selector tests cover main-only registry exports, preserved Admin GHA
policy, CLI output and image/evidence wiring. They must not contact the
production VPS, delete a live package version, or replace the manual GHCR dry-run review. Dockerfile
changes also require Docker runtime-target builds and config inspection:

```bash
docker build --target runtime -t schoolorbit/backend-admin:verification backend-admin
docker build --target runtime -t schoolorbit/backend-school:verification backend-school
docker image inspect schoolorbit/backend-admin:verification \
  --format '{{.Config.User}} {{json .Config.Cmd}}'
docker image inspect schoolorbit/backend-school:verification \
  --format '{{.Config.User}} {{json .Config.Cmd}}'
```

Backend release builds use separate BuildKit/sccache namespaces for Admin and School. Main writes
shared release caches; local Cargo targets and npm dependencies stay on each developer's machine.
Frontend bundle keys include source, actual Node version, build recipe and compiled public configuration.
Bundles are hashed and checked again before unpacking. Test targets and release binaries never share
interchangeable cache keys. The offline OpenAPI gate executes the locally compiled `api-docs` binary under
`env -i`, verifying export without runtime configuration while preserving its compiler environment.

The `setup-chromium` action caches the exact installed Playwright version's Chromium headless shell.
It always verifies browser files and launches the browser, including on a cache hit, installing
host dependencies when the runner cannot launch it. School promotion retains its deployed browser mount check; browser E2E suites run locally. Verify a change to the browser setup with
the deployment static guards and the release-scope tests above, then confirm a real headless
Chromium launch; a reported browser cache hit alone is insufficient.

The installer Bats directory includes focused Cockpit coverage:

- `cockpit_provider.bats` exercises Tunnel creation/adoption, ingress, connector IP, CNAME drift, memory-only token handling, and management rollback without deleting Tunnels;
- `cockpit_remote.bats` executes the remote configurator against an isolated filesystem and checks loopback-only listening, root prohibition, mode-`0600` token storage, pinned amd64/arm64 cloudflared artifacts, and idempotency;
- `api_tls_remote.bats` executes the shared certificate owner with synthetic CA/key fixtures for public-chain validation, credential cleanup, staged activation, interrupted-link repair and reload rollback; `pipeline-api-tls.test.mjs` covers latest-attempt gates and queued-runtime refusal;
- `vps.bats` verifies separate SSH stdin streams for the tracked script and secret JSON plus a fresh verification session;
- `orchestration.bats` verifies phase ordering, dry-run mutation absence, publish journaling, resume, standalone rollback, and full migration rollback.

Do not replace these focused files with source-only assertions. The deployment static guard supplements them by preventing Compose, firewall, installer-entry-point, and canonical-document drift.

The completed database transfer is guarded by `scripts/tests/postgres-runtime.test.mjs`;
no Neon client, partial Admin migration or provider-transfer operation may return.
The Admin suite's `database_cutover_cleanup` fixture verifies that migration 009
removes only transfer checkpoint fields while preserving all other school fields,
encrypted credential bytes and prior SQLx checksums, including safe repeat execution.
School's runtime pool regression continues to reject request-time permission DDL.
The `deployment_runtime_verification` cases verify that a held request connection
cannot block the independent deployment check, and incomplete history fails closed.
The tenancy retirement test holds old clones, retires the cached pool, checks that
all old clones are closed, and verifies a fresh pool against the unchanged history.

## Permission Contract

`contracts/permissions.json` is the handwritten permission source. The registries and lock are generated files; do not edit generated files directly.

After a permission definition changes, from `frontend-school`:

```bash
npm run generate:permissions
npm run check:permissions
npm run test:permissions
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Commit the contract, `contracts/permissions.lock.json`, backend registry, frontend registry, migration when DB permission data changes, and focused authorization tests together.

## API Contract

The local Backend School verification owner also runs curriculum migration preservation/refusal and Academic Core, activation, promotion, delivery and certificate consumer tests with the Docker database runner. Frontend validation includes full lint. Developer machines retain compiled test targets; GitHub prepares release artifacts without repeating these tests.

Rust DTOs and OpenAPI annotations own the wire contract. The tracked output is `contracts/openapi/school-api.json`; generated TypeScript lives under `frontend-school/src/lib/api/generated/`. These are generated files; do not edit generated files directly.

After a documented DTO or endpoint changes, from `frontend-school`:

```bash
npm run generate:api-contracts
npm run check:api-contracts
npm run test:api-contracts
npm run test:static
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
```

Generation must work offline without database credentials or a running backend.
The generator enables the default-off `api-docs` Cargo feature automatically. Production builds
exclude OpenAPI composition, schema derives and path metadata; the DTOs and Serde wire behavior
remain available. A production binary rejects `export-openapi` with exit code 2 before runtime
startup. To export directly from `backend-school`, use:

```bash
cargo run --locked --features api-docs --bin backend-school -- export-openapi
```

Local verification checks default and documentation-enabled workspace targets, runs the three
domain endpoint-contract tests with `api-docs`, and compares generated JSON/TypeScript with Git.

## Database and Migration Tests

Never edit an applied migration. Add a new sequential file and test it against isolated state.

Routine backend-school database tests run on the developer's computer. From the repository root:

```bash
# Complete root application binary suite; PostgreSQL runs in disposable local Docker.
./scripts/test_backend_school.sh

# Focused database-backed test.
./scripts/test_backend_school.sh --package school-auth \
  session_repository_tests -- --nocapture

# Database-backed seed utility test against the same disposable PostgreSQL boundary.
BACKEND_SCHOOL_TEST_BIN=seed_sandbox ./scripts/test_backend_school.sh \
  tests::canonical_seed_is_idempotent_across_student_year_and_placement \
  -- --exact --nocapture --test-threads=1
```

The runner uses the local Docker engine and rejects remote `DOCKER_HOST`, `CONTAINER_HOST` or `CONTAINER_CONNECTION`, so it cannot accidentally select a remote runtime. Cargo and its compilation cache stay on the computer, while PostgreSQL uses a fresh anonymous disk-backed volume so the complete migration-backed suite is not capped by a 5 GiB data tmpfs. Ensure the local Docker storage has sufficient free disk space. The runner removes its uniquely named PostgreSQL container and associated anonymous volume after success, failure, `INT`, `TERM`, or `HUP`; it never reuses a named volume or prunes unrelated resources. An uncatchable termination such as `SIGKILL` or a host crash can leave these test resources behind and requires exact-target cleanup. It replaces any inherited `TEST_DATABASE_URL` only for the Cargo child and never uses `DATABASE_URL`. Direct Cargo against a persistent database URL is not the routine test recipe.

Tests continue to isolate their schema/data within the disposable database. The local runner removes the whole database container and its anonymous data volume after the command, including on test failure.

`BACKEND_SCHOOL_TEST_BIN` defaults to `backend-school`; set it to another local package binary only
when that binary's tests require the disposable PostgreSQL instance. The value is passed as one
quoted Cargo `--bin` argument and never changes the container or connection boundary. Use
`--package <internal-crate>` instead to test a crate-owned suite; the runner validates the name
against `backend-school/crates/<internal-crate>/Cargo.toml` before starting Podman. A focused filter
must execute at least one test, otherwise the runner fails instead of accepting Cargo's zero-match
success.

Permission reconciliation regressions can be checked with
`./scripts/test_backend_school.sh --release --locked batch_ -- --test-threads=1`.
The focused tests count actual PostgreSQL upsert statements, preserve retired permission
references, verify rollback/retry, and exercise concurrent pool initialization. The centralized
migration runner owns the single permission reconciliation, including when no migration is pending.

Run `node --test --test-concurrency=1 scripts/tests/r2-cors.test.mjs` from the repository
root with Bash, Node and jq available. It executes the workflow CORS fragment against a
controlled AWS CLI boundary, covering unchanged/reordered policies, full-policy drift,
missing configuration and read/write/verification failures without contacting R2.

### Personnel migration and browser verification

Run the canonical personnel owner and its historical SQL migration fixtures against fresh rootless PostgreSQL:

```bash
./scripts/test_backend_school.sh --package school-staff -- --test-threads=1
./scripts/test_backend_school.sh integration_tests::staff -- --test-threads=1
./scripts/test_backend_school.sh --package school-staff staff_access_policy::tests -- --test-threads=1
```

For bounded teaching-licence changes, run `./scripts/test_backend_school.sh --package school-staff staff_licence -- --nocapture` for PII/resource denial, attachment ownership, stale replacement, transactional rollback, masking, retention and delete-lock behavior. Run the Files purpose/inspection tests for PDF/image validation, and `E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/staff-licence-documents.spec.ts --project=chromium` against a rebuilt preview for title selection, compact degree rows, private upload/remove flows and responsive PDF/image previews. Fixtures are synthetic; no tenant credentials or real documents are used.

The staff suite covers exact degree aliases, unmapped/ambiguous inputs, locked-source drift, retry, identity/license/employment preservation, person-owned text validation, preserved custom/inactive positions and deactivation races, nullable patches, scoped directory/aggregate equivalence, current group deduplication and missing-value buckets. Career tests additionally cover migration 085 preservation and deferred owner/kind/value guards, transactional current projections, reasoned corrections, stale revisions, concurrent writers, safe UUID retries, staff-scoped cursors, optional dates and writer-only acknowledgements. Root HTTP tests exercise own/unit/tree/school reads, denied writes and the production session boundary. Historical migration readers are private test-only helpers; production has no personnel preflight endpoint or cutover report.

For provider rehearsal, create a disposable copy and supply its direct non-pooled URL privately as `MIGRATION_SCHEMA_DATABASE_URL`. Set `MIGRATION_SCHEMA_NAME=public` and `MIGRATION_SCHEMA_ALLOW_PUBLIC=1` only for that disposable copy, then run `cargo run --manifest-path backend-school/Cargo.toml --bin migrate_tenant_schema`. This existing CLI calls the centralized runner; never apply individual SQL files manually. Read actual SQLx version and bounded preservation/current-pointer integrity checks afterward; for migration 085 require imported dates/orders/actors to remain unknown and unchanged existing staff fields. Retain the historical `staff_personnel_simplification_audit` checks where relevant. Do not emit credentials, names, source values or national IDs.

The 82→83→84 tests preserve Thai text, nulls, shared labels, complete position UUIDs/timestamps and unrelated staff fields; stale, duplicate or missing preservation evidence blocks cleanup atomically. Run `./scripts/test_backend_school.sh --package school-staff personnel_simplification -- --test-threads=1` for this boundary. The owned `./scripts/test_school_database_suite.sh` runs the staff, auth, file and schema/status selections against fresh Docker databases and removes only its owned resources. Run `node --test scripts/tests/postgres-compatibility.test.mjs` to verify discovery and failure propagation. Run `node --test scripts/tests/migration-completion-gate.test.mjs` to exercise the actual all-tenant release filter through its Docker jq image, including healthy reports without retired personnel fields, all-tenant coverage, future migration versions, other domain audit failures, and the complete thirty-check delivery/timetable reconciliation. Missing, partial, failed or stale delivery cutover evidence must refuse promotion.

Against a production build running in local preview:

```bash
cd frontend-school
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test \
  tests/e2e/personnel-workflow.spec.ts \
  tests/e2e/staff-career-workflow.spec.ts \
  tests/e2e/staff-home-region-loading.spec.ts \
  tests/e2e/staff-directory-region-loading.spec.ts --project=chromium
```

These synthetic browser fixtures block service workers and exercise position UUIDs, direct education-text payloads, lazy options, create/edit draft ownership, retry/refresh, chart drilldown, calendar dates, independent history loading/error/pagination, historical append retries, reasoned corrections, draft preservation on conflicts, pending actions and mobile light/dark layout. Pure helper and bounded draft-migration tests run within `npm run test:static`. Run deployed read-only acceptance separately with existing runtime credentials and tracing, screenshots and video disabled.

### Academic Core migration rehearsal

Run the Academic Core chain against disposable local PostgreSQL from the repository root. These
focused commands prove the read-only legacy preflight and each sequential transition through 041,
042, 043, 044, and the separately gated cleanup at 045; run them one at a time so failures remain
attributable. The preflight implementation retained here is test-only fixture validation; the
one-time runtime CLI is retired after Phase B.

```bash
./scripts/test_backend_school.sh \
  integration_tests::academic::cutover_test_preflight_database_tests -- --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  integration_tests::academic::core::schema_tests::migration_041_maps_core_fixture \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  integration_tests::academic::delivery::services_tests::migration_042_maps_delivery_fixture \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  integration_tests::academic::core::schema_tests::migration_043_maps_all_consumers \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  integration_tests::academic::core::schema_tests::migration_044_exposes_the_clean_academic_core_runtime_contract \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  integration_tests::academic::core::schema_tests::migration_045_removes_legacy_schema \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  integration_tests::academic::core::schema_tests::migration_045_fails_closed_without_current_reconciliation_evidence \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  integration_tests::academic::core::schema_tests::migration_045_accepts_a_valid_marker_with_distinct_reconciliation_counts \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  integration_tests::academic::core::schema_tests::migration_045_rejects_a_mapping_delete_committed_after_the_marker \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  integration_tests::academic::core::schema_tests::migration_045_rejects_a_deleted_expanded_delivery_target \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  integration_tests::academic::core::schema_tests::migration_045_rejects_a_deleted_legacy_mapping_source \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  integration_tests::academic::core::schema_tests::migration_045_rejects_source_field_drift_committed_after_the_marker \
  -- --exact --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  system::handlers::migration::tests -- --nocapture --test-threads=1
```

Then run the complete disposable PostgreSQL suite:

```bash
./scripts/test_backend_school.sh -- --test-threads=1
```

The passing legacy fixture must report stable aggregate source counts without writes, every blocking
fixture must return its expected bounded finding code, and migrations 041-044 must preserve mapped
counts and checksums. Reconciliation must read the actual tenant migration version, reject anything
other than exactly 044, run all aggregate checks without invoking the migration runner, create no
success marker when a check fails, and preserve one idempotent version-44 success marker when every
check passes. Tests and reports may contain schema labels, finding/check codes, durations, aggregate
counts, and checksums only—never source rows, people, database URLs, or credentials.

A protected tenant clone rehearsal is an external, explicitly authorized gate. Use the secret-backed
tenant connection inventory, keep output outside the repository, apply 041-044, run authenticated
multi-year/multi-term reads, and record only duration, aggregate counts/checksums, finding codes, and
pass/fail. When a separately reviewed Phase B branch contains migration 045, rehearse that review
copy on the same disposable clone after a current success marker and verify its cleanup manifest.
Never commit clone data or its output. The owned PostgreSQL compatibility suite remains a
separate credentialed external gate and is unrun when its test-only environment is unavailable.

For browser coverage, discovery does not equal execution. Discovery must work without tenant
credentials:

```bash
cd frontend-school
npx playwright test --list tests/e2e/academic-context.spec.ts \
  tests/e2e/academic-core-cutover.spec.ts
```

Execution requires an isolated deployed target plus the dedicated accounts and permissions expected
by those specs. Report Playwright execution, protected-clone rehearsal, PostgreSQL compatibility,
and deployment smoke as `unrun` with the exact missing account, credential, target, or authorization;
do not report discovery as a passing browser workflow and do not silently omit an external gate.

### Gradebook and results cutover rehearsal

Run the Release 2 schema and migration-status audit against disposable local PostgreSQL from the
repository root:

```bash
./scripts/test_backend_school.sh \
  integration_tests::academic::core::schema_tests::migration_060 -- --nocapture --test-threads=1
./scripts/test_backend_school.sh \
  system::handlers::migration::tests::gradebook_results_status \
  -- --nocapture --test-threads=1
```

The audit verifies only durable schema, seed, and permission invariants. Its response contains
bounded check codes, counts, versions, and pass/fail state; it must never contain student identity,
scores, outcomes, credentials, or source rows. A legitimate school edit to grading or evaluation
configuration is not a deployment invariant and must not keep the tenant in maintenance.

The owned PostgreSQL compatibility suite runs the same focused checks on a fresh disposable
Docker database through its direct endpoint. Schema-isolated migration cases use four test
threads so network round trips do not serialize the entire gate, while each individual test pool
still has one connection. The local owner then deletes the database and its container. Use test-only environment variables
from a private local environment file; output must not expose the connection URI.
An authenticated academic smoke samples at most two canonical terms and reads the Gradebook subject
workspace, learner-evaluation subject workspace, and result-readiness summary for each selected term.
Response bodies remain in the smoke script's private temporary directory and are removed on exit.

### PostgreSQL migration compatibility

`./scripts/test_school_database_suite.sh` owns one disposable Docker PostgreSQL 18.6
server and a fresh database for each selection. It includes the former provider-specific
staff, auth, file, academic projection and gradebook regressions. No Neon API key, branch
or source database is required. Selection discovery remains guarded by
`node --test scripts/tests/postgres-compatibility.test.mjs`; a successful command that
selects zero tests is a failure.

Admin PostgreSQL provisioning/credential tests run through
`./scripts/test_backend_admin.sh`. They exercise a non-superuser provisioner, isolated
runtime roles, retry and refusal to delete another tenant's resources. The canonical
pipeline runs these owned fixtures; do not point them at either VPS.

Active migrations remain contiguous from the existing immutable baseline. Preserve
SQLx checksums; do not rewrite or squash an applied migration to upgrade PostgreSQL.
Runtime rollout, data reconciliation and actual-data restore gates are in
[Operations](./OPERATIONS.md).

## Password work and enrollment failure handling

From the repository root, run the shared worker's deterministic cap, admission deadline,
queued/inflight cancellation, current-thread executor progress and real bcrypt compatibility tests:

```bash
cargo test --manifest-path backend-school/Cargo.toml -p school-crypto password_work::tests --locked
./scripts/test_backend_school.sh --package school-auth session_service_tests -- --test-threads=4
./scripts/test_backend_school.sh --package school-auth session_repository_tests -- --test-threads=4
./scripts/test_backend_school.sh admission::application_service_tests -- --test-threads=4
./scripts/test_backend_school.sh provision_admin_username_lookup -- --test-threads=1
./scripts/test_backend_school.sh students::mutation_tests -- --test-threads=4
```

Use the existing Staff creation fixture for staff hash consumers. Auth regressions must fence a
verified login against actual password change/revocation and disabled, deleted, renamed or retyped
accounts without moving bcrypt under the database lock. Enrollment regressions must distinguish
missing optional form/role data from query/role-write failures and verify complete rollback,
including new parent accounts and links. Keep disposable fixtures owned; concurrency tests retain
their intentional races. Limit unrelated password-heavy fixture parallelism to four, matching the
production budget, and test overload explicitly rather than extending its admission deadline.

For before/after observations, use a temporary local controlled fixture with synthetic passwords,
the same locked bcrypt cost, toolchain, machine and workload. Record batch duration and async timer
responsiveness separately; compare direct synchronous baseline work with the shared worker, retain
raw timings in temporary logs and report limitations. Do not present fixture delays as production
p95 or infer a throughput gain from moving CPU work off the async executor. Avoid a permanent
benchmark framework or real accounts/provider calls merely to obtain timing data.

## Encryption and PII

For changes to encryption, national IDs, blind indexes, or admission PII, from `backend-school`:

```bash
cargo test -p school-crypto
./scripts/test_backend_school.sh --package school-admission services::pii::tests
cargo check --workspace --all-targets
```

These focused tests use test-only keys. Never put a real national ID, `ENCRYPTION_KEY`, or `BLIND_INDEX_KEY` in source, fixtures, command output, screenshots, or logs.

## Smoke Tests

The repository smoke script checks frontend reachability, backend liveness/readiness, CORS and CSRF preflight, legacy-cookie rejection, opaque-session login, `/api/auth/me`, the active-session list, realtime SSE, optional private files, and current-session logout.

```bash
SMOKE_SUBDOMAIN=sandbox \
SMOKE_USERNAME=T0001 \
SMOKE_PASSWORD='provided-at-runtime' \
./scripts/smoke_test.sh
```

Alternatively, copy `.env.smoke.example` to the ignored `.env.smoke.local`. The script loads that file by default; `SMOKE_ENV_FILE` can point elsewhere. Credentials must come from `SMOKE_*` environment variables or the ignored environment file and must never be committed.

When `SMOKE_RESOLVE_IP` pins production API hostnames directly to their origin, use the normal
system CA store. Both APIs must present a publicly trusted certificate with the intended names;
production checks never supply `SMOKE_CA_CERT` or `--insecure`. `SMOKE_CA_CERT` remains available
only for explicit synthetic local TLS fixtures and applies to API requests, not the tenant frontend.

If credentials are absent, authenticated checks are skipped. Report that limitation rather than describing the smoke suite as fully passing.

Set `SMOKE_ACADEMIC_CONTEXT=true` only for an explicitly selected cutover tenant. After login, the
script reads the context options, selects at most two canonical year contexts and two canonical term
contexts, and verifies the Academic Core, offerings, assessment, timetable, exams, supervision,
admission, staff-dashboard, Gradebook subject, learner-evaluation subject, and result-readiness read
paths. It stores response bodies only in its private temporary directory and removes them at exit.
During a reviewed maintenance cutover, run this mode through the backend deployment workflow's
VPS-loopback smoke input; never add a public maintenance bypass or open traffic temporarily for the
test. The workflow alone sets `SMOKE_DIRECT_BACKEND=true`; that mode skips only Nginx-owned
CORS/preflight assertions while login, session, CSRF, readiness, and every selected academic read
remain required.

The API-domain cookie is `__Host-schoolorbit_session`; it is opaque and unavailable to frontend JavaScript. Login and authenticated `/api/auth/me` responses expose `X-CSRF-Token`. The smoke script captures that value only in a shell variable, updates it after rotation-capable responses, and sends it on every authenticated `POST`, `PUT`, `PATCH`, or `DELETE`. Never print, export, persist, or enable trace output for the CSRF value. A manual flow should use private temporary files:

```bash
cookie_jar=$(mktemp)
headers_file=$(mktemp)
chmod 0600 "$cookie_jar" "$headers_file"
# Capture X-CSRF-Token from login or /api/auth/me into csrf_token without printing it.
# Remove both files and unset csrf_token when the check ends.
```

### File Platform smoke

Run this only against an isolated tenant with no retained files. The authenticated account must be allowed to update school settings for the public-logo case; the private-profile case operates on the logged-in user. Supply a small valid PNG through `FILE_SMOKE_PNG`; the repository smoke script creates and removes its own private cookie jar. Never commit the PNG, cookie jar, or captured CSRF value.

1. Upload `school_logo` through `POST /api/files`; retain only the returned file ID.
2. Confirm authenticated metadata from `GET /api/files/{id}` contains `publicContentUrl` but no bucket, object key, storage path, provider URL, or signed URL.
3. Confirm anonymous `GET /api/public/files/{id}/content` redirects and delivers the PNG. Also request `GET /api/public/files/{id}/delivery`, retain `data.url` only in memory, fetch it as a separate credential-free request with the tenant `Origin` and no referrer, and confirm a non-empty PNG plus matching `Access-Control-Allow-Origin`. Never print or persist the delivery URL.
4. Upload `profile_image` through `POST /api/files`; confirm anonymous metadata/download fails.
5. Confirm authenticated `POST /api/files/{id}/download` returns a `200` typed grant without bucket, object-key, or provider details. Keep `data.url` in memory, fetch it separately with the tenant `Origin` and credentials omitted, and confirm it writes bytes to a temporary output file. Never print the response body or grant URL because the URL is a temporary bearer credential.
6. Delete each file with `DELETE /api/files/{id}`. Repeat delete through the owning domain workflow where supported, and confirm delivery remains revoked even if object cleanup is pending.
7. Search backend and proxy logs for leaked signed-query markers, object-key prefixes, filenames, or content. A file ID and safe error code are allowed.

Representative requests, with credentials and IDs supplied only at runtime:

```bash
curl -fsS -b "$FILE_SMOKE_COOKIE_JAR" \
  -H "Origin: $SMOKE_ORIGIN" \
  -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
  -H "X-CSRF-Token: $csrf_token" \
  -F purpose=school_logo \
  -F "file=@$FILE_SMOKE_PNG;type=image/png" \
  "$SMOKE_API_URL/api/files"

curl -fsSL \
  -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
  "$SMOKE_API_URL/api/public/files/$PUBLIC_FILE_ID/content" \
  -o /dev/null

curl -fsS -b "$FILE_SMOKE_COOKIE_JAR" \
  -H "Origin: $SMOKE_ORIGIN" \
  -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
  -H "X-CSRF-Token: $csrf_token" \
  -F purpose=profile_image \
  -F "file=@$FILE_SMOKE_PNG;type=image/png" \
  "$SMOKE_API_URL/api/files"

curl -fsS -X DELETE -b "$FILE_SMOKE_COOKIE_JAR" \
  -H "Origin: $SMOKE_ORIGIN" \
  -H "X-School-Subdomain: $SMOKE_SUBDOMAIN" \
  -H "X-CSRF-Token: $csrf_token" \
  "$SMOKE_API_URL/api/files/$FILE_ID" \
  -o /dev/null
```

Exercise private delivery through the typed frontend helper or an in-memory test harness that
parses the grant and provider response without writing or printing the URL. Do not pipe the
grant envelope through verbose shell output or enable HTTP tracing.

For scanner failure behavior in a non-production environment:

1. stop `schoolorbit-clamd`;
2. confirm `/health` remains `200` and `/ready` becomes `503`;
3. confirm a valid upload returns `503` and creates neither ready metadata nor an object;
4. restart clamd, wait for `clamdcheck.sh`, and confirm `/ready` returns `200`;
5. upload the standard EICAR anti-malware test file and confirm it is rejected without becoming ready or publicly deliverable.

Run the focused adapter tests as well:

```bash
./scripts/test_backend_school.sh --package school-file-platform runtime_config::tests -- --nocapture
./scripts/test_backend_school.sh --package school-file-platform malware_scanner::tests -- --nocapture
./scripts/test_backend_school.sh --package school-file-platform r2_storage_provider::tests -- --nocapture
./scripts/test_backend_school.sh --package school-file-platform platform_service::tests -- --nocapture
```

## Browser E2E

Session-cache and hidden-tab regressions can be exercised without a live tenant:

```bash
./scripts/test_backend_school.sh integration_tests::auth -- --nocapture --test-threads=1
./scripts/test_backend_school.sh --package school-notifications http::handlers::tests -- --nocapture --test-threads=1
./scripts/test_backend_school.sh --package school-academic-http websockets::security_tests -- --nocapture --test-threads=1
cd frontend-school
node --test tests/static/realtime-idle.test.mjs tests/static/notification-idle-runtime.test.mjs tests/static/auth-refresh-races.test.mjs tests/static/timetable-socket-runtime.test.mjs
```

Cache tests cover shared query work, tenant isolation, expiry/maintenance deadlines, uncached
previous tokens, invalidation during reads, and cache hits with an unavailable database. A passing
query-count test is not a database cost measurement. Browser execution and deployed-proxy smoke remain
separate required rollout checks.

The notification recovery browser fixture uses real Chromium `EventSource`, the actual central
notification/work/auth stores and HTTP client, and real Nginx with the rendered School SSE/API
locations and Origin maps unchanged. From `frontend-school`, with the matching Playwright Chromium
installed, Docker running, `envsubst` on `PATH`, and local ports 3000/8081 free:

```bash
npx playwright test tests/e2e/notification-recovery-proxy.spec.ts --project=chromium --workers=1
```

The fixture creates and removes one uniquely named container using the same pinned Nginx image as
the executable proxy tests. It serves synthetic HTTP/SSE responses, never accesses a tenant or
requires account credentials, and checks streamed small messages, credentialed CORS/cookie
forwarding, native reconnect and lag catch-up, unavailable/revoked sessions, and late snapshots
after logout. It uses the proxy's existing `http://127.0.0.1:3000` Origin allowance and Chromium's
trustworthy loopback handling for a synthetic Secure/HttpOnly `__Host-schoolorbit_session` cookie;
it does not establish production TLS, cross-host cookie behavior, backend database authorization,
deployed-proxy acceptance or device receipt. Chromium bypasses the external proxy for this local
fixture. Run this focused spec alone because its canonical fixture ports are exclusive.

From `frontend-school`:

```bash
E2E_BASE_URL='https://sandbox.schoolorbit.app' \
E2E_USERNAME='provided-at-runtime' \
E2E_PASSWORD='provided-at-runtime' \
npm run test:e2e
```

`SMOKE_TENANT_URL`, `SMOKE_SUBDOMAIN`, `SMOKE_USERNAME`, and `SMOKE_PASSWORD` are accepted fallbacks only for the non-destructive login spec and the explicitly enabled readonly route acceptance spec. The backend sets `__Host-schoolorbit_session` for the API domain, so assertions inspect Playwright browser-context cookies rather than only cookies visible for the tenant page domain.

Run destructive multi-context session coverage separately with a dedicated disposable account:

```bash
E2E_BASE_URL='https://sandbox.schoolorbit.app' \
E2E_API_URL='https://school-api.schoolorbit.app' \
E2E_SESSION_USERNAME='dedicated-disposable-account' \
E2E_SESSION_PASSWORD='provided-at-runtime' \
npx playwright test tests/e2e/session-security.spec.ts
```

Set `E2E_OTHER_TENANT_URL` to another tenant when tenant-isolation proof is available; only that optional case skips when the variable is absent. The suite never changes the account password.

### School font library

Run central authorization, inspection, atomic attach, reference-safe delete, and File Platform relationship checks from the repository root:

```bash
./scripts/test_backend_school.sh --package school-fonts -- --nocapture --test-threads=1
./scripts/test_backend_school.sh --package school-certificates -- --nocapture --test-threads=1
./scripts/test_backend_school.sh --package school-certificates-http handlers::tests -- --nocapture --test-threads=1
```

Run the generated-contract, reusable-uploader, manager-only route, upload retry/cleanup, and delete-conflict coverage from `frontend-school`:

```bash
node --test tests/static/school-font-library.test.mjs \
  tests/static/certificate-*.test.mjs --test-concurrency=1
npx playwright test tests/e2e/school-font-library.spec.ts \
  tests/e2e/certificate-editor.spec.ts \
  tests/e2e/certificate-renderer.spec.ts --workers=1
npx playwright test --list tests/e2e/certificate-lifecycle.spec.ts --workers=1
```

The non-live browser suites use local harnesses and do not mutate a tenant. The live certificate lifecycle is separate: it uploads one private `school_font` through an exact template context, attaches and renders with it, proves the font survives campaign purge in the central manager list with a zero reference count, and deletes it only through `DELETE /api/school-fonts/{font_id}` afterward. Do not substitute direct File Platform deletion for this cleanup sequence.

Run the complete certificate lifecycle against an isolated tenant with dedicated preparer, issuer, and student accounts. From the repository root, run the focused backend checks first:

```bash
./scripts/test_backend_school.sh --package school-certificates -- --nocapture --test-threads=1
./scripts/test_backend_school.sh --package school-certificates-http handlers::tests -- --nocapture --test-threads=1
CARGO_BUILD_JOBS=1 cargo test --manifest-path backend-school/Cargo.toml --test static_architecture certificate_runtime_keeps_handlers_thin_proofs_private_and_renders_ephemeral -- --exact --test-threads=1
```

Then, from `frontend-school`, run the static contract and browser discovery before the live lifecycle:

```bash
node --test tests/static/certificate-*.test.mjs --test-concurrency=1
npx playwright test --list tests/e2e/certificate-lifecycle.spec.ts --workers=1
npx playwright test tests/e2e/certificate-lifecycle.spec.ts --workers=1
```

The live lifecycle requires all of these variables, supplied only at runtime:

- `E2E_CERT_PREPARER_USERNAME`
- `E2E_CERT_PREPARER_PASSWORD`
- `E2E_CERT_ISSUER_USERNAME`
- `E2E_CERT_ISSUER_PASSWORD`
- `E2E_CERT_STUDENT_USERNAME`
- `E2E_CERT_STUDENT_PASSWORD`

The three accounts must be distinct. The preparer needs exact-unit campaign/template/candidate/submit/delete access, `font.manage.school` for the post-purge central list/delete assertion, and no school issue access; the issuer needs school read, issue, revoke, and download access; and the student must be an active linked student account. The isolated tenant also needs a current academic year, a second active organization unit outside the preparer's owner options, and working private-file storage and scanning.

Do not print or retain credential values, recipient data, verification proofs, render receipts, or delivery grants. Run this destructive lifecycle only against an isolated tenant: the fixture permanently purges the campaign it creates through the guarded impact/start/status API, then verifies that its issued and replacement certificates, own-certificate rows, campaign-owned file objects, and file metadata are unavailable while the school font survives campaign purge. It then confirms the font reference count is zero and removes the font through the central delete API. A failed purge is retried only through the supported purge API and the fixture still attempts the same guarded cleanup from `finally`. If any required variable is unavailable, the `--list` command must still pass; report the live lifecycle as unrun, not passing or skipped coverage.

Use `npm run test:e2e:headed` only when interactive debugging is needed. Retain traces, screenshots, and videos only when they contain no sensitive data.

## Realtime Rollout Checks

When WebSocket identity, proxying, or authentication changes:

1. Verify the handshake without legacy query identity fields.
2. Search proxy/backend access logs for unexpected query parameters, while ensuring sensitive query strings are not logged.
3. Confirm no deployed client sends legacy query identity parameters: `user_id`, `name`, or `school_key`.
4. Confirm the backend derives actor and tenant identity from authenticated server context and fails closed when authorization is lost.
5. Confirm ping/pong heartbeat, stale-client cleanup, reconnect backoff, and permission-change disconnect/refresh behavior through the reverse proxy.

Do not log tokens, cookies, identity query values, or full request URIs during rollout checks.

## Ubuntu 26.04 Playwright

Until Playwright provides native Ubuntu 26.04 browser builds, install and run with:

```bash
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 npx playwright install chromium
PLAYWRIGHT_HOST_PLATFORM_OVERRIDE=ubuntu24.04-x64 npm run test:e2e
```

Do not rely on `npx playwright install-deps` on Ubuntu 26.04 when it requests unavailable Ubuntu 24.04 packages. Install required native packages explicitly when needed:

```bash
sudo apt install -y libnspr4 libnss3 libasound2t64 libxss1 fonts-liberation
```

The sandbox E2E workflow uses Ubuntu 24.04.

## Date calendar and activity requests

Run `cargo test -p school-calendar services::tests` and `cargo test -p school-calendar requests::tests` for pure date/time validation and request authorization policy; run `./scripts/test_backend_school.sh --package school-calendar` from the repository root for the complete package, including reminder fairness/cancellation database cases. Run `./scripts/test_backend_school.sh integration_tests::calendar -- --nocapture` from the repository root for database coverage: own-request isolation, manager decisions, failed approval rollback, concurrent approval deduplication, and migration 098 preservation of existing events, audience scope, reminders and timestamps. The request notification cases compare recipient resolution with actor authorization across roles, organization positions, wildcard and delegated grants; exclude inactive users and expired/revoked grants when resolving the notification recipients; and verify immediate inbox visibility with central dispatch acceptance, tenant/user-scoped realtime delivery, concurrent decision deduplication and committed outcomes when notification enqueue fails. Calendar retains its business post-commit enqueue boundary; these checks do not imply atomicity between its business mutation and central enqueue or external device receipt. Use disposable PostgreSQL and synthetic fixtures; never point this suite at a tenant database.

Against a local frontend preview, run `E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/calendar-region-loading.spec.ts tests/e2e/calendar-requests.spec.ts tests/e2e/calendar-pending-overlay.spec.ts tests/e2e/calendar-timed-display.spec.ts tests/e2e/calendar-event-popovers.spec.ts tests/e2e/calendar-embed-dialog.spec.ts tests/e2e/calendar-action-menus.spec.ts --project=chromium`. The fixtures use synthetic staff sessions and intercept calendar APIs. Action-menu cases cover header and selected-day creation menus, date inheritance, manager/requester/reader permissions, disabled catalog actions, settings tools and the absence of an extra empty-month box. They do not exercise a deployed database. The request tests cover selected-day defaults in a future month, timed defaults, 24-hour compact input normalization, invalid time refusal, minimal payloads, failure draft retention, own status tracking, approval and rejection, including desktop and mobile layouts. Timed-display cases inspect centered dates, continuous all-day bars, per-day timed dots, retained edit values and desktop/mobile light/dark layouts; pure helper tests cover time boundaries, week-spanning layout and per-day timed lane packing. Event popover cases verify full-width staff calendars, direct event selection, complete details, manager/reader actions, keyboard focus return, outside mouse/touch dismissal without reopening another day, context changes and mobile/desktop light/dark layouts. Region-loading cases verify stable paired navigation buttons and an immediately visible new-month grid during delayed event reads. Public and staff mobile cases hide grid times while retaining full times in details and on desktop. Pending overlay cases cover inline manager approval/rejection with local draft retry, lazy detail loading and cancellation, compact filter popovers, removed notices, lazy reads, multiday and timed markers, toggle cancellation, month/history ownership, independent errors/retry, explicit overflow, scope controls and mutation patches. The calendar database suite also verifies oldest-first review paging and ties with approved requests excluded before pagination, manager-only detail access, newest-first own history, pending-only date overlap, private scope and bounded overlay results.

## Attendance and supervised webcam check-in

For per-person note removal, run `./scripts/test_backend_school.sh attendance_note_migration -- --test-threads=1 --nocapture` with the documented local Cargo profile against its owned Docker PostgreSQL fixture. It covers non-note record equality, retained live and orphaned audit identities/results/revisions, calendar/unrelated note preservation, rejected unknown or partial snapshots, reconciliation failure before cleanup and retry. The ordinary Attendance suite derives the latest version from the canonical migration timeline; historical cutover fixtures retain their explicitly requested predecessors through `fixture_at`. The generated contract and domain input tests reject removed per-person note fields.

The Attendance browser suite checks seven visible single-selection status buttons plus explicit unchecked clearing, one-tap changes, repeated-tap retention, keyboard focus/activation, disabled read-only access and per-person note absence. Exercise narrow mobile and desktop light/dark layouts against a ready production build, together with sparse saves, correction reasons, same-day arrival context, special-round score overrides and overlapping scan conflicts. No browser fixture mutates a real tenant.

Run `cargo test -p school-attendance` from `backend-school` for result validation, calendar rules and ambiguous-face rejection. From the repository root, run `ENCRYPTION_KEY=$(openssl rand -hex 32) ./scripts/test_backend_school.sh attendance -- --test-threads=4` against the script-owned Docker PostgreSQL fixture. These synthetic cases exercise actual dated academic providers, read-only workspace loading, first-save calendar defaults with stale-revision refusal, read-only previews and kiosk preparation, sparse first saves and unchecked coverage, all-guardian notification deduplication, digest deduplication, holiday/cancelled-round exclusion, special-round overlap, encrypted templates with stale-gallery rejection after withdrawal/re-enrollment, mixed own/assigned history and private-evidence isolation, school-writer configuration access, accepted evidence and bounded expiry/reconciliation across multiple batches, and verified term purge with retained teacher-scoped summaries, migration 103→104→105 preservation/refusal/retry, and module reset with stale-token refusal and Attendance-only evidence cleanup. Never use a tenant database for this suite. For focused concurrency checks, run `./scripts/test_backend_school.sh attendance_history_snapshot -- --test-threads=1` and `./scripts/test_backend_school.sh attendance_report_snapshot -- --test-threads=1` from the repository root; these verify parallel readers and first-save progress without mixed snapshot facts, snapshots queued behind reset, and exclusion against reset, purge and lifecycle changes. Central notification transport checks belong to `school-notifications`; the Attendance suite retains domain recipient/deduplication, history scope, reset and purge coverage. These fixture checks do not establish production capacity or browser/WebPush receipt.

Run `./scripts/test_backend_school.sh attendance_history_student_scope -- --test-threads=1 --nocapture` for dated Core/Delivery provider equivalence and frozen-roster exclusion before and after cancellation. Its disposable 100-room, 3,000-pupil fixture reports full versus selected-student row counts and median snapshot hold time without asserting machine-dependent timings; these measurements cover provider work rather than production request capacity. Keep the existing transfer, history/report snapshot, virtual coverage and mixed-permission cases when changing this path.

From the repository root, run `./scripts/test_backend_school.sh notification_dispatch -- --test-threads=4` for central provider/runtime coverage: transactional deferred enqueue and immediate direct inbox visibility, disjoint bounded claims and stale-token fencing, direct acceptance refusing missing/inactive recipients, immutable queued recipients surviving later deactivation/guardian unlink, identity drift and missing stored inbox refusal, exact-source/owned cancellation, and fair two-tenant backlog delivery. This root module filter is separate from the external migration test target below. From `backend-school`, run `cargo test -p school-notifications runtime::tests::delivery_batch_bounds_concurrency_and_joins_failed_and_timed_out_siblings` for the central worker's four-item bound, independent sibling outcomes and timeout cancellation without detached work. From the repository root, run `./scripts/test_backend_school.sh --integration notification_dispatch_migration -- --test-threads=1` against the owned disposable PostgreSQL fixture. Its four cases verify complete queue/ledger/inbox state preservation and stable replay across migrations 119/120, atomic refusal of identity/payload/inbox conflicts followed by canonical retry, reconciliation-drift refusal before contraction, and rejection of unsupported source shapes. Existing inbox read timestamps, terminal/suppressed job state and intentionally absent inbox rows must remain unchanged; replacing IDs or truncating source payloads is not a valid migration repair.

Against a local School frontend, run `E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/attendance.spec.ts --project=chromium --workers=1` from `frontend-school`. Its intercepted synthetic APIs and camera driver test unsaved-input preservation, repeated reactive settings saves, typed partial result patches, unchecked preservation, additions without correction reasons, read-only previews, calendar overrides, independent settings/options failure and focused retry, saved-versus-draft weekday rules, exact route/action permissions with immediate identity disposal and delayed-response rejection, malformed-date recovery, pending-month write exclusion, report-only parent retries, Bangkok-midnight camera cleanup, special audience/teacher creation, enrollment context/stop cleanup, separate student history, desktop/mobile light/dark layouts, and acknowledgment only after evidence upload and scan commit. The route-glob matching guard exercises Vite's actual glob implementation against all app route metadata owners so literal route-group parentheses cannot silently empty the permission or preview registry. A separate case loads the real pinned models against a blank synthetic webcam and rejects face enrollment. This does not measure physical recognition accuracy or demonstrate resistance to photo/video spoofing.

Before operational use, enroll consenting synthetic/test subjects on the intended HTTPS tenant, pair an operator and webcam, and check single-person capture, lighting, look-alike refusal, present/late cutoff in Asia/Bangkok, manual fallback, private image access for linked guardians, every-guardian delivery, unchanged-save deduplication and correction messages. Test both selected and unselected calendar days. Verify scheduled digests and offline/retry behavior using dedicated disposable accounts; production acceptance remains separate from fixture results.

## Behavior scores and Attendance delivery

The central School database suite runs `school-behavior` against its own fresh Docker database. For diagnosis, run `./scripts/test_backend_school.sh --package school-behavior -- --test-threads=4` from the repository root using the documented local Cargo profile. These fixtures cover mixed and unassigned rosters, assigned/own access unions, immutable item versions, group replay and cancellation, source correction and frozen zero-point decisions, split-side cycles, same-day reset, physical detail deletion, stale preview refusal, source suppression and durable cleanup jobs exceeding 20,000 entries. The Attendance database suite also covers midnight finalization, counted and cancelled rounds, catch-up progress and monotonic score-delivery revisions. Test fixtures never use a tenant database.

From `frontend-school`, run `E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/behavior-workflow.spec.ts tests/e2e/attendance.spec.ts --project=chromium --workers=1` against a ready local frontend. Attendance identity-change fixtures require Vite development module imports; also verify production build and the Behavior fixtures against a production preview. Behavior fixtures exercise read-first loading, mixed student selection and conflict draft retention, separate current/history totals, own cancellation, exact source correction links, pending cleanup and narrow mobile layouts in both themes. Intercepted APIs establish browser behavior; the Docker fixtures establish database ownership and replay behavior.

`school-behavior` owns score models and services; application adapters own Attendance delivery and lifecycle ordering. Pure domain tests can run with `cargo test -p school-behavior --lib`. Keep fixture checks distinct: a passing pure test without `TEST_DATABASE_URL` does not verify database mutations.

## Student Care assessments and support

Student Care is independent of Behavior points. Its domain and resource policy live in
`backend-school/crates/school-student-care/`; the frontend route loaders, forms and typed
draft transformations live in `frontend-school/src/lib/features/student-care/`,
`frontend-school/src/lib/components/student-care/` and the dedicated Student Care routes.
Use the existing disposable database runner for persistence, encryption, replay and access
checks; never point these fixtures at a tenant database:

```bash
./scripts/test_backend_school.sh --package school-student-care -- --test-threads=1
./scripts/test_backend_school.sh concrete_student_care_routes -- --test-threads=1
```

From `frontend-school`, run the feature helpers and route-loading guards, then the
synthetic Chromium workflows against a ready local frontend:

```bash
node --test tests/static/student-care-drafts.test.mjs tests/static/route-data-loading-inventory.test.mjs tests/static/route-data-loading-policy.test.mjs
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/student-care.spec.ts tests/e2e/student-care-usability.spec.ts --project=chromium --workers=1
```

The browser fixtures use explicitly synthetic, nonclinical instruments. They exercise
validated library configuration without instrument statuses, pinned round versions,
respondent/reviewer assignment, server draft failure/retry/reopen and final submission,
paper-score provenance, guardian restrictions, resource-specific mixed permissions,
visit planning/completion, case sources and event history, late-response disposal, and
reviewer handoff without reading private responses. The usability suite checks rendered
route regions, long names, accessible controls and navigation on narrow and desktop layouts.
Light/dark fixtures use the application's `ui-preferences` theme setting and verify the
rendered theme. Keyboard and emulated Chromium touch workflows cover picker selection,
clearing and retention, with native page scrolling at both bounded-list boundaries.
Representative screenshots include scrolled lower form regions. Keep this evidence separate
from physical-device testing, publisher authorization, approved clinical
content/interpretation and real-tenant staff/guardian acceptance. SDQ digital content must
not be reproduced without prior publisher authorization; manual-result mode does not
provide questionnaire content or invented norms. Follow `.rules` section 11 for generated
contract/permission, Svelte/TypeScript, focused lint and production-build checks.

## Budget workspace

Budget planning, reservations, payments and the append-only financial ledger belong to
[`school-budget`](../backend-school/crates/school-budget/src/lib.rs). From the repository root,
run its package suite through the owned disposable PostgreSQL runner:

```bash
./scripts/test_backend_school.sh --package school-budget -- --test-threads=4
```

The focused fixtures exercise plan approval/amendments, exact money and quantity rounding,
allocation credits above an individual line estimate, concurrent balance checks, partial payments,
closure release, adjustments/loans, ledger reconciliation, denied access and transaction rollback.
Include migration 130 attempt ownership and known adjustment-revision preservation in release
verification; unknown prior draft documents must never be fabricated. Select affected provider,
HTTP, API/permission-contract and workspace checks under `.rules` section 11 in addition to this
domain suite; a package result alone does not prove browser or deployed behavior.

From `frontend-school`, using the pinned repository Node version and a ready loopback frontend:

```bash
node --test tests/static/budget-money.test.mjs tests/static/budget-drafts.test.mjs
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/budget-workspace.spec.ts tests/e2e/budget-settings.spec.ts --project=chromium --workers=1
```

Helper tests cover integer-satang strings beyond safe browser integers, at-most-three-decimal
quantities, one exact half-up rounding step, mutation intent, drafts and pivot totals. The browser
spec uses synthetic intercepted APIs for the minimal project form, plan/request/payment flow,
regional retry, permissions, settings identities, explicit bulk defaults, amendment context,
allocation choices and mobile/desktop light/dark reports. Settings fixtures also cover adding routing steps, remote recipient search and selection, draft retention and retry, and aligned controls on mobile/desktop in both themes. It is not evidence of real tenant
transactions or private object storage. For deployed acceptance use an isolated disposable budget
project and authorized accounts in a test tenant: reconcile reservations, paid/available balances,
adjustments, outstanding loans and exports; confirm optional evidence remains private and payment
requires a voucher number. Exercise actual allowed/denied evidence reads and process transitions
without logging private file grants or contents. Record commands and observed results separately;
these recipes do not claim that any check has passed.

## Supply catalog

Supply catalog ownership is in `backend-school/crates/school-administration/src/supply_catalog/`, its reusable access policy and migrations 110–112. It does not create projects. Run its focused transactional tests on an owned disposable PostgreSQL fixture:

```bash
./scripts/test_backend_school.sh integration_tests::supply_catalog -- --test-threads=4
cargo test --manifest-path backend-school/Cargo.toml -p school-file-platform --lib catalog_images
cargo test --manifest-path backend-school/Cargo.toml -p school-file-platform --lib supply
```

The domain fixtures cover exact duplicates, simultaneous price appends, current-price bundle totals, actor ownership, clean/unexpired file attachment, proposal approval/rejection and rollback, current-image replacement, and attachment versus generic deletion. Provider failures are synthetic and leave durable object-deletion work. Image cases cover JPEG/PNG/WebP/BMP/GIF/TIFF normalization, transparency, camera orientation/metadata removal, and malformed/oversized input. HEIC/HEIF conversion uses the existing browser-only decoder; AVIF conversion needs the browser decoder.

From `frontend-school`, run exact money/draft helper coverage and the synthetic browser workflow against a ready local frontend:

```bash
node --test tests/static/supply-catalog-money.test.mjs
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/supply-catalog.spec.ts --project=chromium --workers=1
```

Keep fixture browser evidence distinct from real device-format and private-storage/scanner acceptance. Use only disposable catalog items/proposals in an isolated tenant for the latter: verify uploads from iPhone/Android/desktop, replacement cleanup, pending-source private access and permanent source removal after approval. Do not print grants, source URLs or private contents. Select final verification under `.rules` section 11 and reuse valid scoped evidence under section 1.

For bilingual staff identity and multiple qualifications, use `./scripts/test_backend_school.sh --package school-staff staff_profile_ -- --test-threads=1 --nocapture` and `./scripts/test_backend_school.sh --package school-auth bilingual_ -- --nocapture`. These cover language-set validation, Thai preference/English fallback, both-language search, pagination, raw-value preservation, partial legacy qualifications, revision and ownership rejection, rollback, migration retry, and failed/missing/stale reconciliation. Run the changed personnel form/draft static files directly and `tests/e2e/staff-bilingual-profile.spec.ts` against a ready frontend preview for responsive create/edit/detail flows; retain the focused personnel workflow cases when changing their shared controls. The all-tenant migration gate is exercised by `node --test scripts/tests/migration-completion-gate.test.mjs`.

## Admin customer account preservation

Run `./scripts/test_backend_admin.sh` for the Admin migration and account cutover.
The fixture also exercises first-release CLI preflight before schema changes,
bootstrap-secret requirements, unchanged account/password/name on retries,
customer-adoption and conflicting-operator refusal, pending-login denial,
expired/active verification-link readiness, and subsequent releases without
bootstrap secrets. It never sends real email. Run
`node --test scripts/tests/admin-account-release.test.mjs` for the remote account
helper's immutable-image selection, private environment transport and failure
propagation; the deployment verification scope includes this test.
`node --test scripts/tests/admin-worker-bootstrap.test.mjs` covers exact Admin
service lookup, recognized absence, existing-worker preservation, fail-closed
permission/transport/response errors and the isolated 503 bootstrap configuration.
It also covers exact-alias ownership, idempotent retirement, malformed/ambiguous
lookups, failed deletion, delayed absence and protection of unrelated domains.
Use the owning locked Wrangler `deploy --dry-run` against that generated bootstrap
to verify packaging without publishing code. Actual Worker recreation and route
acceptance require a reviewed production release and are separate from offline tests.
The disposable PostgreSQL suite covers complete school/deployment-row preservation,
unknown-FK rollback, repeated migrations, expired-but-accessible schools, scoped
memberships, idempotent requests/concurrent approval, opaque session revocation,
legacy cookie denial, exact-Origin/CSRF checks, verification/reset expiry and replay,
and unverified registration without role escalation. Missing Resend configuration
is tested without sending email. Real Resend delivery and sender-domain readiness
require explicit cutover acceptance with the operator's dedicated account.

Admin checks: `npm run lint`, `npm run check`, `npm run test:unit`, and
`npm run build` from `frontend-admin`. Run the deployment suite for apex routes,
installer/runtime configuration and Worker promotion. Public deployment smoke and
real email acceptance must be reported unrun until the required target/credentials
are configured; successful compilation is not proof of production cutover.

For the isolated Admin browser fixture, start an HTTP fixture on
`127.0.0.1:8089` that returns `{ "status": "ready" }` at `/deployment-status`, then
run the Admin dev server with `PUBLIC_API_URL=http://127.0.0.1:8089` on port 4174
(the existing local TLS plugin serves HTTPS). From `frontend-school`, run
`npx playwright test tests/e2e/admin-customer-portal.spec.ts --project=chromium --workers=1`.
The browser intercepts fixture API calls, aborts external fonts, and covers
registration/explicit verification, email login, operator approval/CSRF, denied
operator rendering, expiry warnings and mobile password reset. It uses only
synthetic accounts and cannot contact Resend. `E2E_ADMIN_URL` can select another
isolated Admin preview; never point this mocked fixture at a production tenant.

## Organization and learning areas

The organization route owns separate administration and learning-area sections. The academic
subject-group route redirects to the latter. Run the affected service and scope checks on
disposable PostgreSQL, then the mocked browser workflow against the local frontend preview:

```bash
./scripts/test_backend_school.sh --package school-academic-core learning_area -- --test-threads=4
./scripts/test_backend_school.sh --package school-staff organization_auto_codes -- --test-threads=1
cargo test --manifest-path backend-school/Cargo.toml -p school-academic-http learning_area_registry --lib
./scripts/test_backend_school.sh organization_learning_area_cutover -- --test-threads=1
```

From `frontend-school`, run API contract generation/check/tests, `npm run check`, focused
ESLint/Prettier and `E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test
tests/e2e/organization-learning-areas.spec.ts tests/e2e/organization-region-loading.spec.ts
tests/e2e/academic-catalog-route-loading.spec.ts --project=chromium --workers=2`.
The fixtures cover academic-only access, absent code inputs, failed-draft retention,
local mutation reconciliation, lazy member reads, URL/history selection and mobile/desktop
light/dark layouts. Database cases exercise pair rollback, concurrent codes, stale revisions,
active-catalog deactivation refusal and concurrent creation/deactivation in both lock orders,
migration preflight/retry and identity/grant preservation.

Organization display membership derives from direct assignments through every active administrative descendant;
exact-unit grants continue to use direct membership. Run the domain and public projection regressions:

```bash
./scripts/test_backend_school.sh --package school-staff organization_membership_ -- --test-threads=2
./scripts/test_backend_school.sh --package school-administration public_organization_reports_actual_learning_area -- --test-threads=1
```

These disposable fixtures cover deep/cyclic display traversal, source preservation and permission isolation,
independent dated primary domains and raw-writer concurrency, migration refusal/retry, membership history
reconciliation, and permanent deletion refusing referenced identities. Permanent deletion uses a separate
operation from deactivation and protects the school root and linked learning-area identities.

## Admin Worker creation and recovery

Run `node --test scripts/tests/admin-worker-bootstrap.test.mjs` for exact Admin
service lookup, recognized absence, existing-worker preservation, fail-closed
permission/transport/response errors and the isolated 503 bootstrap configuration.
It also covers fixed-name deletion without force, already-absent retries,
deletion refusal, frontend-only scope and the successful preparation gate required
before deliberate recreation.
The deployment verification scope includes these cases. Use the owning locked
Wrangler `deploy --dry-run` against the generated bootstrap to verify packaging
without publishing code. Actual Worker recreation and route acceptance require
a reviewed production release and are separate from offline tests.

Tenant directory durable barriers are covered by the Admin `tenant_directory_barriers`
integration test through the existing disposable Admin database runner. It verifies
pending get/list/migration-connection denial after a School restart, transaction
rollback, exact-token recovery and refusal of a stale writer claim. Never use a
shared control-plane database for this fixture.

## Correspondence and reusable processes

The Workflow owner contains pure validation/geometry tests and disposable database cases for
registration sequences, scope union, frozen recipients, all/any progression, return/resend,
concurrent/replayed decisions, image signature ownership and immutable file/document evidence.
Use the same pinned toolchain and shared Cargo target as other School checks:

```bash
./scripts/test_backend_school.sh --package school-workflow -- --test-threads=2
cd backend-school
WORKFLOW_PDF_FIXTURE_DIR=/tmp/schoolorbit-workflow-renderer cargo test -p school-workflow --lib document_export::tests
cargo test -p school-file-platform --lib private_original_consumer_checks_ready_visibility_and_purpose_byte_bound
```

The optional renderer directory receives synthetic Thai, cropped/native-rotation, annotation,
drawn-signature and private-image-signature PDF fixtures. Inspect both PDFs through the real
browser PDF renderer; source preview and final composition must share visual page coordinates,
Thai mark positioning and original crop/rotation. Fixture output contains no tenant data.
Default and `api-docs` builds, generated API/permission checks, focused Files policy tests and
independent frontend/browser checks also apply to changes across these boundaries.

For list usability and saved-draft metadata changes, the focused correspondence target covers
counts beyond the first 30 items, authorization before aggregation/pagination, independent view
and cursor filtering, register-number/year search, current/future/all/any/reassigned recipients,
frozen names/deadlines and action/signature permissions. It also verifies draft metadata
validation, atomic revisions, author-only access, returned-record denial and old idempotency
payload preservation. It also covers concurrent first-render read receipts, original revision/time
preservation, read-versus-acknowledgment authority, newly activated slot timestamps, historical/current
render projection boundaries, no-op projection effects, explicit-read replay,
formatted-label search and preserved numbering/profile snapshots. Synthetic migration 128 cases
backfill the earliest valid immutable Read and refuse missing-document or ambiguous first-read
evidence without changing audit rows:

```bash
./scripts/test_backend_school.sh --package school-workflow --test correspondence -- --test-threads=2
```

Run the generated API generation/check/tests after changing the list or revision DTOs. Frontend
correspondence browser fixtures should verify server-count tabs, creation/draft retention,
failed-file retry, recipient selection, timeline/main action, direct signature placement and
starter template copying at desktop and mobile sizes.

The renderer fixtures include bordered Thai text with four-point inner padding and cropped pages
at all native rotations. Text fitting must satisfy both width and height, retain attached Thai marks,
and refuse a box that cannot fit at the minimum size; frontend helpers also normalize tabs/CRLF.
Pure Workflow settings tests cover placeholder/format bounds, UUIDv7
template identities and pre-reader payload serialization. Frontend reading fixtures should report
a read only after successful page rendering, retain retry/cancellation behavior, open the current
revision automatically, focus invalid fields, and distinguish registration dates from insertion
dates when expanding editable stamp text. These synthetic checks do not measure production
database size, rendering capacity, or prove that a human read the content.

Text annotation layout is versioned: missing `textLayoutVersion` preserves accepted legacy
height-only fitting, while version 1 enforces width and height. The pure renderer cases preserve
legacy narrow-glyph export and later signing validation, and reject new strict boxes that cannot
fit at 5pt. Disposable correspondence cases cover old create/action/revision replay, unchanged
legacy base-annotation carryforward, subsequent signing, and rejection of missing/unknown new
versions or edited unversioned text. Omitted legacy fields must keep old payload hashes intact.

Staff leave and planned school exit use `./scripts/test_backend_school.sh --package school-staff leave -- --nocapture` for the focused policy and isolated lifecycle/authorization/ledger fixtures. Signature consent and revision binding use `./scripts/test_backend_school.sh --package school-workflow signature_requires -- --nocapture`. Policies count school-configured calendar or working dates; half-day granularity represents a full counted day as two units. Quota-year crossing requires separate requests. No statutory entitlement is seeded by these fixtures.

Directed routing uses synthetic staff and document fixtures in
`frontend-school/tests/e2e/correspondence-directed.spec.ts`. After building the School frontend
with `PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run build`, start an
owned production preview with `npm run preview -- --host 127.0.0.1 --port 4183 --strictPort`.
Run from `frontend-school`:

```bash
E2E_BASE_URL=http://127.0.0.1:4183 npx playwright test tests/e2e/correspondence-directed.spec.ts tests/e2e/correspondence-workflow.spec.ts tests/e2e/correspondence-usability.spec.ts tests/e2e/correspondence-reader.spec.ts tests/e2e/correspondence-text-stamps.spec.ts --grep-invert 'revoked identity|same-revision accepted annotation reconciles|delayed accepted action preserves|historical revision selection' --project=chromium --workers=2
node --test tests/static/correspondence-*.test.mjs
```

The identity/realtime workflow cases named by that exclusion exercise development
module imports. Run them against a separate owned development server started with
`PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run dev -- --host 127.0.0.1 --port 4173 --strictPort`:

```bash
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/correspondence-workflow.spec.ts --grep 'revoked identity|same-revision accepted annotation reconciles|delayed accepted action preserves|historical revision selection' --project=chromium --workers=1
```

The browser fixtures check the first-decision-maker default and backtracking, exact replay after
an accepted send response is lost, branch-specific instruction/annotation/signature payloads,
copy-only actions, explicit and inherited review, first-assignee completion and per-register
settings. The wizard covers narrow/desktop light/dark layouts; these intercepted APIs do not
establish database authorization or deployed delivery. Retain the Workflow disposable database
coverage for recipient resolution, graph completion, review constraints and replay safety.

Subject catalog revision regression coverage uses the existing disposable PostgreSQL runner:

```bash
./scripts/test_backend_school.sh subject_revision -- --test-threads=2
./scripts/test_backend_school.sh catalog_overview -- --test-threads=2
```

These fixtures verify migration 131 preserves published history, pinned curriculum/offering
references and unrelated subjects, permits same-year/same-date revisions, rejects semantic
duplicates with unordered grade sets and canonical decimal equality, allows historical
restoration, serializes concurrent commands, and selects current/upcoming revisions with
exclusive effective ends. Historical migration 041 overlap coverage remains valid before
131; activity overlap and published immutability protections remain in force.
The subject new-version form retains the source snapshot dates, description and term
when prefilled, preserves the working draft on background refresh, and allows same-day
grade-only changes. The provider returns the existing conflict response for exact semantic
duplicates. Run the synthetic `subject-catalog-workspace.spec.ts` Playwright workflow
against an owned local preview for the form and history behavior.
From `frontend-school`, also run the typed draft helper cases explicitly (the generic
static suite does not include this runtime target):

```bash
node --experimental-strip-types --test tests/runtime/catalog-version-draft.test.ts
```

Curriculum additions and preserved subject revisions use these focused disposable fixtures:

```bash
./scripts/test_backend_school.sh curriculum_current_subject -- --test-threads=2
cargo test --manifest-path backend-school/Cargo.toml -p school-academic-core --lib revision_selection_tests
```

The provider cases cover Bangkok midnight, same-day revision ordering and exclusive ends.
Database cases exercise old/current option metadata, no future/expired/draft fallback,
unchanged existing and copied version IDs, current-registry outdated warnings (including
historical publication reads), no warnings from future/draft/expired-only revisions or a higher
future pin, explicit replacement, grandfathered duplicate buckets, atomic stale-publication rejection under stable-subject locks, and activity parity.
Run affected curriculum structure/copy and curriculum permission cases alongside them.
Regenerate and check/test API contracts after DTO changes; compile affected Core/HTTP
consumers both with default features and `api-docs`. These fixtures never use tenant data.
From `frontend-school`, run `node --experimental-strip-types --test
tests/runtime/curriculum-catalog-selection.test.ts` and the synthetic browser workflow
`tests/e2e/curriculum-catalog-revisions.spec.ts` against an owned local preview. The
fixtures cover current-only additions, preserved pins with current-registry outdated badges
and hidden revision numbers, explicit replacement, legacy duplicate buckets and conflict retention in the editor and read-only tables.


Curriculum matrix contracts and default removal use the same owned disposable PostgreSQL runner:

```bash
CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 ./scripts/test_backend_school.sh curriculum_program_default -- --test-threads=1
./scripts/test_backend_school.sh curriculum_current_subject -- --test-threads=2
./scripts/test_backend_school.sh curriculum_publication -- --test-threads=2
./scripts/test_backend_school.sh curriculum_structure -- --test-threads=2
cargo test --manifest-path backend-school/Cargo.toml -p school-academic-core --lib study_program_rejects_removed_default_input
```

The 132→133 fixture keeps a deliberately different active amendment and immutable history,
compares all surviving program data including identities/revisions/timestamps, retries the
canonical migration, then discards and captures another publication. The clean fixture
checks migration/retry and preserved bell-schedule defaults. Migration assertions independently
verify graph/reference and published-view counts and checksums. Current-subject cases verify
immutable grade/section options, pinned historical subject-group metadata, activity null groups,
current-only admission and unchanged denied access. Run affected copy/options and curriculum
permission cases, Core/HTTP in default and `api-docs` modes, schema/Serde removed-input tests,
and the existing API generate/check/test commands. These checks never use production data.


From `frontend-school`, run the matrix and picker helper cases with the pinned Node 24 runtime:

```bash
node --experimental-strip-types --test tests/runtime/curriculum-structure.test.ts tests/runtime/curriculum-catalog-selection.test.ts
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/curriculum-structure-matrix.spec.ts tests/e2e/curriculum-catalog-revisions.spec.ts tests/e2e/curriculum-delivery-alignment.spec.ts --project=chromium
```

Use an owned local preview for the synthetic intercepted-API browser fixtures. They cover
all configured terms (including empty and more than two), grade and classification filters,
conservative pairing and partial totals, sticky table context and screenshots across the
configured layouts/themes. These fixtures never write production curriculum data and do not
replace deferred live UI acceptance.


Custom curriculum program order uses owned disposable fixtures:

```bash
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 ./scripts/test_backend_school.sh curriculum_program_order -- --test-threads=2
```

These cases cover clean migration/retry and populated 133→134 preservation, legacy zero ties,
complete-set order validation, stale/foreign/duplicate inputs, no-op revisions, concurrent
reorders/creation, archived presentation-only changes, append-on-create/copy, pinned publication
order, history and discard. Run affected curriculum publication/current-subject/options/copy,
Delivery and promotion-option consumers, HTTP permission/schema checks, API generation/check/tests
and the frontend ordering helpers/browser fixtures. Predecessor migrations use historical SQL
fixtures; current runtime services run only after their required schema is available.

## Curriculum credit criteria

Curriculum basic-credit criteria use the same owned disposable runner:

```bash
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 ./scripts/test_backend_school.sh curriculum_credit -- --test-threads=2
cargo test --manifest-path backend-school/Cargo.toml -p school-academic-core credit_criteria
cargo test --manifest-path backend-school/Cargo.toml -p school-academic-core credit_category
```

The fixtures cover clean 135/retry and populated 134 preservation with an active amendment,
view identity/grants, empty legacy settings, strict bounded JSON, normalized decimal no-ops,
stale/concurrent/token/tenant refusal, explicit covered-grade review and independent criteria
revisions. Publication/history/discard/preview cases retain actual rules while excluding only
revision metadata from semantic comparisons; unmet warning targets permit publication. Copy
retains destination settings. Unknown pinned catalog classifications keep null credit categories
in current and historical requirements/options, even after a later known revision exists.
Run affected publication/order/current-subject/options/copy consumers, HTTP permissions,
API schema/generate/check/test and documentation policy with the existing focused recipes.
All mutation fixtures are synthetic; no production criteria or curricula are written.
