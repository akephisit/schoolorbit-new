use crate::{
    models::*,
    rules::{digest, invalid},
};
use chrono::{NaiveDate, Utc};
use school_errors::AppError;
use serde::{de::DeserializeOwned, Serialize};
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

/// Caller takes any Attendance and academic lifecycle locks first.
pub async fn lock(connection: &mut PgConnection) -> Result<(), AppError> {
    raw_lock(connection).await?;
    let busy: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM behavior_cleanup_jobs WHERE status='pending')",
    )
    .fetch_one(connection)
    .await?;
    if busy {
        return Err(AppError::Conflict(
            "กำลังล้างรายละเอียดคะแนน กรุณารอให้เสร็จก่อนบันทึกหรือแก้ไข".into(),
        ));
    }
    Ok(())
}
pub(super) async fn raw_lock(connection: &mut PgConnection) -> Result<(), AppError> {
    sqlx::query("SELECT pg_advisory_xact_lock(17020000)")
        .execute(connection)
        .await?;
    Ok(())
}
pub async fn bump(connection: &mut PgConnection) -> Result<(), AppError> {
    sqlx::query("UPDATE behavior_state SET revision=revision+1 WHERE singleton")
        .execute(connection)
        .await?;
    Ok(())
}
pub async fn revision(connection: &mut PgConnection) -> Result<i64, AppError> {
    Ok(
        sqlx::query_scalar("SELECT revision FROM behavior_state WHERE singleton")
            .fetch_one(connection)
            .await?,
    )
}
pub async fn current_cycle(
    connection: &mut PgConnection,
    side: BehaviorSide,
) -> Result<BehaviorCycle, AppError> {
    Ok(sqlx::query_as("SELECT id,side,starts_on,starts_at,ends_at,ends_on,state,label FROM behavior_cycles WHERE side=$1 AND state='current'").bind(side).fetch_one(connection).await?)
}
pub async fn require_open_date(
    connection: &mut PgConnection,
    side: BehaviorSide,
    date: NaiveDate,
    source: BehaviorSource,
    term: Uuid,
) -> Result<BehaviorCycle, AppError> {
    let cycle = current_cycle(connection, side).await?;
    if date < cycle.starts_on
        || (date != today() && suppressed(connection, date, side, source, term).await?)
    {
        return Err(AppError::Conflict(
            "วันที่นี้ปิดรอบหรือล้างรายละเอียดแล้ว ใช้การปรับยอดประวัติแทน".into(),
        ));
    }
    Ok(cycle)
}
pub async fn suppressed(
    connection: &mut PgConnection,
    date: NaiveDate,
    side: BehaviorSide,
    source: BehaviorSource,
    term: Uuid,
) -> Result<bool, AppError> {
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM behavior_suppressions WHERE $1 BETWEEN start_date AND end_date AND side=$2 AND source=$3 AND(academic_term_id IS NULL OR academic_term_id=$4)AND(academic_year_id IS NULL OR academic_year_id=(SELECT academic_year_id FROM academic_terms WHERE id=$4)))").bind(date).bind(side).bind(source).bind(term).fetch_one(connection).await?)
}
pub async fn receipt<P: Serialize, T: DeserializeOwned + Send + Unpin + 'static>(
    connection: &mut PgConnection,
    actor: Uuid,
    request: Uuid,
    operation: &str,
    payload: &P,
) -> Result<Option<T>, AppError> {
    let row:Option<(String,String,sqlx::types::Json<T>)>=sqlx::query_as("SELECT operation,digest,outcome FROM behavior_receipts WHERE actor_id=$1 AND request_id=$2").bind(actor).bind(request).fetch_optional(connection).await?;
    if let Some((old_operation, old_digest, outcome)) = row {
        if old_operation != operation || old_digest != digest(payload)? {
            return Err(AppError::Conflict("รหัสคำขอนี้ถูกใช้กับข้อมูลอื่นแล้ว".into()));
        }
        return Ok(Some(outcome.0));
    }
    Ok(None)
}
pub async fn save_receipt<P: Serialize, T: Serialize>(
    connection: &mut PgConnection,
    actor: Uuid,
    request: Uuid,
    operation: &str,
    payload: &P,
    outcome: &T,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO behavior_receipts(actor_id,request_id,operation,digest,outcome)VALUES($1,$2,$3,$4,$5)").bind(actor).bind(request).bind(operation).bind(digest(payload)?).bind(sqlx::types::Json(outcome)).execute(connection).await?;
    Ok(())
}
pub async fn begin_manual(
    pool: &PgPool,
    term: Uuid,
    date: NaiveDate,
) -> Result<Transaction<'_, Postgres>, AppError> {
    let mut tx = pool.begin().await?;
    let year:Uuid=sqlx::query_scalar("SELECT academic_year_id FROM academic_terms WHERE id=$1 AND $2 BETWEEN start_date AND COALESCE(closed_on,planned_end_date,(SELECT end_date FROM academic_years WHERE id=academic_year_id))").bind(term).bind(date).fetch_optional(&mut *tx).await?.ok_or_else(||invalid("วันที่อยู่นอกภาคเรียน"))?;
    school_academic_core::services::lifecycle_guard::require_term_write(&mut tx, year, term)
        .await?;
    lock(&mut tx).await?;
    Ok(tx)
}
pub fn today() -> NaiveDate {
    Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipt_hash_changes_when_mutation_changes() {
        assert_ne!(digest(&vec![1, 2]).unwrap(), digest(&vec![1, 3]).unwrap());
    }
}
