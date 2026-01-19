use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Generic lookup item - minimal data for dropdowns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupItem {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Staff lookup item with title
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffLookupItem {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

/// Role lookup item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleLookupItem {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub user_type: String,
}

/// Department lookup item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepartmentLookupItem {
    pub id: Uuid,
    pub code: String,
    pub name: String,
}

/// Grade level lookup item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradeLevelLookupItem {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub level_order: i32,
}

/// Classroom lookup item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassroomLookupItem {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grade_level: Option<String>,
}

/// Academic year lookup item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcademicYearLookupItem {
    pub id: Uuid,
    pub name: String,
    pub is_current: bool,
}

/// Student lookup item with student_id and class_room for enrollment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentLookupItem {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub student_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_room: Option<String>,
}

/// Lookup response wrapper
#[derive(Debug, Serialize)]
pub struct LookupResponse<T> {
    pub success: bool,
    pub data: Vec<T>,
}

/// Query parameters for lookup endpoints
#[derive(Debug, Deserialize)]
pub struct LookupQuery {
    /// Filter for active items only (default: true)
    pub active_only: Option<bool>,
    /// Search term
    pub search: Option<String>,
    /// Maximum items to return (default: 100)
    pub limit: Option<i32>,
}
