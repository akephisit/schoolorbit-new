use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Achievement {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    #[schema(required = true)]
    pub description: Option<String>,
    pub achievement_date: NaiveDate,
    #[schema(required = true)]
    pub image_path: Option<String>,
    #[schema(required = true)]
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Joined fields
    #[sqlx(default)]
    #[schema(required = true)]
    pub user_first_name: Option<String>,
    #[sqlx(default)]
    #[schema(required = true)]
    pub user_last_name: Option<String>,
    #[sqlx(default)]
    #[schema(required = true)]
    pub user_profile_image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateAchievementRequest {
    pub user_id: Option<Uuid>, // Optional: if null, defaults to current user. If current user is same as target, allow. If different, check permission.
    pub title: String,
    pub description: Option<String>,
    pub achievement_date: NaiveDate,
    pub image_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateAchievementRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub achievement_date: Option<NaiveDate>,
    pub image_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct AchievementListFilter {
    pub user_id: Option<Uuid>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}
