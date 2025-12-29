use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Menu Group
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MenuGroup {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub icon: Option<String>,
    pub display_order: i32,
    pub is_active: bool,
}

/// Menu Item
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MenuItem {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub path: String,
    pub icon: Option<String>,
    pub required_permission: Option<String>,
    pub group_id: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub display_order: i32,
    pub is_active: bool,
}

/// Feature Toggle
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FeatureToggle {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub module: Option<String>,
    pub is_enabled: bool,
}

/// Menu Item with Group Info (for API response)
#[derive(Debug, Serialize)]
pub struct MenuItemWithGroup {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub path: String,
    pub icon: Option<String>,
    pub group_code: String,
    pub group_name: String,
}

/// Menu Item Response (for user menu API)
#[derive(Debug, Serialize)]
pub struct MenuItemResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub path: String,
    pub icon: Option<String>,
}

/// Menu Group Response (for user menu API)
#[derive(Debug, Serialize)]
pub struct MenuGroupResponse {
    pub code: String,
    pub name: String,
    pub icon: Option<String>,
    pub items: Vec<MenuItemResponse>,
}

/// User Menu API Response
#[derive(Debug, Serialize)]
pub struct UserMenuResponse {
    pub success: bool,
    pub groups: Vec<MenuGroupResponse>,
}

// ==================== Route Registration (Build-time) ====================

/// Request payload for route registration from frontend build
#[derive(Debug, Deserialize)]
pub struct RouteRegistration {
    pub routes: Vec<RouteItem>,
    pub timestamp: Option<String>,
    pub environment: Option<String>,
}

/// Single route item from frontend  
#[derive(Debug, Deserialize, Serialize)]
pub struct RouteItem {
    pub path: String,
    pub title: String,
    pub icon: Option<String>,
    pub group: String,  // group code
    pub order: i32,
    pub permission: Option<String>,  // module name
}

/// Response for route registration
#[derive(Debug, Serialize)]
pub struct RouteRegistrationResponse {
    pub success: bool,
    pub registered: usize,
    pub message: String,
}
