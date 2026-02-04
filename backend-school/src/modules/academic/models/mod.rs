use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::NaiveDate;

// ==========================================
// Curriculum Models
// ==========================================
pub mod curriculum;
pub mod course_planning;
pub mod timetable;

// ==========================================
// Academic Year Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct AcademicYear {
    pub id: Uuid,
    pub year: i32,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub is_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAcademicYearRequest {
    pub year: i32,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAcademicYearRequest {
    pub year: Option<i32>,
    pub name: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub is_active: Option<bool>,
}

// ==========================================
// Semester Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Semester {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub term: String,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub is_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSemesterRequest {
    pub academic_year_id: Uuid,
    pub term: String,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSemesterRequest {
    pub term: Option<String>,
    pub name: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub is_active: Option<bool>,
}

// ==========================================
// Grade Level Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct GradeLevel {
    pub id: Uuid,
    pub level_type: String,  // "kindergarten", "primary", "secondary"
    pub year: i32,           // 1, 2, 3...
    pub next_grade_level_id: Option<Uuid>,
    pub is_active: bool,
}

impl GradeLevel {
    /// Get the code like "K1", "P1", "M1"
    pub fn code(&self) -> String {
        let prefix = match self.level_type.as_str() {
            "kindergarten" => "K",
            "primary" => "P",
            "secondary" => "M",
            _ => "X",
        };
        format!("{}{}", prefix, self.year)
    }

    /// Get the short name like "อ.1", "ป.1", "ม.1"
    pub fn short_name(&self) -> String {
        let prefix = match self.level_type.as_str() {
            "kindergarten" => "อ.",
            "primary" => "ป.",
            "secondary" => "ม.",
            _ => "?.",
        };
        format!("{}{}", prefix, self.year)
    }

    /// Get the full name like "อนุบาลศึกษาปีที่ 1", "ประถมศึกษาปีที่ 1", "มัธยมศึกษาปีที่ 1"
    pub fn full_name(&self) -> String {
        let prefix = match self.level_type.as_str() {
            "kindergarten" => "อนุบาลศึกษาปีที่",
            "primary" => "ประถมศึกษาปีที่",
            "secondary" => "มัธยมศึกษาปีที่",
            _ => "ระดับชั้นปีที่",
        };
        format!("{} {}", prefix, self.year)
    }
}

/// Serializable version with computed fields for API responses
#[derive(Debug, Serialize, Deserialize)]
pub struct GradeLevelResponse {
    pub id: Uuid,
    pub level_type: String,
    pub year: i32,
    pub code: String,
    pub name: String,
    pub short_name: String,
    pub next_grade_level_id: Option<Uuid>,
    pub is_active: bool,
}

impl From<GradeLevel> for GradeLevelResponse {
    fn from(level: GradeLevel) -> Self {
        GradeLevelResponse {
            id: level.id,
            code: level.code(),
            name: level.full_name(),
            short_name: level.short_name(),
            level_type: level.level_type,
            year: level.year,
            next_grade_level_id: level.next_grade_level_id,
            is_active: level.is_active,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateGradeLevelRequest {
    pub level_type: String,  // "kindergarten", "primary", "secondary"
    pub year: i32,           // 1, 2, 3...
    pub next_grade_level_id: Option<Uuid>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGradeLevelRequest {
    pub level_type: Option<String>,
    pub year: Option<i32>,
    pub next_grade_level_id: Option<Uuid>,
    pub is_active: Option<bool>,
}

// ==========================================
// Classroom Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Classroom {
    pub id: Uuid,
    pub code: String,
    pub name: String, // Full display name e.g. "ม.1/2"
    pub academic_year_id: Uuid,
    pub grade_level_id: Uuid,
    pub room_number: Option<String>, // Defines the variant e.g. "1", "2", "EP"
    pub advisor_id: Option<Uuid>,
    pub co_advisor_id: Option<Uuid>,
    pub is_active: bool,
    // Joined fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grade_level_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub academic_year_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advisor_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub student_count: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateClassroomRequest {
    pub academic_year_id: Uuid,
    pub grade_level_id: Uuid,
    pub room_number: String,
    pub advisor_id: Option<Uuid>,
    pub co_advisor_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateClassroomRequest {
    pub room_number: Option<String>,
    pub advisor_id: Option<Uuid>,
    pub co_advisor_id: Option<Uuid>,
    pub is_active: Option<bool>,
}

// ==========================================
// Enrollment Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct StudentEnrollment {
    pub id: Uuid,
    pub student_id: Uuid,
    pub class_room_id: Uuid,
    pub enrollment_date: NaiveDate,
    pub status: String,
    pub class_number: Option<i32>, // Added class number
    // Joined fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub student_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub student_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EnrollStudentRequest {
    pub student_ids: Vec<Uuid>,
    pub class_room_id: Uuid,
    pub enrollment_date: Option<NaiveDate>,
}

// ==========================================
// Year-Level Configuration Models
// ==========================================

#[derive(Debug, Deserialize)]
pub struct UpdateYearLevelsRequest {
    pub grade_level_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct YearLevelMapping {
    pub academic_year_id: Uuid,
    pub grade_level_id: Uuid,
}

