//! Current weekly placements for an explicitly selected term. Saved Attendance
//! occurrences remain owned by Attendance and never reconstruct a former timetable.
use super::{daily_teaching::day_code_from_date, timetable_block_queries, timetable_service};
use crate::models::timetable_block::TimetableBlock;
use chrono::NaiveDate;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub async fn current_pattern_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
) -> Result<Vec<TimetableBlock>, AppError> {
    let Some(timetable) = timetable_service::get_for_term_on_connection(connection, term).await?
    else {
        return Ok(Vec::new());
    };
    let ids: Vec<Uuid> = DbSession::new(connection).scalar("academic_timetable.attendance.current_pattern", "SELECT id FROM academic_timetable_blocks WHERE timetable_id=$1 AND academic_term_id=$2 AND is_active ORDER BY day_of_week,bell_schedule_period_id,id LIMIT 20001").bind(timetable.id).bind(term).fetch_all().await?;
    if ids.len() > 20_000 {
        return Err(AppError::ValidationError("ตารางเรียนเกินขนาดที่รองรับ".into()));
    }
    timetable_block_queries::get_blocks_on_connection(connection, &ids).await
}
pub async fn blocks(
    pool: &PgPool,
    term: Uuid,
    date: NaiveDate,
) -> Result<Vec<TimetableBlock>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_timetable.attendance.blocks",
    )
    .await?;
    blocks_on_connection(&mut connection, term, date).await
}
pub async fn blocks_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
) -> Result<Vec<TimetableBlock>, AppError> {
    let day = day_code_from_date(date);
    Ok(current_pattern_on_connection(connection, term)
        .await?
        .into_iter()
        .filter(|block| block.day_of_week == day)
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weekday_matches_existing_timetable_codes() {
        assert_eq!(
            day_code_from_date(NaiveDate::from_ymd_opt(2026, 10, 9).unwrap()),
            "FRI"
        );
    }
}
