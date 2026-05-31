use crate::modules::admission::models::applications::AdmissionApplication;
use crate::utils::field_encryption;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedAdmissionPii {
    pub national_id: String,
    pub national_id_hash: String,
    pub father_national_id: Option<String>,
    pub father_national_id_hash: Option<String>,
    pub mother_national_id: Option<String>,
    pub mother_national_id_hash: Option<String>,
    pub guardian_national_id: Option<String>,
    pub guardian_national_id_hash: Option<String>,
}

fn normalize_national_id(value: &str) -> String {
    value.trim().to_string()
}

pub fn hash_required(value: &str) -> Result<String, String> {
    let normalized = normalize_national_id(value);
    if normalized.is_empty() {
        return Err("national_id is required".to_string());
    }

    field_encryption::hash_for_search(&normalized)
}

pub fn hash_optional(value: Option<&str>) -> Result<Option<String>, String> {
    match value
        .map(normalize_national_id)
        .filter(|value| !value.is_empty())
    {
        Some(value) => field_encryption::hash_for_search(&value).map(Some),
        None => Ok(None),
    }
}

pub fn encrypt_required(value: &str) -> Result<String, String> {
    let normalized = normalize_national_id(value);
    if normalized.is_empty() {
        return Err("national_id is required".to_string());
    }

    field_encryption::encrypt(&normalized)
}

pub fn encrypt_optional(value: Option<&str>) -> Result<Option<String>, String> {
    match value
        .map(normalize_national_id)
        .filter(|value| !value.is_empty())
    {
        Some(value) => field_encryption::encrypt(&value).map(Some),
        None => Ok(None),
    }
}

pub fn decrypt_required(value: &str) -> Result<String, String> {
    field_encryption::decrypt(value)
}

pub fn decrypt_optional(value: Option<&str>) -> Result<Option<String>, String> {
    field_encryption::decrypt_optional(value)
}

pub fn encrypt_application_pii(
    national_id: &str,
    father_national_id: Option<&str>,
    mother_national_id: Option<&str>,
    guardian_national_id: Option<&str>,
) -> Result<EncryptedAdmissionPii, String> {
    Ok(EncryptedAdmissionPii {
        national_id: encrypt_required(national_id)?,
        national_id_hash: hash_required(national_id)?,
        father_national_id: encrypt_optional(father_national_id)?,
        father_national_id_hash: hash_optional(father_national_id)?,
        mother_national_id: encrypt_optional(mother_national_id)?,
        mother_national_id_hash: hash_optional(mother_national_id)?,
        guardian_national_id: encrypt_optional(guardian_national_id)?,
        guardian_national_id_hash: hash_optional(guardian_national_id)?,
    })
}

pub fn decrypt_application_pii(application: &mut AdmissionApplication) -> Result<(), String> {
    application.national_id = decrypt_required(&application.national_id)?;
    application.father_national_id = decrypt_optional(application.father_national_id.as_deref())?;
    application.mother_national_id = decrypt_optional(application.mother_national_id.as_deref())?;
    application.guardian_national_id =
        decrypt_optional(application.guardian_national_id.as_deref())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{decrypt_optional, decrypt_required, encrypt_application_pii};
    use crate::utils::field_encryption;

    #[test]
    fn encrypt_application_pii_encrypts_values_and_hashes_for_lookup() {
        let _guard = field_encryption::test_env_lock();
        std::env::set_var("ENCRYPTION_KEY", "admission-pii-test-key");
        std::env::set_var("BLIND_INDEX_KEY", "admission-pii-blind-index-test-key");

        let pii = encrypt_application_pii(
            "1234567890123",
            Some("1111111111111"),
            None,
            Some("3333333333333"),
        )
        .expect("PII should encrypt");

        assert_eq!(
            pii.national_id_hash,
            field_encryption::hash_for_search("1234567890123").unwrap()
        );
        assert_ne!(pii.national_id, "1234567890123");
        assert_eq!(decrypt_required(&pii.national_id).unwrap(), "1234567890123");

        assert_eq!(
            pii.father_national_id_hash,
            Some(field_encryption::hash_for_search("1111111111111").unwrap())
        );
        assert_eq!(
            decrypt_optional(pii.father_national_id.as_deref()).unwrap(),
            Some("1111111111111".to_string())
        );
        assert!(pii.mother_national_id.is_none());
        assert!(pii.mother_national_id_hash.is_none());
        assert_eq!(
            decrypt_optional(pii.guardian_national_id.as_deref()).unwrap(),
            Some("3333333333333".to_string())
        );
    }
}
