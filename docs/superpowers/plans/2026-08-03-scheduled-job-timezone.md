# Scheduled Job Timezone Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run calendar reminders at 07:00 Bangkok time and reduce File Platform tenant sweeps from every five minutes to hourly without making normal uploads or deletions asynchronous.

**Architecture:** A focused `scheduling` module owns the Bangkok timezone, cron expressions, timezone-aware job construction, next-tick lookup, and schedule logging. `main.rs` keeps the existing job closures but registers both through that module, so behavior is independent of VPS/container timezone and observable at startup.

**Tech Stack:** Rust 2021, Tokio, `tokio-cron-scheduler` 0.15.1, `chrono` 0.4, `chrono-tz` 0.10.4, static architecture tests.

## Global Constraints

- Never edit an applied migration; this change requires no migration.
- Keep normal File Platform upload and explicit deletion synchronous.
- Calendar reminders must run at `07:00 Asia/Bangkok`, equivalent to `00:00 UTC`.
- File Platform reconciliation must run at the start of every Bangkok hour, reducing tenant sweeps from 288 to 24 per day.
- Runtime scheduling must not depend on the VPS or container timezone.
- Logs must contain schedule metadata only, with no tenant or personal data.

---

### Task 1: Timezone-aware scheduled jobs

**Files:**
- Modify: `backend-school/Cargo.toml`
- Modify: `backend-school/Cargo.lock`
- Create: `backend-school/src/scheduling.rs`
- Modify: `backend-school/src/main.rs:1-32,832-922`
- Test: `backend-school/src/scheduling.rs`
- Test: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Produces: `SCHOOL_TIMEZONE`, `SCHOOL_TIMEZONE_NAME`, `FILE_PLATFORM_RECONCILIATION_CRON`, `CALENDAR_REMINDER_CRON`.
- Produces: `new_school_cron_job<T>(schedule: &str, run: T) -> Result<Job, JobSchedulerError>` using `Job::new_async_tz`.
- Produces: `next_run_for_job(scheduler: &mut JobScheduler, job_id: Uuid) -> Result<ScheduledJobNextRun, JobSchedulerError>`.
- Produces: `log_next_run(job_name: &str, schedule: &str, next_run: ScheduledJobNextRun)`.
- Consumes: the existing File Platform and calendar async closures from `main.rs` unchanged.

- [ ] **Step 1: Add the direct timezone dependency and failing focused tests**

Add `chrono-tz = "0.10.4"` beside `chrono` and update the lockfile. Create `src/scheduling.rs` with tests that express the desired API before defining it:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    async fn next_run(schedule: &str) -> ScheduledJobNextRun {
        let mut scheduler = JobScheduler::new().await.unwrap();
        let job = new_school_cron_job(schedule, |_id, _scheduler| Box::pin(async {})).unwrap();
        let job_id = job.guid();
        scheduler.add(job).await.unwrap();
        next_run_for_job(&mut scheduler, job_id).await.unwrap()
    }

    #[tokio::test]
    async fn calendar_reminder_runs_at_seven_bangkok_and_midnight_utc() {
        let next = next_run(CALENDAR_REMINDER_CRON).await;
        assert_eq!((next.bangkok.hour(), next.bangkok.minute()), (7, 0));
        assert_eq!((next.utc.hour(), next.utc.minute()), (0, 0));
    }

    #[tokio::test]
    async fn file_platform_reconciliation_runs_only_at_the_top_of_each_hour() {
        let next = next_run(FILE_PLATFORM_RECONCILIATION_CRON).await;
        assert_eq!((next.bangkok.minute(), next.bangkok.second()), (0, 0));
    }
}
```

Declare `mod scheduling;` in `main.rs` so the tests compile far enough to fail on the missing production API.

- [ ] **Step 2: Run the focused tests and verify RED**

Run:

```bash
cd backend-school
cargo test scheduling::tests
```

Expected: compilation fails because `ScheduledJobNextRun`, `new_school_cron_job`, and the cron constants are not defined. This confirms the tests require the missing timezone-aware scheduling boundary.

- [ ] **Step 3: Implement the minimal scheduling module**

Define the direct timezone and cron ownership:

```rust
pub const SCHOOL_TIMEZONE: Tz = chrono_tz::Asia::Bangkok;
pub const SCHOOL_TIMEZONE_NAME: &str = "Asia/Bangkok";
pub const FILE_PLATFORM_RECONCILIATION_CRON: &str = "0 0 * * * *";
pub const CALENDAR_REMINDER_CRON: &str = "0 0 7 * * *";

#[derive(Clone, Copy, Debug)]
pub struct ScheduledJobNextRun {
    pub utc: DateTime<Utc>,
    pub bangkok: DateTime<Tz>,
}
```

Implement `new_school_cron_job` by delegating to `Job::new_async_tz(schedule, SCHOOL_TIMEZONE, run)`. Implement `next_run_for_job` with `scheduler.next_tick_for_job(job_id)` and convert the returned UTC instant with `with_timezone(&SCHOOL_TIMEZONE)`; map an absent tick to `JobSchedulerError::CantGetTimeUntil`. Implement `log_next_run` with structured `tracing::info!` fields for job name, timezone, cron, Bangkok next run, and UTC next run.

- [ ] **Step 4: Wire both jobs through the scheduling module**

In `main.rs`, replace both `Job::new_async` calls with `scheduling::new_school_cron_job` and the owned constants. Capture each job GUID before registration, register both jobs, resolve both next ticks, call `scheduling::log_next_run`, then start the scheduler. Keep the existing job bodies unchanged.

- [ ] **Step 5: Add the regression architecture guard**

Add a focused test to `tests/static_architecture.rs` that reads `src/main.rs` and `src/scheduling.rs` and asserts:

```rust
assert!(!main.contains("Job::new_async("));
assert!(scheduling.contains("Job::new_async_tz"));
assert!(scheduling.contains("chrono_tz::Asia::Bangkok"));
assert!(scheduling.contains("0 0 * * * *"));
assert!(!scheduling.contains("0 */5 * * * *"));
```

This prevents a future wall-clock job from silently returning to UTC-default scheduling and prevents restoration of the five-minute tenant sweep.

- [ ] **Step 6: Run focused tests and verify GREEN**

Run:

```bash
cd backend-school
cargo test scheduling::tests
cargo test scheduled_jobs_use_explicit_bangkok_timezone --test static_architecture
```

Expected: both scheduling tests and the architecture guard pass.

- [ ] **Step 7: Commit the implementation**

```bash
git add backend-school/Cargo.toml backend-school/Cargo.lock backend-school/src/scheduling.rs backend-school/src/main.rs backend-school/tests/static_architecture.rs
git commit -m "fix: run scheduled jobs on Bangkok time"
```

### Task 2: Verification and temporary workflow artifact cleanup

**Files:**
- Delete after successful verification: `docs/superpowers/specs/2026-08-03-scheduled-job-timezone-design.md`
- Delete after successful verification: `docs/superpowers/plans/2026-08-03-scheduled-job-timezone.md`

**Interfaces:**
- Consumes: Task 1's timezone-aware scheduling implementation.
- Produces: a clean canonical documentation tree with the reviewed design and plan retained in Git history.

- [ ] **Step 1: Run the Backend School verification matrix**

Run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo test --test static_architecture
cargo check
```

Expected: formatting, all static architecture tests, and compilation pass.

- [ ] **Step 2: Review repository integrity**

Run from the repository root:

```bash
git diff --check
git diff --stat
git status --short
```

Expected: only the intentional scheduling implementation and temporary workflow artifacts appear.

- [ ] **Step 3: Remove completed Superpowers artifacts and commit**

Remove the approved spec and completed plan from the working tree after verification, preserving them in Git history, then commit:

```bash
git add -u docs/superpowers/specs/2026-08-03-scheduled-job-timezone-design.md docs/superpowers/plans/2026-08-03-scheduled-job-timezone.md
git commit -m "docs: retire scheduled job workflow artifacts"
```

- [ ] **Step 4: Run final clean-state checks**

Run:

```bash
git diff --check
git status --short --branch
```

Expected: no uncommitted files. Local `main` contains the implementation and is ready to push after final review.
