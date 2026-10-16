use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Deserializer, Serialize};
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

/// Administrative registration assertion, independent of login and annual enrollment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum StudentRegistrationStatus {
    Unrecorded,
    Pending,
    Studying,
    TransferredOut,
    Withdrawn,
    Graduated,
}

#[derive(Debug, Serialize, FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct StudentListItem {
    pub id: Uuid,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub username: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub student_id: Option<String>,
    pub registration_status: StudentRegistrationStatus,
    pub account_status: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub enrollment_date: Option<NaiveDate>,
}

/// Staff projection deliberately names account status separately from registration.
/// Annual placement fields are null unless the caller supplies an academic year.
#[derive(Debug, Serialize, FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct StudentRegisterInfo {
    pub id: Uuid,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub username: Option<String>,
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
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub student_id: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub blood_type: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub allergies: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub medical_conditions: Option<String>,
    pub account_status: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub enrollment_date: Option<NaiveDate>,
    pub registration_status: StudentRegistrationStatus,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub registration_status_updated_at: Option<DateTime<Utc>>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub registration_status_updated_by: Option<Uuid>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub grade_level: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub homeroom: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub student_number: Option<i32>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct StudentRegisterProfile {
    #[serde(flatten)]
    pub info: StudentRegisterInfo,
    pub parents: Vec<ParentDto>,
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
#[serde(deny_unknown_fields)]
pub struct CreateStudentRequest {
    pub national_id: Option<String>,
    pub username: Option<String>, // Optional, will be generated if not provided
    pub email: Option<String>,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub title: Option<String>,
    pub student_id: String,
    pub registration_status: Option<StudentRegistrationStatus>,
    pub enrollment_date: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub date_of_birth: Option<String>,
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

fn deserialize_optional_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(Debug, Default, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(deny_unknown_fields)]
pub struct UpdateStudentRequest {
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub email: Option<Option<String>>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub title: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub phone: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub address: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub date_of_birth: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub gender: Option<Option<String>>,
    pub registration_status: Option<StudentRegistrationStatus>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub enrollment_date: Option<Option<String>>,
}

#[derive(Debug, Default, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::IntoParams))]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Query))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListStudentsQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub search: Option<String>,
    pub registration_status: Option<StudentRegistrationStatus>,
    pub account_status: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::IntoParams))]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Query))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudentRegisterQuery {
    pub academic_year_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::IntoParams))]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Query))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudentAcademicYearQuery {
    pub academic_year_id: Uuid,
}
