use chrono::NaiveDate;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::models::versions::{DeliveryPublicationContext, DeliveryVersion, PublicationDateRules};

pub async fn date_rules_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
) -> Result<PublicationDateRules, AppError> {
    let active: bool = DbSession::new(connection)
        .scalar(
            "academic_delivery.services.publication.date_rules",
            "SELECT status='active' FROM academic_terms WHERE id=$1",
        )
        .bind(term)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".into()))?;
    let today = super::bangkok_today();
    Ok(PublicationDateRules {
        today,
        minimum_effective_from: active.then_some(today),
    })
}

pub fn context(version: &DeliveryVersion) -> Result<DeliveryPublicationContext, AppError> {
    let publication_no = version
        .publication_no
        .ok_or_else(|| AppError::Conflict("รุ่นเปิดสอนยังไม่ได้เผยแพร่".into()))?;
    let effective_from = version
        .effective_from
        .ok_or_else(|| AppError::Conflict("รุ่นเปิดสอนไม่มีวันที่เริ่มใช้".into()))?;
    Ok(DeliveryPublicationContext {
        id: version.id,
        publication_no,
        effective_from,
        effective_until: version.effective_until,
        published_at: version.published_at,
        is_superseded: version.is_superseded,
    })
}

pub async fn context_on_connection(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<DeliveryPublicationContext, AppError> {
    context(&super::versions::get_version_on_connection(connection, id).await?)
}

/// Select the date's winner, independently of the latest planning publication.
pub async fn applicable_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
) -> Result<Option<DeliveryPublicationContext>, AppError> {
    let id: Option<Uuid> = DbSession::new(&mut *connection).scalar("academic_delivery.services.publication.applicable", "SELECT id FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published' AND effective_from<=$2 ORDER BY publication_no DESC LIMIT 1")
        .bind(term).bind(date).fetch_optional().await?;
    match id {
        Some(id) => Ok(Some(context_on_connection(connection, id).await?)),
        None => Ok(None),
    }
}
