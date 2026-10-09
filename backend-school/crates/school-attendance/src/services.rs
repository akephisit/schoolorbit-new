pub mod faces;
pub mod notifications;
pub mod reports;
pub mod sessions;
pub mod settings;
pub mod specials;
use crate::rules::invalid;
use chrono::NaiveDate;
use school_errors::AppError;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;
pub async fn lock_term(
    tx: &mut Transaction<'_, Postgres>,
    term: Uuid,
    write: bool,
    exclusive: bool,
) -> Result<(), AppError> {
    let year: Uuid = sqlx::query_scalar("SELECT academic_year_id FROM academic_terms WHERE id=$1")
        .bind(term)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".into()))?;
    let state =
        school_academic_core::services::lifecycle_guard::lock_context(tx, year, term).await?;
    if write {
        state.require_writable()?;
    }
    let sql = if exclusive {
        "SELECT pg_advisory_xact_lock(hashtextextended($1,1701))"
    } else {
        "SELECT pg_advisory_xact_lock_shared(hashtextextended($1,1701))"
    };
    sqlx::query(sql)
        .bind(term.to_string())
        .execute(&mut **tx)
        .await?;
    if write
        && sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM attendance_term_archives WHERE academic_term_id=$1)",
        )
        .bind(term)
        .fetch_one(&mut **tx)
        .await?
    {
        return Err(AppError::Conflict(
            "รายละเอียดภาคเรียนนี้ถูกล้างแล้ว ดูได้เฉพาะสรุป".into(),
        ));
    }
    Ok(())
}
pub async fn validate_date(pool: &PgPool, term: Uuid, date: NaiveDate) -> Result<(), AppError> {
    let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id WHERE t.id=$1 AND $2 BETWEEN t.start_date AND COALESCE(t.closed_on,t.planned_end_date,y.end_date))").bind(term).bind(date).fetch_one(pool).await?;
    if valid {
        Ok(())
    } else {
        Err(invalid("วันที่อยู่นอกภาคเรียน"))
    }
}
pub async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    entity: Uuid,
    action: &str,
    reason: &str,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO audit_logs(user_id,action,entity_type,entity_id,description) VALUES($1,$2,'attendance',$3,$4)").bind(actor).bind(action).bind(entity).bind(reason).execute(&mut **tx).await?;
    Ok(())
}
