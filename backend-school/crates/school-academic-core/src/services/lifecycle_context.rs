use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::models::{
    TermLifecycleContext, YearLifecycleContext, YearRecoveryState, YearTermLifecycleState,
};

pub async fn read_term_context(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    term: Uuid,
) -> Result<TermLifecycleContext, AppError> {
    sqlx::query_as(
        "SELECT year.id AS academic_year_id,term.id AS academic_term_id,year.name AS year_name,
         term.name AS term_name,year.status AS year_status,term.status AS term_status,
         year.row_version AS year_row_version,term.row_version AS term_row_version,
         year.start_date AS year_start_date,year.end_date AS year_end_date,term.start_date AS term_start_date,
         term.planned_end_date,term.closed_on,term.sequence_no AS sequence,term.bell_schedule_id,
         term.included_in_year_result,term.blocks_year_closure
         FROM academic_terms term JOIN academic_years year ON year.id=term.academic_year_id
         WHERE year.id=$1 AND term.id=$2",
    )
    .bind(year)
    .bind(term)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียนในปีการศึกษาที่เลือก".into()))
}

pub async fn read_year_context(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
) -> Result<YearLifecycleContext, AppError> {
    sqlx::query_as("SELECT id AS academic_year_id,year,name,start_date,end_date,status,row_version FROM academic_years WHERE id=$1")
        .bind(year)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบปีการศึกษา".into()))
}

pub async fn read_year_terms(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
) -> Result<Vec<YearTermLifecycleState>, AppError> {
    let terms: Vec<YearTermLifecycleState> = sqlx::query_as(
        "SELECT id AS academic_term_id,name,sequence_no AS sequence,status,included_in_year_result,blocks_year_closure,row_version
         FROM academic_terms WHERE academic_year_id=$1 ORDER BY sequence_no,id LIMIT 101",
    )
    .bind(year)
    .fetch_all(&mut **tx)
    .await?;
    if terms.len() > 100 {
        return Err(AppError::ValidationError(
            "ปีการศึกษามีภาคเรียนเกิน 100 รายการ".into(),
        ));
    }
    Ok(terms)
}

pub async fn read_year_recovery_state(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
) -> Result<YearRecoveryState, AppError> {
    let context = read_year_context(tx, year).await?;
    let (running_years, successor_years, running_terms): (i64, i64, i64) = sqlx::query_as(
        "SELECT
         (SELECT count(*) FROM academic_years WHERE id<>$1 AND status IN ('active','closing')),
         (SELECT count(*) FROM academic_years successor WHERE successor.id<>$1 AND successor.start_date>$2 AND
          (successor.status IN ('active','closing','closed','archived') OR EXISTS(
           SELECT 1 FROM academic_year_transition_receipts receipt WHERE receipt.academic_year_id=successor.id AND receipt.action='activate'))),
         (SELECT count(*) FROM academic_terms WHERE status IN ('active','closing'))",
    )
    .bind(year)
    .bind(context.start_date)
    .fetch_one(&mut **tx)
    .await?;
    Ok(YearRecoveryState {
        context,
        running_years,
        successor_years,
        running_terms,
    })
}

pub fn stable_checksum(value: &impl serde::Serialize) -> Result<String, AppError> {
    use sha2::{Digest, Sha256};

    let bytes = serde_json::to_vec(value)
        .map_err(|_| AppError::InternalServerError("ไม่สามารถสร้างรุ่นความพร้อมได้".into()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
