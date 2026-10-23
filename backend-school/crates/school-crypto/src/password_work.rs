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

#[derive(Clone, Copy)]
enum PasswordOperation {
    Hash,
    NonTruncatingHash,
    Verify,
}

impl PasswordOperation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Hash => "hash",
            Self::NonTruncatingHash => "non_truncating_hash",
            Self::Verify => "verify",
        }
    }
}

#[derive(Clone, Copy)]
enum PasswordOutcome {
    Completed,
    AdmissionTimeout,
    AdmissionUnavailable,
    CancelledBeforeAdmission,
    BcryptFailed,
    WorkerFailed,
}

impl PasswordOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::AdmissionTimeout => "admission_timeout",
            Self::AdmissionUnavailable => "admission_unavailable",
            Self::CancelledBeforeAdmission => "cancelled_before_admission",
            Self::BcryptFailed => "bcrypt_failed",
            Self::WorkerFailed => "worker_failed",
        }
    }
}

// One owner moves from the waiting future into the blocking closure. Its final drop
// records exactly once, including queued cancellation and detached or panicking work.
struct PasswordObservation {
    kind: PasswordOperation,
    outcome: PasswordOutcome,
    attempted_at: std::time::Instant,
    admitted_at: Option<std::time::Instant>,
    execution_started_at: Option<std::time::Instant>,
    dispatcher: tracing::Dispatch,
}

impl PasswordObservation {
    fn new(kind: PasswordOperation) -> Self {
        Self {
            kind,
            outcome: PasswordOutcome::CancelledBeforeAdmission,
            attempted_at: std::time::Instant::now(),
            admitted_at: None,
            execution_started_at: None,
            dispatcher: tracing::dispatcher::get_default(Clone::clone),
        }
    }

    fn admitted(&mut self) {
        self.admitted_at = Some(std::time::Instant::now());
        self.outcome = PasswordOutcome::WorkerFailed;
    }
}

impl Drop for PasswordObservation {
    fn drop(&mut self) {
        let finished_at = std::time::Instant::now();
        let admission_ms = self
            .admitted_at
            .unwrap_or(finished_at)
            .saturating_duration_since(self.attempted_at)
            .as_secs_f64()
            * 1000.0;
        let blocking_queue_ms =
            self.admitted_at
                .zip(self.execution_started_at)
                .map(|(admitted, started)| {
                    started.saturating_duration_since(admitted).as_secs_f64() * 1000.0
                });
        let execution_ms = self
            .execution_started_at
            .map(|started| finished_at.saturating_duration_since(started).as_secs_f64() * 1000.0);
        tracing::dispatcher::with_default(&self.dispatcher, || {
            tracing::info!(
                operation = "password_work",
                kind = self.kind.as_str(),
                outcome = self.outcome.as_str(),
                admission_ms,
                blocking_queue_ms,
                execution_ms,
                capacity = MAX_ACTIVE_JOBS
            );
        });
    }
}

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

    async fn execute<T, F>(&self, kind: PasswordOperation, work: F) -> Result<T, PasswordWorkError>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, PasswordWorkError> + Send + 'static,
    {
        let mut observation = PasswordObservation::new(kind);
        let permit = match tokio::time::timeout(
            self.admission_timeout,
            self.permits.clone().acquire_owned(),
        )
        .await
        {
            Err(_) => {
                observation.outcome = PasswordOutcome::AdmissionTimeout;
                return Err(PasswordWorkError::AdmissionTimeout);
            }
            Ok(Err(_)) => {
                observation.outcome = PasswordOutcome::AdmissionUnavailable;
                return Err(PasswordWorkError::WorkerUnavailable);
            }
            Ok(Ok(permit)) => permit,
        };
        observation.admitted();
        tokio::task::spawn_blocking(move || {
            // Cancellation of the async caller must not release a running CPU slot.
            let _permit = permit;
            // Move the whole Drop owner into a local declared after the permit. This
            // prevents disjoint field captures and emits before capacity is returned.
            let mut observation = observation;
            observation.execution_started_at = Some(std::time::Instant::now());
            let result = work();
            observation.outcome = match &result {
                Ok(_) => PasswordOutcome::Completed,
                Err(PasswordWorkError::BcryptFailed) => PasswordOutcome::BcryptFailed,
                Err(_) => PasswordOutcome::WorkerFailed,
            };
            result
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
        .execute(PasswordOperation::Verify, move || {
            bcrypt::verify(password.as_bytes(), &password_hash)
                .map_err(|_| PasswordWorkError::BcryptFailed)
        })
        .await
}

/// Preserve the caller's legacy truncating bcrypt policy and cost.
pub async fn hash(password: &str, cost: u32) -> Result<String, PasswordWorkError> {
    let password = Zeroizing::new(password.to_owned());
    password_work()
        .execute(PasswordOperation::Hash, move || {
            bcrypt::hash(password.as_bytes(), cost).map_err(|_| PasswordWorkError::BcryptFailed)
        })
        .await
}

/// Preserve password-change rejection of inputs exceeding bcrypt's byte limit.
pub async fn non_truncating_hash(password: &str, cost: u32) -> Result<String, PasswordWorkError> {
    let password = Zeroizing::new(password.to_owned());
    password_work()
        .execute(PasswordOperation::NonTruncatingHash, move || {
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
        let expected_jobs = 4;
        assert_eq!(
            MAX_ACTIVE_JOBS, expected_jobs,
            "School password CPU policy is four jobs"
        );
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
                    .execute(PasswordOperation::Hash, move || {
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
        for _ in 0..expected_jobs {
            tokio::time::timeout(Duration::from_secs(2), starts.recv())
                .await
                .expect("password jobs must start without blocking the executor")
                .unwrap();
        }
        // On one async executor thread this code can run while all four CPU jobs are blocked.
        tokio::task::yield_now().await;
        assert_eq!(active.load(Ordering::SeqCst), expected_jobs);
        assert_eq!(budget.permits.available_permits(), 0);
        gate.open();
        for job in jobs {
            job.await.unwrap().unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), expected_jobs);
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
            .execute(PasswordOperation::Hash, move || {
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
        let mut queued = Box::pin(budget.execute(PasswordOperation::Hash, move || {
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
        budget
            .execute(PasswordOperation::Hash, || Ok(()))
            .await
            .unwrap();
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
                .execute(PasswordOperation::Hash, move || {
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
        let mut next = Box::pin(budget.execute(PasswordOperation::Hash, || Ok(())));
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

#[cfg(test)]
mod observation_tests {
    use super::*;
    use std::{io::Write, sync::Mutex};
    use tracing::instrument::WithSubscriber;

    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);

    impl Write for Capture {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Capture {
        fn dispatcher(&self) -> tracing::Dispatch {
            let writer = self.clone();
            tracing::Dispatch::new(
                tracing_subscriber::fmt()
                    .json()
                    .with_ansi(false)
                    .without_time()
                    .with_writer(move || writer.clone())
                    .finish(),
            )
        }
        fn text(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
        fn events(&self) -> Vec<serde_json::Map<String, serde_json::Value>> {
            self.text()
                .lines()
                .map(|line| {
                    let event: serde_json::Value = serde_json::from_str(line).unwrap();
                    event["fields"].as_object().unwrap().clone()
                })
                .collect()
        }
        fn assert_events(&self, expected: &[(&str, &str, bool)]) {
            let events = self.events();
            assert_eq!(
                events.len(),
                expected.len(),
                "one terminal observation per attempt"
            );
            for (event, (kind, outcome, executed)) in events.iter().zip(expected) {
                assert_eq!(event["operation"], "password_work");
                assert_eq!(event["kind"], *kind);
                assert_eq!(event["outcome"], *outcome);
                assert_eq!(event["capacity"].as_f64(), Some(4.0));
                for name in ["admission_ms", "blocking_queue_ms", "execution_ms"] {
                    if name == "admission_ms" || *executed {
                        let number = event[name].as_f64().expect("duration must be numeric");
                        assert!(number.is_finite() && number >= 0.0);
                    } else {
                        assert!(!event.contains_key(name), "unreached stages must be absent");
                    }
                }
                assert!(event.keys().all(|name| matches!(
                    name.as_str(),
                    "operation"
                        | "kind"
                        | "outcome"
                        | "capacity"
                        | "admission_ms"
                        | "blocking_queue_ms"
                        | "execution_ms"
                )));
            }
            assert!(!self.text().contains("private-sentinel"));
        }
    }

    #[tokio::test]
    async fn rejection_and_closed_admission_emit_one_observation_without_execution_fields() {
        let budget = PasswordWork::new(MAX_ACTIVE_JOBS, Duration::from_millis(10));
        let output = Capture::default();
        let dispatcher = output.dispatcher();
        let held = budget.permits.clone().acquire_many_owned(4).await.unwrap();
        assert_eq!(
            budget
                .execute(PasswordOperation::Verify, || Ok(()))
                .with_subscriber(dispatcher.clone())
                .await,
            Err(PasswordWorkError::AdmissionTimeout)
        );
        drop(held);
        budget.permits.close();
        assert_eq!(
            budget
                .execute(PasswordOperation::Hash, || Ok(()))
                .with_subscriber(dispatcher)
                .await,
            Err(PasswordWorkError::WorkerUnavailable)
        );
        output.assert_events(&[
            ("verify", "admission_timeout", false),
            ("hash", "admission_unavailable", false),
        ]);
    }

    #[tokio::test]
    async fn queued_cancellation_emits_once_with_unreached_stages_absent() {
        let budget = PasswordWork::new(MAX_ACTIVE_JOBS, Duration::from_millis(10));
        let output = Capture::default();
        let held = budget.permits.clone().acquire_many_owned(4).await.unwrap();
        let mut waiting = Box::pin(
            budget
                .execute(PasswordOperation::Verify, || Ok(()))
                .with_subscriber(output.dispatcher()),
        );
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(waiting.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(waiting);
        drop(held);
        output.assert_events(&[("verify", "cancelled_before_admission", false)]);
        assert_eq!(budget.permits.available_permits(), 4);
    }

    #[tokio::test]
    async fn inflight_cancellation_keeps_capacity_and_emits_only_when_blocking_work_finishes() {
        let budget = Arc::new(PasswordWork::new(
            MAX_ACTIVE_JOBS,
            Duration::from_millis(10),
        ));
        let held = budget.permits.clone().acquire_many_owned(3).await.unwrap();
        let output = Capture::default();
        let dispatcher = output.dispatcher();
        let (started, entered) = tokio::sync::oneshot::channel();
        let (release, released) = tokio::sync::oneshot::channel();
        let running_budget = budget.clone();
        let running = tokio::spawn(async move {
            running_budget
                .execute(PasswordOperation::Hash, move || {
                    started.send(()).unwrap();
                    released.blocking_recv().unwrap();
                    Ok(())
                })
                .with_subscriber(dispatcher)
                .await
        });
        entered.await.unwrap();
        running.abort();
        assert!(running.await.unwrap_err().is_cancelled());
        assert_eq!(budget.permits.available_permits(), 0);
        assert!(
            output.events().is_empty(),
            "a running job has no terminal observation yet"
        );
        release.send(()).unwrap();
        let finished = tokio::time::timeout(
            Duration::from_secs(2),
            budget.permits.clone().acquire_owned(),
        )
        .await
        .unwrap()
        .unwrap();
        output.assert_events(&[("hash", "completed", true)]);
        drop(finished);
        drop(held);
        assert_eq!(budget.permits.available_permits(), 4);
    }

    #[tokio::test]
    async fn panicking_work_emits_one_safe_worker_failure_before_returning_capacity() {
        let budget = PasswordWork::new(MAX_ACTIVE_JOBS, Duration::from_millis(10));
        let output = Capture::default();
        let result = budget
            .execute(
                PasswordOperation::Hash,
                || -> Result<(), PasswordWorkError> {
                    // An opaque payload tests redaction without printing its value in the panic hook.
                    std::panic::panic_any(Box::new("panic-private-sentinel".to_string()))
                },
            )
            .with_subscriber(output.dispatcher())
            .await;
        assert_eq!(result, Err(PasswordWorkError::WorkerUnavailable));
        output.assert_events(&[("hash", "worker_failed", true)]);
        assert_eq!(budget.permits.available_permits(), 4);
    }

    #[tokio::test]
    async fn real_bcrypt_outcomes_emit_numeric_json_without_credentials_or_match_results() {
        let output = Capture::default();
        let dispatcher = output.dispatcher();
        let password = "password-private-sentinel";
        let encoded = hash(password, 4)
            .with_subscriber(dispatcher.clone())
            .await
            .unwrap();
        assert!(verify(password, &encoded)
            .with_subscriber(dispatcher.clone())
            .await
            .unwrap());
        assert!(!verify("wrong-private-sentinel", &encoded)
            .with_subscriber(dispatcher.clone())
            .await
            .unwrap());
        assert_eq!(
            verify(password, "hash-private-sentinel")
                .with_subscriber(dispatcher.clone())
                .await,
            Err(PasswordWorkError::BcryptFailed)
        );
        non_truncating_hash(password, 4)
            .with_subscriber(dispatcher.clone())
            .await
            .unwrap();
        assert_eq!(
            non_truncating_hash(&"x".repeat(73), 4)
                .with_subscriber(dispatcher)
                .await,
            Err(PasswordWorkError::BcryptFailed)
        );
        output.assert_events(&[
            ("hash", "completed", true),
            ("verify", "completed", true),
            ("verify", "completed", true),
            ("verify", "bcrypt_failed", true),
            ("non_truncating_hash", "completed", true),
            ("non_truncating_hash", "bcrypt_failed", true),
        ]);
        assert!(!output.text().contains(&encoded));
    }
}
