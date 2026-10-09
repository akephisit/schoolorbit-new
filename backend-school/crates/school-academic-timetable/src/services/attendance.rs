use super::{
    daily_teaching::day_code_from_date, timetable_block_queries, timetable_version_service,
};
use crate::models::timetable_block::TimetableBlock;
use chrono::NaiveDate;
use school_errors::AppError;
use sqlx::PgPool;
use uuid::Uuid;
pub async fn blocks(
    pool: &PgPool,
    term: Uuid,
    date: NaiveDate,
) -> Result<Vec<TimetableBlock>, AppError> {
    let version = timetable_version_service::resolve_for_date(pool, term, date).await?;
    let ids:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM academic_timetable_blocks WHERE timetable_version_id=$1 AND academic_term_id=$2 AND day_of_week=$3 AND is_active ORDER BY bell_schedule_period_id,id LIMIT 2001").bind(version.id).bind(term).bind(day_code_from_date(date)).fetch_all(pool).await?;
    if ids.len() > 2000 {
        return Err(AppError::ValidationError(
            "ตารางประจำวันเกินขนาดที่รองรับ".into(),
        ));
    }
    timetable_block_queries::get_blocks(pool, &ids).await
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
