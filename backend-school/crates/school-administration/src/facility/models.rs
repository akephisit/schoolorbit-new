use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
#[cfg(feature = "api-docs")]
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct Building {
    pub id: Uuid,
    pub name_th: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub name_en: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub code: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct CreateBuildingRequest {
    pub name_th: String,
    pub name_en: Option<String>,
    pub code: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct UpdateBuildingRequest {
    pub name_th: Option<String>,
    pub name_en: Option<String>,
    pub code: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct Room {
    pub id: Uuid,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub building_id: Option<Uuid>,
    pub name_th: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub name_en: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub code: Option<String>,
    pub room_type: String,
    pub capacity: i32,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub floor: Option<i32>,
    pub status: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // Joined
    #[sqlx(default)]
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub building_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct CreateRoomRequest {
    pub building_id: Option<Uuid>,
    pub name_th: String,
    pub name_en: Option<String>,
    pub code: Option<String>,
    pub room_type: String, // GENERAL, LAB, etc.
    pub capacity: Option<i32>,
    pub floor: Option<i32>,
    pub status: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct UpdateRoomRequest {
    pub building_id: Option<Uuid>,
    pub name_th: Option<String>,
    pub name_en: Option<String>,
    pub code: Option<String>,
    pub room_type: Option<String>,
    pub capacity: Option<i32>,
    pub floor: Option<i32>,
    pub status: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RoomFilter {
    pub building_id: Option<Uuid>,
    pub room_type: Option<String>,
    pub search: Option<String>,
}
