use chrono::NaiveDate;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::PgConnection;
use uuid::Uuid;

/// Lesson identities and finalized days are accepted unpinned source evidence.
/// Arrival/special sessions have independent sources. Old reset generations do not
/// constrain the current module. The caller serializes publication against term writes.
pub async fn has_consumed_lesson_history_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    effective_from: NaiveDate,
) -> Result<bool, AppError> {
    Ok(DbSession::new(connection).scalar("attendance.services.publication.consumed_lesson_history",r#"
        SELECT EXISTS(SELECT 1 FROM attendance_sessions WHERE academic_term_id=$1 AND kind='lesson' AND date>=$2)
            OR EXISTS(SELECT 1 FROM attendance_finalized_dates day CROSS JOIN attendance_module_state module
                WHERE module.singleton AND day.generation=module.generation AND day.academic_term_id=$1 AND day.date>=$2)
    "#).bind(term).bind(effective_from).fetch_one().await?)
}
