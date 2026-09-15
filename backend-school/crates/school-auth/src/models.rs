use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

fn deserialize_optional_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

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
    pub profile_image_file_id: Option<Uuid>,
    pub hired_date: Option<chrono::NaiveDate>,
    pub resigned_date: Option<chrono::NaiveDate>,
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
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub profile_image_file_id: Option<Option<Uuid>>,
}

// Change password request
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
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
    pub profile_image_file_id: Option<Uuid>,
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
            profile_image_file_id: user.profile_image_file_id,
            hired_date: user.hired_date,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CurrentUserResponse {
    pub id: Uuid,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub user_type: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_role_name: Option<String>,
    #[schema(required = true)]
    pub profile_image_file_id: Option<Uuid>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginData {
    pub user: CurrentUserResponse,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
    pub id: Uuid,
    pub device_label: String,
    pub remember_me: bool,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub idle_expires_at: DateTime<Utc>,
    pub absolute_expires_at: DateTime<Utc>,
    pub is_current: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionListData {
    pub sessions: Vec<SessionResponse>,
}

#[cfg(test)]
mod tests {
    use super::UpdateProfileRequest;

    #[test]
    fn update_profile_distinguishes_omitted_image_from_explicit_null() {
        let omitted: UpdateProfileRequest =
            serde_json::from_value(serde_json::json!({})).expect("omitted request should parse");
        let clear: UpdateProfileRequest =
            serde_json::from_value(serde_json::json!({ "profileImageFileId": null }))
                .expect("explicit-null request should parse");

        assert!(
            omitted.profile_image_file_id.is_none(),
            "omitting profileImageFileId must preserve the current image"
        );
        assert!(
            clear.profile_image_file_id.is_some(),
            "explicit null must request clearing the current image"
        );
        assert_eq!(clear.profile_image_file_id, Some(None));
    }
}
