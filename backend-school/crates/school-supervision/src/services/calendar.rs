use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone, Utc};
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{types::Json, PgPool};
use uuid::Uuid;

use super::booking::SCHOOL_TIMEZONE;
use crate::models::{
    SupervisionCalendarEntry, SupervisionCalendarPeriod, SupervisionCalendarQuery,
    SupervisionCalendarStatus,
};

#[derive(sqlx::FromRow)]
struct CalendarRow {
    id: Uuid,
    cycle_id: Uuid,
    cycle_title: String,
    observed_display_name: Option<String>,
    requested: bool,
    evaluator_display_names: Vec<String>,
    periods: Json<Vec<SupervisionCalendarPeriod>>,
}

fn calendar_bounds(
    query: &SupervisionCalendarQuery,
) -> Result<(DateTime<Utc>, DateTime<Utc>), AppError> {
    let invalid_range = || AppError::BadRequest("ช่วงวันที่ปฏิทินนิเทศไม่ถูกต้อง (สูงสุด 42 วัน)".into());
    if !(0..42).contains(&(query.to_date - query.from_date).num_days()) {
        return Err(invalid_range());
    }
    let next_date = query.to_date.succ_opt().ok_or_else(invalid_range)?;
    let at_midnight = |date: NaiveDate| {
        SCHOOL_TIMEZONE
            .from_local_datetime(&date.and_time(NaiveTime::MIN))
            .single()
            .map(|midnight| midnight.with_timezone(&Utc))
            .ok_or_else(invalid_range)
    };
    Ok((at_midnight(query.from_date)?, at_midnight(next_date)?))
}

pub async fn list_calendar(
    pool: &PgPool,
    query: SupervisionCalendarQuery,
) -> Result<Vec<SupervisionCalendarEntry>, AppError> {
    let (start, end) = calendar_bounds(&query)?;
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "supervision.calendar.list")
            .await?;
    let rows = DbSession::new(&mut connection)
        .query_as::<CalendarRow>(
            "supervision.calendar.list",
            r#"
            WITH staff_names AS NOT MATERIALIZED (
                SELECT id, display_name
                FROM user_display_names
                WHERE user_type = 'staff' AND concat(title, first_name, last_name) <> ''
            ), matching_periods AS (
                SELECT p.observation_id, min(p.observed_at) AS first_observed_at,
                       jsonb_agg(jsonb_build_object(
                           'id', p.id, 'position', p.position,
                           'observedAt', p.observed_at, 'endsAt', p.ends_at,
                           'periodLabel', COALESCE(
                               NULLIF(btrim(p.lesson_snapshot->>'periodLabel'), ''),
                               NULLIF(btrim(p.manual_period_label), '')
                           )
                       ) ORDER BY p.position, p.id) AS periods
                FROM supervision_observation_periods p
                WHERE p.academic_year_id = $1
                  AND ($2::uuid IS NULL OR p.academic_term_id = $2)
                  AND p.observed_at >= $3 AND p.observed_at < $4
                GROUP BY p.observation_id
            )
            SELECT o.id, o.cycle_id, c.title AS cycle_title,
                   observed.display_name AS observed_display_name,
                   o.status = 'requested' AS requested,
                   ARRAY(
                       SELECT evaluator.display_name
                       FROM supervision_evaluators e
                       JOIN staff_names evaluator ON evaluator.id = e.evaluator_user_id
                       WHERE e.observation_id = o.id
                       ORDER BY e.is_required DESC, e.created_at, e.id
                   ) AS evaluator_display_names,
                   p.periods
            FROM matching_periods p
            JOIN supervision_observations o ON o.id = p.observation_id
            JOIN supervision_cycles c ON c.id = o.cycle_id
            LEFT JOIN staff_names observed ON observed.id = o.observed_user_id
            WHERE o.status IN ('requested', 'planned', 'in_progress', 'published', 'acknowledged', 'completed')
            ORDER BY p.first_observed_at, o.id
            "#,
        )
        .bind(query.academic_year_id)
        .bind(query.academic_term_id)
        .bind(start)
        .bind(end)
        .fetch_all()
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| SupervisionCalendarEntry {
            id: row.id,
            cycle_id: row.cycle_id,
            cycle_title: row.cycle_title,
            observed_display_name: row.observed_display_name,
            status: if row.requested {
                SupervisionCalendarStatus::Requested
            } else {
                SupervisionCalendarStatus::Confirmed
            },
            evaluator_display_names: row.evaluator_display_names,
            periods: row.periods.0,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_use_bangkok_midnights_and_include_at_most_42_dates() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 28).unwrap();
        let mut query = SupervisionCalendarQuery {
            academic_year_id: Uuid::nil(),
            academic_term_id: None,
            from_date: date,
            to_date: date,
        };
        let (start, end) = calendar_bounds(&query).unwrap();
        assert_eq!(start, Utc.with_ymd_and_hms(2026, 11, 27, 17, 0, 0).unwrap());
        assert_eq!(end, Utc.with_ymd_and_hms(2026, 11, 28, 17, 0, 0).unwrap());

        query.to_date = date + chrono::Days::new(41);
        assert!(calendar_bounds(&query).is_ok());
        query.to_date = date + chrono::Days::new(42);
        assert!(matches!(
            calendar_bounds(&query),
            Err(AppError::BadRequest(_))
        ));
        query.to_date = date.pred_opt().unwrap();
        assert!(matches!(
            calendar_bounds(&query),
            Err(AppError::BadRequest(_))
        ));
    }
}
