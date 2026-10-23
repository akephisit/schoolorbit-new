//! Bound CPU-heavy password work before it enters Tokio's blocking queue.
use super::{hash_password, verify_password};
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::sync::Semaphore;
use zeroize::Zeroizing;

const MAX_PASSWORD_JOBS: usize = 4;
const ADMISSION_WAIT: Duration = Duration::from_secs(1);

struct PasswordWorker {
    permits: Arc<Semaphore>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PasswordWorkError {
    AdmissionTimedOut,
    WorkerUnavailable,
    PasswordOperationFailed,
}

impl PasswordWorker {
    fn new() -> Self {
        Self {
            permits: Arc::new(Semaphore::new(MAX_PASSWORD_JOBS)),
        }
    }

    async fn run<T, F>(&self, job: F) -> Result<T, PasswordWorkError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let permit = tokio::time::timeout(ADMISSION_WAIT, self.permits.clone().acquire_owned())
            .await
            .map_err(|_| PasswordWorkError::AdmissionTimedOut)?
            .map_err(|_| PasswordWorkError::WorkerUnavailable)?;
        tokio::task::spawn_blocking(move || {
            // Cancellation of the awaiting request must not release capacity while
            // the blocking password computation is still running.
            let _permit = permit;
            job()
        })
        .await
        .map_err(|_| PasswordWorkError::WorkerUnavailable)
    }
}

fn worker() -> &'static PasswordWorker {
    static WORKER: OnceLock<PasswordWorker> = OnceLock::new();
    WORKER.get_or_init(PasswordWorker::new)
}

pub(crate) async fn hash(password: String) -> Result<String, PasswordWorkError> {
    let password = Zeroizing::new(password);
    worker()
        .run(move || hash_password(password.as_str()))
        .await?
        .map_err(|_| PasswordWorkError::PasswordOperationFailed)
}

pub(crate) async fn verify(password: String, stored: String) -> Result<bool, PasswordWorkError> {
    let password = Zeroizing::new(password);
    worker()
        .run(move || verify_password(password.as_str(), &stored))
        .await?
        .map_err(|_| PasswordWorkError::PasswordOperationFailed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tokio::sync::{mpsc, oneshot};

    #[tokio::test]
    async fn admission_caps_concurrency_and_times_out_without_starting_excess_work() {
        let worker = Arc::new(PasswordWorker::new());
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let (entered_tx, mut entered_rx) = mpsc::unbounded_channel();
        let mut releases = Vec::new();
        let mut jobs = Vec::new();
        for _ in 0..MAX_PASSWORD_JOBS {
            let worker = worker.clone();
            let active = active.clone();
            let peak = peak.clone();
            let entered = entered_tx.clone();
            let (release_tx, release_rx) = oneshot::channel();
            releases.push(release_tx);
            jobs.push(tokio::spawn(async move {
                worker
                    .run(move || {
                        let running = active.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(running, Ordering::SeqCst);
                        entered.send(()).unwrap();
                        release_rx.blocking_recv().ok();
                        active.fetch_sub(1, Ordering::SeqCst);
                    })
                    .await
            }));
        }
        for _ in 0..MAX_PASSWORD_JOBS {
            entered_rx.recv().await.unwrap();
        }
        let excess_started = Arc::new(AtomicBool::new(false));
        let started = excess_started.clone();
        let unavailable = worker
            .run(move || started.store(true, Ordering::SeqCst))
            .await
            .unwrap_err();
        assert_eq!(unavailable, PasswordWorkError::AdmissionTimedOut);
        assert!(!excess_started.load(Ordering::SeqCst));
        assert_eq!(peak.load(Ordering::SeqCst), MAX_PASSWORD_JOBS);
        for release in releases {
            release.send(()).unwrap();
        }
        for job in jobs {
            job.await.unwrap().unwrap();
        }
        assert_eq!(worker.permits.available_permits(), MAX_PASSWORD_JOBS);
    }

    #[tokio::test]
    async fn cancelled_waiting_job_never_runs_or_consumes_future_capacity() {
        let worker = PasswordWorker::new();
        let held = worker
            .permits
            .clone()
            .acquire_many_owned(MAX_PASSWORD_JOBS as u32)
            .await
            .unwrap();
        let started = Arc::new(AtomicBool::new(false));
        let mark = started.clone();
        let mut waiting = Box::pin(worker.run(move || mark.store(true, Ordering::SeqCst)));
        assert!(futures::poll!(waiting.as_mut()).is_pending());
        drop(waiting);
        drop(held);
        worker.run(|| ()).await.unwrap();
        assert!(!started.load(Ordering::SeqCst));
        assert_eq!(worker.permits.available_permits(), MAX_PASSWORD_JOBS);
    }

    #[tokio::test]
    async fn cancelled_inflight_job_keeps_its_permit_until_blocking_work_finishes() {
        let worker = Arc::new(PasswordWorker::new());
        let held = worker
            .permits
            .clone()
            .acquire_many_owned((MAX_PASSWORD_JOBS - 1) as u32)
            .await
            .unwrap();
        let (entered_tx, entered_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let task_worker = worker.clone();
        let job = tokio::spawn(async move {
            task_worker
                .run(move || {
                    entered_tx.send(()).unwrap();
                    release_rx.blocking_recv().ok();
                })
                .await
        });
        entered_rx.await.unwrap();
        job.abort();
        assert!(job.await.unwrap_err().is_cancelled());
        assert_eq!(worker.permits.available_permits(), 0);
        let error = worker.run(|| ()).await.unwrap_err();
        assert_eq!(error, PasswordWorkError::AdmissionTimedOut);
        release_tx.send(()).unwrap();
        // The queued acquire completes only when the detached blocking job exits.
        let returned = tokio::time::timeout(
            Duration::from_secs(2),
            worker.permits.clone().acquire_owned(),
        )
        .await
        .unwrap()
        .unwrap();
        drop(returned);
        drop(held);
        assert_eq!(worker.permits.available_permits(), MAX_PASSWORD_JOBS);
    }
}
