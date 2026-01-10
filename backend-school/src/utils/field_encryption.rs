use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use rand::RngCore;
use sha2::{Sha256, Digest};
use std::env;

/// Get cipher from environment key
fn get_cipher() -> Result<Aes256Gcm, String> {
    let key_str = env::var("ENCRYPTION_KEY")
        .map_err(|_| "ENCRYPTION_KEY not set".to_string())?;
    
    // Derive 32-byte key using SHA-256
    let mut hasher = Sha256::new();
    hasher.update(key_str.as_bytes());
    let key_bytes = hasher.finalize();
    
    Ok(Aes256Gcm::new(&key_bytes.into()))
}

/// Encrypt any string data
/// Returns base64-encoded ciphertext with nonce prepended
pub fn encrypt(plaintext: &str) -> Result<String, String> {
    if plaintext.is_empty() {
        return Ok(String::new());
    }

    let cipher = get_cipher()?;
    
    // Generate random 12-byte nonce
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;
    
    // Prepend nonce to ciphertext
    let mut result = nonce_bytes.to_vec();
    result.extend_from_slice(&ciphertext);
    
    // Encode as base64
    Ok(general_purpose::STANDARD.encode(result))
}

/// Decrypt base64-encoded ciphertext
pub fn decrypt(encrypted_base64: &str) -> Result<String, String> {
    if encrypted_base64.is_empty() {
        return Ok(String::new());
    }

    // Decode from base64
    let encrypted = general_purpose::STANDARD
        .decode(encrypted_base64)
        .map_err(|e| format!("Base64 decode failed: {}", e))?;
    
    // Extract nonce (first 12 bytes) and ciphertext
    if encrypted.len() < 12 {
        return Err("Invalid encrypted data: too short".to_string());
    }
    
    let (nonce_bytes, ciphertext) = encrypted.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    
    let cipher = get_cipher()?;
    
    // Decrypt
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed: {}", e))?;
    
    String::from_utf8(plaintext)
        .map_err(|e| format!("UTF-8 decode failed: {}", e))
}

/// Encrypt Option<String> - handles None gracefully
pub fn encrypt_optional(value: Option<&str>) -> Result<Option<String>, String> {
    match value {
        Some(v) if !v.is_empty() => Ok(Some(encrypt(v)?)),
        _ => Ok(None),
    }
}

/// Decrypt Option<String> - handles None gracefully  
pub fn decrypt_optional(encrypted: Option<&str>) -> Result<Option<String>, String> {
    match encrypted {
        Some(v) if !v.is_empty() => Ok(Some(decrypt(v)?)),
        _ => Ok(None),
    }
}

/// Hash text for searching (Deterministic)
/// Uses SHA-256
pub fn hash_for_search(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        env::set_var("ENCRYPTION_KEY", "test-key-for-testing-only");
        
        let original = "1234567890123";
        let encrypted = encrypt(original).unwrap();
        let decrypted = decrypt(&encrypted).unwrap();
        
        assert_eq!(original, decrypted);
        assert_ne!(original, encrypted);
    }

    #[test]
    fn test_empty_string() {
        env::set_var("ENCRYPTION_KEY", "test-key");
        
        let encrypted = encrypt("").unwrap();
        assert_eq!(encrypted, "");
        
        let decrypted = decrypt("").unwrap();
        assert_eq!(decrypted, "");
    }

    #[test]
    fn test_optional() {
        env::set_var("ENCRYPTION_KEY", "test-key");
        
        let encrypted = encrypt_optional(Some("test")).unwrap();
        assert!(encrypted.is_some());
        
        let decrypted = decrypt_optional(encrypted.as_deref()).unwrap();
        assert_eq!(decrypted, Some("test".to_string()));
    }
}
