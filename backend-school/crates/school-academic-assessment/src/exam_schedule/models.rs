use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
#[cfg(feature = "api-docs")]
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamRound {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub exam_kind: String,
    pub row_version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateExamRoundRequest {
    pub academic_term_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub exam_kind: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateExamRoundRequest {
    pub row_version: i64,
    pub name: Option<String>,
    pub description: Option<String>,
    pub exam_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamDay {
    pub id: Uuid,
    pub exam_round_id: Uuid,
    pub exam_date: NaiveDate,
    pub label: Option<String>,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpsertExamDayRequest {
    pub exam_date: NaiveDate,
    pub label: Option<String>,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub grade_level_ids: Vec<Uuid>,
    pub blocked_windows: Vec<BlockedWindowInput>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BlockedWindow {
    pub id: Option<Uuid>,
    pub label: String,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BlockedWindowInput {
    pub label: String,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpsertDayRoomAssignmentRequest {
    pub homeroom_id: Uuid,
    pub room_id: Uuid,
    pub capacity_override: Option<i32>,
    #[serde(default)]
    pub invigilator_staff_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateExamInvigilatorsRequest {
    pub invigilator_staff_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DayRoomAssignmentView {
    pub id: Uuid,
    pub exam_day_id: Uuid,
    pub homeroom_id: Uuid,
    pub homeroom_name: String,
    pub room_id: Uuid,
    pub room_name: String,
    pub building_name: Option<String>,
    pub room_capacity: Option<i32>,
    pub capacity_override: Option<i32>,
    #[sqlx(default)]
    pub invigilators: Vec<InvigilatorView>,
    pub seats_generated: bool,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct InvigilatorView {
    pub staff_id: Uuid,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamInvigilatorAssignmentSummary {
    pub assignment_id: Uuid,
    pub exam_day_id: Uuid,
    pub homeroom_id: Uuid,
    pub homeroom_name: String,
    pub room_id: Uuid,
    pub room_name: String,
    pub session_minutes: i32,
    pub invigilators: Vec<InvigilatorView>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamInvigilatorDayWorkload {
    pub exam_day_id: Uuid,
    pub minutes: i32,
    pub assignment_count: i32,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamInvigilatorStaffWorkload {
    pub staff_id: Uuid,
    pub staff_name: String,
    pub total_minutes: i32,
    pub assigned_day_count: i32,
    pub assignment_count: i32,
    pub days: Vec<ExamInvigilatorDayWorkload>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamInvigilatorWorkspace {
    pub round_id: Uuid,
    pub assignments: Vec<ExamInvigilatorAssignmentSummary>,
    pub staff_workloads: Vec<ExamInvigilatorStaffWorkload>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamInvigilatorStaffOption {
    pub staff_id: Uuid,
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerateSeatsRequest {
    pub regenerate: bool,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SeatAssignmentView {
    pub id: Uuid,
    pub day_room_assignment_id: Uuid,
    pub student_id: Uuid,
    pub student_name: String,
    pub seat_number: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlaceExamSessionRequest {
    pub exam_schedule_item_id: Uuid,
    pub exam_day_id: Uuid,
    pub starts_at: NaiveTime,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamDayDetail {
    pub id: Uuid,
    pub exam_round_id: Uuid,
    pub exam_date: NaiveDate,
    pub label: Option<String>,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub grade_level_ids: Vec<Uuid>,
    pub blocked_windows: Vec<BlockedWindow>,
    pub room_assignments: Vec<ExamDayRoomAssignmentView>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamDayRoomAssignmentView {
    pub id: Uuid,
    pub exam_day_id: Uuid,
    pub homeroom_id: Uuid,
    pub room_id: Uuid,
    pub capacity_override: Option<i32>,
    pub homeroom_name: Option<String>,
    pub room_name: Option<String>,
    pub room_capacity: Option<i32>,
    pub invigilators: Vec<ExamInvigilatorView>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamInvigilatorView {
    pub id: Uuid,
    pub exam_day_id: Uuid,
    pub day_room_assignment_id: Uuid,
    pub staff_id: Uuid,
    pub staff_name: Option<String>,
    pub role_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamScheduleItem {
    pub id: Uuid,
    pub exam_round_id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub assessment_phase_id: Uuid,
    pub course_assessment_plan_id: Uuid,
    pub learning_offering_id: Uuid,
    pub learning_group_id: Uuid,
    pub homeroom_id: Uuid,
    pub subject_id: Uuid,
    pub grade_level_id: Uuid,
    pub duration_minutes: i32,
    pub imported_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamScheduleItemView {
    pub id: Uuid,
    pub exam_round_id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub assessment_phase_id: Uuid,
    pub course_assessment_plan_id: Uuid,
    pub learning_offering_id: Uuid,
    pub learning_group_id: Uuid,
    pub homeroom_id: Uuid,
    pub subject_id: Uuid,
    pub grade_level_id: Uuid,
    pub duration_minutes: i32,
    pub imported_at: DateTime<Utc>,
    pub assessment_phase_name: Option<String>,
    pub subject_code: Option<String>,
    pub subject_name_th: Option<String>,
    pub subject_name_en: Option<String>,
    pub subject_version_display_label: Option<String>,
    pub subject_group_id: Option<Uuid>,
    pub subject_group_name: Option<String>,
    pub subject_group_display_order: Option<i32>,
    pub subject_type: Option<String>,
    pub homeroom_name: Option<String>,
    pub grade_level_name: Option<String>,
    pub grade_level_type: Option<String>,
    pub grade_level_year: Option<i32>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamSession {
    pub id: Uuid,
    pub exam_schedule_item_id: Uuid,
    pub exam_round_id: Uuid,
    pub exam_day_id: Uuid,
    pub starts_at: NaiveTime,
    pub ends_at: NaiveTime,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamSessionView {
    pub id: Uuid,
    pub exam_schedule_item_id: Uuid,
    pub exam_round_id: Uuid,
    pub exam_day_id: Uuid,
    pub starts_at: NaiveTime,
    pub ends_at: NaiveTime,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub assessment_phase_id: Uuid,
    pub course_assessment_plan_id: Uuid,
    pub learning_offering_id: Uuid,
    pub learning_group_id: Uuid,
    pub homeroom_id: Uuid,
    pub subject_id: Uuid,
    pub grade_level_id: Uuid,
    pub duration_minutes: i32,
    pub imported_at: DateTime<Utc>,
    pub exam_date: Option<NaiveDate>,
    pub assessment_phase_name: Option<String>,
    pub subject_code: Option<String>,
    pub subject_name_th: Option<String>,
    pub subject_name_en: Option<String>,
    pub subject_version_display_label: Option<String>,
    pub subject_group_id: Option<Uuid>,
    pub subject_group_name: Option<String>,
    pub subject_group_display_order: Option<i32>,
    pub subject_type: Option<String>,
    pub homeroom_name: Option<String>,
    pub grade_level_name: Option<String>,
    pub grade_level_type: Option<String>,
    pub grade_level_year: Option<i32>,
    pub room_id: Option<Uuid>,
    pub room_name: Option<String>,
    pub building_name: Option<String>,
    pub invigilators: Vec<ExamInvigilatorView>,
}

/// Current assessment sources for the printed teacher paper receipt register.
#[derive(Debug, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamPaperReceiptItem {
    pub assessment_phase_id: Uuid,
    pub learning_group_id: Uuid,
    pub homeroom_id: Uuid,
    pub subject_code: String,
    pub subject_name_th: Option<String>,
    pub subject_name_en: Option<String>,
    pub subject_group_id: Option<Uuid>,
    pub subject_group_name: Option<String>,
    pub subject_group_display_order: Option<i32>,
    pub grade_level_type: String,
    pub grade_level_year: i32,
    pub homeroom_name: String,
    pub exam_arrangement: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamScheduleWorkspace {
    pub round: ExamRound,
    pub days: Vec<ExamDayDetail>,
    pub unscheduled_items: Vec<ExamScheduleItemView>,
    pub scheduled_sessions: Vec<ExamSessionView>,
    pub paper_receipt_items: Vec<ExamPaperReceiptItem>,
    pub readiness: ExamScheduleReadiness,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamScheduleReadiness {
    pub complete: bool,
    pub findings: Vec<ExamScheduleReadinessFinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ExamScheduleReadinessCode {
    MissingExamDay,
    MissingExamItems,
    UnscheduledExamItems,
    MissingRoomAssignments,
    InvalidExamSessions,
    MissingSeatAssignments,
    InvigilatorConflicts,
    InvalidCurrentSources,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ExamScheduleReadinessFinding {
    pub code: ExamScheduleReadinessCode,
    pub count: i64,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PersonalExamScheduleRound {
    pub round_id: Uuid,
    pub round_name: String,
    pub academic_term_id: Uuid,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub sessions: Vec<PersonalExamSessionView>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PersonalExamSessionView {
    pub exam_date: NaiveDate,
    pub starts_at: NaiveTime,
    pub ends_at: NaiveTime,
    pub subject_name: String,
    pub assessment_phase_name: String,
    pub homeroom_name: String,
    pub room_name: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub building_name: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub seat_number: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct StaffExamScheduleRound {
    pub round_id: Uuid,
    pub round_name: String,
    pub academic_term_id: Uuid,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub days: Vec<StaffExamDay>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct StaffExamDay {
    pub exam_day_id: Uuid,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub label: Option<String>,
    pub exam_date: NaiveDate,
    pub sessions: Vec<StaffExamSession>,
    pub room_assignments: Vec<StaffExamRoomAssignment>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct StaffExamSession {
    pub session_id: Uuid,
    pub starts_at: NaiveTime,
    pub ends_at: NaiveTime,
    pub duration_minutes: i32,
    pub subject_id: Uuid,
    pub learning_offering_id: Uuid,
    pub learning_group_id: Uuid,
    pub subject_code: String,
    pub subject_name: String,
    pub assessment_phase_name: String,
    pub grade_level_id: Uuid,
    pub grade_level_name: String,
    pub grade_level_type: String,
    pub grade_level_year: i32,
    pub homeroom_id: Uuid,
    pub homeroom_name: String,
    pub day_room_assignment_id: Uuid,
    pub room_id: Uuid,
    pub room_name: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub building_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct StaffExamRoomAssignment {
    pub assignment_id: Uuid,
    pub homeroom_id: Uuid,
    pub homeroom_name: String,
    pub room_id: Uuid,
    pub room_name: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub building_name: Option<String>,
    pub session_minutes: i32,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub earliest_starts_at: Option<NaiveTime>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub latest_ends_at: Option<NaiveTime>,
    pub invigilators: Vec<StaffExamInvigilator>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct StaffExamInvigilator {
    pub staff_id: Uuid,
    pub display_name: String,
}
