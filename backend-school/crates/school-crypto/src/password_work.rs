//! Process-wide admission for CPU-bound password work.

use std::{fmt, sync::Arc, sync::OnceLock, time::Duration};
use tokio::sync::Semaphore;
use zeroize::Zeroizing;

const MAX_ACTIVE_JOBS: usize = 4;
const ADMISSION_TIMEOUT: Duration = Duration::from_secs(1);
static PASSWORD_WORK: OnceLock<PasswordWork> = OnceLock::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PasswordWorkError {
    AdmissionTimeout,
    WorkerUnavailable,
    BcryptFailed,
}

impl fmt::Display for PasswordWorkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::AdmissionTimeout => "password_work_admission_timeout",
            Self::WorkerUnavailable => "password_worker_unavailable",
            Self::BcryptFailed => "password_bcrypt_failed",
        })
    }
}

impl std::error::Error for PasswordWorkError {}

struct PasswordWork {
    permits: Arc<Semaphore>,
    admission_timeout: Duration,
}

impl PasswordWork {
    fn new(max_active_jobs: usize, admission_timeout: Duration) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(max_active_jobs)),
            admission_timeout,
        }
    }

    async fn execute<T, F>(&self, work: F) -> Result<T, PasswordWorkError>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, PasswordWorkError> + Send + 'static,
    {
        let permit =
            tokio::time::timeout(self.admission_timeout, self.permits.clone().acquire_owned())
                .await
                .map_err(|_| PasswordWorkError::AdmissionTimeout)?
                .map_err(|_| PasswordWorkError::WorkerUnavailable)?;
        tokio::task::spawn_blocking(move || {
            // Cancellation of the async caller must not release a running CPU slot.
            let _permit = permit;
            work()
        })
        .await
        .map_err(|_| PasswordWorkError::WorkerUnavailable)?
    }
}

fn password_work() -> &'static PasswordWork {
    PASSWORD_WORK.get_or_init(|| PasswordWork::new(MAX_ACTIVE_JOBS, ADMISSION_TIMEOUT))
}

/// Verify an existing bcrypt hash without changing its stored policy.
pub async fn verify(password: &str, password_hash: &str) -> Result<bool, PasswordWorkError> {
    let password = Zeroizing::new(password.to_owned());
    let password_hash = password_hash.to_owned();
    password_work()
        .execute(move || {
            bcrypt::verify(password.as_bytes(), &password_hash)
                .map_err(|_| PasswordWorkError::BcryptFailed)
        })
        .await
}

/// Preserve the caller's legacy truncating bcrypt policy and cost.
pub async fn hash(password: &str, cost: u32) -> Result<String, PasswordWorkError> {
    let password = Zeroizing::new(password.to_owned());
    password_work()
        .execute(move || {
            bcrypt::hash(password.as_bytes(), cost).map_err(|_| PasswordWorkError::BcryptFailed)
        })
        .await
}

/// Preserve password-change rejection of inputs exceeding bcrypt's byte limit.
pub async fn non_truncating_hash(password: &str, cost: u32) -> Result<String, PasswordWorkError> {
    let password = Zeroizing::new(password.to_owned());
    password_work()
        .execute(move || {
            bcrypt::non_truncating_hash(password.as_bytes(), cost)
                .map_err(|_| PasswordWorkError::BcryptFailed)
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::{poll_fn, Future},
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Condvar, Mutex,
        },
        task::Poll,
    };

    type GateState = Arc<(Mutex<bool>, Condvar)>;

    struct Gate {
        state: GateState,
    }

    impl Gate {
        fn new() -> Self {
            Self {
                state: Arc::new((Mutex::new(false), Condvar::new())),
            }
        }

        fn open(&self) {
            *self.state.0.lock().unwrap() = true;
            self.state.1.notify_all();
        }
    }

    impl Drop for Gate {
        fn drop(&mut self) {
            self.open();
        }
    }

    fn wait_for_gate(gate: &GateState) {
        // A broken synchronous implementation must fail rather than hang the runtime forever.
        let (_guard, timeout) = gate
            .1
            .wait_timeout_while(gate.0.lock().unwrap(), Duration::from_secs(5), |open| {
                !*open
            })
            .unwrap();
        assert!(
            !timeout.timed_out(),
            "async executor could not release password work"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn shared_budget_caps_cpu_jobs_without_blocking_the_async_executor() {
        let budget = Arc::new(PasswordWork::new(MAX_ACTIVE_JOBS, ADMISSION_TIMEOUT));
        let gate = Gate::new();
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let (started, mut starts) = tokio::sync::mpsc::unbounded_channel();
        let mut jobs = Vec::new();
        for _ in 0..6 {
            let budget = budget.clone();
            let gate = gate.state.clone();
            let active = active.clone();
            let peak = peak.clone();
            let started = started.clone();
            jobs.push(tokio::spawn(async move {
                budget
                    .execute(move || {
                        let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(count, Ordering::SeqCst);
                        started.send(()).unwrap();
                        wait_for_gate(&gate);
                        active.fetch_sub(1, Ordering::SeqCst);
                        Ok(())
                    })
                    .await
            }));
        }
        for _ in 0..MAX_ACTIVE_JOBS {
            starts.recv().await.unwrap();
        }
        // On one async executor thread this code can run while all four CPU jobs are blocked.
        tokio::task::yield_now().await;
        assert_eq!(active.load(Ordering::SeqCst), MAX_ACTIVE_JOBS);
        assert_eq!(budget.permits.available_permits(), 0);
        gate.open();
        for job in jobs {
            job.await.unwrap().unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), MAX_ACTIVE_JOBS);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn admission_has_one_deadline_and_never_starts_rejected_work() {
        let budget = PasswordWork::new(1, Duration::from_millis(30));
        let _occupied = budget.permits.acquire().await.unwrap();
        let started = Arc::new(AtomicBool::new(false));
        let attempted = started.clone();
        let before = std::time::Instant::now();
        let result = budget
            .execute(move || {
                attempted.store(true, Ordering::SeqCst);
                Ok(())
            })
            .await;
        assert_eq!(result, Err(PasswordWorkError::AdmissionTimeout));
        assert!(before.elapsed() >= Duration::from_millis(30));
        assert!(!started.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn cancelling_queued_work_removes_it_without_consuming_a_slot() {
        let budget = PasswordWork::new(1, ADMISSION_TIMEOUT);
        let occupied = budget.permits.acquire().await.unwrap();
        let started = Arc::new(AtomicBool::new(false));
        let attempted = started.clone();
        let mut queued = Box::pin(budget.execute(move || {
            attempted.store(true, Ordering::SeqCst);
            Ok(())
        }));
        poll_fn(|context| {
            assert!(queued.as_mut().poll(context).is_pending());
            Poll::Ready(())
        })
        .await;
        drop(queued);
        drop(occupied);
        budget.execute(|| Ok(())).await.unwrap();
        assert!(!started.load(Ordering::SeqCst));
        assert_eq!(budget.permits.available_permits(), 1);
    }

    #[tokio::test]
    async fn cancelling_inflight_work_retains_its_slot_until_the_blocking_job_finishes() {
        let budget = Arc::new(PasswordWork::new(1, ADMISSION_TIMEOUT));
        let gate = Gate::new();
        let blocked = gate.state.clone();
        let (started, entered) = tokio::sync::oneshot::channel();
        let running_budget = budget.clone();
        let running = tokio::spawn(async move {
            running_budget
                .execute(move || {
                    started.send(()).unwrap();
                    wait_for_gate(&blocked);
                    Ok(())
                })
                .await
        });
        entered.await.unwrap();
        running.abort();
        assert!(running.await.unwrap_err().is_cancelled());
        assert_eq!(budget.permits.available_permits(), 0);
        let mut next = Box::pin(budget.execute(|| Ok(())));
        poll_fn(|context| {
            assert!(next.as_mut().poll(context).is_pending());
            Poll::Ready(())
        })
        .await;
        gate.open();
        next.await.unwrap();
        assert_eq!(budget.permits.available_permits(), 1);
    }

    #[tokio::test]
    async fn real_bcrypt_preserves_cost_legacy_truncation_and_strict_password_change_policy() {
        let password = "synthetic-password";
        for cost in [8, bcrypt::DEFAULT_COST] {
            let encoded = hash(password, cost).await.unwrap();
            assert_eq!(
                encoded.parse::<bcrypt::HashParts>().unwrap().get_cost(),
                cost
            );
            assert!(bcrypt::verify(password, &encoded).unwrap());
            assert!(verify(password, &encoded).await.unwrap());
            assert!(!verify("different-password", &encoded).await.unwrap());
        }
        let long = "x".repeat(73);
        let legacy = hash(&long, 4).await.unwrap();
        assert!(verify(&long[..72], &legacy).await.unwrap());
        assert_eq!(
            non_truncating_hash(&long, 4).await,
            Err(PasswordWorkError::BcryptFailed)
        );
        let strict = non_truncating_hash(password, 4).await.unwrap();
        assert!(bcrypt::verify(password, &strict).unwrap());
        assert_eq!(
            verify(password, "malformed-hash").await,
            Err(PasswordWorkError::BcryptFailed)
        );
    }
}
