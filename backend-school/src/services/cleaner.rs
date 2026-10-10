use sqlx::PgPool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, warn};

const MAX_RECONCILIATION_BATCHES: usize = 100;
const RECONCILIATION_BUDGET: Duration = Duration::from_secs(120);
const EXPIRY_BATCH_SIZE: i64 = 50;
use uuid::Uuid;

use school_certificates::services::purge_service;
use school_file_platform::{
    platform_service::FilePlatform, reconciler::reconcile_due_operations,
    repository::SqlFileRepository,
};

pub struct FileCleaner {
    repository: SqlFileRepository,
    file_platform: Arc<FilePlatform>,
    worker_id: String,
}

impl FileCleaner {
    pub fn new(db_pool: PgPool, file_platform: Arc<FilePlatform>) -> Self {
        Self {
            repository: SqlFileRepository::new(db_pool),
            file_platform,
            worker_id: format!("file-reconciler-{}", Uuid::new_v4()),
        }
    }

    /// Domain relationships own attachment cleanup. This worker handles only
    /// explicit expiry and durable File Platform operations; it never guesses
    /// liveness from provider paths or hard-deletes immutable metadata.
    pub async fn reconcile_file_operations(&self) {
        let started = Instant::now();
        for _ in 0..MAX_RECONCILIATION_BATCHES {
            let expired = self.request_expired_file_deletions().await;
            let mut leased = 0;
            match reconcile_due_operations(&self.file_platform, &self.repository, &self.worker_id)
                .await
            {
                Ok(summary) => {
                    leased = summary.leased;
                    info!(
                        leased = summary.leased,
                        succeeded = summary.succeeded,
                        retried = summary.retried,
                        terminal = summary.terminal,
                        "File Platform reconciliation batch completed"
                    );
                }
                Err(error) => {
                    warn!(
                        error_code = error.log_safe_code(),
                        "File Platform reconciliation batch could not be leased"
                    );
                }
            }

            if started.elapsed() >= RECONCILIATION_BUDGET
                || (expired < EXPIRY_BATCH_SIZE as usize && leased == 0)
            {
                break;
            }
        }

        match purge_service::reconcile_pending_purges(self.repository.pool()).await {
            Ok(checked) => {
                info!(
                    checked,
                    "Certificate campaign purge jobs advanced after File Platform reconciliation"
                );
            }
            Err(_) => {
                warn!(
                    error_code = "certificate_purge_reconcile_failed",
                    "Certificate campaign purge jobs could not be advanced"
                );
            }
        }
    }

    async fn request_expired_file_deletions(&self) -> usize {
        let file_ids = match sqlx::query_scalar::<_, Uuid>(
            r#"
SELECT id
FROM files
WHERE retention_class = 'temporary'
  AND expires_at <= now()
  AND deleted_at IS NULL
  AND lifecycle_status NOT IN ('deleted', 'delete_requested')
ORDER BY expires_at, id
LIMIT $1
"#,
        )
        .bind(EXPIRY_BATCH_SIZE)
        .fetch_all(self.repository.pool())
        .await
        {
            Ok(file_ids) => file_ids,
            Err(_) => {
                warn!("Expired File Platform rows could not be listed");
                return 0;
            }
        };

        let selected = file_ids.len();
        for file_id in file_ids {
            match self
                .file_platform
                .request_delete(&self.repository, file_id)
                .await
            {
                Ok(outcome) => {
                    info!(
                        file_id = %file_id,
                        pending_retry = outcome.pending_retry,
                        "Expired file deletion requested"
                    );
                }
                Err(error) => {
                    warn!(
                        file_id = %file_id,
                        error_code = error.log_safe_code(),
                        "Expired file deletion request failed safely"
                    );
                }
            }
        }
        selected
    }
}
