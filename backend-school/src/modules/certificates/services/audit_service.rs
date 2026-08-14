use serde::Serialize;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CertificateCampaignAuditMetadata {
    pub campaign_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_organization_unit_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_status: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub changed_fields: Vec<String>,
}

pub async fn record_campaign_audit(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    action: &'static str,
    metadata: CertificateCampaignAuditMetadata,
) -> Result<(), AppError> {
    let campaign_id = metadata.campaign_id;
    let metadata = serde_json::to_value(metadata).map_err(|error| {
        tracing::error!(%campaign_id, %error, "failed to serialize certificate campaign audit");
        AppError::InternalServerError("ไม่สามารถบันทึกประวัติรายการได้".to_string())
    })?;

    sqlx::query(
        "INSERT INTO audit_logs
            (user_id, action, entity_type, entity_id, metadata)
         VALUES ($1, $2, 'certificate_campaign', $3, $4)",
    )
    .bind(actor_user_id)
    .bind(action)
    .bind(campaign_id)
    .bind(metadata)
    .execute(&mut **tx)
    .await
    .map_err(|error| {
        tracing::error!(%campaign_id, %error, "failed to persist certificate campaign audit");
        AppError::DbError(error)
    })?;

    Ok(())
}
