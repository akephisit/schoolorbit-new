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
    pub group_icon: Option<String>,
}

/// Grouped Menu Response
#[derive(Debug, Serialize)]
pub struct MenuGroupResponse {
    pub code: String,
    pub name: String,
    pub icon: Option<String>,
    pub items: Vec<MenuItemResponse>,
}

#[derive(Debug, Serialize)]
pub struct MenuItemResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub path: String,
    pub icon: Option<String>,
}

/// User Menu API Response
#[derive(Debug, Serialize)]
pub struct UserMenuResponse {
    pub success: bool,
    pub groups: Vec<MenuGroupResponse>,
}
