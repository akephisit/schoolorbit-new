use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct SchoolSettingsRow {
    pub logo_file_id: Option<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SchoolSettingsResponse {
    #[schema(required = true)]
    pub logo_file_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSchoolSettingsRequest {
    pub logo_file_id: Option<Uuid>,
}

#[derive(Debug, Clone, Default, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicStudentCounts {
    pub total: i64,
    pub male: i64,
    pub female: i64,
    pub other_or_unspecified: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicHomeroomStatistics {
    pub name: String,
    pub students: PublicStudentCounts,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicGradeStatistics {
    pub level_type: String,
    pub year: i32,
    pub students: PublicStudentCounts,
    pub unassigned_students: PublicStudentCounts,
    pub homerooms: Vec<PublicHomeroomStatistics>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicAcademicYear {
    pub year: i32,
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicSchoolStatistics {
    #[schema(required = true)]
    pub academic_year: Option<PublicAcademicYear>,
    pub students: PublicStudentCounts,
    pub unassigned_students: PublicStudentCounts,
    pub total_teachers: i64,
    pub total_staff: i64,
    pub total_homerooms: i64,
    pub grades: Vec<PublicGradeStatistics>,
    pub as_of: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicOrganizationMember {
    pub name: String,
    pub position_code: String,
    #[schema(required = true)]
    pub position_title: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicOrganizationUnit {
    pub id: Uuid,
    #[schema(required = true)]
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub unit_type: String,
    pub members: Vec<PublicOrganizationMember>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicSchoolOrganization {
    pub units: Vec<PublicOrganizationUnit>,
}
