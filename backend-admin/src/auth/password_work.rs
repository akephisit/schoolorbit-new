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

#[derive(Clone, Copy)]
enum PasswordOperation {
    Hash,
    Verify,
}

impl PasswordOperation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Hash => "hash",
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
                capacity = MAX_PASSWORD_JOBS
            );
        });
    }
}

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

    async fn run<T, F>(&self, kind: PasswordOperation, job: F) -> Result<T, PasswordWorkError>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, PasswordWorkError> + Send + 'static,
    {
        let mut observation = PasswordObservation::new(kind);
        let permit = match tokio::time::timeout(
            ADMISSION_WAIT,
            self.permits.clone().acquire_owned(),
        )
        .await
        {
            Err(_) => {
                observation.outcome = PasswordOutcome::AdmissionTimeout;
                return Err(PasswordWorkError::AdmissionTimedOut);
            }
            Ok(Err(_)) => {
                observation.outcome = PasswordOutcome::AdmissionUnavailable;
                return Err(PasswordWorkError::WorkerUnavailable);
            }
            Ok(Ok(permit)) => permit,
        };
        observation.admitted();
        tokio::task::spawn_blocking(move || {
            // Retain capacity until the blocking computation and its observation finish.
            let _permit = permit;
            // Move the whole Drop owner into a local declared after the permit. This
            // prevents disjoint field captures and emits before capacity is returned.
            let mut observation = observation;
            observation.execution_started_at = Some(std::time::Instant::now());
            let result = job();
            observation.outcome = match &result {
                Ok(_) => PasswordOutcome::Completed,
                Err(PasswordWorkError::PasswordOperationFailed) => PasswordOutcome::BcryptFailed,
                Err(_) => PasswordOutcome::WorkerFailed,
            };
            result
        })
        .await
        .map_err(|_| PasswordWorkError::WorkerUnavailable)?
    }
}

fn worker() -> &'static PasswordWorker {
    static WORKER: OnceLock<PasswordWorker> = OnceLock::new();
    WORKER.get_or_init(PasswordWorker::new)
}

pub(crate) async fn hash(password: String) -> Result<String, PasswordWorkError> {
    let password = Zeroizing::new(password);
    worker()
        .run(PasswordOperation::Hash, move || {
            hash_password(password.as_str()).map_err(|_| PasswordWorkError::PasswordOperationFailed)
        })
        .await
}

pub(crate) async fn verify(password: String, stored: String) -> Result<bool, PasswordWorkError> {
    let password = Zeroizing::new(password);
    worker()
        .run(PasswordOperation::Verify, move || {
            verify_password(password.as_str(), &stored)
                .map_err(|_| PasswordWorkError::PasswordOperationFailed)
        })
        .await
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
                    .run(PasswordOperation::Hash, move || {
                        let running = active.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(running, Ordering::SeqCst);
                        entered.send(()).unwrap();
                        release_rx.blocking_recv().ok();
                        active.fetch_sub(1, Ordering::SeqCst);
                        Ok(())
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
            .run(PasswordOperation::Hash, move || {
                started.store(true, Ordering::SeqCst);
                Ok(())
            })
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
        let mut waiting = Box::pin(worker.run(PasswordOperation::Hash, move || {
            mark.store(true, Ordering::SeqCst);
            Ok(())
        }));
        assert!(futures::poll!(waiting.as_mut()).is_pending());
        drop(waiting);
        drop(held);
        worker
            .run(PasswordOperation::Hash, || Ok(()))
            .await
            .unwrap();
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
                .run(PasswordOperation::Hash, move || {
                    entered_tx.send(()).unwrap();
                    release_rx.blocking_recv().ok();
                    Ok(())
                })
                .await
        });
        entered_rx.await.unwrap();
        job.abort();
        assert!(job.await.unwrap_err().is_cancelled());
        assert_eq!(worker.permits.available_permits(), 0);
        let error = worker
            .run(PasswordOperation::Hash, || Ok(()))
            .await
            .unwrap_err();
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
                    let mut fields = serde_json::Map::new();
                    for token in line.split_whitespace() {
                        if let Some((name, value)) = token.split_once('=') {
                            let value = match name {
                                "operation" | "kind" | "outcome" => {
                                    serde_json::Value::String(value.trim_matches('"').to_string())
                                }
                                "capacity" | "admission_ms" | "blocking_queue_ms"
                                | "execution_ms" => serde_json::Value::Number(
                                    serde_json::Number::from_f64(value.parse().unwrap()).unwrap(),
                                ),
                                _ => panic!("unexpected field in password observation"),
                            };
                            fields.insert(name.to_string(), value);
                        }
                    }
                    fields
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
        let budget = PasswordWorker::new();
        let output = Capture::default();
        let dispatcher = output.dispatcher();
        let held = budget.permits.clone().acquire_many_owned(4).await.unwrap();
        assert_eq!(
            budget
                .run(PasswordOperation::Verify, || Ok(()))
                .with_subscriber(dispatcher.clone())
                .await,
            Err(PasswordWorkError::AdmissionTimedOut)
        );
        drop(held);
        budget.permits.close();
        assert_eq!(
            budget
                .run(PasswordOperation::Hash, || Ok(()))
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
        let budget = PasswordWorker::new();
        let output = Capture::default();
        let held = budget.permits.clone().acquire_many_owned(4).await.unwrap();
        let mut waiting = Box::pin(
            budget
                .run(PasswordOperation::Verify, || Ok(()))
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
        let budget = Arc::new(PasswordWorker::new());
        let held = budget.permits.clone().acquire_many_owned(3).await.unwrap();
        let output = Capture::default();
        let dispatcher = output.dispatcher();
        let (started, entered) = tokio::sync::oneshot::channel();
        let (release, released) = tokio::sync::oneshot::channel();
        let running_budget = budget.clone();
        let running = tokio::spawn(async move {
            running_budget
                .run(PasswordOperation::Hash, move || {
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
        let budget = PasswordWorker::new();
        let output = Capture::default();
        let result = budget
            .run(
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
    async fn real_bcrypt_outcomes_emit_numeric_fmt_without_credentials_or_match_results() {
        let output = Capture::default();
        let dispatcher = output.dispatcher();
        let password = "password-private-sentinel";
        let encoded = hash(password.to_string())
            .with_subscriber(dispatcher.clone())
            .await
            .unwrap();
        assert!(verify(password.to_string(), encoded.clone())
            .with_subscriber(dispatcher.clone())
            .await
            .unwrap());
        assert!(
            !verify("wrong-private-sentinel".to_string(), encoded.clone())
                .with_subscriber(dispatcher.clone())
                .await
                .unwrap()
        );
        assert_eq!(
            verify(password.to_string(), "hash-private-sentinel".to_string())
                .with_subscriber(dispatcher)
                .await,
            Err(PasswordWorkError::PasswordOperationFailed)
        );
        output.assert_events(&[
            ("hash", "completed", true),
            ("verify", "completed", true),
            ("verify", "completed", true),
            ("verify", "bcrypt_failed", true),
        ]);
        assert!(!output.text().contains(&encoded));
    }
}
