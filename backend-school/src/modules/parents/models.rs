use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::NaiveDate;

#[derive(Debug, Serialize, Deserialize)]
pub struct ParentProfile {
    // User fields
    pub id: Uuid,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub title: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub national_id: Option<String>, // Encrypted
    
    // Children
    pub children: Vec<ChildDto>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct ParentDbRow {
    pub id: Uuid,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub title: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub national_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct ChildDto {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub student_id: Option<String>,
    pub grade_level: Option<String>,
    pub class_room: Option<String>,
    pub profile_image_url: Option<String>,
    pub relationship: String,
}
