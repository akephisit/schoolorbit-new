use chrono::NaiveDate;
use school_database::DbSession;
use serde::Serialize;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use school_errors::AppError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct PublishedOfferingReference {
    id: Uuid,
    kind: String,
    starts_on: NaiveDate,
    ends_on: Option<NaiveDate>,
    row_version: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedOfferingEvidence {
    delivery_version_id: Option<Uuid>,
    references: Vec<PublishedOfferingReference>,
    pub source_checksum: String,
}

impl PublishedOfferingEvidence {
    pub fn count(&self) -> usize {
        self.references.len()
    }
}

pub async fn published_for_term(
    tx: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    on_date: NaiveDate,
) -> Result<PublishedOfferingEvidence, AppError> {
    let delivery_version_id: Option<Uuid> = DbSession::new(&mut **tx)
        .scalar(
            "academic_delivery.services.opening.applicable_publication",
            "SELECT academic_delivery_applicable_publication($1,$2)",
        )
        .bind(academic_term_id)
        .bind(on_date)
        .fetch_one()
        .await?;
    let references: Vec<PublishedOfferingReference> = DbSession::new(&mut **tx)
        .query_as(
            "academic_delivery.services.opening.published_for_term",
            "WITH selected AS MATERIALIZED(SELECT snapshot FROM academic_delivery_versions WHERE id=$3), included AS MATERIALIZED(
            SELECT (offering->>'id')::uuid id FROM selected,jsonb_array_elements(snapshot->'offerings') offering)
         SELECT registry.id,registry.kind,registry.starts_on,registry.ends_on,registry.row_version
         FROM learning_offerings registry JOIN included ON included.id=registry.id
         WHERE registry.academic_year_id=$1 AND registry.academic_term_id=$2 AND registry.status='published'
         ORDER BY registry.id LIMIT 5001",
        )
        .bind(academic_year_id)
        .bind(academic_term_id)
        .bind(delivery_version_id)
        .fetch_all()
        .await?;
    if references.len() > 5_000 {
        return Err(AppError::ValidationError(
            "รายการเปิดสอนเกิน 5,000 รายการ กรุณาติดต่อผู้ดูแลระบบ".into(),
        ));
    }
    let source_checksum = super::stable_hash(&(delivery_version_id, &references))?;
    Ok(PublishedOfferingEvidence {
        delivery_version_id,
        references,
        source_checksum,
    })
}
