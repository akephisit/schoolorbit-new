use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Serialize, Clone, FromRow, ToSchema)]
pub struct Notification {
    pub id: Uuid,
    pub title: String,
    pub message: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[schema(required = true)]
    pub link: Option<String>,
    #[schema(required = true)]
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListNotificationsQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub unread_only: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListNotificationsResponse {
    pub items: Vec<Notification>,
    pub unread_count: i64,
    pub page: i64,
    pub limit: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateNotificationRequest {
    #[serde(default)]
    pub user_id: Option<Uuid>,
    pub title: String,
    pub message: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub link: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubscribePushRequest {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}
