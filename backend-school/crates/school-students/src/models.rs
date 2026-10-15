use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
#[cfg(feature = "api-docs")]
use utoipa::ToSchema;
use uuid::Uuid;

// =========================================
// API Models (from handlers/students.rs)
// =========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct ParentDto {
    pub id: Uuid,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub phone: Option<String>,
    pub relationship: String,
    pub is_primary: bool,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct StudentDbRow {
    // User fields
    pub id: Uuid,
    pub username: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub national_id: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub email: Option<String>,
    pub first_name: String,
    pub last_name: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub title: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub nickname: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub phone: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub date_of_birth: Option<NaiveDate>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub gender: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub address: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub profile_image_file_id: Option<Uuid>,

    // Student info fields
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub student_id: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub student_number: Option<i32>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub blood_type: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub allergies: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub medical_conditions: Option<String>,

    // Additional fields needed for Detail View
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub status: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub grade_level: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub homeroom: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct StudentProfile {
    #[serde(flatten)]
    pub info: StudentDbRow,
    pub parents: Vec<ParentDto>,
}

#[derive(Debug, Serialize, FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct StudentListItem {
    pub id: Uuid,
    pub username: String,
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub student_id: Option<String>,
    pub grade_level: Option<String>,
    pub homeroom: Option<String>,
    pub status: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct StudentListResponse {
    pub items: Vec<StudentListItem>,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct CreateStudentResponse {
    pub id: Uuid,
    pub username: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct UpdateOwnProfileRequest {
    pub phone: Option<String>,
    pub address: Option<String>,
    pub nickname: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
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

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
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
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct UpdateStudentRequest {
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub student_number: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::IntoParams))]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Query))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListStudentsQuery {
    pub academic_year_id: Uuid,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::IntoParams))]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Query))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudentAcademicYearQuery {
    pub academic_year_id: Uuid,
}
