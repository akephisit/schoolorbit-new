# File Platform Database Clock Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make PostgreSQL the only absolute clock used for durable file-operation queueing, leasing, expiry, and retry scheduling so backend-school tests and multi-worker runtime behavior are not sensitive to application/database clock skew.

**Architecture:** Keep retry and lease policy in Rust as relative `std::time::Duration` values, then convert those durations to checked microseconds at the SQL repository boundary. PostgreSQL `statement_timestamp()` owns every absolute scheduling transition; the private repository trait no longer accepts `chrono::DateTime` for file-operation scheduling.

**Tech Stack:** Rust, Tokio, SQLx, PostgreSQL, Cargo.

## Global Constraints

- Scope this plan to `backend-school/src/modules/files/{repository,reconciler,platform_service}.rs` and this implementation plan.
- Do not modify any migration, HTTP/API contract, permission contract, frontend, admin application, secret, storage-provider contract, scanner contract, or retry-policy value.
- Never add sleeps, retry wrappers, ignored failures, raw storage locators, credentials, file contents, or private metadata to tests or logs.
- Preserve every existing file lifecycle, terminal-state, deletion, attempt-count, and error-code behavior.
- Use `TEST_DATABASE_URL` only through the repository's existing dotenv-backed isolated test helpers; never print or commit the connection string.
- Follow strict RED-GREEN-REFACTOR: observe each new regression test fail for the intended reason before changing production behavior.

---

### Task 1: Move Durable File Scheduling to the PostgreSQL Clock

**Files:**

- Modify: `backend-school/src/modules/files/repository.rs:1-205,676-805,1008-1155,1362-1540`
- Modify: `backend-school/src/modules/files/reconciler.rs:1-331`
- Modify: `backend-school/src/modules/files/platform_service.rs:1-430,528-1067`
- Test: `backend-school/src/modules/files/repository.rs:1362-1540`
- Test: `backend-school/src/modules/files/platform_service.rs:528-1067`

**Interfaces:**

- Consumes: `FilePlatformRuntimeConfig::retry_delay(attempt: i32) -> Duration`, `FilePlatformRuntimeConfig::reconciliation_lease`, and the existing `file_operations` `TIMESTAMPTZ` columns.
- Produces: `FileRepository` scheduling methods that accept relative `Duration`; `duration_microseconds(Duration) -> Result<i64, RepositoryError>`; SQL transitions based on PostgreSQL `statement_timestamp()`.

- [x] **Step 1: Write a deterministic RED test for mixed-clock immediate leasing**

In `sql_repository_reserves_reclaims_finalizes_and_deletes_durably`, temporarily demonstrate the existing bug through the current interface by replacing the first lease timestamp with an intentionally stale application time:

```rust
let stale_application_time = Utc::now() - chrono::Duration::hours(1);
let first = repository
    .lease_due_operations(
        "worker-one",
        stale_application_time,
        Duration::from_secs(60),
        10,
    )
    .await
    .unwrap();
assert_eq!(first.len(), 1);
```

The production mutation caught by this test is comparing a database-created `next_retry_at` against an application-provided timestamp. The row is due according to PostgreSQL but the old implementation returns no work.

- [x] **Step 2: Run the repository test and verify RED**

Run:

```bash
cargo test --manifest-path backend-school/Cargo.toml \
  modules::files::repository::tests::sql_repository_reserves_reclaims_finalizes_and_deletes_durably \
  -- --nocapture
```

Expected: FAIL at `assert_eq!(first.len(), 1)` with an actual length of `0`. A database connection error or skipped test is not the expected RED state.

- [x] **Step 3: Add and test checked duration conversion before using it in SQL**

Add this test inside the repository test module:

```rust
#[test]
fn duration_microseconds_is_checked_before_sql_binding() {
    assert_eq!(duration_microseconds(Duration::from_micros(25)), Ok(25));
    assert_eq!(
        duration_microseconds(Duration::MAX),
        Err(RepositoryError::InvalidPersistedState)
    );
}
```

Run:

```bash
cargo test --manifest-path backend-school/Cargo.toml \
  duration_microseconds_is_checked_before_sql_binding -- --nocapture
```

Expected: FAIL to compile because `duration_microseconds` does not exist.

Add the minimal production helper near the SQL repository implementation:

```rust
fn duration_microseconds(duration: Duration) -> Result<i64, RepositoryError> {
    i64::try_from(duration.as_micros()).map_err(|_| RepositoryError::InvalidPersistedState)
}
```

Rerun the focused helper test and expect PASS.

- [x] **Step 4: Make lease acquisition use one PostgreSQL statement clock**

Keep the current `now` parameter only for this RED-to-GREEN step and rename it `_now`. Convert the lease duration with `duration_microseconds`, then replace the lease SQL and bindings with:

```rust
let lease_microseconds = duration_microseconds(lease_duration)?;
let rows = sqlx::query(
    r#"
WITH due AS (
    SELECT id
    FROM file_operations
    WHERE (
        status IN ('pending', 'retryable_failure')
        AND next_retry_at <= statement_timestamp()
    ) OR (
        status = 'leased'
        AND lease_expires_at <= statement_timestamp()
    )
    ORDER BY next_retry_at, created_at, id
    FOR UPDATE SKIP LOCKED
    LIMIT $1
)
UPDATE file_operations o
SET status = 'leased', attempt_count = attempt_count + 1,
    lease_owner = $2,
    leased_at = statement_timestamp(),
    lease_expires_at = statement_timestamp()
        + ($3 * INTERVAL '1 microsecond'),
    started_at = COALESCE(started_at, statement_timestamp())
FROM due
WHERE o.id = due.id AND o.attempt_count < 100
RETURNING o.id
"#,
)
.bind(limit)
.bind(worker)
.bind(lease_microseconds)
.fetch_all(&self.pool)
.await
.map_err(Self::database_error)?;
```

Before rerunning, replace the old simulated `now + 61 seconds` reclaim with the explicit PostgreSQL lease-expiry update from Step 8; PostgreSQL-owned leasing intentionally ignores the caller's future timestamp. Run the lifecycle test again. Expected: PASS because neither immediate eligibility nor reclaim depends on a Rust clock.

- [x] **Step 5: Write RED coverage for relative service and reconciler scheduling**

Extend `platform_service.rs` test imports with the real reconciler and operation type:

```rust
use crate::modules::files::{
    malware_scanner::MalwareScanner,
    platform_types::StorageClass,
    purpose_registry::original_object_key,
    reconciler::reconcile_due_operations,
    repository::{
        DeleteWork, LeasedOperation, ObjectTarget, OperationWork, RepositoryError,
    },
    storage_provider::{ObjectMetadata, StorageError},
};
```

Add scheduling observations to `FakeRepository`:

```rust
leased_operations: Mutex<Vec<LeasedOperation>>,
lease_durations: Mutex<Vec<Duration>>,
retry_delays: Mutex<Vec<Duration>>,
```

Change the fake scheduling methods to the desired relative interface. Each retry method pushes its `retry_delay`; leasing pushes `lease_duration` and returns all prepared operations with `std::mem::take`:

```rust
async fn lease_due_operations(
    &self,
    _worker: &str,
    lease_duration: Duration,
    _limit: i64,
) -> Result<Vec<LeasedOperation>, RepositoryError> {
    self.lease_durations.lock().unwrap().push(lease_duration);
    Ok(std::mem::take(&mut *self.leased_operations.lock().unwrap()))
}

async fn retry_operation(
    &self,
    _operation_id: Uuid,
    _error_code: &'static str,
    retry_delay: Duration,
    _terminal: bool,
) -> Result<(), RepositoryError> {
    self.retry_delays.lock().unwrap().push(retry_delay);
    self.events.lock().unwrap().push("operation_retry");
    Ok(())
}

async fn mark_derivative_failed(
    &self,
    _file_id: Uuid,
    _derivative_id: Uuid,
    _operation_id: Uuid,
    _error_code: &'static str,
    retry_delay: Duration,
    _terminal: bool,
) -> Result<(), RepositoryError> {
    self.retry_delays.lock().unwrap().push(retry_delay);
    self.events.lock().unwrap().push("derivative_retry");
    Ok(())
}

async fn mark_reconcile_pending(
    &self,
    _file_id: Uuid,
    _error_code: &'static str,
    retry_delay: Duration,
) -> Result<(), RepositoryError> {
    self.retry_delays.lock().unwrap().push(retry_delay);
    self.events.lock().unwrap().push("reconcile_pending");
    Ok(())
}

async fn queue_delete_retry(
    &self,
    _file_id: Uuid,
    _target: ObjectTarget,
    _error_code: &'static str,
    retry_delay: Duration,
) -> Result<(), RepositoryError> {
    self.retry_delays.lock().unwrap().push(retry_delay);
    self.events.lock().unwrap().push("delete_retry_queued");
    Ok(())
}
```

In `provider_or_finalize_failures_leave_durable_repair_state`, add this assertion after the finalize failure:

```rust
assert_eq!(
    *repository.retry_delays.lock().unwrap(),
    vec![FilePlatformRuntimeConfig::default().retry_delay(1)]
);
```

Add this reconciler behavior test:

```rust
#[tokio::test]
async fn reconciler_passes_relative_lease_and_retry_durations() {
    let provider = Arc::new(FakeProvider::default());
    provider
        .delete_results
        .lock()
        .unwrap()
        .push_back(Err(StorageError::OperationFailed));
    let repository = FakeRepository::default();
    let file_id = Uuid::new_v4();
    let object = StoredObject::new(
        original_object_key(
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            FilePurpose::ProfileImage,
            file_id,
            1,
            crate::modules::files::platform_types::DetectedContent::Png,
        )
        .unwrap(),
        "image/png",
    );
    repository.leased_operations.lock().unwrap().push(LeasedOperation {
        id: Uuid::new_v4(),
        file_id,
        attempt_count: 1,
        work: OperationWork::DeleteObject(DeleteWork {
            operation_id: Uuid::new_v4(),
            file_id,
            target: ObjectTarget::Version(Uuid::new_v4()),
            object,
        }),
    });
    let platform = platform(ScanOutcome::Clean, provider);

    let summary = reconcile_due_operations(&platform, &repository, "worker-one")
        .await
        .unwrap();

    assert_eq!(summary.leased, 1);
    assert_eq!(summary.retried, 1);
    assert_eq!(
        *repository.lease_durations.lock().unwrap(),
        vec![FilePlatformRuntimeConfig::default().reconciliation_lease]
    );
    assert_eq!(
        *repository.retry_delays.lock().unwrap(),
        vec![FilePlatformRuntimeConfig::default().retry_delay(1)]
    );
}
```

Run:

```bash
cargo test --manifest-path backend-school/Cargo.toml \
  reconciler_passes_relative_lease_and_retry_durations -- --nocapture
```

Expected: FAIL to compile because the production `FileRepository` trait and reconciler still require absolute `DateTime` values.

- [x] **Step 6: Replace absolute scheduling parameters with relative durations**

Update the five `FileRepository` scheduling signatures exactly as approved in the design:

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

For each SQL implementation, call `duration_microseconds(retry_delay)?` before opening a transaction or issuing SQL. Replace `retry_operation_query` with this relative-delay boundary:

```rust
async fn retry_operation_query(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    operation_id: Uuid,
    error_code: &'static str,
    retry_microseconds: i64,
    terminal: bool,
) -> Result<(), RepositoryError> {
    sqlx::query(
        r#"
UPDATE file_operations
SET status = CASE WHEN $4 THEN 'failed' ELSE 'retryable_failure' END,
    last_error_code = $2,
    next_retry_at = statement_timestamp() + ($3 * INTERVAL '1 microsecond'),
    completed_at = CASE WHEN $4 THEN statement_timestamp() ELSE NULL END,
    lease_owner = NULL, leased_at = NULL, lease_expires_at = NULL
WHERE id = $1
"#,
    )
    .bind(operation_id)
    .bind(error_code)
    .bind(retry_microseconds)
    .bind(terminal)
    .execute(&mut **transaction)
    .await
    .map_err(SqlFileRepository::database_error)?;
    Ok(())
}
```

`mark_reconcile_pending` uses:

```sql
UPDATE file_operations
SET status = 'retryable_failure', last_error_code = $2,
    next_retry_at = statement_timestamp() + ($3 * INTERVAL '1 microsecond'),
    lease_owner = NULL, leased_at = NULL, lease_expires_at = NULL
WHERE file_id = $1 AND operation_type = 'reconcile' AND status <> 'succeeded'
```

`queue_delete_retry` uses:

```sql
INSERT INTO file_operations (
    file_id, file_version_id, file_derivative_id, operation_type,
    status, next_retry_at, last_error_code
)
VALUES (
    $1, $2, $3, 'delete_object', 'retryable_failure',
    statement_timestamp() + ($4 * INTERVAL '1 microsecond'), $5
)
```

`mark_derivative_failed` and `retry_operation` convert the supplied duration once and pass the resulting `i64` into `retry_operation_query`; terminal status and cleanup remain unchanged.

Remove `chrono::{DateTime, Utc}` from `repository.rs` after the repository test no longer needs it.

- [x] **Step 7: Pass policy durations directly from services and reconciler**

In `platform_service.rs`, replace every expression shaped like:

```rust
Utc::now()
    + chrono::Duration::from_std(self.runtime_config.retry_delay(1))
        .unwrap_or_else(|_| chrono::Duration::seconds(5))
```

with:

```rust
self.runtime_config.retry_delay(1)
```

Remove `use chrono::Utc;` when no call remains.

In `reconciler.rs`, remove `reconcile_due_operations_at` and have `reconcile_due_operations` call:

```rust
let operations = repository
    .lease_due_operations(
        worker,
        runtime_config.reconciliation_lease,
        runtime_config.reconciliation_batch_size,
    )
    .await?;
```

Remove the `now` parameter from `retry` and `retry_derivative`. Pass `runtime_config.retry_delay(attempt)` or `runtime_config.retry_delay(operation.attempt_count)` directly to the repository. Remove the chrono import.

- [x] **Step 8: Refactor the repository lifecycle test to prove database-owned expiry without sleeps**

Remove `stale_application_time` and call the final lease interface:

```rust
let first = repository
    .lease_due_operations("worker-one", Duration::from_secs(60), 10)
    .await
    .unwrap();
```

Keep the second-worker assertion, then expire the lease explicitly using database time:

```rust
sqlx::query(
    "UPDATE file_operations \
     SET leased_at = statement_timestamp() - INTERVAL '61 seconds', \
         lease_expires_at = statement_timestamp() - INTERVAL '1 second' \
     WHERE id = $1",
)
.bind(first[0].id)
.execute(repository.pool())
.await
.unwrap();

let reclaimed = repository
    .lease_due_operations("worker-two", Duration::from_secs(60), 10)
    .await
    .unwrap();
```

Change the terminal delete retry to use `Duration::ZERO` and the final lease interface:

```rust
repository
    .queue_delete_retry(
        file_id,
        ObjectTarget::Version(version_id),
        "storage_operation_failed",
        Duration::ZERO,
    )
    .await
    .unwrap();
let terminal = repository
    .lease_due_operations("terminal-worker", Duration::from_secs(60), 10)
    .await
    .unwrap();
repository
    .retry_operation(
        terminal[0].id,
        "storage_operation_failed",
        Duration::ZERO,
        true,
    )
    .await
    .unwrap();
```

Preserve every existing ready/delete/terminal assertion.

- [x] **Step 9: Run focused GREEN verification**

Run:

```bash
cargo fmt --manifest-path backend-school/Cargo.toml --all
cargo test --manifest-path backend-school/Cargo.toml \
  duration_microseconds_is_checked_before_sql_binding -- --nocapture
cargo test --manifest-path backend-school/Cargo.toml \
  modules::files::repository::tests::sql_repository_reserves_reclaims_finalizes_and_deletes_durably \
  -- --nocapture
cargo test --manifest-path backend-school/Cargo.toml \
  reconciler_passes_relative_lease_and_retry_durations -- --nocapture
cargo test --manifest-path backend-school/Cargo.toml \
  modules::files::platform_service -- --nocapture
```

Expected: every selected test passes. The repository lifecycle test must connect to the real isolated PostgreSQL schema rather than skip.

- [x] **Step 10: Run the backend-school matrix and repeat the full binary suite**

Run:

```bash
cargo fmt --manifest-path backend-school/Cargo.toml --all -- --check
cargo test --manifest-path backend-school/Cargo.toml --test static_architecture
cargo check --manifest-path backend-school/Cargo.toml
cargo test --manifest-path backend-school/Cargo.toml --bin backend-school
cargo test --manifest-path backend-school/Cargo.toml --bin backend-school
git diff --check
git diff --name-only | rg '^(backend-admin|frontend-admin|frontend-school|backend-school/migrations)/' && exit 1 || true
git diff --stat
git status --short
```

Expected: formatting, 130 static architecture tests, compilation, and both complete binary-suite runs pass. The scope guard prints no paths. Existing warnings must not be presented as failures, but any test failure blocks the commit.

- [x] **Step 11: Review and commit the coherent file-platform change**

Review the final diff against `docs/superpowers/specs/2026-08-10-file-platform-database-clock-design.md`. Confirm no scheduling path accepts or constructs an application `DateTime`, no test contains sleep/retry behavior, and no applied migration changed.

Run:

```bash
git add \
  backend-school/src/modules/files/repository.rs \
  backend-school/src/modules/files/reconciler.rs \
  backend-school/src/modules/files/platform_service.rs \
  docs/superpowers/plans/2026-08-10-file-platform-database-clock.md
git diff --cached --check
git diff --cached --stat
git commit -m "fix(files): use database clock for durable work"
```

Do not push until the subsequent school-session documentation/verification change is complete and the combined branch has passed its applicable gates.
