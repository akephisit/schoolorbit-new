use chrono::{DateTime, Utc};
use std::time::Duration;

use crate::utils::file_hash::FileHasher;

use super::{
    platform_service::{generate_derivative_body, FilePlatform},
    purpose_registry::purpose_definition,
    repository::{FileRepository, OperationWork},
    storage_provider::StorageError,
};

pub const MAX_OPERATION_ATTEMPTS: i32 = 8;
pub const DEFAULT_RECONCILE_BATCH: i64 = 25;
pub const DEFAULT_LEASE_DURATION: Duration = Duration::from_secs(60);
const BASE_RETRY_DELAY: Duration = Duration::from_secs(5);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(3600);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReconcileSummary {
    pub leased: usize,
    pub succeeded: usize,
    pub retried: usize,
    pub terminal: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OperationDisposition {
    Succeeded,
    Retried,
    Terminal,
}

pub fn bounded_retry_delay(attempt: i32) -> Duration {
    let exponent = u32::try_from(attempt.saturating_sub(1).clamp(0, 16)).unwrap_or(0);
    let multiplier = 1_u64.checked_shl(exponent).unwrap_or(u64::MAX);
    Duration::from_secs(
        BASE_RETRY_DELAY
            .as_secs()
            .saturating_mul(multiplier)
            .min(MAX_RETRY_DELAY.as_secs()),
    )
}

pub async fn reconcile_due_operations(
    platform: &FilePlatform,
    repository: &dyn FileRepository,
    worker: &str,
) -> Result<ReconcileSummary, super::repository::RepositoryError> {
    reconcile_due_operations_at(platform, repository, worker, Utc::now()).await
}

async fn reconcile_due_operations_at(
    platform: &FilePlatform,
    repository: &dyn FileRepository,
    worker: &str,
    now: DateTime<Utc>,
) -> Result<ReconcileSummary, super::repository::RepositoryError> {
    let operations = repository
        .lease_due_operations(worker, now, DEFAULT_LEASE_DURATION, DEFAULT_RECONCILE_BATCH)
        .await?;
    let mut summary = ReconcileSummary {
        leased: operations.len(),
        ..ReconcileSummary::default()
    };

    for operation in operations {
        let terminal = operation.attempt_count >= MAX_OPERATION_ATTEMPTS;
        let result = match &operation.work {
            OperationWork::ReconcileUpload {
                version_id,
                original,
                required_derivative_ids,
                required_derivatives,
            } => match platform.provider().head(original).await {
                Ok(Some(_)) => {
                    let mut required_failure = None;
                    for derivative in required_derivatives {
                        match platform.provider().head(derivative).await {
                            Ok(Some(_)) => {}
                            Ok(None) => {
                                required_failure = Some("file_required_derivative_pending");
                                break;
                            }
                            Err(error) => {
                                required_failure = Some(error.log_safe_code());
                                break;
                            }
                        }
                    }
                    if let Some(error_code) = required_failure {
                        retry(
                            repository,
                            operation.id,
                            operation.attempt_count,
                            error_code,
                            terminal,
                            now,
                        )
                        .await
                    } else {
                        repository
                            .finalize_ready(operation.file_id, *version_id, required_derivative_ids)
                            .await
                            .map(|_| OperationDisposition::Succeeded)
                    }
                }
                Ok(None) => repository
                    .mark_upload_failed(operation.file_id, *version_id, "file_original_missing")
                    .await
                    .map(|_| OperationDisposition::Terminal),
                Err(error) => {
                    retry(
                        repository,
                        operation.id,
                        operation.attempt_count,
                        error.log_safe_code(),
                        terminal,
                        now,
                    )
                    .await
                }
            },
            OperationWork::GenerateDerivative(work) => {
                let maximum = purpose_definition(work.purpose)
                    .map(|definition| definition.limits.max_bytes)
                    .unwrap_or(1);
                match platform.provider().get(&work.source, maximum).await {
                    Ok(source) => {
                        match generate_derivative_body(work.purpose, work.recipe, &source) {
                            Ok(body) => {
                                let identity_matches = i64::try_from(body.len()).ok()
                                    == Some(work.expected_byte_size)
                                    && FileHasher::sha256(&body) == work.expected_checksum;
                                if !identity_matches {
                                    retry_derivative(
                                        repository,
                                        &operation,
                                        work.derivative_id,
                                        "file_derivative_identity_mismatch",
                                        true,
                                        now,
                                    )
                                    .await
                                } else {
                                    match platform.provider().put(&work.target, body).await {
                                        Ok(()) => {
                                            commit_derivative(
                                                platform,
                                                repository,
                                                &operation,
                                                work.derivative_id,
                                                &work.target,
                                            )
                                            .await
                                        }
                                        Err(StorageError::AlreadyExists) => {
                                            match platform.provider().head(&work.target).await {
                                                Ok(Some(_)) => {
                                                    commit_derivative(
                                                        platform,
                                                        repository,
                                                        &operation,
                                                        work.derivative_id,
                                                        &work.target,
                                                    )
                                                    .await
                                                }
                                                _ => {
                                                    retry_derivative(
                                                        repository,
                                                        &operation,
                                                        work.derivative_id,
                                                        "storage_object_conflict",
                                                        terminal,
                                                        now,
                                                    )
                                                    .await
                                                }
                                            }
                                        }
                                        Err(error) => {
                                            retry_derivative(
                                                repository,
                                                &operation,
                                                work.derivative_id,
                                                error.log_safe_code(),
                                                terminal,
                                                now,
                                            )
                                            .await
                                        }
                                    }
                                }
                            }
                            Err(error) => {
                                retry_derivative(
                                    repository,
                                    &operation,
                                    work.derivative_id,
                                    error.log_safe_code(),
                                    true,
                                    now,
                                )
                                .await
                            }
                        }
                    }
                    Err(error) => {
                        retry_derivative(
                            repository,
                            &operation,
                            work.derivative_id,
                            error.log_safe_code(),
                            terminal,
                            now,
                        )
                        .await
                    }
                }
            }
            OperationWork::DeleteObject(work) => {
                match platform.provider().delete(&work.object).await {
                    Ok(()) => repository
                        .mark_delete_succeeded(work)
                        .await
                        .map(|_| OperationDisposition::Succeeded),
                    Err(error) => {
                        retry(
                            repository,
                            operation.id,
                            operation.attempt_count,
                            error.log_safe_code(),
                            terminal,
                            now,
                        )
                        .await
                    }
                }
            }
        };

        match result {
            Ok(OperationDisposition::Succeeded) => summary.succeeded += 1,
            Ok(OperationDisposition::Retried) => summary.retried += 1,
            Ok(OperationDisposition::Terminal) => summary.terminal += 1,
            Err(_) if terminal => summary.terminal += 1,
            Err(_) => summary.retried += 1,
        }
    }

    Ok(summary)
}

async fn commit_derivative(
    platform: &FilePlatform,
    repository: &dyn FileRepository,
    operation: &super::repository::LeasedOperation,
    derivative_id: uuid::Uuid,
    target: &super::storage_provider::StoredObject,
) -> Result<OperationDisposition, super::repository::RepositoryError> {
    platform
        .commit_reconciled_derivative(
            repository,
            operation.file_id,
            derivative_id,
            operation.id,
            target,
        )
        .await
        .map(|stored| {
            if stored {
                OperationDisposition::Succeeded
            } else {
                OperationDisposition::Terminal
            }
        })
}

async fn retry(
    repository: &dyn FileRepository,
    operation_id: uuid::Uuid,
    attempt: i32,
    error_code: &'static str,
    terminal: bool,
    now: DateTime<Utc>,
) -> Result<OperationDisposition, super::repository::RepositoryError> {
    repository
        .retry_operation(
            operation_id,
            error_code,
            now + chrono::Duration::from_std(bounded_retry_delay(attempt))
                .unwrap_or_else(|_| chrono::Duration::hours(1)),
            terminal,
        )
        .await
        .map(|_| {
            if terminal {
                OperationDisposition::Terminal
            } else {
                OperationDisposition::Retried
            }
        })
}

async fn retry_derivative(
    repository: &dyn FileRepository,
    operation: &super::repository::LeasedOperation,
    derivative_id: uuid::Uuid,
    error_code: &'static str,
    terminal: bool,
    now: DateTime<Utc>,
) -> Result<OperationDisposition, super::repository::RepositoryError> {
    repository
        .mark_derivative_failed(
            operation.file_id,
            derivative_id,
            operation.id,
            error_code,
            now + chrono::Duration::from_std(bounded_retry_delay(operation.attempt_count))
                .unwrap_or_else(|_| chrono::Duration::hours(1)),
            terminal,
        )
        .await
        .map(|_| {
            if terminal {
                OperationDisposition::Terminal
            } else {
                OperationDisposition::Retried
            }
        })
}

#[cfg(test)]
mod tests {
    use super::{bounded_retry_delay, MAX_OPERATION_ATTEMPTS};
    use std::time::Duration;

    #[test]
    fn retry_backoff_is_bounded_and_attempts_become_terminal() {
        assert_eq!(bounded_retry_delay(0), Duration::from_secs(5));
        assert_eq!(bounded_retry_delay(1), Duration::from_secs(5));
        assert_eq!(bounded_retry_delay(2), Duration::from_secs(10));
        assert_eq!(bounded_retry_delay(100), Duration::from_secs(3600));
        assert!(MAX_OPERATION_ATTEMPTS < 100);
    }
}
