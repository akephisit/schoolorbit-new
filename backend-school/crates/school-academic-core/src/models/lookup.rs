use serde::{Deserialize, Serialize};
#[cfg(feature = "api-docs")]
use utoipa::ToSchema;
use uuid::Uuid;

use super::AcademicYearStatus;

/// Grade level lookup item
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct GradeLevelLookupItem {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub short_name: Option<String>,
    pub level_type: String,
    pub level_order: i32,
}

/// Academic year lookup item
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct AcademicYearLookupItem {
    pub id: Uuid,
    pub name: String,
    pub year: i32,
    pub status: AcademicYearStatus,
}

/// Homeroom lookup item for a caller-selected academic year.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct HomeroomLookupItem {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "api-docs", schema(value_type = String))]
    pub grade_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "api-docs", schema(value_type = Uuid))]
    pub grade_level_id: Option<Uuid>,
}
