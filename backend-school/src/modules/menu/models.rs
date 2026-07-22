use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

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
    pub workspace_code: String,
    pub items: Vec<MenuItemResponse>,
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
