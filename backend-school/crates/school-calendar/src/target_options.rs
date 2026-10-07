use chrono::NaiveDate;
use school_errors::AppError;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in=Query)]
pub struct CalendarTargetOptionsQuery {
    pub date: NaiveDate,
}
#[derive(Debug, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CalendarGradeOption {
    pub id: Uuid,
    pub name: String,
}
#[derive(Debug, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CalendarHomeroomOption {
    pub id: Uuid,
    pub name: String,
    pub grade_level_id: Uuid,
}
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CalendarTargetOptions {
    pub grade_levels: Vec<CalendarGradeOption>,
    pub homerooms: Vec<CalendarHomeroomOption>,
}

pub async fn list_target_options(
    pool: &PgPool,
    date: NaiveDate,
) -> Result<CalendarTargetOptions, AppError> {
    let grade_levels=sqlx::query_as("SELECT id, CASE level_type WHEN 'kindergarten' THEN 'อ.' WHEN 'primary' THEN 'ป.' WHEN 'secondary' THEN 'ม.' ELSE level_type END || year::text AS name FROM grade_levels
        WHERE is_active ORDER BY level_type,year,id")
        .fetch_all(pool).await?;
    let homerooms=sqlx::query_as("SELECT room.id,room.name || ' (' || year.name || ')' AS name,room.grade_level_id FROM homerooms room JOIN academic_years year ON year.id=room.academic_year_id
        WHERE room.is_active AND year.start_date<=$1 AND year.end_date>=$1 ORDER BY year.start_date,room.name,room.id")
        .bind(date).fetch_all(pool).await?;
    Ok(CalendarTargetOptions {
        grade_levels,
        homerooms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_date_needs_no_header_context() {
        let query: CalendarTargetOptionsQuery =
            serde_json::from_str("{\"date\":\"2030-01-15\"}").unwrap();
        assert_eq!(query.date, NaiveDate::from_ymd_opt(2030, 1, 15).unwrap());
    }
}
