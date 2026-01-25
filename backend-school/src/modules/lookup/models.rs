use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Generic lookup item - minimal data for dropdowns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupItem {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub grade_level_ids: Option<Vec<Uuid>>,
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
    pub short_name: Option<String>,
    pub level_order: i32,
}

/// Classroom lookup item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassroomLookupItem {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grade_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grade_level_id: Option<Uuid>,
}

/// Academic year lookup item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcademicYearLookupItem {
    pub id: Uuid,
    pub name: String,
    pub year: i32, // Numeric year for easy filtering/selection
    pub is_current: bool,
}

/// Student lookup item with student_id and class_room for enrollment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentLookupItem {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
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
    /// Filter by specific Academic Year ID (for grade_levels, classrooms)
    pub academic_year_id: Option<Uuid>,
    /// Filter by current active Academic Year (default: false unless specified)
    pub current_year: Option<bool>,
    /// Filter by level type (kindergarten, primary, secondary)
    pub level_type: Option<String>,
    pub subject_type: Option<String>,
}
