//! Physical source removal requires a sealed allowance from the owning domain.
//! Notifications retain ownership of their inboxes, receipts and presentations.
use school_database::DbSession;
use school_errors::AppError;
use school_notifications::dispatch::{self, NotificationSourceKind};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::process_models::ProcessSource;

/// Removes every workflow attempt for an exact, transaction-authorized source.
/// The source owner must lock and authorize its aggregate before issuing the
/// database allowance. Calling this provider alone cannot authorize deletion.
pub async fn purge_source_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    source: &ProcessSource,
) -> Result<(), AppError> {
    DbSession::new(&mut **tx)
        .query(
            "workflow.purge.lock_source",
            "SELECT workflow_lock_source_removal($1,$2,$3)",
        )
        .bind(&source.module_code)
        .bind(&source.resource_type)
        .bind(source.resource_id)
        .execute()
        .await?;
    let notification_sources: Vec<Uuid> = DbSession::new(&mut **tx)
        .scalar(
            "workflow.purge.notification_sources",
            "SELECT s.id FROM process_notification_sources s JOIN process_instances i ON i.id=s.instance_id WHERE i.source_module=$1 AND i.source_type=$2 AND i.source_id=$3 ORDER BY s.id",
        )
        .bind(&source.module_code)
        .bind(&source.resource_type)
        .bind(source.resource_id)
        .fetch_all()
        .await?;
    dispatch::cancel_sources_in_transaction(
        tx,
        NotificationSourceKind::Process,
        &notification_sources,
        true,
    )
    .await?;
    DbSession::new(&mut **tx)
        .query(
            "workflow.purge.finalize_source",
            "SELECT workflow_finalize_source_removal($1,$2,$3)",
        )
        .bind(&source.module_code)
        .bind(&source.resource_type)
        .bind(source.resource_id)
        .execute()
        .await?;
    Ok(())
}
