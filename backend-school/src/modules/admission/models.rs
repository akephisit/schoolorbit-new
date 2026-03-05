use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDate};

// ==========================================
// Admission Period Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct AdmissionPeriod {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub open_date: NaiveDate,
    pub close_date: NaiveDate,
    pub announcement_date: Option<NaiveDate>,
    pub confirmation_deadline: Option<NaiveDate>,
    pub status: String,
    pub capacity_per_class: Option<i32>,
    pub total_capacity: Option<i32>,
    pub waitlist_capacity: Option<i32>,
    pub required_documents: serde_json::Value,
    pub application_fee: Option<f64>,
    pub metadata: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Joined fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub academic_year_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed_count: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAdmissionPeriodRequest {
    pub academic_year_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub open_date: NaiveDate,
    pub close_date: NaiveDate,
    pub announcement_date: Option<NaiveDate>,
    pub confirmation_deadline: Option<NaiveDate>,
    pub target_grade_level_ids: Option<Vec<Uuid>>,
    pub capacity_per_class: Option<i32>,
    pub total_capacity: Option<i32>,
    pub waitlist_capacity: Option<i32>,
    pub required_documents: Option<serde_json::Value>,
    pub application_fee: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAdmissionPeriodRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub open_date: Option<NaiveDate>,
    pub close_date: Option<NaiveDate>,
    pub announcement_date: Option<NaiveDate>,
    pub confirmation_deadline: Option<NaiveDate>,
    pub status: Option<String>,
    pub capacity_per_class: Option<i32>,
    pub total_capacity: Option<i32>,
    pub waitlist_capacity: Option<i32>,
    pub required_documents: Option<serde_json::Value>,
    pub application_fee: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct ListAdmissionPeriodsQuery {
    pub academic_year_id: Option<Uuid>,
    pub status: Option<String>,
}

// ==========================================
// Admission Application Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct AdmissionApplication {
    pub id: Uuid,
    pub admission_period_id: Uuid,
    pub application_number: String,
    pub applicant_first_name: String,
    pub applicant_last_name: String,
    pub applicant_title: Option<String>,
    pub applicant_national_id: Option<String>,
    pub applicant_date_of_birth: Option<NaiveDate>,
    pub applicant_gender: Option<String>,
    pub applicant_nationality: Option<String>,
    pub applicant_religion: Option<String>,
    pub applicant_blood_type: Option<String>,
    pub applicant_phone: Option<String>,
    pub applicant_email: Option<String>,
    pub applicant_address: Option<String>,
    pub applicant_photo_url: Option<String>,
    pub previous_school: Option<String>,
    pub previous_grade: Option<String>,
    pub previous_gpa: Option<f64>,
    pub applying_grade_level_id: Option<Uuid>,
    pub applying_classroom_preference: Option<String>,
    pub guardian_name: Option<String>,
    pub guardian_relationship: Option<String>,
    pub guardian_phone: Option<String>,
    pub guardian_email: Option<String>,
    pub guardian_occupation: Option<String>,
    pub guardian_national_id: Option<String>,
    pub status: String,
    pub staff_notes: Option<String>,
    pub rejection_reason: Option<String>,
    pub interview_score: Option<f64>,
    pub exam_score: Option<f64>,
    pub total_score: Option<f64>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Joined fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grade_level_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewer_name: Option<String>,
    // Computed score from exam_scores table
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed_score: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAdmissionApplicationRequest {
    pub admission_period_id: Uuid,
    pub applicant_first_name: String,
    pub applicant_last_name: String,
    pub applicant_title: Option<String>,
    pub applicant_national_id: Option<String>,
    pub applicant_date_of_birth: Option<NaiveDate>,
    pub applicant_gender: Option<String>,
    pub applicant_nationality: Option<String>,
    pub applicant_religion: Option<String>,
    pub applicant_blood_type: Option<String>,
    pub applicant_phone: Option<String>,
    pub applicant_email: Option<String>,
    pub applicant_address: Option<String>,
    pub previous_school: Option<String>,
    pub previous_grade: Option<String>,
    pub previous_gpa: Option<f64>,
    pub applying_grade_level_id: Option<Uuid>,
    pub applying_classroom_preference: Option<String>,
    pub guardian_name: Option<String>,
    pub guardian_relationship: Option<String>,
    pub guardian_phone: Option<String>,
    pub guardian_email: Option<String>,
    pub guardian_occupation: Option<String>,
    pub guardian_national_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateApplicationStatusRequest {
    pub status: String,
    pub staff_notes: Option<String>,
    pub rejection_reason: Option<String>,
    pub interview_score: Option<f64>,
    pub exam_score: Option<f64>,
    pub total_score: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct ListApplicationsQuery {
    pub admission_period_id: Option<Uuid>,
    pub status: Option<String>,
    pub search: Option<String>,
    /// เรียงตาม: "total_score" | "computed_score" | "name" | "submitted_at"
    pub sort_by: Option<String>,
    /// "asc" | "desc"  (default: desc)
    pub sort_dir: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

// ==========================================
// Admission Interview Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct AdmissionInterview {
    pub id: Uuid,
    pub application_id: Uuid,
    pub interview_type: String,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub location: Option<String>,
    pub interviewer_id: Option<Uuid>,
    pub score: Option<f64>,
    pub max_score: Option<f64>,
    pub notes: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Joined
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interviewer_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applicant_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInterviewRequest {
    pub application_id: Uuid,
    pub interview_type: Option<String>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub location: Option<String>,
    pub interviewer_id: Option<Uuid>,
    pub max_score: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateInterviewRequest {
    pub scheduled_at: Option<DateTime<Utc>>,
    pub location: Option<String>,
    pub interviewer_id: Option<Uuid>,
    pub score: Option<f64>,
    pub notes: Option<String>,
    pub status: Option<String>,
}

// ==========================================
// Exam Subject & Score Models  (NEW)
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct AdmissionExamSubject {
    pub id: Uuid,
    pub admission_period_id: Uuid,
    pub subject_name: String,
    pub subject_code: Option<String>,
    pub max_score: f64,
    pub display_order: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpsertExamSubjectRequest {
    pub subject_name: String,
    pub subject_code: Option<String>,
    pub max_score: Option<f64>,
    pub display_order: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct AdmissionExamScore {
    pub id: Uuid,
    pub application_id: Uuid,
    pub exam_subject_id: Uuid,
    pub score: f64,
    pub recorded_by: Option<Uuid>,
    pub recorded_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Joined
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_score: Option<f64>,
}

/// คะแนน 1 รายการ สำหรับ batch upsert
#[derive(Debug, Deserialize)]
pub struct ScoreEntry {
    pub application_id: Uuid,
    pub exam_subject_id: Uuid,
    pub score: f64,
}

#[derive(Debug, Deserialize)]
pub struct BatchUpsertScoresRequest {
    pub scores: Vec<ScoreEntry>,
    /// ถ้า true → คำนวณ total_score ใหม่ด้วย
    pub recalculate_total: Option<bool>,
    /// subject_ids ที่จะใช้คำนวณ total (None = ทุกวิชา)
    pub total_subject_ids: Option<Vec<Uuid>>,
}

// ==========================================
// Admission Selection Models  (UPDATED)
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct AdmissionSelection {
    pub id: Uuid,
    pub application_id: Uuid,
    pub admission_period_id: Uuid,
    pub selection_type: String,
    pub rank: Option<i32>,
    pub assigned_grade_level_id: Option<Uuid>,
    pub assigned_classroom_id: Option<Uuid>,
    pub study_plan_version_id: Option<Uuid>,
    pub is_confirmed: bool,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub confirmation_deadline: Option<DateTime<Utc>>,
    // Check-in
    pub checkin_status: String,
    pub checked_in_at: Option<DateTime<Utc>>,
    pub checked_in_by: Option<Uuid>,
    pub checkin_notes: Option<String>,
    // Student account
    pub student_user_id: Option<Uuid>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Joined applicant info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applicant_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applicant_national_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applicant_gender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applicant_date_of_birth: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardian_phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardian_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grade_level_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applying_grade_level_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classroom_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classroom_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub study_plan_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub study_plan_version_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_total_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked_in_by_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub student_username: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSelectionRequest {
    pub application_ids: Vec<Uuid>,
    pub selection_type: Option<String>,
    pub confirmation_deadline: Option<DateTime<Utc>>,
    pub study_plan_version_id: Option<Uuid>,
    pub classroom_id: Option<Uuid>,
}

/// อัปเดต classroom/study_plan ให้ selection ทีหลังได้
#[derive(Debug, Deserialize)]
pub struct UpdateSelectionRequest {
    pub rank: Option<i32>,
    pub study_plan_version_id: Option<Uuid>,
    pub assigned_classroom_id: Option<Uuid>,
    pub notes: Option<String>,
}

/// Query params สำหรับ list_selections
#[derive(Debug, Deserialize)]
pub struct ListSelectionsQuery {
    /// CSV ของ subject_id ที่ใช้คำนวณ sort score
    pub sort_subject_ids: Option<String>,
    /// "asc" | "desc"
    pub sort_dir: Option<String>,
    pub checkin_status: Option<String>,
    pub search: Option<String>,
    pub study_plan_version_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct GenerateStudentsRequest {
    pub selection_ids: Vec<Uuid>,
    pub classroom_id: Option<Uuid>,
    pub password_prefix: Option<String>,
}

/// Request ครูยืนยันรายงานตัว → สร้าง account ทันที  (NEW)
#[derive(Debug, Deserialize)]
pub struct CheckinRequest {
    pub notes: Option<String>,
}

/// Bulk update checkin หลายคนพร้อมกัน (เช่น ขาดทั้งหมด)
#[derive(Debug, Deserialize)]
pub struct BulkCheckinRequest {
    pub selection_ids: Vec<Uuid>,
    pub checkin_status: String, // "absent"
    pub notes: Option<String>,
}

// ==========================================
// Audit Log
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct AdmissionAuditLog {
    pub id: Uuid,
    pub application_id: Uuid,
    pub action: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub note: Option<String>,
    pub performed_by: Option<Uuid>,
    pub performed_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performer_name: Option<String>,
}

// ==========================================
// Stats
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct AdmissionStats {
    pub period_id: Uuid,
    pub total: i64,
    pub pending: i64,
    pub reviewing: i64,
    pub accepted: i64,
    pub rejected: i64,
    pub waitlisted: i64,
    pub confirmed: i64,
    pub cancelled: i64,
}

/// สถิติรายงานตัว  (NEW)
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct CheckinStats {
    pub period_id: Uuid,
    pub total_confirmed: i64,
    pub pending_checkin: i64,
    pub checked_in: i64,
    pub absent: i64,
}
