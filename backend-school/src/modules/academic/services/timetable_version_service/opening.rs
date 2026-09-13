use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct PublishedTimetableReference {
    id: Uuid,
    effective_from: NaiveDate,
    bell_schedule_id: Uuid,
    row_version: i64,
    published_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublishedTimetableEvidence {
    references: Vec<PublishedTimetableReference>,
    pub source_checksum: String,
}

impl PublishedTimetableEvidence {
    pub(crate) fn count(&self) -> usize {
        self.references.len()
    }
}

pub(crate) async fn published_for_term(
    tx: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
    on_date: NaiveDate,
    bell_schedule_id: Uuid,
) -> Result<PublishedTimetableEvidence, AppError> {
    let references: Vec<PublishedTimetableReference> = sqlx::query_as(
        "SELECT id,effective_from,bell_schedule_id,row_version,published_at
         FROM academic_timetable_versions
         WHERE academic_year_id=$1 AND academic_term_id=$2 AND status='published'
           AND effective_from<=$3 AND bell_schedule_id=$4
         ORDER BY effective_from,id LIMIT 501",
    )
    .bind(academic_year_id)
    .bind(academic_term_id)
    .bind(on_date)
    .bind(bell_schedule_id)
    .fetch_all(&mut **tx)
    .await?;
    if references.len() > 500 {
        return Err(AppError::ValidationError(
            "รุ่นตารางสอนที่เผยแพร่เกิน 500 รุ่น กรุณาติดต่อผู้ดูแลระบบ".into(),
        ));
    }
    let bytes = serde_json::to_vec(&references)
        .map_err(|_| AppError::InternalServerError("ไม่สามารถสร้างรุ่นข้อมูลตารางสอนได้".into()))?;
    Ok(PublishedTimetableEvidence {
        references,
        source_checksum: hex::encode(Sha256::digest(bytes)),
    })
}
