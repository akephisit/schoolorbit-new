pub mod faces;
pub mod finalization;
pub mod notifications;
pub mod reports;
pub mod reset;
pub mod score_changes;
pub mod sessions;
pub mod settings;
pub mod specials;
use crate::rules::invalid;
use chrono::NaiveDate;
use school_errors::AppError;
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;
pub async fn lock_term(
    tx: &mut Transaction<'_, Postgres>,
    term: Uuid,
    write: bool,
    exclusive: bool,
) -> Result<(), AppError> {
    lock_module(tx).await?;
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
/// A report reads one transaction-start snapshot, including when it waits behind
/// a purge/reset. Shared guards protect that read from lifecycle/module teardown
/// while permitting ordinary saves and other reports. No row locks or lazy writes
/// are allowed in this read-only transaction.
pub async fn begin_report_snapshot(
    pool: &PgPool,
    term: Uuid,
) -> Result<Transaction<'_, Postgres>, AppError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    lock_module(&mut tx).await?;
    school_academic_core::services::lifecycle_guard::lock_transition_shared(&mut tx).await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id WHERE t.id=$1)",
    )
    .bind(term)
    .fetch_one(&mut *tx)
    .await?;
    if !exists {
        return Err(AppError::NotFound("ไม่พบภาคเรียน".into()));
    }
    sqlx::query("SELECT pg_advisory_xact_lock_shared(hashtextextended($1,1701))")
        .bind(term.to_string())
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

pub async fn validate_date(pool: &PgPool, term: Uuid, date: NaiveDate) -> Result<(), AppError> {
    validate_date_on_connection(&mut *pool.acquire().await?, term, date).await
}
pub async fn validate_date_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
) -> Result<(), AppError> {
    let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id WHERE t.id=$1 AND $2 BETWEEN t.start_date AND COALESCE(t.closed_on,t.planned_end_date,y.end_date))").bind(term).bind(date).fetch_one(&mut *connection).await?;
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

pub async fn lock_module(tx: &mut Transaction<'_, Postgres>) -> Result<(), AppError> {
    sqlx::query("SELECT pg_advisory_xact_lock_shared(17010000)")
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Serialize cross-domain reset decisions with every Attendance mutation.
pub async fn lock_module_exclusive(tx: &mut Transaction<'_, Postgres>) -> Result<(), AppError> {
    sqlx::query("SELECT pg_advisory_xact_lock(17010000)")
        .execute(&mut **tx)
        .await?;
    Ok(())
}
