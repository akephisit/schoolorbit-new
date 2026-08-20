use std::fmt;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;
use zeroize::Zeroizing;

use crate::{error::AppError, utils::field_encryption};

pub struct CertificateProof {
    encrypted: String,
    hash: String,
    plaintext: Zeroizing<String>,
}

impl CertificateProof {
    pub fn encrypted(&self) -> &str {
        &self.encrypted
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn plaintext(&self) -> &str {
        &self.plaintext
    }
}

impl fmt::Debug for CertificateProof {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CertificateProof")
            .field("encrypted", &"[REDACTED]")
            .field("hash", &"[REDACTED]")
            .field("plaintext", &"[REDACTED]")
            .finish()
    }
}

pub fn generate_certificate_proof() -> Result<CertificateProof, AppError> {
    let mut bytes = Zeroizing::new([0_u8; 32]);
    rand::rng().fill_bytes(&mut *bytes);
    let plaintext = Zeroizing::new(URL_SAFE_NO_PAD.encode(&*bytes));
    let encrypted = field_encryption::encrypt(&plaintext).map_err(proof_crypto_error)?;
    let hash = field_encryption::hash_for_search_with_domain("certificate-qr-proof-v1", &plaintext)
        .map_err(proof_crypto_error)?;
    Ok(CertificateProof {
        encrypted,
        hash,
        plaintext,
    })
}

fn proof_crypto_error(_error: String) -> AppError {
    AppError::InternalServerError("certificate proof cryptography failed".to_string())
}

#[cfg(test)]
mod tests {
    use std::env;

    use crate::{
        modules::certificates::services::proof::generate_certificate_proof, utils::field_encryption,
    };

    #[test]
    fn proof_is_random_encrypted_domain_separated_and_debug_redacted() {
        let _guard = field_encryption::test_env_lock();
        env::set_var("ENCRYPTION_KEY", "certificate-proof-encryption-test-key");
        env::set_var("BLIND_INDEX_KEY", "certificate-proof-blind-index-test-key");

        let proof = generate_certificate_proof().unwrap();
        let second = generate_certificate_proof().unwrap();
        assert_eq!(
            field_encryption::decrypt(proof.encrypted()).unwrap(),
            proof.plaintext()
        );
        assert_eq!(proof.plaintext().len(), 43);
        assert_eq!(
            proof.hash(),
            field_encryption::hash_for_search_with_domain(
                "certificate-qr-proof-v1",
                proof.plaintext(),
            )
            .unwrap()
        );
        assert_ne!(proof.plaintext(), second.plaintext());
        assert_ne!(proof.hash(), second.hash());
        assert_ne!(
            field_encryption::hash_for_search_with_domain("certificate-qr-proof-v1", "proof-value")
                .unwrap(),
            field_encryption::hash_for_search_with_domain("another-domain", "proof-value").unwrap()
        );

        let debug = format!("{proof:?}");
        assert!(!debug.contains(proof.plaintext()));
        assert!(!debug.contains(proof.encrypted()));
        assert!(!debug.contains(proof.hash()));
        assert!(debug.contains("[REDACTED]"));
    }
}
