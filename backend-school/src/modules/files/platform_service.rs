use bytes::Bytes;
use std::{fmt, sync::Arc};
use url::Url;
use uuid::Uuid;

use crate::utils::{file_hash::FileHasher, file_processor::ImageProcessor};

use super::{
    file_inspector::{inspect_file, FileInspectionError, ValidatedFile},
    malware_scanner::{MalwareScanner, ScanOutcome},
    platform_types::{
        DerivativeRecipe, DownloadGrant, FileLifecycleStatus, FilePurpose, FileVisibility,
    },
    purpose_registry::{
        derivative_object_key, original_object_key, purpose_definition, PurposeRegistryError,
    },
    repository::{
        derivative_is_required, DeleteWork, DeliveryRecord, FileRepository, NewDerivative,
        NewUpload, ObjectTarget, PlatformFile, RepositoryError,
    },
    runtime_config::FilePlatformRuntimeConfig,
    storage_provider::{StorageError, StorageProvider, StoredObject},
};

#[derive(Clone)]
pub struct UploadCommand {
    pub tenant_id: Uuid,
    pub actor_user_id: Option<Uuid>,
    pub owner_user_id: Option<Uuid>,
    pub purpose: FilePurpose,
    pub display_filename: String,
    pub bytes: Bytes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicDelivery {
    pub location: Url,
    pub content_type: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeleteOutcome {
    pub pending_retry: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilePlatformError {
    InspectionRejected,
    MalwareDetected,
    ScannerUnavailable,
    MetadataUnavailable,
    StorageUnavailable,
    RequiredDerivativeUnavailable,
    NotFound,
    NotReady,
    VisibilityMismatch,
}

impl FilePlatformError {
    pub const fn log_safe_code(self) -> &'static str {
        match self {
            Self::InspectionRejected => "file_inspection_rejected",
            Self::MalwareDetected => "file_malware_detected",
            Self::ScannerUnavailable => "file_scanner_unavailable",
            Self::MetadataUnavailable => "file_metadata_unavailable",
            Self::StorageUnavailable => "file_storage_unavailable",
            Self::RequiredDerivativeUnavailable => "file_required_derivative_unavailable",
            Self::NotFound => "file_not_found",
            Self::NotReady => "file_not_ready",
            Self::VisibilityMismatch => "file_visibility_mismatch",
        }
    }
}

impl fmt::Display for FilePlatformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.log_safe_code())
    }
}

impl std::error::Error for FilePlatformError {}

impl From<FileInspectionError> for FilePlatformError {
    fn from(_: FileInspectionError) -> Self {
        Self::InspectionRejected
    }
}

impl From<PurposeRegistryError> for FilePlatformError {
    fn from(_: PurposeRegistryError) -> Self {
        Self::InspectionRejected
    }
}

impl From<RepositoryError> for FilePlatformError {
    fn from(_: RepositoryError) -> Self {
        Self::MetadataUnavailable
    }
}

impl From<StorageError> for FilePlatformError {
    fn from(_: StorageError) -> Self {
        Self::StorageUnavailable
    }
}

pub struct FilePlatform {
    provider: Arc<dyn StorageProvider>,
    scanner: Arc<dyn MalwareScanner>,
    runtime_config: FilePlatformRuntimeConfig,
}

impl FilePlatform {
    pub fn new(provider: Arc<dyn StorageProvider>, scanner: Arc<dyn MalwareScanner>) -> Self {
        Self::with_config(provider, scanner, FilePlatformRuntimeConfig::default())
    }

    pub fn with_config(
        provider: Arc<dyn StorageProvider>,
        scanner: Arc<dyn MalwareScanner>,
        runtime_config: FilePlatformRuntimeConfig,
    ) -> Self {
        Self {
            provider,
            scanner,
            runtime_config,
        }
    }

    pub async fn upload(
        &self,
        repository: &dyn FileRepository,
        command: UploadCommand,
    ) -> Result<PlatformFile, FilePlatformError> {
        let definition = purpose_definition(command.purpose)?;
        let inspected = inspect_file(command.purpose, &command.bytes)?;

        match self.scanner.scan(&command.bytes).await {
            ScanOutcome::Clean => {}
            ScanOutcome::Infected => return Err(FilePlatformError::MalwareDetected),
            ScanOutcome::Unavailable | ScanOutcome::Timeout | ScanOutcome::MalformedResponse => {
                return Err(FilePlatformError::ScannerUnavailable);
            }
        }

        let file_id = Uuid::new_v4();
        let version_id = Uuid::new_v4();
        let original_key = original_object_key(
            command.tenant_id,
            command.purpose,
            file_id,
            1,
            inspected.detected_content(),
        )?;
        let original = StoredObject::new(original_key, inspected.canonical_mime_type());
        let derivatives =
            prepare_derivatives(command.tenant_id, command.purpose, file_id, &inspected)?;
        let required_derivative_ids = derivatives
            .iter()
            .filter(|derivative| derivative.metadata.required)
            .map(|derivative| derivative.metadata.id)
            .collect::<Vec<_>>();
        let byte_size = i64::try_from(command.bytes.len())
            .map_err(|_| FilePlatformError::InspectionRejected)?;
        let display_filename = sanitized_display_filename(&command.display_filename);
        let upload = NewUpload {
            file_id,
            version_id,
            reconcile_operation_id: Uuid::new_v4(),
            scan_operation_id: Uuid::new_v4(),
            owner_user_id: command.owner_user_id,
            created_by: command.actor_user_id,
            purpose: command.purpose,
            visibility: definition.visibility,
            retention_class: definition.retention_class,
            display_filename: display_filename.clone(),
            original: original.clone(),
            byte_size,
            checksum: FileHasher::sha256(&command.bytes),
            inspection_metadata: inspected.metadata().clone(),
            derivatives: derivatives
                .iter()
                .map(|derivative| derivative.metadata.clone())
                .collect(),
        };

        repository.reserve_upload(&upload).await?;

        if let Err(error) = self.provider.put(&original, command.bytes.clone()).await {
            let _ = repository
                .mark_upload_failed(file_id, version_id, error.log_safe_code())
                .await;
            return Err(FilePlatformError::StorageUnavailable);
        }
        if repository
            .mark_original_stored(file_id, version_id)
            .await
            .is_err()
        {
            let _ = repository
                .mark_reconcile_pending(
                    file_id,
                    "file_original_finalize_failed",
                    self.runtime_config.retry_delay(1),
                )
                .await;
            return Err(FilePlatformError::MetadataUnavailable);
        }

        let mut required_derivative_failed = false;
        for derivative in derivatives {
            match self
                .provider
                .put(&derivative.metadata.object, derivative.body)
                .await
            {
                Ok(()) => {
                    if repository
                        .mark_derivative_stored(
                            file_id,
                            derivative.metadata.id,
                            derivative.metadata.operation_id,
                        )
                        .await
                        .is_err()
                    {
                        let _ = repository
                            .mark_reconcile_pending(
                                file_id,
                                "file_derivative_finalize_failed",
                                self.runtime_config.retry_delay(1),
                            )
                            .await;
                        if derivative.metadata.required {
                            required_derivative_failed = true;
                        }
                    }
                }
                Err(error) => {
                    let _ = repository
                        .mark_derivative_failed(
                            file_id,
                            derivative.metadata.id,
                            derivative.metadata.operation_id,
                            error.log_safe_code(),
                            self.runtime_config.retry_delay(1),
                            false,
                        )
                        .await;
                    if derivative.metadata.required {
                        required_derivative_failed = true;
                    }
                }
            }
        }

        if required_derivative_failed {
            return Err(FilePlatformError::RequiredDerivativeUnavailable);
        }

        if repository
            .finalize_ready(file_id, version_id, &required_derivative_ids)
            .await
            .is_err()
        {
            let _ = repository
                .mark_reconcile_pending(
                    file_id,
                    "file_ready_finalize_failed",
                    self.runtime_config.retry_delay(1),
                )
                .await;
            return Err(FilePlatformError::MetadataUnavailable);
        }

        Ok(PlatformFile {
            id: file_id,
            owner_user_id: command.owner_user_id,
            purpose: command.purpose,
            visibility: definition.visibility,
            lifecycle_status: FileLifecycleStatus::Ready,
            current_version: Some(1),
            display_filename,
            detected_mime_type: original.content_type,
            byte_size,
        })
    }

    pub async fn public_delivery(
        &self,
        repository: &dyn FileRepository,
        file_id: Uuid,
    ) -> Result<PublicDelivery, FilePlatformError> {
        let delivery = ready_delivery(repository, file_id).await?;
        if delivery.file.visibility != FileVisibility::Public {
            return Err(FilePlatformError::VisibilityMismatch);
        }
        let object = delivery
            .object
            .ok_or(FilePlatformError::MetadataUnavailable)?;
        Ok(PublicDelivery {
            location: self.provider.public_location(&object)?,
            content_type: object.content_type,
        })
    }

    pub async fn metadata(
        &self,
        repository: &dyn FileRepository,
        file_id: Uuid,
    ) -> Result<PlatformFile, FilePlatformError> {
        repository
            .load_delivery(file_id)
            .await?
            .map(|delivery| delivery.file)
            .ok_or(FilePlatformError::NotFound)
    }

    pub async fn private_download(
        &self,
        repository: &dyn FileRepository,
        file_id: Uuid,
    ) -> Result<DownloadGrant, FilePlatformError> {
        let delivery = ready_delivery(repository, file_id).await?;
        self.private_download_delivery(delivery).await
    }

    pub(crate) async fn private_download_delivery(
        &self,
        delivery: DeliveryRecord,
    ) -> Result<DownloadGrant, FilePlatformError> {
        if delivery.file.lifecycle_status != FileLifecycleStatus::Ready {
            return Err(FilePlatformError::NotReady);
        }
        if delivery.file.visibility != FileVisibility::Private {
            return Err(FilePlatformError::VisibilityMismatch);
        }
        let object = delivery
            .object
            .ok_or(FilePlatformError::MetadataUnavailable)?;
        self.provider
            .private_download_grant(
                &object,
                &delivery.file.display_filename,
                self.runtime_config.private_download_grant_ttl,
            )
            .await
            .map_err(Into::into)
    }

    pub async fn request_delete(
        &self,
        repository: &dyn FileRepository,
        file_id: Uuid,
    ) -> Result<DeleteOutcome, FilePlatformError> {
        let work = repository.request_delete(file_id).await?;
        self.complete_prepared_delete(repository, work).await
    }

    /// Completes provider cleanup for a lifecycle transition that was committed
    /// by a domain transaction. Failures remain durable retry operations.
    pub async fn complete_prepared_delete(
        &self,
        repository: &dyn FileRepository,
        work: Vec<DeleteWork>,
    ) -> Result<DeleteOutcome, FilePlatformError> {
        let mut pending_retry = false;
        for object in work {
            if let Err(error) = self.provider.delete(&object.object).await {
                pending_retry = true;
                let _ = repository
                    .retry_operation(
                        object.operation_id,
                        error.log_safe_code(),
                        self.runtime_config.retry_delay(1),
                        false,
                    )
                    .await;
                continue;
            }
            if repository.mark_delete_succeeded(&object).await.is_err() {
                pending_retry = true;
            }
        }
        Ok(DeleteOutcome { pending_retry })
    }

    pub async fn check_readiness(&self) -> Result<(), FilePlatformError> {
        let (storage, scan) = tokio::join!(self.provider.check_readiness(), self.scanner.scan(&[]));
        storage.map_err(|_| FilePlatformError::StorageUnavailable)?;
        match scan {
            ScanOutcome::Clean => Ok(()),
            _ => Err(FilePlatformError::ScannerUnavailable),
        }
    }

    pub(crate) fn provider(&self) -> &Arc<dyn StorageProvider> {
        &self.provider
    }

    pub(crate) const fn runtime_config(&self) -> FilePlatformRuntimeConfig {
        self.runtime_config
    }

    pub(crate) async fn commit_reconciled_derivative(
        &self,
        repository: &dyn FileRepository,
        file_id: Uuid,
        derivative_id: Uuid,
        operation_id: Uuid,
        target: &StoredObject,
    ) -> Result<bool, RepositoryError> {
        match repository
            .mark_derivative_stored(file_id, derivative_id, operation_id)
            .await
        {
            Ok(()) => Ok(true),
            Err(RepositoryError::MaterializationRevoked) => {
                if let Err(error) = self.provider.delete(target).await {
                    repository
                        .queue_delete_retry(
                            file_id,
                            ObjectTarget::Derivative(derivative_id),
                            error.log_safe_code(),
                            self.runtime_config.retry_delay(1),
                        )
                        .await?;
                }
                Ok(false)
            }
            Err(error) => Err(error),
        }
    }
}

async fn ready_delivery(
    repository: &dyn FileRepository,
    file_id: Uuid,
) -> Result<DeliveryRecord, FilePlatformError> {
    let delivery = repository
        .load_delivery(file_id)
        .await?
        .ok_or(FilePlatformError::NotFound)?;
    if delivery.file.lifecycle_status != FileLifecycleStatus::Ready {
        return Err(FilePlatformError::NotReady);
    }
    Ok(delivery)
}

struct PreparedDerivative {
    metadata: NewDerivative,
    body: Bytes,
}

fn prepare_derivatives(
    tenant_id: Uuid,
    purpose: FilePurpose,
    file_id: Uuid,
    inspected: &ValidatedFile<'_>,
) -> Result<Vec<PreparedDerivative>, FilePlatformError> {
    let definition = purpose_definition(purpose)?;
    if definition.derivatives.is_empty() {
        return Ok(Vec::new());
    }
    let image = ImageProcessor::decode_inspected_image(inspected)
        .map_err(|_| FilePlatformError::InspectionRejected)?;
    let mut derivatives = Vec::with_capacity(definition.derivatives.len());
    for recipe in definition.derivatives {
        let bytes = encode_derivative_image(&image, *recipe)?;
        let derivative_id = Uuid::new_v4();
        derivatives.push(PreparedDerivative {
            metadata: NewDerivative {
                id: derivative_id,
                operation_id: Uuid::new_v4(),
                recipe: *recipe,
                object: StoredObject::new(
                    derivative_object_key(tenant_id, purpose, file_id, 1, *recipe)?,
                    recipe.detected_content().mime_type(),
                ),
                byte_size: i64::try_from(bytes.len())
                    .map_err(|_| FilePlatformError::InspectionRejected)?,
                checksum: FileHasher::sha256(&bytes),
                required: derivative_is_required(purpose, *recipe),
            },
            body: bytes.into(),
        });
    }
    Ok(derivatives)
}

pub(crate) fn generate_derivative_body(
    purpose: FilePurpose,
    recipe: DerivativeRecipe,
    source: &[u8],
) -> Result<Bytes, FilePlatformError> {
    let inspected = inspect_file(purpose, source)?;
    let image = ImageProcessor::decode_inspected_image(&inspected)
        .map_err(|_| FilePlatformError::InspectionRejected)?;
    encode_derivative_image(&image, recipe).map(Into::into)
}

fn encode_derivative_image(
    image: &image::DynamicImage,
    recipe: DerivativeRecipe,
) -> Result<Vec<u8>, FilePlatformError> {
    let maximum = match recipe {
        DerivativeRecipe::Thumbnail256Webp => 256,
        DerivativeRecipe::Thumbnail1024Webp => 1024,
    };
    let resized = image.resize(maximum, maximum, image::imageops::FilterType::Lanczos3);
    ImageProcessor::encode_webp(&resized).map_err(|_| FilePlatformError::InspectionRejected)
}

fn sanitized_display_filename(filename: &str) -> String {
    let filename = filename
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .chars()
        .take(128)
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | ' ' | '.' | '-' | '_' => character,
            _ => '_',
        })
        .collect::<String>();
    let filename = filename.trim_matches([' ', '.']);
    if filename.is_empty() {
        "upload".to_string()
    } else {
        filename.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::{collections::VecDeque, io::Cursor, sync::Mutex, time::Duration};

    use crate::modules::files::{
        malware_scanner::MalwareScanner,
        platform_types::{FileInspectionMetadata, StorageClass},
        purpose_registry::original_object_key,
        reconciler::reconcile_due_operations,
        repository::{DeleteWork, LeasedOperation, ObjectTarget, OperationWork, RepositoryError},
        storage_provider::{ObjectMetadata, StorageError},
    };

    fn png() -> Bytes {
        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([4, 8, 15, 255])));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes.into()
    }

    struct FakeScanner(ScanOutcome);

    #[async_trait]
    impl MalwareScanner for FakeScanner {
        async fn scan(&self, _content: &[u8]) -> ScanOutcome {
            self.0
        }
    }

    #[derive(Default)]
    struct FakeProvider {
        readiness_error: Mutex<Option<StorageError>>,
        put_results: Mutex<VecDeque<Result<(), StorageError>>>,
        delete_results: Mutex<VecDeque<Result<(), StorageError>>>,
        puts: Mutex<Vec<StorageClass>>,
        deletes: Mutex<usize>,
    }

    #[async_trait]
    impl StorageProvider for FakeProvider {
        async fn check_readiness(&self) -> Result<(), StorageError> {
            match *self.readiness_error.lock().unwrap() {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }

        async fn put(&self, object: &StoredObject, _body: Bytes) -> Result<(), StorageError> {
            self.puts.lock().unwrap().push(object.storage_class());
            self.put_results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Ok(()))
        }

        async fn get(
            &self,
            _object: &StoredObject,
            _max_bytes: u64,
        ) -> Result<Bytes, StorageError> {
            Ok(png())
        }

        async fn head(
            &self,
            _object: &StoredObject,
        ) -> Result<Option<ObjectMetadata>, StorageError> {
            Ok(Some(ObjectMetadata {
                content_type: Some("image/png".to_string()),
                content_length: Some(1),
            }))
        }

        async fn delete(&self, _object: &StoredObject) -> Result<(), StorageError> {
            *self.deletes.lock().unwrap() += 1;
            self.delete_results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Ok(()))
        }

        async fn private_download_grant(
            &self,
            _object: &StoredObject,
            _filename: &str,
            _ttl: Duration,
        ) -> Result<DownloadGrant, StorageError> {
            Ok(DownloadGrant::Stream {
                content_type: "image/png".to_string(),
                content_length: Some(1),
            })
        }

        fn public_location(&self, _object: &StoredObject) -> Result<Url, StorageError> {
            Url::parse("https://public.example.invalid/file")
                .map_err(|_| StorageError::OperationFailed)
        }
    }

    #[derive(Default)]
    struct FakeRepository {
        events: Mutex<Vec<&'static str>>,
        upload_identity: Mutex<Option<(Option<Uuid>, Option<Uuid>)>>,
        upload_inspection: Mutex<Option<FileInspectionMetadata>>,
        finalize_fails: Mutex<bool>,
        derivative_store_error: Mutex<Option<RepositoryError>>,
        delivery: Mutex<Option<DeliveryRecord>>,
        delete_work: Mutex<Vec<DeleteWork>>,
        leased_operations: Mutex<Vec<LeasedOperation>>,
        lease_durations: Mutex<Vec<Duration>>,
        retry_delays: Mutex<Vec<Duration>>,
    }

    #[async_trait]
    impl FileRepository for FakeRepository {
        async fn reserve_upload(&self, upload: &NewUpload) -> Result<(), RepositoryError> {
            *self.upload_identity.lock().unwrap() = Some((upload.owner_user_id, upload.created_by));
            *self.upload_inspection.lock().unwrap() = Some(upload.inspection_metadata.clone());
            self.events.lock().unwrap().push("processing");
            Ok(())
        }

        async fn mark_original_stored(
            &self,
            _file_id: Uuid,
            _version_id: Uuid,
        ) -> Result<(), RepositoryError> {
            self.events.lock().unwrap().push("original_stored");
            Ok(())
        }

        async fn mark_upload_failed(
            &self,
            _file_id: Uuid,
            _version_id: Uuid,
            _error_code: &'static str,
        ) -> Result<(), RepositoryError> {
            self.events.lock().unwrap().push("upload_failed");
            Ok(())
        }

        async fn mark_derivative_stored(
            &self,
            _file_id: Uuid,
            _derivative_id: Uuid,
            _operation_id: Uuid,
        ) -> Result<(), RepositoryError> {
            if let Some(error) = self.derivative_store_error.lock().unwrap().take() {
                return Err(error);
            }
            self.events.lock().unwrap().push("derivative_stored");
            Ok(())
        }

        async fn mark_derivative_failed(
            &self,
            _file_id: Uuid,
            _derivative_id: Uuid,
            _operation_id: Uuid,
            _error_code: &'static str,
            retry_delay: Duration,
            _terminal: bool,
        ) -> Result<(), RepositoryError> {
            self.retry_delays.lock().unwrap().push(retry_delay);
            self.events.lock().unwrap().push("derivative_retry");
            Ok(())
        }

        async fn finalize_ready(
            &self,
            _file_id: Uuid,
            _version_id: Uuid,
            _required_derivative_ids: &[Uuid],
        ) -> Result<(), RepositoryError> {
            if *self.finalize_fails.lock().unwrap() {
                return Err(RepositoryError::OperationFailed);
            }
            self.events.lock().unwrap().push("ready");
            Ok(())
        }

        async fn mark_reconcile_pending(
            &self,
            _file_id: Uuid,
            _error_code: &'static str,
            retry_delay: Duration,
        ) -> Result<(), RepositoryError> {
            self.retry_delays.lock().unwrap().push(retry_delay);
            self.events.lock().unwrap().push("reconcile_pending");
            Ok(())
        }

        async fn load_delivery(
            &self,
            _file_id: Uuid,
        ) -> Result<Option<DeliveryRecord>, RepositoryError> {
            Ok(self.delivery.lock().unwrap().clone())
        }

        async fn request_delete(&self, _file_id: Uuid) -> Result<Vec<DeleteWork>, RepositoryError> {
            self.events.lock().unwrap().push("delete_requested");
            Ok(self.delete_work.lock().unwrap().clone())
        }

        async fn mark_delete_succeeded(&self, _work: &DeleteWork) -> Result<(), RepositoryError> {
            self.events.lock().unwrap().push("deleted");
            Ok(())
        }

        async fn lease_due_operations(
            &self,
            _worker: &str,
            lease_duration: Duration,
            _limit: i64,
        ) -> Result<Vec<LeasedOperation>, RepositoryError> {
            self.lease_durations.lock().unwrap().push(lease_duration);
            Ok(std::mem::take(&mut *self.leased_operations.lock().unwrap()))
        }

        async fn retry_operation(
            &self,
            _operation_id: Uuid,
            _error_code: &'static str,
            retry_delay: Duration,
            _terminal: bool,
        ) -> Result<(), RepositoryError> {
            self.retry_delays.lock().unwrap().push(retry_delay);
            self.events.lock().unwrap().push("operation_retry");
            Ok(())
        }

        async fn queue_delete_retry(
            &self,
            _file_id: Uuid,
            _target: ObjectTarget,
            _error_code: &'static str,
            retry_delay: Duration,
        ) -> Result<(), RepositoryError> {
            self.retry_delays.lock().unwrap().push(retry_delay);
            self.events.lock().unwrap().push("delete_retry_queued");
            Ok(())
        }
    }

    fn platform(scan: ScanOutcome, provider: Arc<FakeProvider>) -> FilePlatform {
        FilePlatform::new(provider, Arc::new(FakeScanner(scan)))
    }

    #[tokio::test]
    async fn readiness_requires_both_storage_and_scanner() {
        let storage_unavailable = Arc::new(FakeProvider::default());
        *storage_unavailable.readiness_error.lock().unwrap() = Some(StorageError::OperationFailed);
        assert_eq!(
            platform(ScanOutcome::Clean, storage_unavailable)
                .check_readiness()
                .await,
            Err(FilePlatformError::StorageUnavailable)
        );

        assert_eq!(
            platform(ScanOutcome::Unavailable, Arc::new(FakeProvider::default()))
                .check_readiness()
                .await,
            Err(FilePlatformError::ScannerUnavailable)
        );
    }

    fn command(purpose: FilePurpose) -> UploadCommand {
        UploadCommand {
            tenant_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            actor_user_id: Some(Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap()),
            owner_user_id: Some(Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap()),
            purpose,
            display_filename: "../../unsafe ชื่อ.png".to_string(),
            bytes: png(),
        }
    }

    #[tokio::test]
    async fn successful_upload_transitions_processing_to_ready() {
        let provider = Arc::new(FakeProvider::default());
        let repository = FakeRepository::default();
        let file = platform(ScanOutcome::Clean, Arc::clone(&provider))
            .upload(&repository, command(FilePurpose::SchoolLogo))
            .await
            .unwrap();

        assert_eq!(file.lifecycle_status, FileLifecycleStatus::Ready);
        assert!(!file.display_filename.contains(['/', '\\']));
        assert_eq!(
            *repository.events.lock().unwrap(),
            vec![
                "processing",
                "original_stored",
                "derivative_stored",
                "ready"
            ]
        );
        assert_eq!(
            *provider.puts.lock().unwrap(),
            vec![StorageClass::Public, StorageClass::Public]
        );
        assert_eq!(
            *repository.upload_inspection.lock().unwrap(),
            Some(FileInspectionMetadata::Image {
                width_px: 2,
                height_px: 2,
            })
        );
    }

    #[tokio::test]
    async fn portal_upload_can_persist_without_a_user_identity() {
        let provider = Arc::new(FakeProvider::default());
        let repository = FakeRepository::default();
        let mut command = command(FilePurpose::AdmissionApplicationDocument);
        command.actor_user_id = None;
        command.owner_user_id = None;

        let file = platform(ScanOutcome::Clean, provider)
            .upload(&repository, command)
            .await
            .unwrap();

        assert_eq!(file.owner_user_id, None);
        assert_eq!(
            *repository.upload_identity.lock().unwrap(),
            Some((None, None))
        );
    }

    #[tokio::test]
    async fn unsafe_scan_outcomes_never_reserve_or_write_public_objects() {
        for outcome in [
            ScanOutcome::Infected,
            ScanOutcome::Unavailable,
            ScanOutcome::Timeout,
            ScanOutcome::MalformedResponse,
        ] {
            let provider = Arc::new(FakeProvider::default());
            let repository = FakeRepository::default();
            assert!(platform(outcome, Arc::clone(&provider))
                .upload(&repository, command(FilePurpose::SchoolLogo))
                .await
                .is_err());
            assert!(repository.events.lock().unwrap().is_empty());
            assert!(provider.puts.lock().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn provider_or_finalize_failures_leave_durable_repair_state() {
        let provider = Arc::new(FakeProvider::default());
        provider
            .put_results
            .lock()
            .unwrap()
            .push_back(Err(StorageError::OperationFailed));
        let repository = FakeRepository::default();
        assert_eq!(
            platform(ScanOutcome::Clean, Arc::clone(&provider))
                .upload(&repository, command(FilePurpose::ProfileImage))
                .await,
            Err(FilePlatformError::StorageUnavailable)
        );
        assert_eq!(
            *repository.events.lock().unwrap(),
            vec!["processing", "upload_failed"]
        );

        let provider = Arc::new(FakeProvider::default());
        let repository = FakeRepository::default();
        *repository.finalize_fails.lock().unwrap() = true;
        assert_eq!(
            platform(ScanOutcome::Clean, provider)
                .upload(&repository, command(FilePurpose::ProfileImage))
                .await,
            Err(FilePlatformError::MetadataUnavailable)
        );
        assert!(repository
            .events
            .lock()
            .unwrap()
            .contains(&"reconcile_pending"));
        assert_eq!(
            *repository.retry_delays.lock().unwrap(),
            vec![FilePlatformRuntimeConfig::default().retry_delay(1)]
        );
    }

    #[tokio::test]
    async fn reconciler_passes_relative_lease_and_retry_durations() {
        let provider = Arc::new(FakeProvider::default());
        provider
            .delete_results
            .lock()
            .unwrap()
            .push_back(Err(StorageError::OperationFailed));
        let repository = FakeRepository::default();
        let file_id = Uuid::new_v4();
        let object = StoredObject::new(
            original_object_key(
                Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
                FilePurpose::ProfileImage,
                file_id,
                1,
                crate::modules::files::platform_types::DetectedContent::Png,
            )
            .unwrap(),
            "image/png",
        );
        repository
            .leased_operations
            .lock()
            .unwrap()
            .push(LeasedOperation {
                id: Uuid::new_v4(),
                file_id,
                attempt_count: 1,
                work: OperationWork::DeleteObject(DeleteWork {
                    operation_id: Uuid::new_v4(),
                    file_id,
                    target: ObjectTarget::Version(Uuid::new_v4()),
                    object,
                }),
            });
        let platform = platform(ScanOutcome::Clean, provider);

        let summary = reconcile_due_operations(&platform, &repository, "worker-one")
            .await
            .unwrap();

        assert_eq!(summary.leased, 1);
        assert_eq!(summary.retried, 1);
        assert_eq!(
            *repository.lease_durations.lock().unwrap(),
            vec![FilePlatformRuntimeConfig::default().reconciliation_lease]
        );
        assert_eq!(
            *repository.retry_delays.lock().unwrap(),
            vec![FilePlatformRuntimeConfig::default().retry_delay(1)]
        );
    }

    #[tokio::test]
    async fn required_derivative_failure_blocks_ready_but_optional_failure_does_not() {
        let required_provider = Arc::new(FakeProvider::default());
        required_provider
            .put_results
            .lock()
            .unwrap()
            .extend([Ok(()), Err(StorageError::OperationFailed)]);
        let required_repository = FakeRepository::default();
        assert_eq!(
            platform(ScanOutcome::Clean, required_provider)
                .upload(&required_repository, command(FilePurpose::SchoolLogo),)
                .await,
            Err(FilePlatformError::RequiredDerivativeUnavailable)
        );
        assert!(!required_repository
            .events
            .lock()
            .unwrap()
            .contains(&"ready"));

        let optional_provider = Arc::new(FakeProvider::default());
        optional_provider
            .put_results
            .lock()
            .unwrap()
            .extend([Ok(()), Err(StorageError::OperationFailed)]);
        let optional_repository = FakeRepository::default();
        assert!(platform(ScanOutcome::Clean, optional_provider)
            .upload(&optional_repository, command(FilePurpose::ProfileImage),)
            .await
            .is_ok());
        assert_eq!(
            *optional_repository.events.lock().unwrap(),
            vec!["processing", "original_stored", "derivative_retry", "ready"]
        );
    }

    #[tokio::test]
    async fn delivery_rejects_non_ready_files_before_provider_access() {
        let provider = Arc::new(FakeProvider::default());
        let repository = FakeRepository::default();
        *repository.delivery.lock().unwrap() = Some(DeliveryRecord {
            file: PlatformFile {
                id: Uuid::new_v4(),
                owner_user_id: Some(Uuid::new_v4()),
                purpose: FilePurpose::SchoolLogo,
                visibility: FileVisibility::Public,
                lifecycle_status: FileLifecycleStatus::Processing,
                current_version: None,
                display_filename: "logo.png".to_string(),
                detected_mime_type: String::new(),
                byte_size: 1,
            },
            object: None,
        });
        let platform = platform(ScanOutcome::Clean, provider);
        assert_eq!(
            platform.public_delivery(&repository, Uuid::new_v4()).await,
            Err(FilePlatformError::NotReady)
        );
        assert_eq!(
            platform.private_download(&repository, Uuid::new_v4()).await,
            Err(FilePlatformError::NotReady)
        );
    }

    #[tokio::test]
    async fn delete_revokes_first_and_retries_provider_failure() {
        let provider = Arc::new(FakeProvider::default());
        provider
            .delete_results
            .lock()
            .unwrap()
            .extend([Err(StorageError::OperationFailed), Ok(())]);
        let repository = FakeRepository::default();
        let file_id = Uuid::new_v4();
        let object = StoredObject::new(
            original_object_key(
                Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
                FilePurpose::ProfileImage,
                file_id,
                1,
                crate::modules::files::platform_types::DetectedContent::Png,
            )
            .unwrap(),
            "image/png",
        );
        *repository.delete_work.lock().unwrap() = vec![DeleteWork {
            operation_id: Uuid::new_v4(),
            file_id,
            target: ObjectTarget::Version(Uuid::new_v4()),
            object,
        }];

        let platform = platform(ScanOutcome::Clean, provider);
        assert!(
            platform
                .request_delete(&repository, file_id)
                .await
                .unwrap()
                .pending_retry
        );
        assert!(
            !platform
                .request_delete(&repository, file_id)
                .await
                .unwrap()
                .pending_retry
        );
        assert_eq!(
            *repository.events.lock().unwrap(),
            vec![
                "delete_requested",
                "operation_retry",
                "delete_requested",
                "deleted"
            ]
        );
    }

    #[tokio::test]
    async fn late_derivative_write_is_compensated_and_cleanup_failure_is_durable() {
        let provider = Arc::new(FakeProvider::default());
        provider
            .delete_results
            .lock()
            .unwrap()
            .push_back(Err(StorageError::OperationFailed));
        let repository = FakeRepository::default();
        *repository.derivative_store_error.lock().unwrap() =
            Some(RepositoryError::MaterializationRevoked);
        let file_id = Uuid::new_v4();
        let derivative_id = Uuid::new_v4();
        let target = StoredObject::new(
            crate::modules::files::purpose_registry::derivative_object_key(
                Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
                FilePurpose::ProfileImage,
                file_id,
                1,
                DerivativeRecipe::Thumbnail256Webp,
            )
            .unwrap(),
            "image/webp",
        );
        let platform = platform(ScanOutcome::Clean, Arc::clone(&provider));

        assert!(!platform
            .commit_reconciled_derivative(
                &repository,
                file_id,
                derivative_id,
                Uuid::new_v4(),
                &target,
            )
            .await
            .unwrap());
        assert_eq!(*provider.deletes.lock().unwrap(), 1);
        assert_eq!(
            *repository.events.lock().unwrap(),
            vec!["delete_retry_queued"]
        );
    }
}
