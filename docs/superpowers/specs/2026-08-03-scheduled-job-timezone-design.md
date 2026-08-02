# Scheduled Job Timezone Design

## Problem

Backend School creates the daily calendar reminder with `Job::new_async` and the cron expression `0 0 7 * * *`. In `tokio-cron-scheduler` 0.15.1 that constructor delegates to UTC, so the job runs at 07:00 UTC, or 14:00 in Bangkok. The host and container timezone cannot change this behavior.

The File Platform reconciliation currently runs every five minutes and opens every active tenant database even when no work is pending. That produces up to 288 tenant sweeps per day and unnecessarily keeps low-traffic Neon databases active. Pool and WebSocket cleanup use elapsed-time intervals in process memory and do not create this database cost.

## Selected Approach

Make the application own its scheduling timezone explicitly:

- declare `Asia/Bangkok` as the Backend School wall-clock timezone;
- create cron jobs through a small scheduling helper that calls `Job::new_async_tz`;
- keep the calendar expression at the human-readable local time `0 0 7 * * *`;
- change File Platform reconciliation to the start of every hour, reducing tenant sweeps from 288 to 24 per day;
- use the same explicit timezone helper for the hourly File Platform cron job;
- log both jobs' next runs in Bangkok time and UTC after registration.

`chrono-tz` becomes a direct dependency because production code must not rely on a transitive dependency.

## Rejected Approaches

- Changing the expression to `0 0 0 * * *` would happen to produce 07:00 in Thailand while hiding the intended business time and making future maintenance error-prone.
- Setting `TZ=Asia/Bangkok` in Podman or changing the VPS timezone does not work because `Job::new_async` explicitly selects UTC.
- Adding seven hours before job execution would treat the symptom after the scheduler has already selected the wrong instant.
- Running File Platform reconciliation once per day would minimize Neon wakeups but could leave failed upload finalization or object deletion pending for up to 24 hours. Hourly reconciliation is the selected balance between repair latency and database cost.

## Components and Data Flow

`backend-school/src/scheduling.rs` owns the timezone constant, cron expressions, timezone-aware job construction, and next-run formatting. `main.rs` supplies the existing job closures, registers the returned jobs, then logs both registered jobs' next ticks before starting the scheduler.

Calendar processing continues to compute the current Bangkok calendar date and keeps its existing tenant iteration, advisory locking, recipient resolution, notification delivery, and idempotent `sent_at` behavior. Normal upload and explicit deletion remain synchronous. The hourly recovery sweep handles only expired temporary files and durable operations left by failed provider or metadata work. File expiry continues to use PostgreSQL `TIMESTAMPTZ` and `now()`.

## Error Handling and Observability

Invalid cron construction and scheduler registration remain startup-fatal because running without scheduled maintenance or reminders is an invalid runtime state. Failure to obtain the registered next tick is also startup-fatal so a deployment cannot silently lose schedule observability.

Each startup log records the job name, timezone name, cron expression, next Bangkok time, and equivalent UTC time. It contains no tenant or personal data.

## Testing and Verification

Focused asynchronous unit tests register no-op jobs produced by the scheduling helper. They assert that the calendar job's next occurrence is `07:00` in `Asia/Bangkok` and `00:00` UTC, and that File Platform reconciliation advances to the next local top of hour rather than the former five-minute cadence. A static architecture guard ensures cron jobs do not regress to the UTC-default constructor or the five-minute File Platform expression.

Verification follows the backend-school matrix: focused scheduling tests, `cargo fmt --all -- --check`, `cargo test --test static_architecture`, `cargo check`, `git diff --check`, final diff review, and `git status --short`.

## Deployment Acceptance

No database migration or environment variable is required. After Backend School deployment, logs must show the next calendar reminder at 07:00 `Asia/Bangkok` and 00:00 UTC, plus the next File Platform sweep at the following Bangkok top of hour. The same behavior must hold on the current VPS and a replacement Debian/Ubuntu VPS regardless of each host's timezone.
