use crate::utils::file_url::get_file_url_from_string;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
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
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub username: String, // PROMOTED: login with username
    pub password: String,
    pub remember_me: Option<bool>,
}

// Update profile request (editable fields only)
#[derive(Debug, Deserialize, ToSchema)]
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
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

// User response (without sensitive data)
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String, // Added field
    #[schema(required = true)]
    pub national_id: Option<String>,
    #[schema(required = true)]
    pub email: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub user_type: String,
    #[schema(required = true)]
    pub phone: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,

    // Primary role name from roles table (if exists)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub primary_role_name: Option<String>,

    #[schema(required = true)]
    pub profile_image_url: Option<String>,

    // User permissions
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Vec<String>)]
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
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProfileResponse {
    // Basic info (read-only)
    pub id: Uuid,
    pub username: String, // Added field
    #[schema(required = true)]
    pub national_id: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub user_type: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // Primary role (read-only)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    pub primary_role_name: Option<String>,

    // Editable fields
    #[schema(required = true)]
    pub title: Option<String>,
    #[schema(required = true)]
    pub nickname: Option<String>,
    #[schema(required = true)]
    pub email: Option<String>,
    #[schema(required = true)]
    pub phone: Option<String>,
    #[schema(required = true)]
    pub emergency_contact: Option<String>,
    #[schema(required = true)]
    pub line_id: Option<String>,
    #[schema(required = true)]
    pub date_of_birth: Option<chrono::NaiveDate>,
    #[schema(required = true)]
    pub gender: Option<String>,
    #[schema(required = true)]
    pub address: Option<String>,
    #[schema(required = true)]
    pub profile_image_url: Option<String>,
    #[schema(required = true)]
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

// Login response data
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginData {
    pub user: UserResponse,
}

// JWT Claims
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub user_type: String,
    pub tenant: String,
    pub iss: String,
    pub aud: String,
    pub token_version: u8,
    pub exp: i64,
    pub iat: i64,
}
