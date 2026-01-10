use sha2::{Digest, Sha256};

/// File hashing utilities for generating checksums
pub struct FileHasher;

impl FileHasher {
    /// Generate SHA-256 checksum for file data
    ///
    /// # Arguments
    /// * `data` - File data as bytes
    ///
    /// # Returns
    /// Hexadecimal string representation of SHA-256 hash
    ///
    /// # Example
    /// ```
    /// let data = b"hello world";
    /// let checksum = FileHasher::sha256(data);
    /// assert_eq!(checksum.len(), 64); // SHA-256 produces 64 hex characters
    /// ```
    pub fn sha256(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        hex::encode(result)
    }
    
    /// Verify if data matches a given checksum
    ///
    /// # Arguments
    /// * `data` - File data to verify
    /// * `expected_checksum` - Expected SHA-256 checksum
    ///
    /// # Returns
    /// true if checksums match, false otherwise
    pub fn verify_checksum(data: &[u8], expected_checksum: &str) -> bool {
        let actual = Self::sha256(data);
        actual.eq_ignore_ascii_case(expected_checksum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sha256_hash() {
        let data = b"hello world";
        let hash = FileHasher::sha256(data);
        
        // Known SHA-256 hash of "hello world"
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert_eq!(hash, expected);
    }
    
    #[test]
    fn test_sha256_empty() {
        let data = b"";
        let hash = FileHasher::sha256(data);
        
        // Known SHA-256 hash of empty string
        let expected = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(hash, expected);
    }
    
    #[test]
    fn test_verify_checksum() {
        let data = b"test data";
        let checksum = FileHasher::sha256(data);
        
        assert!(FileHasher::verify_checksum(data, &checksum));
        assert!(!FileHasher::verify_checksum(data, "invalid_checksum"));
    }
    
    #[test]
    fn test_verify_checksum_case_insensitive() {
        let data = b"test";
        let checksum = FileHasher::sha256(data);
        let upper = checksum.to_uppercase();
        
        assert!(FileHasher::verify_checksum(data, &upper));
    }
}
