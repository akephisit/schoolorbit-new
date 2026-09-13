use chrono::NaiveDate;
use serde::Serialize;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;

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
pub(crate) struct PublishedOfferingEvidence {
    references: Vec<PublishedOfferingReference>,
    pub source_checksum: String,
}

impl PublishedOfferingEvidence {
    pub(crate) fn count(&self) -> usize {
        self.references.len()
    }
}

pub(crate) async fn published_for_term(
    tx: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    on_date: NaiveDate,
) -> Result<PublishedOfferingEvidence, AppError> {
    let references: Vec<PublishedOfferingReference> = sqlx::query_as(
        "SELECT id,kind,starts_on,ends_on,row_version
         FROM learning_offerings
         WHERE academic_year_id=$1 AND academic_term_id=$2 AND status='published'
           AND starts_on<=$3 AND (ends_on IS NULL OR ends_on>=$3)
         ORDER BY id LIMIT 5001",
    )
    .bind(academic_year_id)
    .bind(academic_term_id)
    .bind(on_date)
    .fetch_all(&mut **tx)
    .await?;
    if references.len() > 5_000 {
        return Err(AppError::ValidationError(
            "รายการเปิดสอนเกิน 5,000 รายการ กรุณาติดต่อผู้ดูแลระบบ".into(),
        ));
    }
    let source_checksum = super::stable_hash(&references)?;
    Ok(PublishedOfferingEvidence {
        references,
        source_checksum,
    })
}
