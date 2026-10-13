//! Admin-owned authenticated storage for tenant database passwords.
use aes_gcm::{
    aead::{consts::U12, Aead, Payload},
    Aes256Gcm, KeyInit, Nonce,
};
use rand::TryRng;
use uuid::Uuid;

pub struct DatabaseCredentials(Aes256Gcm);

impl DatabaseCredentials {
    pub fn from_env() -> Result<Self, String> {
        let key = std::env::var("DATABASE_CREDENTIALS_KEY")
            .map_err(|_| "DATABASE_CREDENTIALS_KEY is required".to_string())?;
        Self::from_key(&key)
    }

    pub fn from_key(key: &str) -> Result<Self, String> {
        let decoded =
            hex::decode(key).map_err(|_| "Invalid database credentials key".to_string())?;
        let cipher = Aes256Gcm::new_from_slice(&decoded).map_err(|_| {
            "Database credentials key must be 32 random bytes encoded as hex".to_string()
        })?;
        Ok(Self(cipher))
    }

    fn aad(tenant: Uuid, purpose: &str) -> Result<String, String> {
        if !matches!(purpose, "runtime" | "migration") {
            return Err("Invalid database credential purpose".into());
        }
        Ok(format!("schoolorbit-db-credentials:v1:{tenant}:{purpose}"))
    }

    pub fn encrypt(&self, tenant: Uuid, purpose: &str, password: &str) -> Result<Vec<u8>, String> {
        let aad = Self::aad(tenant, purpose)?;
        let mut bytes = [0_u8; 12];
        rand::rngs::SysRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| "Database credential randomness unavailable".to_string())?;
        let nonce = Nonce::<U12>::try_from(bytes.as_slice())
            .map_err(|_| "Invalid credential nonce".to_string())?;
        let encrypted = self
            .0
            .encrypt(
                &nonce,
                Payload {
                    msg: password.as_bytes(),
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| "Database credential encryption failed".to_string())?;
        let mut result = vec![1];
        result.extend_from_slice(&bytes);
        result.extend(encrypted);
        Ok(result)
    }

    pub fn decrypt(&self, tenant: Uuid, purpose: &str, encrypted: &[u8]) -> Result<String, String> {
        let aad = Self::aad(tenant, purpose)?;
        if encrypted.len() < 30 || encrypted[0] != 1 {
            return Err("Invalid database credential envelope".into());
        }
        let nonce = Nonce::<U12>::try_from(&encrypted[1..13])
            .map_err(|_| "Invalid credential nonce".to_string())?;
        let plain = self
            .0
            .decrypt(
                &nonce,
                Payload {
                    msg: &encrypted[13..],
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| "Database credential authentication failed".to_string())?;
        String::from_utf8(plain).map_err(|_| "Invalid database credential plaintext".into())
    }
}

pub fn random_password() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    rand::rngs::SysRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| "Database password randomness unavailable".to_string())?;
    Ok(hex::encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_are_authenticated_to_tenant_purpose_and_key() {
        let cipher = DatabaseCredentials::from_key(&"ab".repeat(32)).unwrap();
        let tenant = Uuid::new_v4();
        let envelope = cipher
            .encrypt(tenant, "runtime", "fixture-password")
            .unwrap();
        assert_eq!(
            cipher.decrypt(tenant, "runtime", &envelope).unwrap(),
            "fixture-password"
        );
        assert!(cipher
            .decrypt(Uuid::new_v4(), "runtime", &envelope)
            .is_err());
        assert!(cipher.decrypt(tenant, "migration", &envelope).is_err());
        assert!(DatabaseCredentials::from_key(&"cd".repeat(32))
            .unwrap()
            .decrypt(tenant, "runtime", &envelope)
            .is_err());
        let mut tampered = envelope.clone();
        tampered[13] ^= 1;
        assert!(cipher.decrypt(tenant, "runtime", &tampered).is_err());
        assert_ne!(
            envelope,
            cipher
                .encrypt(tenant, "runtime", "fixture-password")
                .unwrap()
        );
    }
}
