use async_trait::async_trait;
use bytes::Bytes;
use std::{fmt, time::Duration};
use url::Url;

use super::{
    platform_types::{DownloadGrant, StorageClass},
    purpose_registry::ObjectKey,
};

/// Maximum validity for a provider-issued private download grant.
pub const MAX_PRIVATE_DOWNLOAD_GRANT_TTL: Duration = Duration::from_secs(300);

/// A provider input whose storage identity originates from the purpose registry.
#[derive(Clone, Eq, PartialEq)]
pub struct StoredObject {
    pub object_key: ObjectKey,
    pub content_type: String,
}

impl StoredObject {
    pub fn new(object_key: ObjectKey, content_type: impl Into<String>) -> Self {
        Self {
            object_key,
            content_type: content_type.into(),
        }
    }

    pub const fn storage_class(&self) -> StorageClass {
        self.object_key.storage_class()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectMetadata {
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageError {
    ConfigurationInvalid,
    OperationFailed,
    AlreadyExists,
    PublicLocationRequiresPublicObject,
    PrivateGrantRequiresPrivateObject,
    InvalidDownloadGrantTtl,
}

impl StorageError {
    pub const fn log_safe_code(self) -> &'static str {
        match self {
            Self::ConfigurationInvalid => "storage_configuration_invalid",
            Self::OperationFailed => "storage_operation_failed",
            Self::AlreadyExists => "storage_object_already_exists",
            Self::PublicLocationRequiresPublicObject => "storage_public_location_rejected",
            Self::PrivateGrantRequiresPrivateObject => "storage_private_grant_rejected",
            Self::InvalidDownloadGrantTtl => "storage_grant_ttl_invalid",
        }
    }
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ConfigurationInvalid => "storage configuration is invalid",
            Self::OperationFailed => "storage operation failed",
            Self::AlreadyExists => "storage object already exists",
            Self::PublicLocationRequiresPublicObject => {
                "public location requires a public storage object"
            }
            Self::PrivateGrantRequiresPrivateObject => {
                "private download grant requires a private storage object"
            }
            Self::InvalidDownloadGrantTtl => "private download grant lifetime is invalid",
        })
    }
}

impl std::error::Error for StorageError {}

#[async_trait]
pub trait StorageProvider: Send + Sync {
    async fn put(&self, object: &StoredObject, body: Bytes) -> Result<(), StorageError>;
    async fn head(&self, object: &StoredObject) -> Result<Option<ObjectMetadata>, StorageError>;
    async fn delete(&self, object: &StoredObject) -> Result<(), StorageError>;
    async fn private_download_grant(
        &self,
        object: &StoredObject,
        filename: &str,
        ttl: Duration,
    ) -> Result<DownloadGrant, StorageError>;
    fn public_location(&self, object: &StoredObject) -> Result<Url, StorageError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::files::{
        platform_types::{DetectedContent, DownloadGrant, FilePurpose, StorageClass},
        purpose_registry::original_object_key,
    };
    use async_trait::async_trait;
    use bytes::Bytes;
    use chrono::Utc;
    use std::{sync::Mutex, time::Duration};
    use url::Url;
    use uuid::Uuid;

    fn object(purpose: FilePurpose) -> StoredObject {
        StoredObject::new(
            original_object_key(
                Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
                purpose,
                Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
                1,
                DetectedContent::Png,
            )
            .unwrap(),
            "image/png",
        )
    }

    struct FakeStorageProvider {
        selected_classes: Mutex<Vec<StorageClass>>,
        grant_location: Url,
    }

    #[async_trait]
    impl StorageProvider for FakeStorageProvider {
        async fn put(&self, object: &StoredObject, _body: Bytes) -> Result<(), StorageError> {
            self.selected_classes
                .lock()
                .unwrap()
                .push(object.storage_class());
            Ok(())
        }

        async fn head(
            &self,
            _object: &StoredObject,
        ) -> Result<Option<ObjectMetadata>, StorageError> {
            Ok(None)
        }

        async fn delete(&self, _object: &StoredObject) -> Result<(), StorageError> {
            Ok(())
        }

        async fn private_download_grant(
            &self,
            _object: &StoredObject,
            _filename: &str,
            ttl: Duration,
        ) -> Result<DownloadGrant, StorageError> {
            Ok(DownloadGrant::Redirect {
                location: self.grant_location.to_string(),
                expires_at: Utc::now()
                    + chrono::Duration::from_std(ttl.min(MAX_PRIVATE_DOWNLOAD_GRANT_TTL)).unwrap(),
            })
        }

        fn public_location(&self, object: &StoredObject) -> Result<Url, StorageError> {
            if object.storage_class() != StorageClass::Public {
                return Err(StorageError::PublicLocationRequiresPublicObject);
            }
            Url::parse("https://public.example.invalid/object")
                .map_err(|_| StorageError::OperationFailed)
        }
    }

    #[tokio::test]
    async fn storage_provider_contract_preserves_the_selected_storage_class() {
        let provider = FakeStorageProvider {
            selected_classes: Mutex::new(Vec::new()),
            grant_location: Url::parse("https://signed.example.invalid/grant?signature=redacted")
                .unwrap(),
        };

        provider
            .put(&object(FilePurpose::SchoolLogo), Bytes::new())
            .await
            .unwrap();
        provider
            .put(&object(FilePurpose::ProfileImage), Bytes::new())
            .await
            .unwrap();

        assert_eq!(
            *provider.selected_classes.lock().unwrap(),
            vec![StorageClass::Public, StorageClass::Private]
        );
    }

    #[tokio::test]
    async fn storage_provider_contract_bounds_private_grant_expiry() {
        let provider = FakeStorageProvider {
            selected_classes: Mutex::new(Vec::new()),
            grant_location: Url::parse("https://signed.example.invalid/grant?signature=redacted")
                .unwrap(),
        };
        let grant = provider
            .private_download_grant(
                &object(FilePurpose::ProfileImage),
                "statement.pdf",
                MAX_PRIVATE_DOWNLOAD_GRANT_TTL + Duration::from_secs(1),
            )
            .await
            .unwrap();
        let after = Utc::now();

        let DownloadGrant::Redirect { expires_at, .. } = grant else {
            panic!("fake storage provider must return a redirect grant");
        };
        assert!(
            expires_at
                <= after + chrono::Duration::from_std(MAX_PRIVATE_DOWNLOAD_GRANT_TTL).unwrap(),
            "private download grants must not exceed the bounded provider lifetime"
        );
    }

    #[test]
    fn storage_errors_are_safe_for_clients_and_logs() {
        let object = object(FilePurpose::ProfileImage);
        let signed_url = "https://private.example.invalid/object?X-Amz-Signature=super-secret";

        for error in [
            StorageError::ConfigurationInvalid,
            StorageError::OperationFailed,
            StorageError::AlreadyExists,
            StorageError::PublicLocationRequiresPublicObject,
            StorageError::PrivateGrantRequiresPrivateObject,
            StorageError::InvalidDownloadGrantTtl,
        ] {
            let safe_message = error.to_string();
            assert!(!safe_message.contains(object.object_key.as_str()));
            assert!(!safe_message.contains(signed_url));
            assert!(!error.log_safe_code().contains(object.object_key.as_str()));
            assert!(!error.log_safe_code().contains(signed_url));
        }
    }

    #[test]
    fn stored_objects_derive_storage_class_from_registry_keys() {
        assert_eq!(
            object(FilePurpose::SchoolLogo).storage_class(),
            StorageClass::Public
        );
        assert_eq!(
            object(FilePurpose::ProfileImage).storage_class(),
            StorageClass::Private
        );
    }
}
