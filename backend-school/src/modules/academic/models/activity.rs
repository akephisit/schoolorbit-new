use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};

// ==========================================
// Activity Group Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct ActivityGroup {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub activity_type: String,
    pub semester_id: Uuid,
    pub instructor_id: Option<Uuid>,
    pub registration_type: String,
    pub max_capacity: Option<i32>,
    pub registration_open: bool,
    pub allowed_grade_level_ids: Option<serde_json::Value>,
    pub created_by: Option<Uuid>,
    pub day_of_week: Option<String>,
    pub period_ids: Option<serde_json::Value>,
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
    pub semester_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateActivityGroupRequest {
    pub name: String,
    pub description: Option<String>,
    pub activity_type: String,
    pub semester_id: Uuid,
    pub instructor_id: Option<Uuid>,
    pub registration_type: Option<String>, // default: "assigned"
    pub max_capacity: Option<i32>,
    pub registration_open: Option<bool>,   // default: false
    pub allowed_grade_level_ids: Option<Vec<Uuid>>,
    pub day_of_week: Option<String>,
    pub period_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateActivityGroupRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub activity_type: Option<String>,
    pub instructor_id: Option<Uuid>,
    pub registration_type: Option<String>,
    pub max_capacity: Option<i32>,
    pub registration_open: Option<bool>,
    pub allowed_grade_level_ids: Option<Vec<Uuid>>,
    pub is_active: Option<bool>,
    pub day_of_week: Option<String>,
    pub period_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct ActivityGroupFilter {
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
