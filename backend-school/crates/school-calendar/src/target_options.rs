use chrono::NaiveDate;
use school_errors::AppError;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
#[cfg(feature = "api-docs")]
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(IntoParams))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", into_params(parameter_in=Query))]
pub struct CalendarTargetOptionsQuery {
    pub date: NaiveDate,
}
#[derive(Debug, Serialize, FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CalendarGradeOption {
    pub id: Uuid,
    pub name: String,
}
#[derive(Debug, Serialize, FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CalendarHomeroomOption {
    pub id: Uuid,
    pub name: String,
    pub grade_level_id: Uuid,
}
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CalendarTargetOptions {
    pub grade_levels: Vec<CalendarGradeOption>,
    pub homerooms: Vec<CalendarHomeroomOption>,
}

pub async fn list_target_options(
    pool: &PgPool,
    date: NaiveDate,
) -> Result<CalendarTargetOptions, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "calendar.target_options").await?;
    let grade_levels=school_database::DbSession::new(&mut *connection).query_as("calendar.src.target_options.list_target_options", "SELECT id, CASE level_type WHEN 'kindergarten' THEN 'อ.' WHEN 'primary' THEN 'ป.' WHEN 'secondary' THEN 'ม.' ELSE level_type END || year::text AS name FROM grade_levels
        WHERE is_active ORDER BY level_type,year,id")
        .fetch_all().await?;
    let homerooms=school_database::DbSession::new(&mut *connection).query_as("calendar.src.target_options.list_target_options", "SELECT room.id,room.name || ' (' || year.name || ')' AS name,room.grade_level_id FROM homerooms room JOIN academic_years year ON year.id=room.academic_year_id
        WHERE room.is_active AND year.start_date<=$1 AND year.end_date>=$1 ORDER BY year.start_date,room.name,room.id")
        .bind(date).fetch_all().await?;
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
