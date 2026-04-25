use chrono::NaiveTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ============================================
// Academic Periods (คาบเวลา)
// ============================================

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct AcademicPeriod {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub name: Option<String>,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,

    pub order_index: i32,
    pub applicable_days: Option<String>, // "MON,TUE,WED" or null
    pub is_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePeriodRequest {
    pub academic_year_id: Uuid,
    pub name: Option<String>,
    pub start_time: String,  // Format: "HH:MM"
    pub end_time: String,    // Format: "HH:MM"

    /// ถ้าไม่ส่ง backend จะ auto-set เป็น MAX(order_index) + 1 ของปีการศึกษานั้น
    pub order_index: Option<i32>,
    pub applicable_days: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReorderPeriodItem {
    pub id: Uuid,
    pub order_index: i32,
}

#[derive(Debug, Deserialize)]
pub struct ReorderPeriodsRequest {
    pub academic_year_id: Uuid,
    pub items: Vec<ReorderPeriodItem>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePeriodRequest {
    pub name: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,

    pub order_index: Option<i32>,
    pub applicable_days: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct PeriodQuery {
    pub academic_year_id: Option<Uuid>,

    pub active_only: Option<bool>,
}

// ============================================
// Timetable Entries (ตารางสอน)
// ============================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct TimetableEntry {
    pub id: Uuid,
    pub classroom_course_id: Option<Uuid>,
    pub day_of_week: String,
    pub period_id: Uuid,
    pub room_id: Option<Uuid>,
    pub note: Option<String>,

    pub entry_type: String, // COURSE, BREAK, ACTIVITY, HOMEROOM, ACADEMIC
    pub title: Option<String>,
    pub classroom_id: Option<Uuid>,
    pub academic_semester_id: Uuid,
    pub activity_slot_id: Option<Uuid>,
    /// UUID ของ batch ที่สร้าง entry นี้; NULL = สร้างแยก
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<Uuid>,

    pub is_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    
    // Joined fields (for display)
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_code: Option<String>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_name_th: Option<String>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructor_names: Option<Vec<String>>,
    /// UUID ของครูทุกคนใน cell — parallel กับ instructor_names เรียงตาม role+created_at
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructor_ids: Option<Vec<Uuid>>,

    // Keep for backward-compat UI display (first name). Populated from instructor_names[0].
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructor_name: Option<String>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classroom_name: Option<String>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_code: Option<String>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period_name: Option<String>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity_slot_name: Option<String>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity_type: Option<String>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity_scheduling_mode: Option<String>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<NaiveTime>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<NaiveTime>,

}

#[derive(Debug, Deserialize)]
pub struct CreateTimetableEntryRequest {
    pub classroom_course_id: Option<Uuid>,
    pub day_of_week: String,
    pub period_id: Uuid,
    pub room_id: Option<Uuid>,
    pub note: Option<String>,
    // Activity entry support
    pub activity_slot_id: Option<Uuid>,
    pub entry_type: Option<String>,
    pub title: Option<String>,
    pub classroom_id: Option<Uuid>,
    pub academic_semester_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTimetableEntryRequest {
    pub day_of_week: Option<String>,
    pub period_id: Option<Uuid>,
    pub room_id: Option<Uuid>,
    pub note: Option<String>,
    /// Change what this entry teaches (replace content) — used by drag-from-sidebar-onto-occupied flow
    pub classroom_course_id: Option<Uuid>,
    pub activity_slot_id: Option<Uuid>,
    /// Change which classroom this entry belongs to — used by instructor-view replace ข้ามห้อง
    /// เพื่อให้ entry.classroom_id ตรงกับ classroom_course.classroom_id
    pub classroom_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct SwapTimetableEntriesRequest {
    pub entry_a_id: Uuid,
    pub entry_b_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct ValidateMovesRequest {
    pub entry_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct MoveValidityCell {
    pub day_of_week: String,
    pub period_id: Uuid,
    /// "empty" | "occupied" | "source"
    pub state: String,
    /// Target entry id if occupied (for swap). None if empty/source.
    pub target_entry_id: Option<Uuid>,
    pub valid: bool,
    /// Human-readable reason if !valid, or empty string
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateBatchTimetableEntriesRequest {
    pub classroom_ids: Vec<Uuid>,
    pub days_of_week: Vec<String>,
    pub period_ids: Vec<Uuid>,
    pub academic_semester_id: Uuid,
    pub entry_type: String, // ACTIVITY, BREAK, HOMEROOM, ACADEMIC
    pub title: String,
    pub room_id: Option<Uuid>,
    pub note: Option<String>,
    pub subject_id: Option<Uuid>,
    pub force: Option<bool>,
    pub activity_slot_id: Option<Uuid>,
    /// ครูที่ติดคาบด้วย event นี้ (attach ไปที่ tei)
    /// - ถ้า classroom_ids ว่าง + instructor_ids มี → teacher-only (classroom_id = NULL)
    /// - ถ้า classroom_ids มี + instructor_ids มี → attach ครูเหล่านี้เพิ่มเติมใน tei ของแต่ละ entry
    #[serde(default)]
    pub instructor_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct TimetableQuery {
    pub classroom_id: Option<Uuid>,
    pub student_id: Option<Uuid>,          // ดึงตารางของนักเรียน (classroom + activity)
    pub instructor_id: Option<Uuid>,
    pub room_id: Option<Uuid>,
    pub academic_semester_id: Option<Uuid>,
    pub day_of_week: Option<String>,
    pub entry_type: Option<String>,
    /// รวม ghost cells: entries ที่ instructor อยู่ในทีมของ course แต่ไม่ได้อยู่ใน cell นั้น
    /// ใช้คู่กับ instructor_id เท่านั้น (ไม่มี instructor_id → ignored)
    pub include_team_ghosts: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteBatchTimetableEntriesRequest {
    pub activity_slot_id: Uuid,
    pub day_of_week: String,
    pub academic_semester_id: Uuid,
}

// ============================================
// Conflict Detection Response
// ============================================

#[derive(Debug, Serialize)]
pub struct ConflictInfo {
    pub conflict_type: String, // "INSTRUCTOR_CONFLICT", "ROOM_CONFLICT", "CLASSROOM_CONFLICT"
    pub message: String,
    pub existing_entry: Option<TimetableEntry>,
}

#[derive(Debug, Serialize)]
pub struct TimetableValidationResponse {
    pub is_valid: bool,
    pub conflicts: Vec<ConflictInfo>,
}
