# Backend School Disposable Neon Child Branch Design

## Problem

The manual Backend School Neon Compatibility workflow currently asks Neon to create a
`schema-only` branch. Neon returned HTTP 412 from that create operation in two independent runs:
first against the original project, then against a newly configured dedicated test project with a
new project ID, parent branch ID, database, and API key. Both failures occurred before any Rust
test ran and before the action produced a created branch ID.

The dedicated project has no production data, and the Rust compatibility tests create isolated
schemas and run the active migrations themselves. Copying only the parent schema therefore adds no
safety benefit for this gate, while the schema-only creation path is the shared failing condition.

## Goals

- Keep Neon compatibility testing isolated from every production project and branch.
- Create a unique disposable branch only after an explicit manual confirmation.
- Run migration and schema compatibility tests through the branch's direct endpoint.
- Remove the exact branch created by the run on test success or failure.
- Retain an expiration time as recovery when GitHub cancellation prevents finalization.
- Avoid persistent test schemas, production data copies, and pooled endpoint behavior.

## Non-goals

- Do not change routine local PostgreSQL testing.
- Do not change backend application behavior, migrations, permissions, or API contracts.
- Do not modify backend-admin, frontend-admin, or frontend-school application code.
- Do not use Neon for the full routine backend-school suite.

## Considered Approaches

### Keep schema-only branches in the production project

This preserves the original workflow but leaves the test lifecycle inside the production project
and has already failed with HTTP 412. It also gives the workflow API credential a production
project target. This approach is rejected.

### Keep schema-only branches in the dedicated test project

This provides project isolation, but the diagnostic rerun with wholly new test configuration still
failed at the same schema-only create request. The parent contains no production data, so this mode
does not provide additional data protection. This approach is rejected.

### Create ordinary child branches in the dedicated test project

This is the selected design. A normal Neon child branch uses the empty test-only parent through
copy-on-write, avoids the failing schema-only initialization path and root-branch allowance, and is
deleted after the focused checks. Because production data never enters the project, an ordinary
child branch cannot inherit production rows.

## Architecture and Data Flow

The repository keeps one manually dispatched GitHub Actions workflow. Repository configuration
supplies a dedicated test API key plus the test project ID, parent branch ID, database, and role.
The workflow validates that every value is present and that IDs match Neon ID shapes.

After confirmation, the pinned Neon create-branch action creates
`schoolorbit-test-<run_id>-<run_attempt>` from the configured test-only parent. The workflow omits
`branch_type`, selecting the action's normal branch mode. The branch receives a two-hour expiration
and one direct read-write endpoint.

The workflow rejects an existing-branch collision by requiring `created=true` and a valid returned
branch ID. It masks the returned direct database URL, assigns it only to `TEST_DATABASE_URL`, and
runs the auth-session and file-platform schema suites. Those suites create their own isolated
schemas and execute the active tenant migration timeline, so the parent branch needs only the
configured empty database and owner role.

An `always()` finalizer deletes only the branch ID returned by a successful create operation and
accepts Neon HTTP 200 or 204. The create action's `expires_at` value is the fallback if cancellation
or runner loss prevents the finalizer from running.

## Safety and Failure Handling

- The project and parent branch are test-only and contain no production data.
- The workflow remains `workflow_dispatch` only and requires the boolean confirmation input.
- The branch name is unique to the GitHub run attempt.
- The workflow never prints the API key, password, or database URL.
- The pooled URL is never consumed because transaction pooling can obscure schema-local
  `_sqlx_migrations` state.
- A missing value, malformed ID, reused branch, create failure, test failure, or cleanup failure
  fails the gate.
- Cleanup requires both `created=true` and a non-empty returned branch ID, preventing deletion of a
  pre-existing branch after a name collision.
- If branch creation succeeds remotely but the action fails before publishing outputs, the
  server-side expiration remains the recovery boundary.

## Verification

The existing Node workflow contract will first be changed to require normal branch mode and run
against the current schema-only workflow to demonstrate RED. Removing `branch_type: schema-only`
is the only workflow behavior change needed for GREEN. The contract continues to require manual
dispatch, dedicated `NEON_TEST_*` configuration, a direct endpoint, unique creation, expiration,
and unconditional exact-ID cleanup.

Local verification consists of the focused Node contract, documentation-policy tests,
`actionlint`, `git diff --check`, and final status/diff review. The final integration proof is a
manually dispatched GitHub run in which branch creation, both Rust schema suites, and the cleanup
step all succeed on the pushed commit.
