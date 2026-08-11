# Backend School Local Ephemeral PostgreSQL Testing Design

## Status

Approved in conversation on 2026-08-11.

## Problem

Routine `backend-school` database-backed tests currently read `TEST_DATABASE_URL` from the
developer environment and run against a persistent Neon test database. The test helpers isolate
cases with `schoolorbit_test_*` schemas, reset a schema before reusing it, and run the complete
tenant migration timeline inside each schema. They do not drop those schemas when the Cargo test
process finishes.

A read-only catalog inspection of the dedicated test database found:

- 43 retained `schoolorbit_test_*` schemas;
- 4,634 retained test tables and 24,261 relations;
- approximately 371 MB of database storage;
- approximately 108 tables and 565 relations in each migrated test schema.

Stable schema names prevent unlimited name growth across repeated runs, but the database still
retains dozens of complete migrated schemas. Remote round trips and serialized migrations also
make routine tests slower than necessary. Dropping schemas after each remote run would address
retained catalog objects, but it would not remove network latency or the repeated remote write
load.

## Goals

- Make a PostgreSQL container on the developer's own computer the default database for routine
  `backend-school` tests.
- Remove the complete disposable database after every test command, including failed and
  interrupted commands.
- Ensure a routine local run cannot silently fall back to Neon from `backend-school/.env`.
- Reduce database setup and migration time without weakening schema isolation or test coverage.
- Retain an explicit Neon compatibility gate for migrations and Neon-specific connection
  behavior, using disposable remote state rather than the persistent shared test database.
- Remove the already-retained test schemas from the dedicated Neon test database once, after
  validating the target and exact schema set.
- Keep the workflow easy to run for focused tests and for the complete backend-school binary
  test suite.

## Non-goals

- Do not run the local test database on the production VPS.
- Do not connect to production Compose, production Podman, tenant databases, or deployment
  workflows.
- Do not change backend runtime behavior, application SQL, API contracts, permissions, frontend
  code, or any applied migration.
- Do not replace all Neon verification with local PostgreSQL.
- Do not weaken assertions, serialize the entire test suite, add arbitrary sleeps, or retain a
  local database after a command for debugging.
- Do not redesign the existing shared- and named-schema test helpers in the first implementation.
  Further migration deduplication is a measured follow-up only if local execution remains slow.

## Decision

Add one repository-owned local runner for routine database-backed tests. The runner starts one
disposable PostgreSQL container on the machine where the command is invoked, exports its local
connection URL only to the child Cargo process, and removes the container in an exit trap.

The supported developer path is WSL using the local Docker Desktop Linux engine. The runner does
not use SSH, Docker over TCP, a remote Docker context, Podman, or any production service. Because
a program cannot reliably infer whether an arbitrary Unix host is a laptop or a VPS, the concrete
safety boundary is that the script accepts only a local Docker engine and is never called by a
deployment workflow. When invoked from the developer's WSL checkout, all database work therefore
stays on that computer.

Routine and compatibility testing are deliberately separate:

```text
routine developer test
    -> local Docker Desktop PostgreSQL
    -> Cargo test on the developer machine
    -> remove local container and all test data

explicit Neon compatibility gate
    -> disposable Neon branch
    -> migration/schema compatibility tests
    -> delete the branch even when the tests fail
```

## Local Runner Interface

The tracked entry point is `scripts/test_backend_school.sh`, invoked from the repository root.

With no arguments, it runs the complete backend-school binary test target:

```bash
./scripts/test_backend_school.sh
```

Additional arguments are forwarded to `cargo test --bin backend-school`, so a developer can run a
focused module without learning a second test syntax:

```bash
./scripts/test_backend_school.sh modules::auth::session_repository_tests -- --nocapture
```

There is no keep-container option. A retained test database would defeat the cleanup contract and
allow local state to influence a later run.

## Local Container Lifecycle

The runner performs these steps:

1. Resolve the repository and `backend-school` directories from the script location rather than
   the caller's current directory.
2. Verify that Docker is installed, the daemon is reachable, and the active engine endpoint is a
   local Unix/named-pipe connection. Reject TCP and SSH Docker endpoints.
3. Create a collision-resistant container name owned by the current runner process.
4. Register cleanup traps before starting the container.
5. Start one PostgreSQL container bound to a dynamically allocated port on `127.0.0.1` only.
6. Wait for `pg_isready` with a bounded condition-based readiness loop.
7. Resolve the dynamic host port and construct a test-only local `TEST_DATABASE_URL`.
8. Run Cargo from `backend-school`, preserving all caller arguments and the Cargo exit status.
9. Remove the exact container from the cleanup trap on normal exit, test failure, startup failure,
   `INT`, `TERM`, or `HUP`.

Cargo and its `target` directory remain on the developer machine, so ordinary Rust incremental
and dependency caches survive. Only PostgreSQL state is disposable. One container serves the
whole Cargo invocation; the runner does not pay container startup cost once per individual Rust
test.

Cleanup is idempotent. It first checks whether the exact generated container exists, then removes
only that container. A cleanup failure is reported explicitly and must not hide the original
Cargo failure status.

## Disposable PostgreSQL Configuration

The container uses a repository-pinned PostgreSQL major version compatible with the deployed
Neon projects. It has:

- a fixed, non-production test username, password, and database name;
- a loopback-only dynamic host port;
- no named volume or bind-mounted data directory;
- an in-memory or otherwise container-private data directory;
- test-only durability settings such as `fsync=off`, `synchronous_commit=off`, and
  `full_page_writes=off` where supported;
- enough local connections for the existing schema-isolated suite without changing application
  pool limits.

These durability settings are safe only because the database contains no real data and is always
destroyed. They must not appear in production Compose or runtime configuration.

The runner always overwrites `TEST_DATABASE_URL` for its Cargo child. An exported URL or a value in
`backend-school/.env` cannot redirect the routine runner to Neon. The runner never reads, prints,
or forwards the developer's Neon URL.

## Existing Test Helper Behavior

The first implementation preserves the current helper contract:

- database-backed tests continue to use `TEST_DATABASE_URL`;
- shared and named tests continue to isolate state with `schoolorbit_test_*` schemas;
- each schema still applies the active tenant migration timeline;
- the migration lock and lazy application-pool behavior remain unchanged.

Container removal becomes the reliable suite-level cleanup boundary. This avoids adding async
destructors to every test or depending on each test reaching a manual teardown statement after a
panic.

If measured local timings show that full repeated migration remains the dominant cost, a later
review may consider a migrated template or another PostgreSQL-native cloning strategy. That
optimization is outside this change because it alters isolation semantics and requires separate
correctness evidence.

## Explicit Neon Compatibility Gate

Neon remains necessary for checks that a local PostgreSQL process cannot prove, particularly
direct-endpoint migrations, search-path behavior, and Neon connection compatibility. It is not a
routine test default.

The explicit gate must:

- be manually selected rather than triggered by the local runner or a production deployment;
- create a disposable branch from the configured test parent, never a production mutation;
- use the branch's direct PostgreSQL endpoint, not a `-pooler` transaction endpoint, for migration
  and schema tests;
- expose the resulting URL only as a masked environment value to the test process;
- run the documented migration/schema compatibility tests;
- delete the created branch in an unconditional finalization step;
- use provider-side expiration as a secondary defense when available;
- fail visibly if branch deletion fails.

On GitHub Actions, the database lives on Neon and Cargo runs on an ephemeral GitHub-hosted runner;
neither component runs on the production VPS. Required Neon credentials remain GitHub secrets and
must never be committed or printed. The exact workflow scope will stay limited to
`backend-school` compatibility testing.

## One-time Existing Neon Cleanup

After the local runner and compatibility gate pass verification, perform one controlled cleanup
of the dedicated Neon test database. This is an operational action, not a reusable application
migration or a committed one-time script.

Before dropping anything:

1. Use a direct test endpoint and verify that the database identity is the dedicated test
   database, not a production tenant database.
2. Enumerate and count the exact schemas whose names match the literal
   `schoolorbit_test_` prefix.
3. Confirm that `public`, system schemas, and any non-test schema are excluded.
4. Check for active test processes so the cleanup does not race a running suite.

Drop only the validated `schoolorbit_test_*` schemas with `CASCADE`. Report the before/after schema
and relation counts without printing a database URL or credentials. The removed test data is not
recoverable from the application, but every object is reproducible by rerunning migrations and
tests. Provider storage may decline later than the catalog count because remote history retention
is independent of live schemas.

## Failure and Security Behavior

- Missing Docker, an unreachable local daemon, a remote Docker endpoint, image startup failure,
  readiness timeout, or unresolved port causes an actionable non-zero exit before Cargo starts.
- A failing Cargo test returns its original non-zero status after cleanup.
- Cleanup runs for ordinary shell termination signals and never targets a container name not
  generated by the current invocation.
- Docker output and test logs must not contain Neon credentials or production configuration.
- Fixed local test credentials are explicitly non-secret and have no value outside the
  loopback-bound disposable container.
- A manual Neon gate may not fall back to a persistent shared database when branch creation fails.
- Branch deletion failure is a gate failure that includes the branch identifier needed for manual
  cleanup but never its connection credentials.

## Repository Changes

- Add `scripts/test_backend_school.sh` as the routine local entry point.
- Add focused runner tests using a fake Docker/Cargo command boundary to prove argument forwarding,
  URL overriding, status preservation, local-engine enforcement, and cleanup on success/failure.
- Add the explicit backend-school Neon compatibility gate with unconditional disposable-branch
  deletion.
- Update `.rules` so local disposable PostgreSQL is the durable default for routine backend-school
  database tests and Neon is an explicit direct-endpoint compatibility gate.
- Update `docs/TESTING.md` with the local focused/full commands, prerequisites, lifecycle,
  troubleshooting, and manual Neon gate.
- Add only a short pointer in `backend-school/README.md` if needed; do not duplicate the canonical
  test recipe.
- Revalidate `TODO.md` and keep only unfinished follow-up work. Do not record this completed change
  as backlog history.
- Do not modify `backend-admin`, `frontend-admin`, `frontend-school`, application migrations, or
  production topology.

## Verification

Development follows test-driven implementation. Focused runner tests are written to fail before
the script behavior is added. They must prove:

- no-argument and focused Cargo command construction;
- caller arguments are preserved exactly;
- an existing external `TEST_DATABASE_URL` is replaced with the local URL;
- the published database port is loopback-only and dynamically allocated;
- TCP/SSH Docker contexts are rejected;
- readiness failure prevents Cargo execution and still removes a started container;
- successful and failing Cargo commands both remove the exact container;
- the Cargo exit code survives successful cleanup;
- a cleanup error is visible and does not turn a failed test into success;
- no keep-state path or production/Neon URL exists in the routine runner.

Then run a real Docker integration check on the developer machine:

1. Run one focused database-backed backend-school test through the new script.
2. Confirm it uses the local PostgreSQL address.
3. Confirm the generated container no longer exists afterward.
4. Repeat with an intentionally non-matching test filter or controlled failing command to prove
   the failure path cleans up.
5. Run the complete backend-school binary test target through the local runner.
6. Record elapsed time for comparable focused and complete runs without weakening assertions.

Run the backend-school and repository matrix:

```bash
cd backend-school
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check
cd ..
git diff --check
git status --short
```

For the GitHub workflow, also run the workflow/static checks required by `.rules`, including
actionlint and the applicable repository guard. An explicit Neon gate is complete only after one
manual run proves branch creation, direct-endpoint tests, and unconditional deletion.

Finally, verify that an ordinary local run creates no new Neon schema or relation, perform the
validated one-time cleanup, and record the Neon test database's before/after live catalog counts.

## Success Criteria

- The default documented backend-school database test command runs entirely on the developer's
  own computer when invoked from their WSL checkout.
- Routine tests cannot consume Neon storage or wake Neon through `TEST_DATABASE_URL` fallback.
- No runner-owned PostgreSQL container or volume remains after success, failure, or interruption.
- Existing schema-isolated backend-school tests pass without application or migration changes.
- Measured local database-backed tests are faster than the comparable persistent-Neon run; the
  report separates Cargo compilation time from database/test execution time.
- The explicit Neon gate uses a disposable direct-endpoint branch and deletes it after the run.
- The 43 previously observed `schoolorbit_test_*` schemas are removed from the dedicated test
  database without touching non-test schemas.

## Rollback

Revert the local runner, focused runner tests, explicit compatibility gate, and documentation
policy updates. Developers can still invoke Cargo manually with an explicit `TEST_DATABASE_URL`.
No production service, application migration, or tenant data rollback is required. The one-time
deletion of reproducible test schemas is intentionally not reversed.
