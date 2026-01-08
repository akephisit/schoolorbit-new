use std::env;

/// Get encryption key from environment variable
/// 
/// # Security
/// - Key must be set in ENCRYPTION_KEY environment variable
/// - Minimum 32 characters recommended
/// - Never commit the key to version control
pub fn get_encryption_key() -> Result<String, String> {
    env::var("ENCRYPTION_KEY")
        .map_err(|_| "ENCRYPTION_KEY environment variable not set".to_string())
}

/// Setup encryption key in database session
///
/// This must be called before any encrypted column operations
pub async fn setup_encryption_key(pool: &sqlx::PgPool) -> Result<(), String> {
    let key = get_encryption_key()?;
    
    sqlx::query(&format!("SET LOCAL app.encryption_key = '{}'", key))
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to set encryption key: {}", e))?;
    
    Ok(())
}

/// Encrypt sensitive text data
/// 
/// Uses PostgreSQL pgcrypto (pgp_sym_encrypt) for AES-256 encryption
/// 
/// # Arguments
/// * `plaintext` - The text to encrypt
/// * `key` - Encryption key
/// 
/// # Returns
/// SQL expression for encrypted data
pub fn encrypt_sql(plaintext: &str, param_index: i32) -> String {
    format!(
        "pgp_sym_encrypt(${}, current_setting('app.encryption_key'))",
        param_index
    )
}

/// Decrypt sensitive text data
///
/// Uses PostgreSQL pgcrypto (pgp_sym_decrypt) for decryption
///
/// # Arguments
/// * `column_name` - The column name containing encrypted data
///
/// # Returns
/// SQL expression for decrypted data
pub fn decrypt_sql(column_name: &str) -> String {
    format!(
        "COALESCE(pgp_sym_decrypt({}, current_setting('app.encryption_key')), '')",
        column_name
    )
}

/// Helper to set encryption key in PostgreSQL session
///
/// This should be called at the beginning of each database transaction
/// that needs to encrypt/decrypt data
pub fn set_encryption_key_sql(key: &str) -> String {
    format!("SET LOCAL app.encryption_key = '{}'", key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_sql() {
        let sql = encrypt_sql("test", 1);
        assert!(sql.contains("pgp_sym_encrypt"));
        assert!(sql.contains("$1"));
    }

    #[test]
    fn test_decrypt_sql() {
        let sql = decrypt_sql("national_id");
        assert!(sql.contains("pgp_sym_decrypt"));
        assert!(sql.contains("national_id"));
    }
}
