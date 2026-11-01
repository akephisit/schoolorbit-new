use super::{
    daily_teaching::day_code_from_date, timetable_block_queries, timetable_version_service,
};
use crate::models::timetable_block::TimetableBlock;
use chrono::NaiveDate;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;
pub struct AttendanceDatedBlocks {
    pub delivery_version_id: Uuid,
    pub blocks: Vec<TimetableBlock>,
}
pub struct AttendanceTimetableSpan {
    pub delivery_version_id: Uuid,
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
    let versions:Vec<(Uuid,NaiveDate,NaiveDate,Uuid)>=DbSession::new(&mut *connection).query_as("academic_timetable.services.attendance.published_spans", "WITH versions AS (SELECT v.id,v.effective_from,v.delivery_version_id,LEAST(COALESCE((SELECT min(n.effective_from)-1 FROM academic_timetable_versions n WHERE n.academic_term_id=v.academic_term_id AND n.status='published' AND n.publication_no>v.publication_no),$3),$3) effective_until FROM academic_timetable_versions v WHERE v.academic_term_id=$1 AND v.status='published' AND v.effective_from<=$3) SELECT id,GREATEST(effective_from,$2),effective_until,delivery_version_id FROM versions WHERE effective_until>=effective_from AND effective_until>=$2 ORDER BY effective_from,id")
        .bind(term).bind(start).bind(end).fetch_all().await?;
    let ids:Vec<Uuid>=DbSession::new(&mut *connection).scalar("academic_timetable.services.attendance.published_spans", "SELECT id FROM academic_timetable_blocks WHERE timetable_version_id=ANY($1) AND is_active ORDER BY timetable_version_id,id LIMIT 20001")
        .bind(versions.iter().map(|v|v.0).collect::<Vec<_>>()).fetch_all().await?;
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
            |(id, effective_from, effective_until, delivery_version_id)| AttendanceTimetableSpan {
                delivery_version_id,
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
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_timetable.services.attendance.blocks",
    )
    .await?;
    blocks_on_connection(&mut connection, term, date).await
}

pub async fn blocks_on_connection(
    connection: &mut sqlx::PgConnection,
    term: Uuid,
    date: NaiveDate,
) -> Result<Vec<TimetableBlock>, AppError> {
    Ok(source_blocks_on_connection(connection, term, date)
        .await?
        .blocks)
}

/// Daily blocks retain the exact published Delivery source for roster projection.
pub async fn source_blocks_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
) -> Result<AttendanceDatedBlocks, AppError> {
    let version =
        timetable_version_service::resolve_for_date_on_connection(connection, term, date).await?;
    let ids:Vec<Uuid>=DbSession::new(connection).scalar("academic_timetable.services.attendance.blocks_on_connection", "SELECT id FROM academic_timetable_blocks WHERE timetable_version_id=$1 AND academic_term_id=$2 AND day_of_week=$3 AND is_active ORDER BY bell_schedule_period_id,id LIMIT 2001").bind(version.id).bind(term).bind(day_code_from_date(date)).fetch_all().await?;
    if ids.len() > 2000 {
        return Err(AppError::ValidationError(
            "ตารางประจำวันเกินขนาดที่รองรับ".into(),
        ));
    }
    Ok(AttendanceDatedBlocks {
        delivery_version_id: version.delivery_version_id,
        blocks: timetable_block_queries::get_blocks_on_connection(connection, &ids).await?,
    })
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
