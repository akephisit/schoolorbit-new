use serde::{Deserialize, Serialize};
use sqlx::FromRow;
#[cfg(feature = "api-docs")]
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct ParentProfile {
    // User fields
    pub id: Uuid,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub title: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub phone: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub email: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
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
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct ChildDto {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub student_id: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub grade_level: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub homeroom: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub profile_image_file_id: Option<Uuid>,
    pub relationship: String,
}
