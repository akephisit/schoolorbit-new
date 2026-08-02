use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use std::{future::Future, pin::Pin};
use tokio_cron_scheduler::{Job, JobScheduler, JobSchedulerError};
use uuid::Uuid;

pub const SCHOOL_TIMEZONE: Tz = chrono_tz::Asia::Bangkok;
pub const SCHOOL_TIMEZONE_NAME: &str = "Asia/Bangkok";
pub const FILE_PLATFORM_RECONCILIATION_CRON: &str = "0 0 * * * *";
pub const CALENDAR_REMINDER_CRON: &str = "0 0 7 * * *";

#[derive(Clone, Copy, Debug)]
pub struct ScheduledJobNextRun {
    pub utc: DateTime<Utc>,
    pub bangkok: DateTime<Tz>,
}

pub fn new_school_cron_job<T>(schedule: &str, run: T) -> Result<Job, JobSchedulerError>
where
    T: 'static
        + FnMut(Uuid, JobScheduler) -> Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync,
{
    Job::new_async_tz(schedule, SCHOOL_TIMEZONE, run)
}

pub async fn next_run_for_job(
    scheduler: &mut JobScheduler,
    job_id: Uuid,
) -> Result<ScheduledJobNextRun, JobSchedulerError> {
    let utc = scheduler
        .next_tick_for_job(job_id)
        .await?
        .ok_or(JobSchedulerError::CantGetTimeUntil)?;
    Ok(ScheduledJobNextRun {
        utc,
        bangkok: utc.with_timezone(&SCHOOL_TIMEZONE),
    })
}

pub fn log_next_run(job_name: &str, schedule: &str, next_run: ScheduledJobNextRun) {
    tracing::info!(
        job = job_name,
        timezone = SCHOOL_TIMEZONE_NAME,
        cron = schedule,
        next_run_bangkok = %next_run.bangkok.to_rfc3339(),
        next_run_utc = %next_run.utc.to_rfc3339(),
        "Scheduled job registered"
    );
}

#[cfg(test)]
mod tests {
    use super::{
        new_school_cron_job, next_run_for_job, ScheduledJobNextRun, CALENDAR_REMINDER_CRON,
        FILE_PLATFORM_RECONCILIATION_CRON,
    };
    use chrono::Timelike;
    use tokio_cron_scheduler::JobScheduler;

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
