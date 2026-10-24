//! Physical routing identity. Credentials and public/CDN URLs never identify a bucket.
use super::storage_provider::StorageError;
use sha2::{Digest, Sha256};

#[derive(Clone, Eq, PartialEq)]
pub struct StorageNamespace(String);

impl StorageNamespace {
    /// Trusted provider configuration only; never accepts an HTTP upload parameter.
    pub fn from_provider_locator(
        provider: &str,
        endpoint: &str,
        bucket: &str,
    ) -> Result<Self, StorageError> {
        let parsed = url::Url::parse(endpoint).map_err(|_| StorageError::ConfigurationInvalid)?;
        if provider.is_empty()
            || bucket.trim().is_empty()
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(StorageError::ConfigurationInvalid);
        }
        let mut hash = Sha256::new();
        for value in [
            "schoolorbit.storage_namespace.v1",
            provider,
            parsed.as_str(),
            bucket,
        ] {
            hash.update((value.len() as u64).to_be_bytes());
            hash.update(value.as_bytes());
        }
        Ok(Self(hex::encode(hash.finalize())))
    }
    pub(crate) fn parse(value: String) -> Result<Self, StorageError> {
        if value.len() == 64
            && value
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            Ok(Self(value))
        } else {
            Err(StorageError::ConfigurationInvalid)
        }
    }
    pub(crate) fn fingerprint(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routing_identity_is_canonical_and_distinguishes_endpoint_and_bucket() {
        let one =
            StorageNamespace::from_provider_locator("r2", "https://EXAMPLE.invalid", "bucket")
                .unwrap();
        let same =
            StorageNamespace::from_provider_locator("r2", "https://example.invalid/", "bucket")
                .unwrap();
        assert!(one == same);
        assert!(
            one != StorageNamespace::from_provider_locator("r2", "https://other.invalid", "bucket")
                .unwrap()
        );
        assert!(
            one != StorageNamespace::from_provider_locator(
                "r2",
                "https://example.invalid",
                "other-bucket"
            )
            .unwrap()
        );
        assert!(StorageNamespace::from_provider_locator(
            "r2",
            "https://secret@example.invalid",
            "bucket"
        )
        .is_err());
        assert!(StorageNamespace::from_provider_locator(
            "r2",
            "https://example.invalid?signature=secret",
            "bucket"
        )
        .is_err());
    }
}
