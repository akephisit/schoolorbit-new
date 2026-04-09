use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};

// ==========================================
// Activity Slot Models (ช่องกิจกรรม)
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct ActivitySlot {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub activity_type: String,
    pub semester_id: Uuid,
    pub allowed_grade_level_ids: Option<serde_json::Value>,
    pub registration_type: String,
    pub teacher_reg_open: bool,
    pub student_reg_open: bool,
    pub student_reg_start: Option<DateTime<Utc>>,
    pub student_reg_end: Option<DateTime<Utc>>,
    pub periods_per_week: i32,
    pub scheduling_mode: String,
    pub created_by: Option<Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // Joined fields
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub semester_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub group_count: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub total_members: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateActivitySlotRequest {
    pub name: String,
    pub description: Option<String>,
    pub activity_type: String,
    pub semester_id: Uuid,
    pub allowed_grade_level_ids: Option<Vec<Uuid>>,
    pub registration_type: Option<String>,
    pub periods_per_week: Option<i32>,
    pub scheduling_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateActivitySlotRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub activity_type: Option<String>,
    pub allowed_grade_level_ids: Option<Vec<Uuid>>,
    pub registration_type: Option<String>,
    pub teacher_reg_open: Option<bool>,
    pub student_reg_open: Option<bool>,
    pub student_reg_start: Option<String>,
    pub student_reg_end: Option<String>,
    pub is_active: Option<bool>,
    pub periods_per_week: Option<i32>,
    pub scheduling_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ActivitySlotFilter {
    pub semester_id: Option<Uuid>,
    pub activity_type: Option<String>,
    pub teacher_reg_open: Option<bool>,
    pub student_reg_open: Option<bool>,
}

// ==========================================
// Activity Group Models (กิจกรรมจริง ภายใต้ slot)
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct ActivityGroup {
    pub id: Uuid,
    pub slot_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub instructor_id: Option<Uuid>,
    pub max_capacity: Option<i32>,
    pub registration_open: bool,
    pub allowed_grade_level_ids: Option<serde_json::Value>,
    pub created_by: Option<Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // Joined fields
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub instructor_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub member_count: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub slot_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub activity_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub semester_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateActivityGroupRequest {
    pub slot_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub instructor_id: Option<Uuid>,
    pub max_capacity: Option<i32>,
    pub allowed_grade_level_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateActivityGroupRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub instructor_id: Option<Uuid>,
    pub max_capacity: Option<i32>,
    pub registration_open: Option<bool>,
    pub is_active: Option<bool>,
    pub allowed_grade_level_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct ActivityGroupMember {
    pub id: Uuid,
    pub activity_group_id: Uuid,
    pub student_id: Uuid,
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

#[derive(Debug, Deserialize)]
pub struct AddMembersRequest {
    pub student_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct SelfEnrollRequest {
    // นักเรียนสมัครเอง — ไม่ต้องส่ง student_id (ดึงจาก token)
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemberResultRequest {
    pub result: String, // "pass" | "fail"
}

// ==========================================
// Classroom Assignments (ครูต่อห้อง — independent slots)
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
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

#[derive(Debug, Deserialize)]
pub struct UpsertSlotClassroomAssignmentRequest {
    pub classroom_id: Uuid,
    pub instructor_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct BatchUpsertSlotClassroomAssignmentsRequest {
    pub assignments: Vec<UpsertSlotClassroomAssignmentRequest>,
}
