use async_trait::async_trait;
use aws_config::meta::region::RegionProviderChain;
use aws_credential_types::Credentials;
use aws_sdk_s3::{
    config::Region, presigning::PresigningConfig, primitives::ByteStream, Client as S3Client,
};
use chrono::Utc;
use std::{env, time::Duration};
use url::Url;

use super::{
    platform_types::{DownloadGrant, StorageClass},
    storage_provider::{
        ObjectMetadata, StorageError, StorageProvider, StoredObject, MAX_PRIVATE_DOWNLOAD_GRANT_TTL,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StorageBucket {
    Public,
    Private,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum R2OperationError {
    NotFound,
    Failed,
}

/// R2 configuration owned entirely by server environment variables.
#[derive(Clone)]
pub struct R2StorageConfig {
    account_id: String,
    access_key_id: String,
    secret_access_key: String,
    region: String,
    public_bucket_name: String,
    private_bucket_name: String,
    public_base_url: Url,
}

impl R2StorageConfig {
    pub fn from_env() -> Result<Self, StorageError> {
        let public_base_url = env::var("R2_PUBLIC_URL")
            .ok()
            .and_then(|value| Url::parse(&value).ok())
            .ok_or(StorageError::ConfigurationInvalid)?;

        Ok(Self {
            account_id: required_env("R2_ACCOUNT_ID")?,
            access_key_id: required_env("R2_ACCESS_KEY_ID")?,
            secret_access_key: required_env("R2_SECRET_ACCESS_KEY")?,
            region: env::var("R2_REGION").unwrap_or_else(|_| "auto".to_string()),
            public_bucket_name: required_env("R2_PUBLIC_BUCKET_NAME")?,
            private_bucket_name: required_env("R2_PRIVATE_BUCKET_NAME")?,
            public_base_url,
        })
    }

    fn endpoint_url(&self) -> String {
        format!("https://{}.r2.cloudflarestorage.com", self.account_id)
    }

    fn bucket_name(&self, bucket: StorageBucket) -> &str {
        match bucket {
            StorageBucket::Public => &self.public_bucket_name,
            StorageBucket::Private => &self.private_bucket_name,
        }
    }
}

fn required_env(name: &str) -> Result<String, StorageError> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or(StorageError::ConfigurationInvalid)
}

/// Cloudflare R2 implementation of the provider-neutral storage port.
pub struct R2StorageProvider {
    client: S3Client,
    config: R2StorageConfig,
}

impl R2StorageProvider {
    pub async fn new() -> Result<Self, StorageError> {
        let config = R2StorageConfig::from_env()?;
        let credentials = Credentials::new(
            &config.access_key_id,
            &config.secret_access_key,
            None,
            None,
            "r2-storage-provider",
        );
        let region = Region::new(config.region.clone());
        let region_provider = RegionProviderChain::default_provider().or_else(region);
        let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(region_provider)
            .credentials_provider(credentials)
            .load()
            .await;
        let s3_config = aws_sdk_s3::config::Builder::from(&aws_config)
            .endpoint_url(config.endpoint_url())
            .force_path_style(true)
            .build();

        Ok(Self {
            client: S3Client::from_conf(s3_config),
            config,
        })
    }
}

#[async_trait]
impl StorageProvider for R2StorageProvider {
    async fn put(&self, object: &StoredObject, body: bytes::Bytes) -> Result<(), StorageError> {
        self.client
            .put_object()
            .bucket(self.config.bucket_name(select_bucket(object)))
            .key(object.object_key.as_str())
            .body(ByteStream::from(body))
            .content_type(&object.content_type)
            .send()
            .await
            .map_err(|_| StorageError::OperationFailed)?;
        Ok(())
    }

    async fn head(&self, object: &StoredObject) -> Result<Option<ObjectMetadata>, StorageError> {
        match self
            .client
            .head_object()
            .bucket(self.config.bucket_name(select_bucket(object)))
            .key(object.object_key.as_str())
            .send()
            .await
        {
            Ok(output) => Ok(Some(ObjectMetadata {
                content_type: output.content_type().map(ToOwned::to_owned),
                content_length: output
                    .content_length()
                    .and_then(|length| length.try_into().ok()),
            })),
            Err(error)
                if error
                    .as_service_error()
                    .is_some_and(|service_error| service_error.is_not_found()) =>
            {
                Ok(None)
            }
            Err(_) => Err(StorageError::OperationFailed),
        }
    }

    async fn delete(&self, object: &StoredObject) -> Result<(), StorageError> {
        let result = self
            .client
            .delete_object()
            .bucket(self.config.bucket_name(select_bucket(object)))
            .key(object.object_key.as_str())
            .send()
            .await
            .map(|_| ())
            .map_err(|error| {
                if error
                    .raw_response()
                    .is_some_and(|response| response.status().as_u16() == 404)
                {
                    R2OperationError::NotFound
                } else {
                    R2OperationError::Failed
                }
            });
        delete_outcome(result)
    }

    async fn private_download_grant(
        &self,
        object: &StoredObject,
        filename: &str,
        ttl: Duration,
    ) -> Result<DownloadGrant, StorageError> {
        if object.storage_class != StorageClass::Private {
            return Err(StorageError::PrivateGrantRequiresPrivateObject);
        }
        if ttl.is_zero() {
            return Err(StorageError::InvalidDownloadGrantTtl);
        }

        let ttl = bounded_grant_ttl(ttl);
        let presigning =
            PresigningConfig::expires_in(ttl).map_err(|_| StorageError::InvalidDownloadGrantTtl)?;
        let request = self
            .client
            .get_object()
            .bucket(self.config.bucket_name(StorageBucket::Private))
            .key(object.object_key.as_str())
            .response_content_disposition(content_disposition(filename))
            .presigned(presigning)
            .await
            .map_err(|_| StorageError::OperationFailed)?;
        let location =
            Url::parse(&request.uri().to_string()).map_err(|_| StorageError::OperationFailed)?;
        let expires_at = Utc::now()
            + chrono::Duration::from_std(ttl).map_err(|_| StorageError::InvalidDownloadGrantTtl)?;

        Ok(DownloadGrant::Redirect {
            location: location.into(),
            expires_at,
        })
    }

    fn public_location(&self, object: &StoredObject) -> Result<Url, StorageError> {
        public_location_for(&self.config.public_base_url, object)
    }
}

fn select_bucket(object: &StoredObject) -> StorageBucket {
    match object.storage_class {
        StorageClass::Public => StorageBucket::Public,
        StorageClass::Private => StorageBucket::Private,
    }
}

fn delete_outcome(result: Result<(), R2OperationError>) -> Result<(), StorageError> {
    match result {
        Ok(()) | Err(R2OperationError::NotFound) => Ok(()),
        Err(R2OperationError::Failed) => Err(StorageError::OperationFailed),
    }
}

fn bounded_grant_ttl(ttl: Duration) -> Duration {
    ttl.min(MAX_PRIVATE_DOWNLOAD_GRANT_TTL)
}

fn content_disposition(filename: &str) -> String {
    let filename = filename
        .chars()
        .take(128)
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | ' ' | '.' | '-' | '_' => character,
            _ => '_',
        })
        .collect::<String>();
    let filename = if filename.is_empty() {
        "download"
    } else {
        &filename
    };
    format!("attachment; filename=\"{filename}\"")
}

fn public_location_for(base_url: &Url, object: &StoredObject) -> Result<Url, StorageError> {
    if object.storage_class != StorageClass::Public {
        return Err(StorageError::PublicLocationRequiresPublicObject);
    }

    base_url
        .join(object.object_key.as_str())
        .map_err(|_| StorageError::OperationFailed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::files::{
        platform_types::{DetectedContent, FilePurpose, StorageClass},
        purpose_registry::original_object_key,
        storage_provider::{StoredObject, MAX_PRIVATE_DOWNLOAD_GRANT_TTL},
    };
    use std::time::Duration;
    use uuid::Uuid;

    fn object(storage_class: StorageClass) -> StoredObject {
        StoredObject {
            storage_class,
            object_key: original_object_key(
                Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
                FilePurpose::ProfileImage,
                Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
                1,
                DetectedContent::Png,
            )
            .unwrap(),
            content_type: "image/png".to_string(),
        }
    }

    #[test]
    fn r2_selects_only_the_public_bucket_for_public_objects() {
        assert_eq!(
            select_bucket(&object(StorageClass::Public)),
            StorageBucket::Public
        );
    }

    #[test]
    fn r2_selects_only_the_private_bucket_for_private_objects() {
        assert_eq!(
            select_bucket(&object(StorageClass::Private)),
            StorageBucket::Private
        );
    }

    #[test]
    fn r2_delete_treats_not_found_as_success() {
        assert_eq!(delete_outcome(Err(R2OperationError::NotFound)), Ok(()));
    }

    #[test]
    fn r2_private_grants_cap_ttl_and_sanitize_content_disposition() {
        assert_eq!(
            bounded_grant_ttl(MAX_PRIVATE_DOWNLOAD_GRANT_TTL + Duration::from_secs(1)),
            MAX_PRIVATE_DOWNLOAD_GRANT_TTL
        );
        let disposition = content_disposition("report\"\r\nX-Injected: true.pdf");
        assert!(disposition.starts_with("attachment; filename=\"report"));
        let filename = disposition
            .strip_prefix("attachment; filename=\"")
            .and_then(|value| value.strip_suffix('"'))
            .expect("content disposition must contain a quoted filename");
        assert!(!filename.contains(['\r', '\n', '"']));
    }

    #[test]
    fn r2_public_location_rejects_private_objects() {
        let base_url = Url::parse("https://public.example.invalid/").unwrap();

        assert_eq!(
            public_location_for(&base_url, &object(StorageClass::Private)),
            Err(StorageError::PublicLocationRequiresPublicObject)
        );
        assert_eq!(
            public_location_for(&base_url, &object(StorageClass::Public))
                .unwrap()
                .host_str(),
            Some("public.example.invalid")
        );
    }
}
