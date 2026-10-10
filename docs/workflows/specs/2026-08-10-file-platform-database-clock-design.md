# File Platform Database Clock Ownership Design

**Date:** 2026-08-10

**Status:** Approved design

**Scope:** `backend-school` file-operation scheduling only

## Context

The file platform persists durable reconciliation, derivative, and deletion work in `file_operations`. New pending operations are scheduled with PostgreSQL `now()`, but leasing compares those timestamps with a Rust `Utc::now()` supplied by the backend process. Retry deadlines and lease expiry also originate from the application clock.

This mixed-clock boundary makes immediate leasing timing-dependent. The full backend-school suite observed a newly inserted pending operation return no work, while the same focused test passed on a later run. The failure also reproduced on the pre-auth-change baseline, so it is independent of the school login work. A small clock difference between the database host and application host is sufficient to delay an operation or make a lease appear expired at different times to different replicas.

The durable operation table is already the coordination boundary shared by workers. Its scheduling clock must therefore come from the same database rather than from whichever backend process happens to perform a transition.

## Goals

- Make PostgreSQL the single clock owner for durable file-operation scheduling.
- Make newly queued work immediately eligible without relying on application/database clock alignment.
- Make lease acquisition, lease expiry, retry scheduling, and retry eligibility consistent across backend replicas.
- Preserve the existing retry-delay and maximum-attempt policies.
- Replace the timing-dependent repository test with deterministic database-clock assertions.
- Restore a repeatably green full backend-school test suite.

## Non-Goals

- No migration or schema change.
- No API, OpenAPI, permission, frontend, storage-provider, malware-scanner, or file-visibility change.
- No change to backoff values, batch size, lease duration, or maximum attempts.
- No edits to `backend-admin` or `frontend-admin`.
- No arbitrary sleeps, test retries, widened timing windows, or ignored failures.

## Chosen Approach

PostgreSQL owns every absolute timestamp used to coordinate durable file work. Rust continues to own policy and supplies relative `std::time::Duration` values. Repository SQL converts those durations to intervals and adds them to PostgreSQL `statement_timestamp()`.

`statement_timestamp()` is stable for one SQL statement and advances between statements. It gives every replica the same authority for due checks and lease transitions while preserving an exact, testable policy boundary: application code decides *how long*; the database decides *from when*.

The alternatives were rejected as follows:

- Using the application clock everywhere is a smaller patch, but replicas can still disagree about lease and retry deadlines.
- Changing only the test to read the persisted timestamp would hide the mixed-clock runtime behavior.
- Sleeping or retrying the test would preserve the race and make the suite slower.

## Repository Contract

The private `FileRepository` interface will use relative durations for scheduling:

```rust
async fn mark_derivative_failed(
    &self,
    file_id: Uuid,
    derivative_id: Uuid,
    operation_id: Uuid,
    error_code: &'static str,
    retry_delay: Duration,
    terminal: bool,
) -> Result<(), RepositoryError>;

async fn mark_reconcile_pending(
    &self,
    file_id: Uuid,
    error_code: &'static str,
    retry_delay: Duration,
) -> Result<(), RepositoryError>;

async fn lease_due_operations(
    &self,
    worker: &str,
    lease_duration: Duration,
    limit: i64,
) -> Result<Vec<LeasedOperation>, RepositoryError>;

async fn retry_operation(
    &self,
    operation_id: Uuid,
    error_code: &'static str,
    retry_delay: Duration,
    terminal: bool,
) -> Result<(), RepositoryError>;

async fn queue_delete_retry(
    &self,
    file_id: Uuid,
    target: ObjectTarget,
    error_code: &'static str,
    retry_delay: Duration,
) -> Result<(), RepositoryError>;
```

These are internal Rust interfaces; no HTTP contract changes. Fake repositories will record or accept durations rather than fabricated wall-clock timestamps.

## SQL Clock Semantics

Repository code converts `Duration` to a checked signed microsecond count. A duration that cannot fit in `i64` returns `RepositoryError::InvalidPersistedState` before issuing SQL.

Lease acquisition uses PostgreSQL time for all parts of the atomic statement:

```sql
status IN ('pending', 'retryable_failure')
AND next_retry_at <= statement_timestamp()
```

```sql
status = 'leased'
AND lease_expires_at <= statement_timestamp()
```

```sql
leased_at = statement_timestamp(),
lease_expires_at = statement_timestamp()
    + ($lease_microseconds * INTERVAL '1 microsecond')
```

Retry mutations set:

```sql
next_retry_at = statement_timestamp()
    + ($retry_microseconds * INTERVAL '1 microsecond')
```

New operations may continue using PostgreSQL `now()` inside their creation transaction because both their later eligibility check and subsequent transitions use the same database clock.

## Service and Reconciler Flow

`FilePlatformRuntimeConfig::retry_delay(attempt)` remains the single retry-policy owner. `platform_service` and `reconciler` pass the resulting duration directly to the repository instead of adding it to `Utc::now()`.

The production reconciler no longer captures or injects a wall-clock timestamp. It requests a lease using the configured lease duration, processes each operation, and passes only the configured retry delay when persistence must schedule another attempt.

This also makes retry timing semantically clearer: the delay begins when PostgreSQL persists the retry transition, not when a worker started processing the batch.

## Error Handling

- Existing validation for an empty worker and an invalid batch limit remains unchanged.
- Duration conversion failure maps to the existing safe `InvalidPersistedState` repository error.
- SQL and connection errors retain the existing safe `OperationFailed` mapping.
- No timestamp, storage locator, credential, file content, or private metadata is added to logs or responses.
- Terminal operations retain their existing status and error-code behavior; the persisted retry timestamp is not used once terminal.

## Tests

The repository integration test will prove the database-clock lifecycle without sleeping:

1. reserve an upload and immediately lease its pending reconciliation operation;
2. verify a second worker cannot take an unexpired lease;
3. expire the lease explicitly in the isolated test schema using PostgreSQL time;
4. verify another worker reclaims it and increments the attempt count;
5. preserve the ready, delete, retry, and terminal-state assertions.

Focused service and reconciler tests will assert that configured `Duration` values reach the repository boundary. The type change itself prevents reintroducing application-produced absolute deadlines at these call sites.

Verification will run:

```bash
cargo fmt --manifest-path backend-school/Cargo.toml --all -- --check
cargo test --manifest-path backend-school/Cargo.toml \
  modules::files::repository::tests::sql_repository_reserves_reclaims_finalizes_and_deletes_durably \
  -- --nocapture
cargo test --manifest-path backend-school/Cargo.toml modules::files::reconciler -- --nocapture
cargo test --manifest-path backend-school/Cargo.toml modules::files::platform_service -- --nocapture
cargo test --manifest-path backend-school/Cargo.toml --test static_architecture
cargo check --manifest-path backend-school/Cargo.toml
cargo test --manifest-path backend-school/Cargo.toml --bin backend-school
git diff --check
git status --short
```

The full backend-school binary suite will be run repeatedly after focused tests pass so a single lucky timing result is not accepted as evidence.

## Impact and Rollout

The change affects only internal scheduling behavior in the backend-school image. It requires no migration, data backfill, secret, configuration, or frontend deployment. Existing pending, retryable, leased, succeeded, failed, and delete operations remain compatible because their stored `TIMESTAMPTZ` values do not change shape.

Normal backend-school deployment is sufficient. Readiness must remain green after rollout. A rollback to the previous image remains data-compatible, although it would restore mixed-clock scheduling behavior.

## Success Criteria

- No file-operation repository or production reconciler call site supplies an application `DateTime` for lease or retry scheduling.
- PostgreSQL time controls due checks, lease start, lease expiry, and retry deadlines.
- The focused repository lifecycle test passes without sleep or retry logic.
- File platform service/reconciler tests, static architecture tests, formatting, and compilation pass.
- The full backend-school suite passes repeatedly without the previous lease timing failure.
- The file-platform implementation diff contains no admin, frontend, migration, API-contract, permission, or secret changes.
