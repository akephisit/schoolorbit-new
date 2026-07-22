use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

// =========================================
// API Models (from handlers/students.rs)
// =========================================

#[derive(Debug, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ParentDto {
    pub id: Uuid,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    #[schema(required = true)]
    pub phone: Option<String>,
    pub relationship: String,
    pub is_primary: bool,
}

#[derive(Debug, Serialize, Deserialize, FromRow, ToSchema)]
pub struct StudentDbRow {
    // User fields
    pub id: Uuid,
    pub username: String,
    #[schema(required = true)]
    pub national_id: Option<String>,
    #[schema(required = true)]
    pub email: Option<String>,
    pub first_name: String,
    pub last_name: String,
    #[schema(required = true)]
    pub title: Option<String>,
    #[schema(required = true)]
    pub nickname: Option<String>,
    #[schema(required = true)]
    pub phone: Option<String>,
    #[schema(required = true)]
    pub date_of_birth: Option<NaiveDate>,
    #[schema(required = true)]
    pub gender: Option<String>,
    #[schema(required = true)]
    pub address: Option<String>,
    #[schema(required = true)]
    pub profile_image_url: Option<String>,

    // Student info fields
    #[schema(required = true)]
    pub student_id: Option<String>,
    #[schema(required = true)]
    pub student_number: Option<i32>,
    #[schema(required = true)]
    pub blood_type: Option<String>,
    #[schema(required = true)]
    pub allergies: Option<String>,
    #[schema(required = true)]
    pub medical_conditions: Option<String>,

    // Additional fields needed for Detail View
    #[schema(required = true)]
    pub status: Option<String>,
    #[schema(required = true)]
    pub grade_level: Option<String>,
    #[schema(required = true)]
    pub class_room: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
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

#[derive(Debug, Serialize)]
pub struct StudentListResponse {
    pub items: Vec<StudentListItem>,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateStudentResponse {
    pub id: Uuid,
    pub username: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateOwnProfileRequest {
    pub phone: Option<String>,
    pub address: Option<String>,
    pub nickname: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
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

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateParentRequest {
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub phone: String,
    pub relationship: String,
    pub national_id: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateStudentRequest {
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub student_number: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct ListStudentsQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
}
