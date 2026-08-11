# Backend School Disposable Neon Child Branch Design

## Problem

The manual Backend School Neon Compatibility workflow returned HTTP 412 while creating a branch
in three independent runs. Two runs requested a `schema-only` branch, first against the original
project and then against a newly configured dedicated test project. A third run used an ordinary
child branch in the dedicated project and failed at the same point before any Rust test ran. That
third result disproved the initial hypothesis that schema-only creation caused the 412 response.

The next suspect was `suspend_timeout: 60`. Run `31495216927` used ordinary branch mode and changed
the interval to 300 seconds but still returned HTTP 412. Because the pinned action reports only the
Axios status and drops the Neon response body, run `31496094480` added a bounded diagnostic request.
Neon then identified the exact failed precondition: `modifying the suspend interval is not
permitted on this account`.

The pinned action cannot represent the required request. It always sends
`suspend_timeout_seconds`, mapping an omitted workflow input to `0` (disabled auto-suspend). The
repository must therefore own the small API request and omit that endpoint field entirely, allowing
the Neon project to apply its account-controlled suspension behavior.

The dedicated project still provides the required data-isolation boundary. It has no production
data, and the Rust compatibility tests create isolated schemas and run the active migrations
themselves, so an ordinary disposable child branch is sufficient for this gate.

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
copy-on-write, avoids root-branch allowance consumption, and is deleted after the focused checks.
Because production data never enters the project, an ordinary child branch cannot inherit
production rows. The first ordinary-branch run exposed a separate, plan-restricted suspension
override; normal branch mode remains the selected data-isolation architecture rather than the root
cause of the HTTP 412 response.

### Force a 60-second compute suspension timeout

This minimizes idle compute time but is not supported by the configured Neon Free project, whose
scale-to-zero interval is fixed at five minutes. This approach is rejected.

### Send a 300-second compute suspension

Although 300 seconds matches the visible Free-plan interval, the account rejects any request that
modifies this field. Run `31495216927` disproved this approach.

### Omit compute suspension with a repository-owned API client

This is the selected lifecycle behavior. A dependency-free Node client sends an ordinary branch,
parent, expiration, and `read_write` endpoint but no `suspend_timeout_seconds`. It then retrieves a
non-pooled connection URI through Neon's documented API. This is the smallest request that lets the
account retain ownership of compute suspension while preserving branch expiration and exact-ID
cleanup.

## Architecture and Data Flow

The repository keeps one manually dispatched GitHub Actions workflow. Repository configuration
supplies a dedicated test API key plus the test project ID, parent branch ID, database, and role.
The workflow validates that every value is present and that IDs match Neon ID shapes.

After confirmation, the repository-owned Node client creates
`schoolorbit-test-<run_id>-<run_attempt>` from the configured test-only parent. Its POST omits both
`init_source` and `suspend_timeout_seconds`, selecting ordinary branch behavior and account-owned
compute suspension. The branch receives a two-hour expiration and one read-write endpoint.

Immediately after HTTP 201, the client validates the returned branch ID and publishes
`created=true` plus that exact ID before making another API request. This preserves cleanup
ownership even if connection retrieval fails. It requests the configured database and role with
`pooled=false`, validates the returned authenticated PostgreSQL URI, registers it with GitHub's
masking command, and publishes it as `db_url`. The workflow assigns that value only to
`TEST_DATABASE_URL` and runs the auth-session and file-platform schema suites. Those suites create
their own isolated schemas and execute the active tenant migration timeline, so the parent branch
needs only the configured empty database and owner role.

Before either Rust suite, the workflow uses the masked direct URI to provision `uuid-ossp` and
`pg_trgm` explicitly in `public` on the disposable child. This mirrors the local ephemeral runner's
database prerequisites. It is required because baseline migration 001 honors the isolated test
search path while its table defaults refer to `public.uuid_generate_v4()`. Provisioning happens
only after fresh-branch verification, and deleting the branch removes the extensions with it.

An `always()` finalizer deletes only the branch ID returned by a successful create operation and
accepts Neon HTTP 200 or 204. The create request's `expires_at` value is the fallback if cancellation
or runner loss prevents the finalizer from running. API failures expose only bounded, sanitized
scalar `code` and `message` fields; raw request and response bodies remain hidden.

## Safety and Failure Handling

- The project and parent branch are test-only and contain no production data.
- The workflow remains `workflow_dispatch` only and requires the boolean confirmation input.
- The branch name is unique to the GitHub run attempt.
- The create request omits compute suspension entirely, leaving that setting under Neon account
  control.
- The workflow never prints the API key, password, or database URL.
- Baseline extensions are installed only in the verified disposable child, never in the test
  parent or a production project.
- The pooled URL is never consumed because transaction pooling can obscure schema-local
  `_sqlx_migrations` state.
- A missing value, malformed ID, reused branch, create failure, test failure, or cleanup failure
  fails the gate.
- Cleanup requires both `created=true` and a non-empty returned branch ID, preventing deletion of a
  pre-existing branch after a name collision.
- Branch ownership outputs are published before connection retrieval so a later failure still runs
  exact-ID cleanup. If creation succeeds remotely but its response is lost, server-side expiration
  remains the recovery boundary.
- Error output never includes a raw request or response body and redacts secrets and URL-like
  values.

## Verification

The Node workflow contract rejects the third-party create action plus every `branch_type` and
`suspend_timeout` input, and requires the repository-owned creator. Creator behavior tests require
an endpoint without `suspend_timeout_seconds`, early ownership outputs, a masked non-pooled
PostgreSQL URI, sanitized failures, and configuration validation. Each behavior change is
demonstrated RED before implementation makes it GREEN.

The workflow contract also requires `uuid-ossp` and `pg_trgm` with `WITH SCHEMA public`, through the
direct step output, after branch verification and before Rust migrations.

Local verification consists of the focused Node contract, creator behavior tests,
documentation-policy tests, `actionlint`, `git diff --check`, and final status/diff review. The
final integration proof is a manually dispatched GitHub run in which branch creation, both Rust
schema suites, and the cleanup step all succeed on the pushed commit.
