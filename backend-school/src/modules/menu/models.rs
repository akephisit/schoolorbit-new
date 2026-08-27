use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

/// Top-level navigation workspace such as academic or personnel administration.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct MenuWorkspace {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    #[schema(required = true)]
    pub name_en: Option<String>,
    #[schema(required = true)]
    pub icon: Option<String>,
    pub display_order: i32,
    pub is_active: bool,
}

/// Menu Group
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct MenuGroup {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    #[schema(required = true)]
    pub name_en: Option<String>,
    #[schema(required = true)]
    pub icon: Option<String>,
    pub workspace_code: String,
    pub display_order: i32,
    pub is_active: bool,
}

/// Menu Item
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct MenuItem {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    #[schema(required = true)]
    pub name_en: Option<String>,
    pub path: String,
    #[schema(required = true)]
    pub icon: Option<String>,
    #[schema(required = true)]
    pub required_permission: Option<String>,
    pub user_type: String, // 'staff', 'student', or 'parent'
    #[schema(required = true)]
    pub group_id: Option<Uuid>,
    #[schema(required = true)]
    pub parent_id: Option<Uuid>,
    pub display_order: i32,
    pub is_active: bool,
}

/// Feature Toggle
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct FeatureToggle {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    #[schema(required = true)]
    pub name_en: Option<String>,
    #[schema(required = true)]
    pub module: Option<String>,
    pub is_enabled: bool,
}

/// Menu Item Response (for user menu API)
#[derive(Debug, Serialize, ToSchema)]
pub struct MenuItemResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub path: String,
    #[schema(required = true)]
    pub icon: Option<String>,
}

/// Menu Group Response (for user menu API)
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MenuGroupResponse {
    pub code: String,
    pub name: String,
    #[schema(required = true)]
    pub icon: Option<String>,
    pub display_order: i32,
    pub workspace_code: String,
    pub workspace_name: String,
    #[schema(required = true)]
    pub workspace_icon: Option<String>,
    pub workspace_order: i32,
    pub items: Vec<MenuItemResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcademicMenuTemplateSection {
    pub code: String,
    pub name: String,
    pub workspace_code: String,
    pub display_order: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcademicMenuTemplateMove {
    pub menu_item_id: Uuid,
    pub menu_item_name: String,
    #[schema(required = true)]
    pub current_group_name: Option<String>,
    pub target_group_code: String,
    pub target_group_name: String,
    pub current_order: i32,
    pub target_order: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcademicMenuTemplatePreview {
    pub revision: String,
    pub recommendations_ready: bool,
    pub sections_to_create: Vec<AcademicMenuTemplateSection>,
    pub moves: Vec<AcademicMenuTemplateMove>,
    pub untouched_custom_item_count: i64,
    pub untouched_non_academic_route_count: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcademicMenuTemplateApplyResult {
    pub revision: String,
    pub created_section_count: u64,
    pub moved_count: u64,
}

// ==================== Route Registration (Build-time) ====================

/// Request payload for route registration from frontend build
#[derive(Debug, Deserialize)]
pub struct RouteRegistration {
    pub routes: Vec<RouteItem>,
    pub environment: Option<String>,
}

/// Single route item from frontend  
#[derive(Debug, Deserialize, Serialize)]
pub struct RouteItem {
    pub path: String,
    pub title: String,
    pub icon: Option<String>,
    pub group: String,             // group code
    pub workspace: Option<String>, // stable sidebar workspace code
    pub order: i32,
    pub permission: Option<String>, // module name
    pub user_type: Option<String>,  // 'staff', 'student', or 'parent' - defaults to 'staff'
}

/// Response for route registration
#[derive(Debug, Serialize)]
pub struct RouteRegistrationResponse {
    pub success: bool,
    pub registered: usize,
    pub message: String,
}
