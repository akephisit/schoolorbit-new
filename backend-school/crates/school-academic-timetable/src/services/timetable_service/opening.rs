use school_database::DbSession;
use school_errors::AppError;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct CurrentTimetableReference {
    id: Uuid,
    bell_schedule_id: Uuid,
    row_version: i64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentTimetableEvidence {
    references: Vec<CurrentTimetableReference>,
    pub source_checksum: String,
}
impl CurrentTimetableEvidence {
    pub fn count(&self) -> usize {
        self.references.len()
    }
}

pub async fn current_for_term(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    term: Uuid,
    schedule: Uuid,
) -> Result<CurrentTimetableEvidence, AppError> {
    let references: Vec<CurrentTimetableReference> = DbSession::new(&mut **tx).query_as("academic_timetable.current.opening", "SELECT id,bell_schedule_id,row_version FROM academic_timetables WHERE academic_year_id=$1 AND academic_term_id=$2 AND bell_schedule_id=$3").bind(year).bind(term).bind(schedule).fetch_all().await?;
    let bytes = serde_json::to_vec(&references)
        .map_err(|_| AppError::InternalServerError("ไม่สามารถตรวจข้อมูลตารางเรียน".into()))?;
    Ok(CurrentTimetableEvidence {
        references,
        source_checksum: hex::encode(Sha256::digest(bytes)),
    })
}
