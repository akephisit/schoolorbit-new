use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Achievement {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub achievement_date: NaiveDate,
    pub image_path: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Joined fields
    pub user_first_name: Option<String>,
    pub user_last_name: Option<String>,
    pub user_profile_image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAchievementRequest {
    pub user_id: Option<Uuid>, // Optional: if null, defaults to current user. If current user is same as target, allow. If different, check permission.
    pub title: String,
    pub description: Option<String>,
    pub achievement_date: NaiveDate,
    pub image_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAchievementRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub achievement_date: Option<NaiveDate>,
    pub image_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementListFilter {
    pub user_id: Option<Uuid>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}
