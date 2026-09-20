use crate::error::AppError;
use bcrypt::{hash, verify, DEFAULT_COST};

pub fn hash_password(password: &str) -> Result<String, AppError> {
    hash(password, DEFAULT_COST)
        .map_err(|e| AppError::InternalServerError(format!("Password hashing failed: {}", e)))
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    verify(password, hash)
        .map_err(|e| AppError::InternalServerError(format!("Password verification failed: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::{hash_password, verify_password};

    const LEGACY_BCRYPT_HASH: &str = "$2b$12$rri4KlbA4XkEM.JADCiBdu3/cjxESITAXMOYiKoklPN9HW6vgjEP2";

    #[test]
    fn legacy_bcrypt_hashes_remain_verifiable() {
        assert!(verify_password("synthetic-password-for-wave-four", LEGACY_BCRYPT_HASH,).unwrap());
        assert!(!verify_password("wrong-synthetic-password", LEGACY_BCRYPT_HASH).unwrap());
    }

    #[test]
    fn newly_generated_bcrypt_hashes_keep_the_existing_format_and_cost() {
        let hash = hash_password("another-synthetic-password").unwrap();
        assert!(hash.starts_with("$2b$12$"));
        assert!(verify_password("another-synthetic-password", &hash).unwrap());
    }
}
