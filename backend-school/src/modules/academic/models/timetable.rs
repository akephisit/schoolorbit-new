use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersonalTimetableQuery {
    pub academic_term_id: Uuid,
    pub date: NaiveDate,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableTemplate {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableTemplateEntry {
    pub id: Uuid,
    pub template_id: Uuid,
    pub day_of_week: String,
    pub bell_period_order_index: i32,
    pub entry_type: String,
    pub title: Option<String>,
    pub resource_kind: String,
    pub stable_resource_id: Option<Uuid>,
    pub learning_group_code: Option<String>,
    pub target_selector: TimetableTemplateTargetSelector,
    pub instructor_ids: Vec<Uuid>,
    pub room_id: Option<Uuid>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableTemplateTargetSelector {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grade_level_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub study_program_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_number: Option<String>,
    #[serde(default)]
    pub instructor_only: bool,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateTemplateRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateTemplateRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FromCurrentRequest {
    pub timetable_version_id: Uuid,
    pub academic_term_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub entry_types: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyTemplateRequest {
    pub timetable_version_id: Uuid,
    pub academic_term_id: Uuid,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClearTimetableRequest {
    pub timetable_version_id: Uuid,
    pub academic_term_id: Uuid,
    pub entry_types: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TemplateWithEntries {
    pub template: TimetableTemplate,
    pub entries: Vec<TimetableTemplateEntry>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TemplateApplyResult {
    pub applied: usize,
    pub entry_ids: Vec<Uuid>,
}
