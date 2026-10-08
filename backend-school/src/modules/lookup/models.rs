use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

pub use school_academic_core::models::HomeroomLookupItem;

/// Generic lookup item - minimal data for dropdowns
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LookupItem {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Vec<Uuid>)]
    pub grade_level_ids: Option<Vec<Uuid>>,
}

/// Staff lookup item with title
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StaffLookupItem {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub title: Option<String>,
}

/// Role lookup item
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RoleLookupItem {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub user_type: String,
}

pub use school_staff::models::OrganizationUnitLookupItem;

/// Student lookup item with the placement in the caller-selected academic year.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StudentLookupItem {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub student_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub homeroom: Option<String>,
}

/// Query parameters for lookup endpoints
#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct LookupQuery {
    /// Filter for active items only (default: true)
    pub active_only: Option<bool>,
    /// Search term
    pub search: Option<String>,
    /// Maximum items to return (default: 100)
    pub limit: Option<i32>,
    /// Filter to organization units where the current user is a member
    pub member_only: Option<bool>,
}

/// Query parameters for lookup data whose meaning changes by academic year.
#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct AcademicLookupQuery {
    /// Academic year selected by the caller. The server never infers the active year.
    pub academic_year_id: Uuid,
    /// Filter for active items only (default: true)
    pub active_only: Option<bool>,
    /// Search term
    pub search: Option<String>,
    /// Maximum items to return (default: 100)
    pub limit: Option<i32>,
    /// Filter by level type (kindergarten, primary, secondary)
    pub level_type: Option<String>,
    /// Filter by course or activity subject type
    pub subject_type: Option<String>,
}
