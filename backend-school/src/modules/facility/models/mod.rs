use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Building {
    pub id: Uuid,
    pub name_th: String,
    pub name_en: Option<String>,
    pub code: Option<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBuildingRequest {
    pub name_th: String,
    pub name_en: Option<String>,
    pub code: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBuildingRequest {
    pub name_th: Option<String>,
    pub name_en: Option<String>,
    pub code: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Room {
    pub id: Uuid,
    pub building_id: Option<Uuid>,
    pub name_th: String,
    pub name_en: Option<String>,
    pub code: Option<String>,
    pub room_type: String,
    pub capacity: i32,
    pub floor: Option<i32>,
    pub status: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    
    // Joined
    #[sqlx(default)]
    pub building_name: Option<String>,
}

#[derive(Debug, Deserialize)]
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
