/// Helper to decrypt User's encrypted fields after fetching from database
/// 
/// This decrypts:
/// - national_id (always encrypted)
/// - Other sensitive fields as needed
use crate::models::auth::User;
use crate::utils::field_encryption;

pub trait DecryptUser {
    fn decrypt_sensitive_fields(&mut self) -> Result<(), String>;
}

impl DecryptUser for User {
    fn decrypt_sensitive_fields(&mut self) -> Result<(), String> {
        // Decrypt national_id if not empty
        if !self.national_id.is_empty() {
            self.national_id = field_encryption::decrypt(&self.national_id)
                .map_err(|e| format!("Failed to decrypt national_id: {}", e))?;
        }
        
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
