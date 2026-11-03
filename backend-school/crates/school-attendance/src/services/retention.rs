//! Attendance-owned retention manifest and the canonical whole-term archive operation.
#[cfg(test)]
#[path = "retention_tests.rs"]
mod tests;
use crate::models::{AttendancePurgeImpact, PurgeAttendanceTerm};
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub async fn evidence_manifest(
    connection: &mut PgConnection,
    term: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    Ok(DbSession::new(connection)
        .scalar("attendance.retention.evidence_manifest", "SELECT DISTINCT e.evidence_file_id FROM attendance_scan_events e JOIN attendance_sessions s ON s.id=e.session_id WHERE s.academic_term_id=$1 ORDER BY e.evidence_file_id")
        .bind(term).fetch_all().await?)
}

pub async fn require_reopen_allowed(
    connection: &mut PgConnection,
    year: Uuid,
    term: Option<Uuid>,
) -> Result<(), AppError> {
    let archived: bool = DbSession::new(connection)
        .scalar("attendance.retention.reopen_guard", "SELECT EXISTS(SELECT 1 FROM attendance_term_archives a JOIN academic_terms t ON t.id=a.academic_term_id WHERE t.academic_year_id=$1 AND ($2::uuid IS NULL OR t.id=$2))")
        .bind(year).bind(term).fetch_one().await?;
    if archived {
        return Err(AppError::Conflict(
            "รายละเอียดเช็กชื่อถูกเก็บเป็นสรุปแล้ว จึงเปิดช่วงนี้กลับเพื่อแก้รายละเอียดไม่ได้".into(),
        ));
    }
    Ok(())
}

/// Caller may persist its manifest and source-boundary acceptance in this same
/// transaction. The existing archive commits all summary, evidence and detail work atomically.
pub async fn archive_in_transaction(
    pool: &PgPool,
    mut transaction: school_tenancy::ObservedTransaction,
    actor: Uuid,
    term: Uuid,
) -> Result<AttendancePurgeImpact, AppError> {
    super::lock_term(&mut transaction, term, false, true).await?;
    let impact = super::reports::impact_on_connection(&mut transaction, term).await?;
    if impact.archived {
        transaction.commit().await?;
        return Ok(impact);
    }
    let evidence = evidence_manifest(&mut transaction, term).await?;
    school_file_platform::retention::require_retention_allowed(&mut transaction, &evidence).await?;
    let expected = crate::application::projection::expected(&mut transaction, term, None).await?;
    super::reports::purge_in_transaction(
        pool,
        transaction,
        actor,
        term,
        PurgeAttendanceTerm {
            expected_records: impact.records,
            reason: "เก็บยอดสรุปตามนโยบายอายุรายละเอียดของโรงเรียน".into(),
        },
        &expected,
    )
    .await
}
