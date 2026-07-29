use sqlx::PgPool;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::modules::files::{
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
        self.request_expired_file_deletions().await;
        match reconcile_due_operations(&self.file_platform, &self.repository, &self.worker_id).await
        {
            Ok(summary) => {
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
    }

    async fn request_expired_file_deletions(&self) {
        let file_ids = match sqlx::query_scalar::<_, Uuid>(
            r#"
SELECT id
FROM files
WHERE retention_class = 'temporary'
  AND expires_at <= now()
  AND deleted_at IS NULL
  AND lifecycle_status <> 'deleted'
ORDER BY expires_at, id
LIMIT 50
"#,
        )
        .fetch_all(self.repository.pool())
        .await
        {
            Ok(file_ids) => file_ids,
            Err(_) => {
                warn!("Expired File Platform rows could not be listed");
                return;
            }
        };

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
    }
}
