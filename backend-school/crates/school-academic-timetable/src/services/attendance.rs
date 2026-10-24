use super::{
    daily_teaching::day_code_from_date, timetable_block_queries, timetable_version_service,
};
use crate::models::timetable_block::TimetableBlock;
use chrono::NaiveDate;
use school_errors::AppError;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;
pub struct AttendanceTimetableSpan {
    pub effective_from: NaiveDate,
    pub effective_until: NaiveDate,
    pub blocks: Vec<TimetableBlock>,
}
/// Resolve published versions once, then hydrate all blocks with the canonical batch owner.
pub async fn published_spans(
    connection: &mut PgConnection,
    term: Uuid,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<AttendanceTimetableSpan>, AppError> {
    let versions:Vec<(Uuid,NaiveDate,NaiveDate)>=sqlx::query_as("WITH selected AS (SELECT DISTINCT ON(effective_from) id,effective_from FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='published' AND effective_from<=$3 ORDER BY effective_from,id),versions AS (SELECT id,effective_from,COALESCE(lead(effective_from) OVER(ORDER BY effective_from)-1,$3) effective_until FROM selected) SELECT id,effective_from,effective_until FROM versions WHERE effective_until>=$2 ORDER BY effective_from,id")
        .bind(term).bind(start).bind(end).fetch_all(&mut *connection).await?;
    let ids:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM academic_timetable_blocks WHERE timetable_version_id=ANY($1) AND is_active ORDER BY timetable_version_id,id LIMIT 20001")
        .bind(versions.iter().map(|v|v.0).collect::<Vec<_>>()).fetch_all(&mut *connection).await?;
    if ids.len() > 20000 {
        return Err(AppError::ValidationError(
            "ตารางเรียนย้อนหลังเกินขนาดที่รองรับ".into(),
        ));
    }
    let blocks = timetable_block_queries::get_blocks_on_connection(connection, &ids).await?;
    let mut grouped = std::collections::HashMap::<Uuid, Vec<TimetableBlock>>::new();
    for block in blocks {
        grouped
            .entry(block.timetable_version_id)
            .or_default()
            .push(block);
    }
    Ok(versions
        .into_iter()
        .map(
            |(id, effective_from, effective_until)| AttendanceTimetableSpan {
                effective_from,
                effective_until,
                blocks: grouped.remove(&id).unwrap_or_default(),
            },
        )
        .collect())
}
pub async fn blocks(
    pool: &PgPool,
    term: Uuid,
    date: NaiveDate,
) -> Result<Vec<TimetableBlock>, AppError> {
    blocks_on_connection(&mut *pool.acquire().await?, term, date).await
}
pub async fn blocks_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
) -> Result<Vec<TimetableBlock>, AppError> {
    let version =
        timetable_version_service::resolve_for_date_on_connection(connection, term, date).await?;
    let ids:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM academic_timetable_blocks WHERE timetable_version_id=$1 AND academic_term_id=$2 AND day_of_week=$3 AND is_active ORDER BY bell_schedule_period_id,id LIMIT 2001").bind(version.id).bind(term).bind(day_code_from_date(date)).fetch_all(&mut *connection).await?;
    if ids.len() > 2000 {
        return Err(AppError::ValidationError(
            "ตารางประจำวันเกินขนาดที่รองรับ".into(),
        ));
    }
    timetable_block_queries::get_blocks_on_connection(connection, &ids).await
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
