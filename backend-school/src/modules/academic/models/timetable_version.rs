use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum TimetableVersionStatus {
    Draft,
    Published,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimetableVersionDisplayState {
    Current,
    Upcoming,
    Historical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableVersionTarget {
    pub timetable_version_id: Uuid,
    pub learning_offering_id: Uuid,
    pub weekly_period_target: i32,
    #[schema(required = true)]
    pub standard_periods_per_week: Option<i32>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableVersion {
    pub id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub effective_from: NaiveDate,
    #[schema(required = true)]
    pub effective_until: Option<NaiveDate>,
    pub status: TimetableVersionStatus,
    #[schema(required = true)]
    pub display_state: Option<TimetableVersionDisplayState>,
    #[schema(required = true)]
    pub source_version_id: Option<Uuid>,
    #[schema(required = true)]
    pub change_set_id: Option<Uuid>,
    pub bell_schedule_id: Uuid,
    pub row_version: i64,
    #[schema(required = true)]
    pub created_by: Option<Uuid>,
    #[schema(required = true)]
    pub published_by: Option<Uuid>,
    #[schema(required = true)]
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub targets: Vec<TimetableVersionTarget>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableVersionQuery {
    pub academic_term_id: Uuid,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveTimetableVersionQuery {
    pub academic_term_id: Uuid,
    pub date: NaiveDate,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloneTimetableVersionRequest {
    pub effective_from: NaiveDate,
    pub source_row_version: i64,
}
