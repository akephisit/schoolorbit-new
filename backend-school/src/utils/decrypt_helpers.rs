/// Helper to decrypt User's encrypted fields after fetching from database
use crate::models::auth::User;
use crate::utils::field_encryption;

pub trait DecryptUser {
    fn decrypt_sensitive_fields(&mut self) -> Result<(), String>;
}

impl DecryptUser for User {
    fn decrypt_sensitive_fields(&mut self) -> Result<(), String> {
        self.national_id = field_encryption::decrypt_optional(self.national_id.as_deref())?;
        Ok(())
    }
}

/// Convenience function to fetch and decrypt user by ID
pub async fn fetch_and_decrypt_user(
    pool: &sqlx::PgPool,
    user_id: &uuid::Uuid,
) -> Result<User, String> {
    let mut user: User = sqlx::query_as(
        "SELECT * FROM users WHERE id = $1"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Failed to fetch user: {}", e))?;
    
    user.decrypt_sensitive_fields()?;
    
    Ok(user)
}
