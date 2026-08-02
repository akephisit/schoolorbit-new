# Scheduled Job Timezone Design

## Problem

Backend School creates the daily calendar reminder with `Job::new_async` and the cron expression `0 0 7 * * *`. In `tokio-cron-scheduler` 0.15.1 that constructor delegates to UTC, so the job runs at 07:00 UTC, or 14:00 in Bangkok. The host and container timezone cannot change this behavior.

The File Platform reconciliation runs every five minutes, while pool and WebSocket cleanup use elapsed-time intervals. Those jobs do not depend on a wall-clock hour and are not the source of the observed seven-hour delay.

## Selected Approach

Make the application own its scheduling timezone explicitly:

- declare `Asia/Bangkok` as the Backend School wall-clock timezone;
- create cron jobs through a small scheduling helper that calls `Job::new_async_tz`;
- keep the calendar expression at the human-readable local time `0 0 7 * * *`;
- use the same explicit timezone helper for the five-minute cron job, without changing its effective cadence;
- log the calendar job's next run in both Bangkok time and UTC after registration.

`chrono-tz` becomes a direct dependency because production code must not rely on a transitive dependency.

## Rejected Approaches

- Changing the expression to `0 0 0 * * *` would happen to produce 07:00 in Thailand while hiding the intended business time and making future maintenance error-prone.
- Setting `TZ=Asia/Bangkok` in Podman or changing the VPS timezone does not work because `Job::new_async` explicitly selects UTC.
- Adding seven hours before job execution would treat the symptom after the scheduler has already selected the wrong instant.

## Components and Data Flow

`backend-school/src/scheduling.rs` owns the timezone constant, cron expressions, timezone-aware job construction, and next-run formatting. `main.rs` supplies the existing job closures, registers the returned jobs, then logs the registered calendar job's next tick before starting the scheduler.

Calendar processing continues to compute the current Bangkok calendar date and keeps its existing tenant iteration, advisory locking, recipient resolution, notification delivery, and idempotent `sent_at` behavior. File expiry continues to use PostgreSQL `TIMESTAMPTZ` and `now()`.

## Error Handling and Observability

Invalid cron construction and scheduler registration remain startup-fatal because running without scheduled maintenance or reminders is an invalid runtime state. Failure to obtain the registered next tick is also startup-fatal so a deployment cannot silently lose schedule observability.

The startup log records the job name, timezone name, cron expression, next Bangkok time, and equivalent UTC time. It contains no tenant or personal data.

## Testing and Verification

A focused asynchronous unit test registers a no-op job produced by the scheduling helper and asserts that its next occurrence has hour `07:00` when converted to `Asia/Bangkok`, plus the equivalent `00:00` UTC offset. A static architecture guard ensures wall-clock cron jobs do not regress to the UTC-default constructor.

Verification follows the backend-school matrix: focused scheduling tests, `cargo fmt --all -- --check`, `cargo test --test static_architecture`, `cargo check`, `git diff --check`, final diff review, and `git status --short`.

## Deployment Acceptance

No database migration or environment variable is required. After Backend School deployment, logs must show the next calendar reminder at 07:00 `Asia/Bangkok` and 00:00 UTC. The same behavior must hold on the current VPS and a replacement Debian/Ubuntu VPS regardless of each host's timezone.
