use serde::{Deserialize, Serialize};
use sqlx::FromRow;
#[cfg(feature = "api-docs")]
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Default, Deserialize)]
pub struct PublicSchoolIconQuery {
    #[serde(default)]
    pub size: school_file_platform::platform_types::PublicSchoolIconSize,
}

#[derive(Debug, FromRow)]
pub struct SchoolSettingsRow {
    pub logo_file_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SchoolSettingsResponse {
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub logo_file_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSchoolSettingsRequest {
    pub logo_file_id: Option<Uuid>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PublicStudentCounts {
    pub total: i64,
    pub male: i64,
    pub female: i64,
    pub other_or_unspecified: i64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PublicHomeroomStatistics {
    pub name: String,
    pub students: PublicStudentCounts,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PublicGradeStatistics {
    pub level_type: String,
    pub year: i32,
    pub students: PublicStudentCounts,
    pub unassigned_students: PublicStudentCounts,
    pub homerooms: Vec<PublicHomeroomStatistics>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PublicAcademicYear {
    pub year: i32,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PublicSchoolStatistics {
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub academic_year: Option<PublicAcademicYear>,
    pub students: PublicStudentCounts,
    pub unassigned_students: PublicStudentCounts,
    pub total_teachers: i64,
    pub total_staff: i64,
    pub total_homerooms: i64,
    pub grades: Vec<PublicGradeStatistics>,
    pub as_of: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PublicOrganizationMember {
    /// Directory-only identity for combining current responsibilities without exposing user IDs.
    pub person_key: String,
    pub name: String,
    pub position_code: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub position_title: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PublicOrganizationUnit {
    pub id: Uuid,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub unit_type: String,
    pub members: Vec<PublicOrganizationMember>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PublicSchoolOrganization {
    pub units: Vec<PublicOrganizationUnit>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum SchoolAppearancePalette {
    #[default]
    Teal,
    Blue,
    Purple,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum SchoolAppearanceFont {
    #[default]
    Kanit,
    Noto,
    Sarabun,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum SchoolAppearanceDensity {
    #[default]
    Comfortable,
    Compact,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchoolAppearance {
    pub palette: SchoolAppearancePalette,
    pub font: SchoolAppearanceFont,
    pub density: SchoolAppearanceDensity,
    #[cfg_attr(
        feature = "api-docs",
        schema(required = true, pattern = "^#[0-9a-fA-F]{6}$")
    )]
    #[serde(deserialize_with = "deserialize_appearance_primary")]
    pub primary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchoolAppearanceResponse {
    pub appearance: SchoolAppearance,
    #[cfg_attr(feature = "api-docs", schema(minimum = 0))]
    pub revision: i64,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateSchoolAppearanceRequest {
    pub appearance: SchoolAppearance,
    #[cfg_attr(feature = "api-docs", schema(minimum = 0))]
    pub expected_revision: i64,
}

fn deserialize_appearance_primary<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)
}
