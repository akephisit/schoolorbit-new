use crate::file_hash::FileHasher;
use std::sync::atomic::{AtomicBool, Ordering};

use super::{
    platform_service::{generate_derivative_body, FilePlatform},
    purpose_registry::purpose_definition,
    repository::{
        FileRepository, LeasedOperation, LeasedOperationResult, OperationWork, RepositoryError,
    },
    storage_provider::StorageError,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReconcileSummary {
    pub leased: usize,
    pub succeeded: usize,
    pub retried: usize,
    pub terminal: usize,
}

/// Standalone orchestration also claims just before each item, so waiting for
/// earlier provider calls cannot consume a later item's lease.
pub async fn reconcile_due_operations(
    platform: &FilePlatform,
    repository: &dyn FileRepository,
    worker: &str,
) -> Result<ReconcileSummary, RepositoryError> {
    let config = platform.runtime_config();
    let mut summary = ReconcileSummary::default();
    for _ in 0..config.reconciliation_batch_size {
        let Some(operation) = repository
            .lease_due_operations(worker, config.reconciliation_lease, 1)
            .await?
            .pop()
        else {
            break;
        };
        summary.leased += 1;
        let result = perform_leased_operation(platform, &operation).await;
        match repository.finish_leased_operation(&operation, result).await {
            Ok(()) => match result {
                LeasedOperationResult::Succeeded => summary.succeeded += 1,
                LeasedOperationResult::OriginalMissing
                | LeasedOperationResult::Failed { terminal: true, .. }
                | LeasedOperationResult::DerivativeWriteUncertain { terminal: true, .. } => {
                    summary.terminal += 1
                }
                LeasedOperationResult::Failed {
                    terminal: false, ..
                }
                | LeasedOperationResult::DerivativeWriteUncertain {
                    terminal: false, ..
                } => summary.retried += 1,
            },
            Err(RepositoryError::LeaseLost) => {}
            Err(_) => summary.retried += 1,
        }
    }
    Ok(summary)
}

/// Only owned operation data enters provider work. No pool, connection or
/// transaction is retained here. Leave half of the lease for the fenced commit.
pub async fn perform_leased_operation(
    platform: &FilePlatform,
    operation: &LeasedOperation,
) -> LeasedOperationResult {
    let config = platform.runtime_config();
    let write_started = AtomicBool::new(false);
    let result = tokio::time::timeout(
        config.reconciliation_lease / 2,
        perform(platform, operation, &write_started),
    )
    .await;
    match result {
        Ok(Ok(())) => LeasedOperationResult::Succeeded,
        Ok(Err(ProviderFailure::OriginalMissing)) => LeasedOperationResult::OriginalMissing,
        failure => {
            let (error_code, permanent) = match failure {
                Ok(Err(ProviderFailure::Retry(code, permanent))) => (code, permanent),
                _ => ("file_operation_deadline", false),
            };
            let retry_delay = config.retry_delay(operation.attempt_count);
            let terminal = permanent || operation.attempt_count >= config.max_operation_attempts;
            if write_started.load(Ordering::Relaxed) {
                LeasedOperationResult::DerivativeWriteUncertain {
                    error_code,
                    retry_delay,
                    terminal,
                }
            } else {
                LeasedOperationResult::Failed {
                    error_code,
                    retry_delay,
                    terminal,
                }
            }
        }
    }
}

enum ProviderFailure {
    OriginalMissing,
    Retry(&'static str, bool),
}
impl From<StorageError> for ProviderFailure {
    fn from(error: StorageError) -> Self {
        Self::Retry(error.log_safe_code(), false)
    }
}

async fn perform(
    platform: &FilePlatform,
    operation: &LeasedOperation,
    write_started: &AtomicBool,
) -> Result<(), ProviderFailure> {
    match &operation.work {
        OperationWork::ReconcileUpload {
            original,
            required_derivatives,
            ..
        } => {
            if platform.provider().head(original).await?.is_none() {
                return Err(ProviderFailure::OriginalMissing);
            }
            for derivative in required_derivatives {
                if platform.provider().head(derivative).await?.is_none() {
                    return Err(ProviderFailure::Retry(
                        "file_required_derivative_pending",
                        false,
                    ));
                }
            }
            Ok(())
        }
        OperationWork::GenerateDerivative(work) => {
            let maximum = purpose_definition(work.purpose)
                .map(|definition| definition.limits.max_bytes)
                .unwrap_or(1);
            let source = platform.provider().get(&work.source, maximum).await?;
            let body = generate_derivative_body(work.purpose, work.recipe, &source)
                .map_err(|error| ProviderFailure::Retry(error.log_safe_code(), true))?;
            if i64::try_from(body.len()).ok() != Some(work.expected_byte_size)
                || FileHasher::sha256(&body) != work.expected_checksum
            {
                return Err(ProviderFailure::Retry(
                    "file_derivative_identity_mismatch",
                    true,
                ));
            }
            write_started.store(true, Ordering::Relaxed);
            match platform.provider().put(&work.target, body).await {
                Ok(()) => Ok(()),
                Err(StorageError::AlreadyExists) => {
                    if platform.provider().head(&work.target).await?.is_some() {
                        Ok(())
                    } else {
                        Err(ProviderFailure::Retry("storage_object_conflict", false))
                    }
                }
                Err(error) => Err(error.into()),
            }
        }
        OperationWork::DeleteObject(work) => platform
            .provider()
            .delete(&work.object)
            .await
            .map_err(Into::into),
    }
}
