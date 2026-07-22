use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ParentProfile {
    // User fields
    pub id: Uuid,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    #[schema(required = true)]
    pub title: Option<String>,
    #[schema(required = true)]
    pub phone: Option<String>,
    #[schema(required = true)]
    pub email: Option<String>,
    #[schema(required = true)]
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

#[derive(Debug, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ChildDto {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    #[schema(required = true)]
    pub student_id: Option<String>,
    #[schema(required = true)]
    pub grade_level: Option<String>,
    #[schema(required = true)]
    pub class_room: Option<String>,
    #[schema(required = true)]
    pub profile_image_url: Option<String>,
    pub relationship: String,
}
