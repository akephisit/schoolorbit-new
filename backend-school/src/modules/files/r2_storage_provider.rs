use async_trait::async_trait;
use aws_config::meta::region::RegionProviderChain;
use aws_credential_types::Credentials;
use aws_sdk_s3::{
    config::Region, presigning::PresigningConfig, primitives::ByteStream, Client as S3Client,
};
use chrono::Utc;
use std::{env, time::Duration};
use tokio::io::AsyncReadExt;
use url::Url;

use super::{
    platform_types::{DownloadGrant, StorageClass},
    storage_provider::{
        ObjectMetadata, StorageError, StorageProvider, StoredObject, MAX_PRIVATE_DOWNLOAD_GRANT_TTL,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObjectRequest {
    bucket: String,
    key: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum R2ClientError {
    NotFound,
    AlreadyExists,
    TooLarge,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PutRequest {
    bucket: String,
    key: String,
    body: bytes::Bytes,
    content_type: String,
    if_none_match: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GetRequest {
    bucket: String,
    key: String,
    max_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PresignRequest {
    bucket: String,
    key: String,
    content_disposition: String,
    ttl: Duration,
    issued_at: chrono::DateTime<Utc>,
}

#[async_trait]
trait R2Transport: Send + Sync {
    async fn head_bucket(&self, bucket: &str) -> Result<(), R2ClientError>;
    async fn put(&self, request: PutRequest) -> Result<(), R2ClientError>;
    async fn get(&self, request: GetRequest) -> Result<bytes::Bytes, R2ClientError>;
    async fn head(&self, request: ObjectRequest) -> Result<ObjectMetadata, R2ClientError>;
    async fn delete(&self, request: ObjectRequest) -> Result<(), R2ClientError>;
    async fn presign_get(&self, request: PresignRequest) -> Result<Url, R2ClientError>;
}

struct AwsR2Transport {
    client: S3Client,
}

#[async_trait]
impl R2Transport for AwsR2Transport {
    async fn head_bucket(&self, bucket: &str) -> Result<(), R2ClientError> {
        self.client
            .head_bucket()
            .bucket(bucket)
            .send()
            .await
            .map(|_| ())
            .map_err(|_| R2ClientError::Failed)
    }

    async fn put(&self, request: PutRequest) -> Result<(), R2ClientError> {
        self.client
            .put_object()
            .bucket(request.bucket)
            .key(request.key)
            .body(ByteStream::from(request.body))
            .content_type(request.content_type)
            .if_none_match(request.if_none_match)
            .send()
            .await
            .map(|_| ())
            .map_err(|error| {
                classify_write_status(
                    error
                        .raw_response()
                        .map(|response| response.status().as_u16()),
                )
            })
    }

    async fn get(&self, request: GetRequest) -> Result<bytes::Bytes, R2ClientError> {
        let output = self
            .client
            .get_object()
            .bucket(request.bucket)
            .key(request.key)
            .send()
            .await
            .map_err(|_| R2ClientError::Failed)?;
        let read_limit = request
            .max_bytes
            .checked_add(1)
            .ok_or(R2ClientError::Failed)?;
        let mut reader = output.body.into_async_read().take(read_limit);
        let mut body = Vec::new();
        reader
            .read_to_end(&mut body)
            .await
            .map_err(|_| R2ClientError::Failed)?;
        if body.len() as u64 > request.max_bytes {
            return Err(R2ClientError::TooLarge);
        }
        Ok(body.into())
    }

    async fn head(&self, request: ObjectRequest) -> Result<ObjectMetadata, R2ClientError> {
        self.client
            .head_object()
            .bucket(request.bucket)
            .key(request.key)
            .send()
            .await
            .map(|output| ObjectMetadata {
                content_type: output.content_type().map(ToOwned::to_owned),
                content_length: output
                    .content_length()
                    .and_then(|length| length.try_into().ok()),
            })
            .map_err(|error| {
                if error
                    .as_service_error()
                    .is_some_and(|service_error| service_error.is_not_found())
                    || error
                        .raw_response()
                        .is_some_and(|response| response.status().as_u16() == 404)
                {
                    R2ClientError::NotFound
                } else {
                    R2ClientError::Failed
                }
            })
    }

    async fn delete(&self, request: ObjectRequest) -> Result<(), R2ClientError> {
        self.client
            .delete_object()
            .bucket(request.bucket)
            .key(request.key)
            .send()
            .await
            .map(|_| ())
            .map_err(|error| {
                classify_missing_status(
                    error
                        .raw_response()
                        .map(|response| response.status().as_u16()),
                )
            })
    }

    async fn presign_get(&self, request: PresignRequest) -> Result<Url, R2ClientError> {
        let presigning = PresigningConfig::builder()
            .start_time(request.issued_at.into())
            .expires_in(request.ttl)
            .build()
            .map_err(|_| R2ClientError::Failed)?;
        let request = self
            .client
            .get_object()
            .bucket(request.bucket)
            .key(request.key)
            .response_content_disposition(request.content_disposition)
            .presigned(presigning)
            .await
            .map_err(|_| R2ClientError::Failed)?;
        Url::parse(&request.uri().to_string()).map_err(|_| R2ClientError::Failed)
    }
}

fn classify_write_status(status: Option<u16>) -> R2ClientError {
    match status {
        Some(409 | 412) => R2ClientError::AlreadyExists,
        _ => R2ClientError::Failed,
    }
}

fn classify_missing_status(status: Option<u16>) -> R2ClientError {
    match status {
        Some(404) => R2ClientError::NotFound,
        _ => R2ClientError::Failed,
    }
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
        Self::from_values(
            &required_env("R2_ACCOUNT_ID")?,
            &required_env("R2_ACCESS_KEY_ID")?,
            &required_env("R2_SECRET_ACCESS_KEY")?,
            &env::var("R2_REGION").unwrap_or_else(|_| "auto".to_string()),
            &required_env("R2_PUBLIC_BUCKET_NAME")?,
            &required_env("R2_PRIVATE_BUCKET_NAME")?,
            &required_env("R2_PUBLIC_URL")?,
        )
    }

    fn from_values(
        account_id: &str,
        access_key_id: &str,
        secret_access_key: &str,
        region: &str,
        public_bucket_name: &str,
        private_bucket_name: &str,
        public_base_url: &str,
    ) -> Result<Self, StorageError> {
        let public_bucket_name = normalized_bucket(public_bucket_name)?;
        let private_bucket_name = normalized_bucket(private_bucket_name)?;
        if public_bucket_name == private_bucket_name {
            return Err(StorageError::ConfigurationInvalid);
        }

        Ok(Self {
            account_id: required_value(account_id)?,
            access_key_id: required_value(access_key_id)?,
            secret_access_key: required_value(secret_access_key)?,
            region: required_value(region)?,
            public_bucket_name,
            private_bucket_name,
            public_base_url: validated_public_base_url(public_base_url)?,
        })
    }

    fn endpoint_url(&self) -> String {
        format!("https://{}.r2.cloudflarestorage.com", self.account_id)
    }

    fn bucket_name(&self, storage_class: StorageClass) -> &str {
        match storage_class {
            StorageClass::Public => &self.public_bucket_name,
            StorageClass::Private => &self.private_bucket_name,
        }
    }
}

fn required_env(name: &str) -> Result<String, StorageError> {
    env::var(name).map_err(|_| StorageError::ConfigurationInvalid)
}

fn required_value(value: &str) -> Result<String, StorageError> {
    let value = value.trim();
    (!value.is_empty() && !is_placeholder(value))
        .then(|| value.to_string())
        .ok_or(StorageError::ConfigurationInvalid)
}

fn is_placeholder(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    normalized.contains("your-")
        || normalized.contains("change-this")
        || normalized.contains("replace-me")
        || normalized.contains("xxxxx")
        || normalized.contains('<')
        || normalized.contains('>')
}

fn normalized_bucket(value: &str) -> Result<String, StorageError> {
    let value = required_value(value)?;
    let bytes = value.as_bytes();
    if !(3..=63).contains(&bytes.len())
        || !bytes
            .first()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        || !bytes
            .last()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
    {
        return Err(StorageError::ConfigurationInvalid);
    }
    Ok(value)
}

fn validated_public_base_url(value: &str) -> Result<Url, StorageError> {
    let value = required_value(value)?;
    let url = Url::parse(&value).map_err(|_| StorageError::ConfigurationInvalid)?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.cannot_be_a_base()
    {
        return Err(StorageError::ConfigurationInvalid);
    }
    Ok(url)
}

/// Cloudflare R2 implementation of the provider-neutral storage port.
pub struct R2StorageProvider {
    transport: std::sync::Arc<dyn R2Transport>,
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
            transport: std::sync::Arc::new(AwsR2Transport {
                client: S3Client::from_conf(s3_config),
            }),
            config,
        })
    }

    #[cfg(test)]
    fn with_transport(config: R2StorageConfig, transport: std::sync::Arc<dyn R2Transport>) -> Self {
        Self { transport, config }
    }
}

#[async_trait]
impl StorageProvider for R2StorageProvider {
    async fn check_readiness(&self) -> Result<(), StorageError> {
        self.transport
            .head_bucket(self.config.bucket_name(StorageClass::Public))
            .await
            .map_err(map_transport_error)?;
        self.transport
            .head_bucket(self.config.bucket_name(StorageClass::Private))
            .await
            .map_err(map_transport_error)
    }

    async fn put(&self, object: &StoredObject, body: bytes::Bytes) -> Result<(), StorageError> {
        self.transport
            .put(PutRequest {
                bucket: self.config.bucket_name(object.storage_class()).to_string(),
                key: object.object_key.as_str().to_string(),
                body,
                content_type: object.content_type.clone(),
                if_none_match: "*".to_string(),
            })
            .await
            .map_err(map_transport_error)
    }

    async fn get(
        &self,
        object: &StoredObject,
        max_bytes: u64,
    ) -> Result<bytes::Bytes, StorageError> {
        if max_bytes == 0 {
            return Err(StorageError::ObjectTooLarge);
        }
        let body = self
            .transport
            .get(GetRequest {
                bucket: self.config.bucket_name(object.storage_class()).to_string(),
                key: object.object_key.as_str().to_string(),
                max_bytes,
            })
            .await
            .map_err(map_transport_error)?;
        if body.len() as u64 > max_bytes {
            return Err(StorageError::ObjectTooLarge);
        }
        Ok(body)
    }

    async fn head(&self, object: &StoredObject) -> Result<Option<ObjectMetadata>, StorageError> {
        match self
            .transport
            .head(object_request(&self.config, object))
            .await
        {
            Ok(metadata) => Ok(Some(metadata)),
            Err(R2ClientError::NotFound) => Ok(None),
            Err(error) => Err(map_transport_error(error)),
        }
    }

    async fn delete(&self, object: &StoredObject) -> Result<(), StorageError> {
        match self
            .transport
            .delete(object_request(&self.config, object))
            .await
        {
            Ok(()) | Err(R2ClientError::NotFound) => Ok(()),
            Err(error) => Err(map_transport_error(error)),
        }
    }

    async fn private_download_grant(
        &self,
        object: &StoredObject,
        filename: &str,
        ttl: Duration,
    ) -> Result<DownloadGrant, StorageError> {
        if object.storage_class() != StorageClass::Private {
            return Err(StorageError::PrivateGrantRequiresPrivateObject);
        }
        if ttl.is_zero() {
            return Err(StorageError::InvalidDownloadGrantTtl);
        }

        let ttl = bounded_grant_ttl(ttl);
        let issued_at = Utc::now();
        let location = self
            .transport
            .presign_get(PresignRequest {
                bucket: self.config.bucket_name(StorageClass::Private).to_string(),
                key: object.object_key.as_str().to_string(),
                content_disposition: content_disposition(filename),
                ttl,
                issued_at,
            })
            .await
            .map_err(map_transport_error)?;
        let expires_at = issued_at
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

fn object_request(config: &R2StorageConfig, object: &StoredObject) -> ObjectRequest {
    ObjectRequest {
        bucket: config.bucket_name(object.storage_class()).to_string(),
        key: object.object_key.as_str().to_string(),
    }
}

fn map_transport_error(error: R2ClientError) -> StorageError {
    match error {
        R2ClientError::AlreadyExists => StorageError::AlreadyExists,
        R2ClientError::TooLarge => StorageError::ObjectTooLarge,
        R2ClientError::NotFound | R2ClientError::Failed => StorageError::OperationFailed,
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
    if object.storage_class() != StorageClass::Public {
        return Err(StorageError::PublicLocationRequiresPublicObject);
    }

    let mut location = base_url.clone();
    let mut segments = location
        .path_segments_mut()
        .map_err(|_| StorageError::OperationFailed)?;
    segments.pop_if_empty();
    for segment in object.object_key.as_str().split('/') {
        segments.push(segment);
    }
    drop(segments);
    Ok(location)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::files::{
        platform_types::{DetectedContent, FilePurpose},
        purpose_registry::original_object_key,
        storage_provider::{StoredObject, MAX_PRIVATE_DOWNLOAD_GRANT_TTL},
    };
    use bytes::Bytes;
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
        time::Duration,
    };
    use uuid::Uuid;

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum CapturedR2Request {
        Bucket(String),
        Put(PutRequest),
        Get(GetRequest),
        Head(ObjectRequest),
        Delete(ObjectRequest),
        Presign(PresignRequest),
    }

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

    #[derive(Default)]
    struct CapturedR2Client {
        requests: Mutex<Vec<CapturedR2Request>>,
        bucket_results: Mutex<VecDeque<Result<(), R2ClientError>>>,
        put_results: Mutex<VecDeque<Result<(), R2ClientError>>>,
        get_results: Mutex<VecDeque<Result<Bytes, R2ClientError>>>,
        head_results: Mutex<VecDeque<Result<ObjectMetadata, R2ClientError>>>,
        delete_results: Mutex<VecDeque<Result<(), R2ClientError>>>,
        presign_results: Mutex<VecDeque<Result<Url, R2ClientError>>>,
    }

    #[async_trait]
    impl R2Transport for CapturedR2Client {
        async fn head_bucket(&self, bucket: &str) -> Result<(), R2ClientError> {
            self.requests
                .lock()
                .unwrap()
                .push(CapturedR2Request::Bucket(bucket.to_string()));
            self.bucket_results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Ok(()))
        }

        async fn put(&self, request: PutRequest) -> Result<(), R2ClientError> {
            self.requests
                .lock()
                .unwrap()
                .push(CapturedR2Request::Put(request));
            self.put_results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Ok(()))
        }

        async fn get(&self, request: GetRequest) -> Result<Bytes, R2ClientError> {
            self.requests
                .lock()
                .unwrap()
                .push(CapturedR2Request::Get(request));
            self.get_results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Ok(Bytes::from_static(b"data")))
        }

        async fn head(&self, request: ObjectRequest) -> Result<ObjectMetadata, R2ClientError> {
            self.requests
                .lock()
                .unwrap()
                .push(CapturedR2Request::Head(request));
            self.head_results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Err(R2ClientError::NotFound))
        }

        async fn delete(&self, request: ObjectRequest) -> Result<(), R2ClientError> {
            self.requests
                .lock()
                .unwrap()
                .push(CapturedR2Request::Delete(request));
            self.delete_results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Ok(()))
        }

        async fn presign_get(&self, request: PresignRequest) -> Result<Url, R2ClientError> {
            self.requests
                .lock()
                .unwrap()
                .push(CapturedR2Request::Presign(request));
            self.presign_results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| {
                    Url::parse("https://private.example.invalid/grant?signature=test")
                        .map_err(|_| R2ClientError::Failed)
                })
        }
    }

    fn config(public_bucket: &str, private_bucket: &str, base_url: &str) -> R2StorageConfig {
        R2StorageConfig::from_values(
            "account",
            "access",
            "secret",
            "auto",
            public_bucket,
            private_bucket,
            base_url,
        )
        .unwrap()
    }

    fn provider(client: Arc<CapturedR2Client>) -> R2StorageProvider {
        R2StorageProvider::with_transport(
            config(
                "public-assets",
                "private-files",
                "https://cdn.example.invalid/media/v1",
            ),
            client,
        )
    }

    #[tokio::test]
    async fn readiness_verifies_both_configured_buckets() {
        let client = Arc::new(CapturedR2Client::default());
        let provider = provider(Arc::clone(&client));

        provider.check_readiness().await.unwrap();

        assert_eq!(
            *client.requests.lock().unwrap(),
            vec![
                CapturedR2Request::Bucket("public-assets".to_string()),
                CapturedR2Request::Bucket("private-files".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn readiness_fails_closed_when_either_bucket_is_unavailable() {
        let client = Arc::new(CapturedR2Client::default());
        client
            .bucket_results
            .lock()
            .unwrap()
            .extend([Ok(()), Err(R2ClientError::Failed)]);
        let provider = provider(Arc::clone(&client));

        assert_eq!(
            provider.check_readiness().await,
            Err(StorageError::OperationFailed)
        );
        assert_eq!(client.requests.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn provider_operations_select_buckets_and_preserve_immutable_private_uploads() {
        let client = Arc::new(CapturedR2Client::default());
        client
            .head_results
            .lock()
            .unwrap()
            .push_back(Ok(ObjectMetadata {
                content_type: Some("image/png".to_string()),
                content_length: Some(4),
            }));
        let provider = provider(Arc::clone(&client));
        let private = object(FilePurpose::ProfileImage);

        provider
            .put(&private, Bytes::from_static(b"data"))
            .await
            .unwrap();
        assert_eq!(provider.get(&private, 4).await.unwrap(), b"data".as_slice());
        assert_eq!(
            provider
                .head(&private)
                .await
                .unwrap()
                .unwrap()
                .content_length,
            Some(4)
        );
        provider.delete(&private).await.unwrap();
        let grant = provider
            .private_download_grant(&private, "Résumé\r\nX-Evil: yes.pdf", Duration::MAX)
            .await
            .unwrap();

        let requests = client.requests.lock().unwrap();
        assert!(matches!(
            &requests[0],
            CapturedR2Request::Put(PutRequest { bucket, if_none_match, .. })
                if bucket == "private-files" && if_none_match == "*"
        ));
        assert!(
            matches!(&requests[1], CapturedR2Request::Get(GetRequest { bucket, max_bytes: 4, .. }) if bucket == "private-files")
        );
        assert!(
            matches!(&requests[2], CapturedR2Request::Head(ObjectRequest { bucket, .. }) if bucket == "private-files")
        );
        assert!(
            matches!(&requests[3], CapturedR2Request::Delete(ObjectRequest { bucket, .. }) if bucket == "private-files")
        );
        let CapturedR2Request::Presign(request) = &requests[4] else {
            panic!("expected presign request")
        };
        assert_eq!(request.bucket, "private-files");
        assert_eq!(request.ttl, MAX_PRIVATE_DOWNLOAD_GRANT_TTL);
        let filename = request
            .content_disposition
            .strip_prefix("attachment; filename=\"")
            .and_then(|value| value.strip_suffix('"'))
            .expect("content disposition must contain a quoted filename");
        assert!(!filename.contains(['\r', '\n', '"']));
        assert!(filename.is_ascii());
        let DownloadGrant::Redirect { expires_at, .. } = grant else {
            panic!("expected redirect")
        };
        assert_eq!(
            expires_at,
            request.issued_at + chrono::Duration::from_std(request.ttl).unwrap()
        );
    }

    #[tokio::test]
    async fn provider_bounds_internal_object_reads() {
        let client = Arc::new(CapturedR2Client::default());
        client
            .get_results
            .lock()
            .unwrap()
            .push_back(Ok(Bytes::from_static(b"five!")));
        let provider = provider(Arc::clone(&client));
        let private = object(FilePurpose::ProfileImage);

        assert_eq!(
            provider.get(&private, 4).await,
            Err(StorageError::ObjectTooLarge)
        );
        assert_eq!(
            provider.get(&private, 0).await,
            Err(StorageError::ObjectTooLarge)
        );
        assert_eq!(client.requests.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn provider_handles_missing_objects_and_safe_provider_failures() {
        let client = Arc::new(CapturedR2Client::default());
        client
            .put_results
            .lock()
            .unwrap()
            .push_back(Err(R2ClientError::AlreadyExists));
        client
            .delete_results
            .lock()
            .unwrap()
            .push_back(Err(R2ClientError::NotFound));
        let provider = provider(Arc::clone(&client));
        let public = object(FilePurpose::SchoolLogo);

        let error = provider.put(&public, Bytes::new()).await.unwrap_err();
        assert_eq!(error, StorageError::AlreadyExists);
        assert!(!error.to_string().contains(public.object_key.as_str()));
        assert_eq!(provider.head(&public).await.unwrap(), None);
        provider.delete(&public).await.unwrap();

        let location = provider.public_location(&public).unwrap();
        assert_eq!(location.path(), "/media/v1/tenants/11111111-1111-1111-1111-111111111111/school/logo/22222222-2222-2222-2222-222222222222/v1/original.png");
        let requests = client.requests.lock().unwrap();
        assert!(
            matches!(&requests[0], CapturedR2Request::Put(PutRequest { bucket, .. }) if bucket == "public-assets")
        );
        assert!(
            matches!(&requests[1], CapturedR2Request::Head(ObjectRequest { bucket, .. }) if bucket == "public-assets")
        );
        assert!(
            matches!(&requests[2], CapturedR2Request::Delete(ObjectRequest { bucket, .. }) if bucket == "public-assets")
        );
    }

    #[test]
    fn configuration_rejects_unsafe_or_ambiguous_bucket_topology() {
        for (public_bucket, private_bucket) in
            [("", "private"), ("public", "  "), ("same", " same ")]
        {
            assert!(R2StorageConfig::from_values(
                "account",
                "access",
                "secret",
                "auto",
                public_bucket,
                private_bucket,
                "https://cdn.example.invalid/files/"
            )
            .is_err());
        }
        for url in [
            "ftp://cdn.example.invalid/files/",
            "https://user:password@cdn.example.invalid/files/",
            "https://cdn.example.invalid/files/?token=secret",
            "https://cdn.example.invalid/files/#fragment",
        ] {
            assert!(R2StorageConfig::from_values(
                "account", "access", "secret", "auto", "public", "private", url
            )
            .is_err());
        }
    }

    #[test]
    fn configuration_rejects_example_placeholders() {
        for (account_id, access_key_id, secret_access_key, public_bucket, private_bucket, url) in [
            (
                "your-cloudflare-account-id",
                "access",
                "secret",
                "public",
                "private",
                "https://cdn.example.invalid/files/",
            ),
            (
                "account",
                "your-r2-access-key-id",
                "secret",
                "public",
                "private",
                "https://cdn.example.invalid/files/",
            ),
            (
                "account",
                "access",
                "change-this-r2-secret",
                "public",
                "private",
                "https://cdn.example.invalid/files/",
            ),
            (
                "account",
                "access",
                "secret",
                "your-public-bucket",
                "private",
                "https://cdn.example.invalid/files/",
            ),
            (
                "account",
                "access",
                "secret",
                "public",
                "your-private-bucket",
                "https://cdn.example.invalid/files/",
            ),
            (
                "account",
                "access",
                "secret",
                "public",
                "private",
                "https://pub-xxxxx.r2.dev",
            ),
        ] {
            assert!(R2StorageConfig::from_values(
                account_id,
                access_key_id,
                secret_access_key,
                "auto",
                public_bucket,
                private_bucket,
                url,
            )
            .is_err());
        }
    }

    #[test]
    fn configuration_rejects_invalid_bucket_names() {
        for bucket in [
            "ab",
            "Public-Files",
            "-private-files",
            "private-files-",
            "private files",
            "private_files",
        ] {
            assert!(R2StorageConfig::from_values(
                "account",
                "access",
                "secret",
                "auto",
                "public-files",
                bucket,
                "https://cdn.example.invalid/files/",
            )
            .is_err());
        }
    }

    #[tokio::test]
    async fn private_grant_rejects_zero_ttl_without_calling_transport() {
        let client = Arc::new(CapturedR2Client::default());
        let provider = provider(Arc::clone(&client));
        assert_eq!(
            provider
                .private_download_grant(
                    &object(FilePurpose::ProfileImage),
                    "file.pdf",
                    Duration::ZERO
                )
                .await,
            Err(StorageError::InvalidDownloadGrantTtl)
        );
        assert!(client.requests.lock().unwrap().is_empty());
    }

    #[test]
    fn conditional_write_conflicts_never_become_retries_or_leak_details() {
        for status in [409, 412] {
            assert_eq!(
                classify_write_status(Some(status)),
                R2ClientError::AlreadyExists
            );
        }
        assert_eq!(
            map_transport_error(R2ClientError::AlreadyExists),
            StorageError::AlreadyExists
        );
        assert!(!StorageError::AlreadyExists.to_string().contains("bucket"));
        assert!(!StorageError::AlreadyExists.log_safe_code().contains("key"));
    }
}
