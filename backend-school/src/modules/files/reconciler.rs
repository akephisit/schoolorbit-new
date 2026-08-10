use crate::utils::file_hash::FileHasher;

use super::{
    platform_service::{generate_derivative_body, FilePlatform},
    purpose_registry::purpose_definition,
    repository::{FileRepository, OperationWork},
    runtime_config::FilePlatformRuntimeConfig,
    storage_provider::StorageError,
};

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

pub async fn reconcile_due_operations(
    platform: &FilePlatform,
    repository: &dyn FileRepository,
    worker: &str,
) -> Result<ReconcileSummary, super::repository::RepositoryError> {
    let runtime_config = platform.runtime_config();
    let operations = repository
        .lease_due_operations(
            worker,
            runtime_config.reconciliation_lease,
            runtime_config.reconciliation_batch_size,
        )
        .await?;
    let mut summary = ReconcileSummary {
        leased: operations.len(),
        ..ReconcileSummary::default()
    };

    for operation in operations {
        let terminal = operation.attempt_count >= runtime_config.max_operation_attempts;
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
                            runtime_config,
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
                        runtime_config,
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
                                        runtime_config,
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
                                                        runtime_config,
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
                                                runtime_config,
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
                                    runtime_config,
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
                            runtime_config,
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
                            runtime_config,
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
    runtime_config: FilePlatformRuntimeConfig,
) -> Result<OperationDisposition, super::repository::RepositoryError> {
    repository
        .retry_operation(
            operation_id,
            error_code,
            runtime_config.retry_delay(attempt),
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
    runtime_config: FilePlatformRuntimeConfig,
) -> Result<OperationDisposition, super::repository::RepositoryError> {
    repository
        .mark_derivative_failed(
            operation.file_id,
            derivative_id,
            operation.id,
            error_code,
            runtime_config.retry_delay(operation.attempt_count),
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
