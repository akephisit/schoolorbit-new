use crate::utils::file_url::get_file_url_from_string;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// User model (from database)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String, // Added field
    pub national_id: Option<String>,
    pub email: Option<String>,
    pub password_hash: String,
    pub first_name: String,
    pub last_name: String,
    pub user_type: String,
    pub phone: Option<String>,
    pub date_of_birth: Option<chrono::NaiveDate>,
    pub address: Option<String>,
    pub status: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    
    // Additional fields from migration 005
    pub title: Option<String>,
    pub nickname: Option<String>,
    pub emergency_contact: Option<String>,
    pub line_id: Option<String>,
    pub gender: Option<String>,
    pub profile_image_url: Option<String>,
    pub hired_date: Option<chrono::NaiveDate>,
    pub resigned_date: Option<chrono::NaiveDate>,
}

// Lightweight user model for login (only essential fields)
// Reduces query overhead by ~70% compared to full User struct
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct LoginUser {
    pub id: Uuid,
    pub username: String, // Added field
    pub password_hash: String,
    pub status: String,
    pub user_type: String,
    pub first_name: String,
    pub last_name: String,
    pub email: Option<String>,
    pub date_of_birth: Option<chrono::NaiveDate>,
    pub profile_image_url: Option<String>,
}

// Login request
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub username: String, // PROMOTED: login with username
    pub password: String,
    pub remember_me: Option<bool>,
}

// Update profile request (editable fields only)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileRequest {
    pub title: Option<String>,
    pub nickname: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub emergency_contact: Option<String>,
    pub line_id: Option<String>,
    pub date_of_birth: Option<String>, // Will be parsed to NaiveDate
    pub gender: Option<String>,
    pub address: Option<String>,
    pub profile_image_url: Option<String>,
}

// Change password request
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

// User response (without sensitive data)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String, // Added field
    pub national_id: Option<String>,
    pub email: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub user_type: String,
    pub phone: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    
    // Primary role name from roles table (if exists)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_role_name: Option<String>,
    
    pub profile_image_url: Option<String>,
    
    // User permissions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            national_id: user.national_id,
            email: user.email,
            first_name: user.first_name,
            last_name: user.last_name,
            user_type: user.user_type,
            phone: user.phone,
            status: user.status,
            created_at: user.created_at,
            primary_role_name: None, // Will be populated separately
            profile_image_url: get_file_url_from_string(&user.profile_image_url),
            permissions: None, // Will be populated separately
        }
    }
}

// Full profile response (for /me/profile endpoint)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileResponse {
    // Basic info (read-only)
    pub id: Uuid,
    pub username: String, // Added field
    pub national_id: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub user_type: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    
    // Primary role (read-only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_role_name: Option<String>,
    
    // Editable fields
    pub title: Option<String>,
    pub nickname: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub emergency_contact: Option<String>,
    pub line_id: Option<String>,
    pub date_of_birth: Option<chrono::NaiveDate>,
    pub gender: Option<String>,
    pub address: Option<String>,
    pub profile_image_url: Option<String>,
    pub hired_date: Option<chrono::NaiveDate>,
}

impl From<User> for ProfileResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            national_id: user.national_id.clone(),
            first_name: user.first_name.clone(),
            last_name: user.last_name.clone(),
            user_type: user.user_type.clone(),
            status: user.status.clone(),
            created_at: user.created_at,
            updated_at: user.updated_at,
            primary_role_name: None, // Will be populated separately in handler
            title: user.title,
            nickname: user.nickname,
            email: user.email,
            phone: user.phone,
            emergency_contact: user.emergency_contact,
            line_id: user.line_id,
            date_of_birth: user.date_of_birth,
            gender: user.gender,
            address: user.address,
            profile_image_url: get_file_url_from_string(&user.profile_image_url),
            hired_date: user.hired_date,
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
    pub username: String,      // Changed from national_id
    pub user_type: String,
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
