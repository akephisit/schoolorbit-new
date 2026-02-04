use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDate};

// Student Info (database model)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StudentInfo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub student_id: String,
    pub student_number: Option<i32>,
    // parent_id removed, use student_parents table linked by user_id
    pub enrollment_date: Option<NaiveDate>,
    pub expected_graduation_date: Option<NaiveDate>,
    pub blood_type: Option<String>,
    pub allergies: Option<String>,
    pub medical_conditions: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// =========================================
// API Models (from handlers/students.rs)
// =========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct ParentDto {
    pub id: Uuid,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub relationship: String,
    pub is_primary: bool,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct StudentDbRow {
    // User fields
    pub id: Uuid,
    pub username: String,
    pub national_id: Option<String>,
    pub email: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub title: Option<String>,
    pub nickname: Option<String>,
    pub phone: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub gender: Option<String>,
    pub address: Option<String>,
    pub profile_image_url: Option<String>,
    
    // Student info fields
    pub student_id: Option<String>,
    pub student_number: Option<i32>,
    pub blood_type: Option<String>,
    pub allergies: Option<String>,
    pub medical_conditions: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StudentProfile {
    #[serde(flatten)]
    pub info: StudentDbRow,
    pub parents: Vec<ParentDto>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct StudentListItem {
    pub id: Uuid,
    pub username: String,
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub student_id: Option<String>,
    pub grade_level: Option<String>,
    pub class_room: Option<String>,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOwnProfileRequest {
    pub phone: Option<String>,
    pub address: Option<String>,
    pub nickname: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateStudentRequest {
    pub national_id: Option<String>,
    pub username: Option<String>, // Optional, will be generated if not provided
    pub email: Option<String>,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub title: Option<String>,
    pub student_id: String,
    pub student_number: Option<i32>,
    pub date_of_birth: Option<String>, // Changed from NaiveDate to String for flexible parsing
    pub gender: Option<String>,
    pub parents: Option<Vec<CreateParentRequest>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateParentRequest {
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub phone: String,
    pub relationship: String,
    pub national_id: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStudentRequest {
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub grade_level: Option<String>,
    pub class_room: Option<String>,
    pub student_number: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct ListStudentsQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
}
