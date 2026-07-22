use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use chrono::{DateTime, Utc};

// ==========================================
// Activity Slot Models (ช่องกิจกรรม)
// ==========================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ActivityRegistrationType {
    #[serde(rename = "self")]
    SelfRegistration,
    Assigned,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ActivityMemberResult {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ActivityGroupInstructorRole {
    Primary,
    Assistant,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ActivitySlot {
    pub id: Uuid,
    pub activity_catalog_id: Uuid,
    pub semester_id: Uuid,
    #[schema(value_type = ActivityRegistrationType)]
    pub registration_type: String,
    pub teacher_reg_open: bool,
    pub student_reg_open: bool,
    pub student_reg_start: Option<DateTime<Utc>>,
    pub student_reg_end: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // Joined fields from activity_catalog (live link — version snapshot via activity_catalog_id)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub periods_per_week: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduling_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_grade_level_ids: Option<Vec<Uuid>>,

    // Other joins
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semester_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_count: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_members: Option<i64>,

    /// UUIDs of classrooms participating in this slot (from activity_slot_classrooms junction).
    /// Used on the activities page to show only real participants (instead of grade-matched set).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classroom_ids: Option<Vec<Uuid>>,
}

/// Semester-specific fields only. Template fields (name/type/periods/mode/grade)
/// come from activity_catalog and are edited there — not here.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateActivitySlotRequest {
    #[schema(value_type = Option<ActivityRegistrationType>)]
    pub registration_type: Option<String>,
    pub teacher_reg_open: Option<bool>,
    pub student_reg_open: Option<bool>,
    pub student_reg_start: Option<String>,
    pub student_reg_end: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ActivitySlotFilter {
    pub semester_id: Option<Uuid>,
    pub activity_type: Option<String>,
    pub teacher_reg_open: Option<bool>,
    pub student_reg_open: Option<bool>,
}

// ==========================================
// Activity Group Models (กิจกรรมจริง ภายใต้ slot)
// ==========================================

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ActivityGroup {
    pub id: Uuid,
    pub slot_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub instructor_id: Option<Uuid>,
    pub max_capacity: Option<i32>,
    pub registration_open: bool,
    /// ห้องที่ group นี้รับ (override slot). NULL = รับทุกห้องที่ slot รับ
    pub allowed_classroom_ids: Option<Vec<Uuid>>,
    pub created_by: Option<Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // Joined fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructor_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_count: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub semester_name: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateActivityGroupRequest {
    pub slot_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub instructor_id: Option<Uuid>,
    pub max_capacity: Option<i32>,
    pub allowed_classroom_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateActivityGroupRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub instructor_id: Option<Uuid>,
    pub max_capacity: Option<i32>,
    pub registration_open: Option<bool>,
    pub is_active: Option<bool>,
    pub allowed_classroom_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ActivityGroupFilter {
    pub slot_id: Option<Uuid>,
    pub semester_id: Option<Uuid>,
    pub activity_type: Option<String>,
    pub instructor_id: Option<Uuid>,
    pub registration_open: Option<bool>,
    pub search: Option<String>,
}

// ==========================================
// Activity Group Member Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ActivityGroupMember {
    pub id: Uuid,
    pub activity_group_id: Uuid,
    /// FK → users(id) (เปลี่ยนจาก student_info(id) ใน M114)
    pub student_id: Uuid,
    #[schema(value_type = Option<ActivityMemberResult>)]
    pub result: Option<String>,
    pub enrolled_by: Option<Uuid>,
    pub enrolled_at: DateTime<Utc>,

    // Joined fields
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub student_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub student_code: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub classroom_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub grade_level_name: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddMembersRequest {
    pub student_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMemberResultRequest {
    #[schema(value_type = ActivityMemberResult)]
    pub result: String, // "pass" | "fail"
}

// ==========================================
// Classroom Assignments (ครูต่อห้อง — independent slots)
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow, ToSchema)]
pub struct SlotClassroomAssignment {
    pub id: Uuid,
    pub slot_id: Uuid,
    pub classroom_id: Uuid,
    pub instructor_id: Uuid,
    pub created_at: DateTime<Utc>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub classroom_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub instructor_name: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpsertSlotClassroomAssignmentRequest {
    pub classroom_id: Uuid,
    pub instructor_id: Uuid,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchUpsertSlotClassroomAssignmentsRequest {
    pub assignments: Vec<UpsertSlotClassroomAssignmentRequest>,
}
