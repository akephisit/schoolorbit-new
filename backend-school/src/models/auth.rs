use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// User model (from database)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub national_id: Option<String>,
    pub email: Option<String>,
    pub password_hash: String,
    pub first_name: String,
    pub last_name: String,
    pub role: String,
    pub phone: Option<String>,
    pub date_of_birth: Option<chrono::NaiveDate>,
    pub address: Option<String>,
    pub status: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Login request
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub national_id: String,
    pub password: String,
    pub remember_me: Option<bool>,
}

// User response (without sensitive data)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub id: Uuid,
    pub national_id: Option<String>,
    pub email: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub role: String,
    pub phone: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            national_id: user.national_id,
            email: user.email,
            first_name: user.first_name,
            last_name: user.last_name,
            role: user.role,
            phone: user.phone,
            status: user.status,
            created_at: user.created_at,
        }
    }
}

// Login response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub success: bool,
    pub message: String,
    pub user: UserResponse,
}

// JWT Claims
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,           // user_id
    pub national_id: String,
    pub role: String,
    pub exp: i64,              // Expiry timestamp
    pub iat: i64,              // Issued at timestamp
}

// Notification model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub message: String,
    #[serde(rename = "type")]
    pub notification_type: String,
    pub link: Option<String>,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

// Create notification request
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNotification {
    pub user_id: Option<Uuid>, // If None, broadcast to all
    pub title: String,
    pub message: String,
    #[serde(rename = "type")]
    pub notification_type: String,
    pub link: Option<String>,
}
