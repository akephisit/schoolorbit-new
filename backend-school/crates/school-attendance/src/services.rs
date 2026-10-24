use school_database::DbSession;
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
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;
pub async fn lock_term(
    tx: &mut Transaction<'_, Postgres>,
    term: Uuid,
    write: bool,
    exclusive: bool,
) -> Result<(), AppError> {
    lock_module(tx).await?;
    let year: Uuid = DbSession::new(&mut **tx)
        .scalar(
            "attendance.src.services.lock_term",
            "SELECT academic_year_id FROM academic_terms WHERE id=$1",
        )
        .bind(term)
        .fetch_optional()
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
    DbSession::new(&mut **tx)
        .query("attendance.src.services.lock_term", sql)
        .bind(term.to_string())
        .execute()
        .await?;
    if write
        && DbSession::new(&mut **tx)
            .scalar::<bool>(
                "attendance.src.services.lock_term",
                "SELECT EXISTS(SELECT 1 FROM attendance_term_archives WHERE academic_term_id=$1)",
            )
            .bind(term)
            .fetch_one()
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
) -> Result<school_tenancy::ObservedTransaction, AppError> {
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "attendance.services.transaction").await?;
    DbSession::new(&mut *tx)
        .query(
            "attendance.src.services.begin_report_snapshot",
            "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY",
        )
        .execute()
        .await?;
    lock_module(&mut tx).await?;
    school_academic_core::services::lifecycle_guard::lock_transition_shared(&mut tx).await?;
    let exists: bool = DbSession::new(&mut *tx).scalar("attendance.src.services.begin_report_snapshot",
        "SELECT EXISTS(SELECT 1 FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id WHERE t.id=$1)",
    )
    .bind(term)
    .fetch_one()
    .await?;
    if !exists {
        return Err(AppError::NotFound("ไม่พบภาคเรียน".into()));
    }
    DbSession::new(&mut *tx)
        .query(
            "attendance.src.services.begin_report_snapshot",
            "SELECT pg_advisory_xact_lock_shared(hashtextextended($1,1701))",
        )
        .bind(term.to_string())
        .execute()
        .await?;
    Ok(tx)
}

pub async fn validate_date(pool: &PgPool, term: Uuid, date: NaiveDate) -> Result<(), AppError> {
    validate_date_on_connection(
        &mut *school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.services.read")
            .await?,
        term,
        date,
    )
    .await
}
pub(crate) async fn validate_date_on_connection(
    connection: &mut sqlx::PgConnection,
    term: Uuid,
    date: NaiveDate,
) -> Result<(), AppError> {
    let valid:bool=DbSession::new(&mut *connection).scalar("attendance.src.services.validate_date_on_connection", "SELECT EXISTS(SELECT 1 FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id WHERE t.id=$1 AND $2 BETWEEN t.start_date AND COALESCE(t.closed_on,t.planned_end_date,y.end_date))").bind(term).bind(date).fetch_one().await?;
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
    DbSession::new(&mut **tx).query("attendance.src.services.audit", "INSERT INTO audit_logs(user_id,action,entity_type,entity_id,description) VALUES($1,$2,'attendance',$3,$4)").bind(actor).bind(action).bind(entity).bind(reason).execute().await?;
    Ok(())
}

pub async fn lock_module(tx: &mut Transaction<'_, Postgres>) -> Result<(), AppError> {
    DbSession::new(&mut **tx)
        .query(
            "attendance.src.services.lock_module",
            "SELECT pg_advisory_xact_lock_shared(17010000)",
        )
        .execute()
        .await?;
    Ok(())
}

/// Serialize cross-domain reset decisions with every Attendance mutation.
pub async fn lock_module_exclusive(tx: &mut Transaction<'_, Postgres>) -> Result<(), AppError> {
    DbSession::new(&mut **tx)
        .query(
            "attendance.src.services.lock_module_exclusive",
            "SELECT pg_advisory_xact_lock(17010000)",
        )
        .execute()
        .await?;
    Ok(())
}
