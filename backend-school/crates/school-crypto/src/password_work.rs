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
