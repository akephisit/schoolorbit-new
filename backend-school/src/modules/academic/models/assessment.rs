use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{types::Json, FromRow};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::modules::academic::delivery::models::CourseGradingPolicy;

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssessmentPlanListQuery {
    pub academic_term_id: Uuid,
    pub subject_id: Option<Uuid>,
    pub instructor_id: Option<Uuid>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveAssessmentPlanRequest {
    /// Omit only when the offering does not have a persisted plan yet.
    pub row_version: Option<i64>,
    pub categories: Vec<SaveAssessmentCategoryRequest>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveAssessmentCategoryRequest {
    pub id: Option<Uuid>,
    pub code: Option<String>,
    pub name: String,
    #[schema(value_type = String, pattern = r"^(0|[1-9]\d*)(\.\d{1,2})?$")]
    pub max_score: String,
    pub exam_mode: String,
    pub exam_duration_minutes: Option<i32>,
    pub display_order: i32,
    pub items: Vec<SaveAssessmentItemRequest>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveAssessmentItemRequest {
    pub id: Option<Uuid>,
    pub name: String,
    #[schema(value_type = String, pattern = r"^(0|[1-9]\d*)(\.\d{1,2})?$")]
    pub max_score: String,
    pub display_order: i32,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentPlanSummary {
    pub plan_id: Option<Uuid>,
    pub offering_id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub subject_id: Uuid,
    pub subject_version_display_label: String,
    pub offering_code: String,
    pub offering_name: String,
    pub status: String,
    pub row_version: Option<i64>,
    pub learning_group_ids: Vec<Uuid>,
    pub learning_group_count: i64,
    pub category_count: i64,
    pub item_count: i64,
    pub total_score: String,
    pub expected_total_score: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentPlanDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    pub offering_id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub subject_id: Uuid,
    pub subject_version_display_label: String,
    pub offering_code: String,
    pub offering_name: String,
    pub grading_policy: CourseGradingPolicy,
    pub expected_total_score: String,
    pub status: String,
    pub row_version: Option<i64>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub locked_at: Option<DateTime<Utc>>,
    pub learning_group_ids: Vec<Uuid>,
    pub categories: Vec<AssessmentCategory>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentCategory {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    pub code: Option<String>,
    pub name: String,
    pub max_score: String,
    pub exam_mode: String,
    pub exam_duration_minutes: Option<i32>,
    pub display_order: i32,
    pub item_total_score: String,
    pub allocation_status: String,
    pub items: Vec<AssessmentItem>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentItem {
    pub id: Uuid,
    pub category_id: Uuid,
    pub name: String,
    pub max_score: String,
    pub display_order: i32,
    pub is_active: bool,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct AssessmentOfferingScopeRow {
    pub offering_id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_term_status: String,
    pub subject_version_id: Uuid,
    pub subject_id: Uuid,
    pub subject_version_display_label: String,
    pub offering_code: String,
    pub offering_name: String,
    pub grading_policy: Json<CourseGradingPolicy>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct AssessmentPlanRow {
    pub id: Uuid,
    pub learning_offering_id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub status: String,
    pub row_version: i64,
    pub submitted_at: Option<DateTime<Utc>>,
    pub locked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct AssessmentCategoryRow {
    pub id: Uuid,
    pub code: Option<String>,
    pub name: String,
    pub max_score: BigDecimal,
    pub exam_mode: String,
    pub exam_duration_minutes: Option<i32>,
    pub display_order: i32,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct AssessmentItemRow {
    pub id: Uuid,
    pub category_id: Uuid,
    pub name: String,
    pub max_score: BigDecimal,
    pub display_order: i32,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentSettingsResponse {
    pub teacher_access_enabled: bool,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAssessmentSettingsRequest {
    pub teacher_access_enabled: bool,
}
