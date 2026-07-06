#![allow(dead_code)]

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExamRound {
    pub id: Uuid,
    pub academic_semester_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateExamRoundRequest {
    pub academic_semester_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExamRoundRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExamDay {
    pub id: Uuid,
    pub exam_round_id: Uuid,
    pub exam_date: NaiveDate,
    pub label: Option<String>,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub sort_order: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertExamDayRequest {
    pub exam_date: NaiveDate,
    pub label: Option<String>,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub sort_order: i32,
    pub grade_level_ids: Vec<Uuid>,
    pub blocked_windows: Vec<BlockedWindowInput>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct BlockedWindow {
    pub id: Option<Uuid>,
    pub label: String,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedWindowInput {
    pub label: String,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertDayRoomAssignmentRequest {
    pub classroom_id: Uuid,
    pub room_id: Uuid,
    pub capacity_override: Option<i32>,
    #[serde(default)]
    pub invigilator_staff_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExamInvigilatorsRequest {
    pub invigilator_staff_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportExamItemsRequest {
    pub grade_level_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportExamItemsResult {
    pub inserted_count: i64,
    pub skipped_existing_count: i64,
    pub skipped_missing_duration_count: i64,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DayRoomAssignmentView {
    pub id: Uuid,
    pub exam_day_id: Uuid,
    pub classroom_id: Uuid,
    pub classroom_name: String,
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
#[serde(rename_all = "camelCase")]
pub struct InvigilatorView {
    pub staff_id: Uuid,
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateSeatsRequest {
    pub regenerate: bool,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SeatAssignmentView {
    pub id: Uuid,
    pub day_room_assignment_id: Uuid,
    pub student_id: Uuid,
    pub student_name: String,
    pub seat_number: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceExamSessionRequest {
    pub exam_schedule_item_id: Uuid,
    pub exam_day_id: Uuid,
    pub starts_at: NaiveTime,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamDayDetail {
    pub id: Uuid,
    pub exam_round_id: Uuid,
    pub exam_date: NaiveDate,
    pub label: Option<String>,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub sort_order: i32,
    pub grade_level_ids: Vec<Uuid>,
    pub blocked_windows: Vec<BlockedWindow>,
    pub room_assignments: Vec<ExamDayRoomAssignmentView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamDayRoomAssignmentView {
    pub id: Uuid,
    pub exam_day_id: Uuid,
    pub classroom_id: Uuid,
    pub room_id: Uuid,
    pub capacity_override: Option<i32>,
    pub classroom_name: Option<String>,
    pub room_name: Option<String>,
    pub room_capacity: Option<i32>,
    pub invigilators: Vec<ExamInvigilatorView>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
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
#[serde(rename_all = "camelCase")]
pub struct ExamScheduleItem {
    pub id: Uuid,
    pub exam_round_id: Uuid,
    pub academic_semester_id: Uuid,
    pub assessment_category_id: Uuid,
    pub assessment_plan_id: Uuid,
    pub classroom_course_id: Uuid,
    pub classroom_id: Uuid,
    pub subject_id: Uuid,
    pub grade_level_id: Uuid,
    pub duration_minutes: i32,
    pub imported_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExamScheduleItemView {
    pub id: Uuid,
    pub exam_round_id: Uuid,
    pub academic_semester_id: Uuid,
    pub assessment_category_id: Uuid,
    pub assessment_plan_id: Uuid,
    pub classroom_course_id: Uuid,
    pub classroom_id: Uuid,
    pub subject_id: Uuid,
    pub grade_level_id: Uuid,
    pub duration_minutes: i32,
    pub imported_at: DateTime<Utc>,
    pub assessment_category_name: Option<String>,
    pub subject_code: Option<String>,
    pub subject_name_th: Option<String>,
    pub subject_name_en: Option<String>,
    pub classroom_name: Option<String>,
    pub grade_level_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
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
#[serde(rename_all = "camelCase")]
pub struct ExamSessionView {
    pub id: Uuid,
    pub exam_schedule_item_id: Uuid,
    pub exam_round_id: Uuid,
    pub exam_day_id: Uuid,
    pub starts_at: NaiveTime,
    pub ends_at: NaiveTime,
    pub academic_semester_id: Uuid,
    pub assessment_category_id: Uuid,
    pub assessment_plan_id: Uuid,
    pub classroom_course_id: Uuid,
    pub classroom_id: Uuid,
    pub subject_id: Uuid,
    pub grade_level_id: Uuid,
    pub duration_minutes: i32,
    pub exam_date: Option<NaiveDate>,
    pub assessment_category_name: Option<String>,
    pub subject_code: Option<String>,
    pub subject_name_th: Option<String>,
    pub subject_name_en: Option<String>,
    pub classroom_name: Option<String>,
    pub room_id: Option<Uuid>,
    pub room_name: Option<String>,
    pub building_name: Option<String>,
    pub invigilators: Vec<ExamInvigilatorView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamScheduleWorkspace {
    pub round: ExamRound,
    pub days: Vec<ExamDayDetail>,
    pub unscheduled_items: Vec<ExamScheduleItemView>,
    pub scheduled_sessions: Vec<ExamSessionView>,
    pub readiness: ExamScheduleReadiness,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamScheduleReadiness {
    pub can_publish: bool,
    pub blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalExamScheduleRound {
    pub round_id: Uuid,
    pub round_name: String,
    pub academic_semester_id: Uuid,
    pub published_at: Option<DateTime<Utc>>,
    pub sessions: Vec<PersonalExamSessionView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalExamSessionView {
    pub exam_date: NaiveDate,
    pub starts_at: NaiveTime,
    pub ends_at: NaiveTime,
    pub subject_name: String,
    pub assessment_category_name: String,
    pub classroom_name: String,
    pub room_name: String,
    pub building_name: Option<String>,
    pub seat_number: Option<String>,
}
