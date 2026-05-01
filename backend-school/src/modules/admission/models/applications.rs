use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, NaiveDate, Utc};

// ==========================================
// Application Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionApplication {
    pub id: Uuid,
    pub admission_round_id: Uuid,
    pub admission_track_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_number: Option<String>,

    // ข้อมูลส่วนตัว
    pub national_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_of_birth: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    // ที่อยู่
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_line: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_district: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub district: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub province: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<String>,

    // โรงเรียนเดิม
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_school: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_grade: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_gpa: Option<f64>,

    // บิดา
    #[serde(skip_serializing_if = "Option::is_none")]
    pub father_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub father_phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub father_occupation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub father_national_id: Option<String>,

    // มารดา
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mother_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mother_phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mother_occupation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mother_national_id: Option<String>,

    // ผู้ปกครอง
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardian_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardian_phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardian_relation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardian_national_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardian_occupation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardian_income: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardian_is: Option<String>,

    // ข้อมูลส่วนตัวเพิ่มเติม
    #[serde(skip_serializing_if = "Option::is_none")]
    pub religion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ethnicity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nationality: Option<String>,

    // ที่อยู่ตามทะเบียนบ้าน (เพิ่มเติมจาก address_line/sub_district/district/province/postal_code)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_house_no: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_moo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_soi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_road: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_phone: Option<String>,

    // ที่อยู่ปัจจุบัน
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_house_no: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_moo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_soi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_road: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_sub_district: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_district: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_province: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_postal_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_phone: Option<String>,

    // โรงเรียนเดิม เพิ่มเติม
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_study_year: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_school_province: Option<String>,

    // ครอบครัว เพิ่มเติม
    #[serde(skip_serializing_if = "Option::is_none")]
    pub father_income: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mother_income: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_status: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_status_other: Option<String>,

    pub status: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_student_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_assignment_track_id: Option<Uuid>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub enrolled_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enrolled_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_user_id: Option<Uuid>,

    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // Joined fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_track_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub round_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitApplicationRequest {
    pub admission_track_id: Uuid,

    // ข้อมูลผู้สมัคร
    pub national_id: String,
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub gender: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub phone: Option<String>,
    pub email: Option<String>,

    // ที่อยู่
    pub address_line: Option<String>,
    pub sub_district: Option<String>,
    pub district: Option<String>,
    pub province: Option<String>,
    pub postal_code: Option<String>,

    // โรงเรียนเดิม
    pub previous_school: Option<String>,
    pub previous_grade: Option<String>,
    pub previous_gpa: Option<f64>,

    // บิดา
    pub father_name: Option<String>,
    pub father_phone: Option<String>,
    pub father_occupation: Option<String>,
    pub father_national_id: Option<String>,

    // มารดา
    pub mother_name: Option<String>,
    pub mother_phone: Option<String>,
    pub mother_occupation: Option<String>,
    pub mother_national_id: Option<String>,

    // ผู้ปกครอง
    pub guardian_name: Option<String>,
    pub guardian_phone: Option<String>,
    pub guardian_relation: Option<String>,
    pub guardian_national_id: Option<String>,
    pub guardian_occupation: Option<String>,
    pub guardian_income: Option<f64>,
    pub guardian_is: Option<String>,

    // ข้อมูลส่วนตัวเพิ่มเติม
    pub religion: Option<String>,
    pub ethnicity: Option<String>,
    pub nationality: Option<String>,

    // ที่อยู่ตามทะเบียนบ้าน (เพิ่มเติม)
    pub home_house_no: Option<String>,
    pub home_moo: Option<String>,
    pub home_soi: Option<String>,
    pub home_road: Option<String>,
    pub home_phone: Option<String>,

    // ที่อยู่ปัจจุบัน
    pub current_house_no: Option<String>,
    pub current_moo: Option<String>,
    pub current_soi: Option<String>,
    pub current_road: Option<String>,
    pub current_sub_district: Option<String>,
    pub current_district: Option<String>,
    pub current_province: Option<String>,
    pub current_postal_code: Option<String>,
    pub current_phone: Option<String>,

    // โรงเรียนเดิม เพิ่มเติม
    pub previous_study_year: Option<String>,
    pub previous_school_province: Option<String>,

    // ครอบครัว เพิ่มเติม
    pub father_income: Option<f64>,
    pub mother_income: Option<f64>,
    pub parent_status: Option<serde_json::Value>,
    pub parent_status_other: Option<String>,

    // เอกสารประกอบ (temp file ids จาก portal upload)
    pub documents: Option<Vec<DocumentRef>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRef {
    pub temp_file_id: Uuid,
    pub doc_type: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationDocument {
    pub id: Uuid,
    pub application_id: Uuid,
    pub file_id: Uuid,
    pub doc_type: String,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<DateTime<Utc>>,
    // Joined from files
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TempUploadResponse {
    pub temp_file_id: Uuid,
    pub original_filename: String,
    pub file_size: i64,
    pub doc_type: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalDeleteDocumentQuery {
    pub national_id: String,
    pub date_of_birth: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalStatusQuery {
    pub national_id: String,
    pub date_of_birth: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePortalApplicationRequest {
    pub auth_national_id: String,
    pub auth_date_of_birth: String,
    
    #[serde(flatten)]
    pub data: SubmitApplicationRequest,
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectApplicationRequest {
    pub rejection_reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkAbsentRequest {
    pub absent: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationListItem {
    pub id: Uuid,
    pub application_number: Option<String>,
    pub national_id: String,
    pub full_name: String,
    pub track_name: Option<String>,
    pub status: String,
    pub phone: Option<String>,
    pub previous_school: Option<String>,
    pub previous_gpa: Option<f64>,
    pub created_at: DateTime<Utc>,
}

// ==========================================
// Exam Score Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExamScore {
    pub id: Uuid,
    pub application_id: Uuid,
    pub exam_subject_id: Uuid,
    pub score: Option<f64>,
    pub entered_by: Option<Uuid>,
    pub entered_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,

    // Joined fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_score: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateScoreEntry {
    pub exam_subject_id: Uuid,
    pub score: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApplicationScoresRequest {
    pub scores: Vec<UpdateScoreEntry>,
}

/// สำหรับ bulk update: ส่งหลาย application พร้อมกัน
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkScoreEntry {
    pub application_id: Uuid,
    pub scores: Vec<UpdateScoreEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkUpdateScoresRequest {
    pub entries: Vec<BulkScoreEntry>,
}

// ==========================================
// Room Assignment Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RoomAssignment {
    pub id: Uuid,
    pub application_id: Uuid,
    pub class_room_id: Uuid,
    pub rank_in_track: Option<i32>,
    pub rank_in_room: Option<i32>,
    pub total_score: Option<f64>,
    pub full_score: Option<f64>,
    pub assigned_at: Option<DateTime<Utc>>,
    pub assigned_by: Option<Uuid>,
    pub student_confirmed: bool,
    pub student_confirmed_at: Option<DateTime<Utc>>,

    // Joined fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub student_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub national_id: Option<String>,
}

/// ผลการเรียงคะแนน (Preview ก่อน assign)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankingEntry {
    pub application_id: Uuid,
    pub application_number: Option<String>,
    pub national_id: String,
    pub full_name: String,
    pub track_name: Option<String>,
    pub total_score: f64,      // คะแนนรวมจากวิชาที่ใช้เรียง
    pub full_score: f64,       // คะแนนรวมทุกวิชา
    pub rank_in_track: i32,
    pub assigned_room: Option<String>,  // ห้องที่ได้ (ถ้า preview)
    pub scores: Vec<SubjectScore>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubjectScore {
    pub subject_id: Uuid,
    pub subject_name: String,
    pub score: Option<f64>,
    pub max_score: f64,
    pub is_scoring_subject: bool,  // ใช้ในการเรียงคะแนนหรือเปล่า
}

/// Request สำหรับ assign rooms หลังเรียงคะแนน
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignRoomsRequest {
    pub track_id: Uuid,
    pub selection_subject_ids: Option<Vec<Uuid>>,
    /// "sequential" (default) หรือ "round_robin"
    pub room_assignment_method: Option<String>,
}

/// Request สำหรับ assign rooms แบบ global (รวมทุกสาย ไม่แยกสาย)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignRoomsGlobalRequest {
    pub room_assignment_method: Option<String>,
    /// ลำดับห้องที่ต้องการจัดก่อน-หลัง (list ของ room UUID) — ถ้าไม่ส่งจะเรียงตามชื่อห้อง
    pub room_order: Option<Vec<Uuid>>,
}

/// Request สำหรับย้ายสายการเรียน
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeTrackRequest {
    /// None = ย้อนกลับสายที่สมัคร (ลบ override)
    pub track_id: Option<Uuid>,
}

/// Request สำหรับแก้ไขสายการเรียนที่สมัคร (admission_track_id จริง)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAdmissionTrackRequest {
    pub track_id: Uuid,
}

/// Request สำหรับย้ายห้องเรียนทีละคน
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveRoomRequest {
    pub room_id: Uuid,
}

// ==========================================
// Enrollment Form Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct EnrollmentForm {
    pub id: Uuid,
    pub application_id: Uuid,
    pub form_data: serde_json::Value,
    pub pre_submitted_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub completed_by: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitEnrollmentFormRequest {
    pub form_data: serde_json::Value,
}

// ==========================================
// Portal (Applicant) Request Models
// ==========================================

/// Credentials ที่ผู้สมัครส่งมาทุก request (stateless)
/// ใช้ national_id + date_of_birth (format DDMMYYYY) เพื่อยืนยันตัวตน
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalCredentials {
    pub national_id: String,
    pub date_of_birth: String,  // format: DDMMYYYY e.g. "20082543"
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalConfirmRequest {
    pub national_id: String,
    pub date_of_birth: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalFormRequest {
    pub national_id: String,
    pub date_of_birth: String,
    pub form_data: Option<serde_json::Value>,
}

// ==========================================
// Staff Update Application Request
// ==========================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApplicationRequest {
    pub title: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub gender: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub religion: Option<String>,
    pub ethnicity: Option<String>,
    pub nationality: Option<String>,
    // ที่อยู่ตามทะเบียนบ้าน
    pub address_line: Option<String>,
    pub sub_district: Option<String>,
    pub district: Option<String>,
    pub province: Option<String>,
    pub postal_code: Option<String>,
    pub home_house_no: Option<String>,
    pub home_moo: Option<String>,
    pub home_soi: Option<String>,
    pub home_road: Option<String>,
    pub home_phone: Option<String>,
    // ที่อยู่ปัจจุบัน
    pub current_house_no: Option<String>,
    pub current_moo: Option<String>,
    pub current_soi: Option<String>,
    pub current_road: Option<String>,
    pub current_sub_district: Option<String>,
    pub current_district: Option<String>,
    pub current_province: Option<String>,
    pub current_postal_code: Option<String>,
    pub current_phone: Option<String>,
    // โรงเรียนเดิม
    pub previous_school: Option<String>,
    pub previous_grade: Option<String>,
    pub previous_gpa: Option<f64>,
    pub previous_study_year: Option<String>,
    pub previous_school_province: Option<String>,
    // ครอบครัว
    pub father_name: Option<String>,
    pub father_phone: Option<String>,
    pub father_occupation: Option<String>,
    pub father_national_id: Option<String>,
    pub father_income: Option<f64>,
    pub mother_name: Option<String>,
    pub mother_phone: Option<String>,
    pub mother_occupation: Option<String>,
    pub mother_national_id: Option<String>,
    pub mother_income: Option<f64>,
    pub guardian_name: Option<String>,
    pub guardian_phone: Option<String>,
    pub guardian_relation: Option<String>,
    pub guardian_national_id: Option<String>,
    pub guardian_occupation: Option<String>,
    pub guardian_income: Option<f64>,
    pub guardian_is: Option<String>,
    pub parent_status: Option<serde_json::Value>,
    pub parent_status_other: Option<String>,
}

// ==========================================
// Application Filter
// ==========================================

#[derive(Debug, Deserialize)]
pub struct ApplicationFilter {
    pub status: Option<String>,
    pub track_id: Option<Uuid>,
    pub search: Option<String>,
}

// ==========================================
// Complete Enrollment (มอบตัว) Request
// ==========================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteEnrollmentRequest {
    /// รหัสนักเรียน (student_id field ใน student_info)
    pub student_code: Option<String>,
    /// ข้อมูลมอบตัวที่ staff กรอกแทน (เมื่อนักเรียนยังไม่ pre-submit)
    pub form_data: Option<serde_json::Value>,
}

// ==========================================
// Student ID Pre-Assignment
// ==========================================

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct StudentIdRow {
    pub application_id: Uuid,
    pub application_number: Option<String>,
    pub full_name: String,
    pub first_name: String,
    pub last_name: String,
    pub national_id: Option<String>,
    pub assigned_student_id: Option<String>,
    pub room_name: Option<String>,
    pub rank_in_room: Option<i32>,
    pub rank_in_track: Option<i32>,
    pub previous_school: Option<String>,
    pub original_track_name: Option<String>,
    pub assigned_track_name: Option<String>,
    pub exam_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStudentIdItem {
    pub application_id: Uuid,
    pub student_id: Option<String>,
}
