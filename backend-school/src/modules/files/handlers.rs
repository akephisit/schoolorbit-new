use axum::{
    extract::{multipart::Field, Extension, Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use bytes::{Bytes, BytesMut};
use uuid::Uuid;

use crate::{
    api_response::{ApiErrorResponse, ApiResponse},
    error::AppError,
    modules::auth::session_service::AuthenticatedSession,
    policies::file_access_policy::{self, FilePolicyAction},
    utils::{request_context::actor_tenant_context_from_session, tenant::tenant_context},
    AppState,
};

use super::{
    consumer_service::{
        map_platform_error, record_certificate_school_font_upload,
        record_certificate_template_upload, record_school_font_upload, request_deletions,
    },
    models::{
        FileAccessQuery, FileDeleteResult, FileDownloadGrantResponse, FileMetadata,
        FileUploadMultipart, PublicFileDeliveryResponse,
    },
    platform_service::{FilePlatform, FilePlatformError, UploadCommand},
    platform_types::FilePurpose,
    purpose_registry::{purpose_definition, purpose_from_code},
    repository::SqlFileRepository,
};

const MAX_CONTROL_FIELD_BYTES: usize = 128;

#[utoipa::path(
    post,
    path = "/api/files",
    operation_id = "uploadFile",
    tag = "files",
    request_body(content = FileUploadMultipart, content_type = "multipart/form-data"),
    responses(
        (status = 201, description = "Validated file uploaded", body = ApiResponse<FileMetadata>),
        (status = 400, description = "Invalid multipart content or purpose", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "File purpose policy denied", body = ApiErrorResponse),
        (status = 503, description = "Scanner or storage unavailable", body = ApiErrorResponse)
    )
)]
pub async fn upload_file(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let mut purpose = None;
    let mut resource_id = None;
    let mut upload = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| invalid_multipart())?
    {
        let field_name = field.name().unwrap_or_default().to_string();
        match field_name.as_str() {
            "purpose" if upload.is_none() && purpose.is_none() => {
                let value = read_control_field(field).await?;
                purpose = Some(
                    purpose_from_code(value.trim())
                        .map_err(|_| AppError::BadRequest("purpose ไม่ถูกต้อง".to_string()))?,
                );
            }
            "resource_id" if upload.is_none() && resource_id.is_none() => {
                let value = read_control_field(field).await?;
                resource_id = Some(
                    Uuid::parse_str(value.trim())
                        .map_err(|_| AppError::BadRequest("resourceId ไม่ถูกต้อง".to_string()))?,
                );
            }
            "file" if upload.is_none() => {
                let purpose = purpose
                    .ok_or_else(|| AppError::BadRequest("ต้องส่ง purpose ก่อน file".to_string()))?;
                let owner_user_id = file_access_policy::authorize_create(
                    &context.tenant.pool,
                    &context.actor,
                    purpose,
                    resource_id,
                )
                .await?;
                let filename = field.file_name().unwrap_or("upload").to_string();
                let limit = purpose_definition(purpose)
                    .map_err(|_| AppError::BadRequest("purpose ไม่ถูกต้อง".to_string()))?
                    .limits
                    .max_bytes;
                let bytes = read_file_field(field, limit).await?;
                upload = Some((purpose, resource_id, owner_user_id, filename, bytes));
            }
            _ => return Err(invalid_multipart()),
        }
    }

    let (purpose, resource_id, owner_user_id, display_filename, bytes) =
        upload.ok_or_else(|| AppError::BadRequest("ไม่พบ file".to_string()))?;
    let repository = SqlFileRepository::new(context.tenant.pool.clone());
    let file = state
        .file_platform
        .upload(
            &repository,
            UploadCommand {
                tenant_id: context.tenant.tenant_id,
                actor_user_id: Some(context.actor.user_id),
                owner_user_id: Some(owner_user_id),
                purpose,
                display_filename,
                bytes,
            },
        )
        .await
        .map_err(map_platform_error)?;

    record_upload_relation_or_request_cleanup(
        state.file_platform.as_ref(),
        &repository,
        file.id,
        purpose,
        resource_id,
        context.actor.user_id,
    )
    .await?;

    tracing::info!(
        file_id = %file.id,
        actor_user_id = %context.actor.user_id,
        purpose = purpose.code(),
        action = "create",
        result = "allowed",
        "File Platform policy decision"
    );
    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::ok(FileMetadata::from(file))),
    )
        .into_response())
}

async fn record_upload_relation_or_request_cleanup(
    platform: &FilePlatform,
    repository: &SqlFileRepository,
    file_id: Uuid,
    purpose: FilePurpose,
    resource_id: Option<Uuid>,
    actor_user_id: Uuid,
) -> Result<(), AppError> {
    let relation_result = match purpose {
        FilePurpose::CertificateTemplateBackground | FilePurpose::CertificateTemplateImage => {
            let template_id = resource_id.ok_or_else(|| {
                AppError::InternalServerError(
                    "certificate_template_upload_resource_missing".to_string(),
                )
            })?;
            Some(
                record_certificate_template_upload(
                    repository.pool(),
                    file_id,
                    template_id,
                    purpose,
                    actor_user_id,
                )
                .await,
            )
        }
        FilePurpose::SchoolFont => Some(match resource_id {
            Some(template_id) => {
                record_certificate_school_font_upload(
                    repository.pool(),
                    file_id,
                    template_id,
                    actor_user_id,
                )
                .await
            }
            None => record_school_font_upload(repository.pool(), file_id, actor_user_id).await,
        }),
        _ => None,
    };
    if let Some(Err(error)) = relation_result {
        if request_deletions(platform, repository.pool(), [file_id])
            .await
            .is_err()
        {
            let compensation_action = match purpose {
                FilePurpose::SchoolFont => "school_font_upload_compensation",
                _ => "certificate_template_upload_compensation",
            };
            tracing::warn!(
                file_id = %file_id,
                purpose = purpose.code(),
                action = compensation_action,
                result = "cleanup_request_failed",
                "File Platform compensation needs temporary-retention fallback"
            );
        }
        return Err(error);
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/files/{id}",
    operation_id = "getFileMetadata",
    tag = "files",
    params(
        ("id" = Uuid, Path, description = "Logical file ID"),
        FileAccessQuery
    ),
    responses(
        (status = 200, description = "Authorized file metadata", body = ApiResponse<FileMetadata>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "File policy denied", body = ApiErrorResponse),
        (status = 404, description = "File or resource relationship not found", body = ApiErrorResponse)
    )
)]
pub async fn get_file_metadata(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(file_id): Path<Uuid>,
    Query(query): Query<FileAccessQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let repository = SqlFileRepository::new(context.tenant.pool);
    let file = state
        .file_platform
        .metadata(&repository, file_id)
        .await
        .map_err(map_platform_error)?;
    file_access_policy::authorize_existing(
        repository.pool(),
        &context.actor,
        &file,
        FilePolicyAction::Read,
        query.resource_id,
    )
    .await?;
    audit_allowed(context.actor.user_id, &file, "read");

    Ok(Json(ApiResponse::ok(FileMetadata::from(file))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/files/{id}/download",
    operation_id = "downloadFile",
    tag = "files",
    params(
        ("id" = Uuid, Path, description = "Logical file ID"),
        FileAccessQuery
    ),
    responses(
        (status = 200, description = "Short-lived private download grant", body = ApiResponse<FileDownloadGrantResponse>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "File policy denied", body = ApiErrorResponse),
        (status = 404, description = "File or resource relationship not found", body = ApiErrorResponse),
        (status = 409, description = "File is not ready", body = ApiErrorResponse)
    )
)]
pub async fn download_file(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(file_id): Path<Uuid>,
    Query(query): Query<FileAccessQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let repository = SqlFileRepository::new(context.tenant.pool);
    let file = state
        .file_platform
        .metadata(&repository, file_id)
        .await
        .map_err(map_platform_error)?;
    file_access_policy::authorize_existing(
        repository.pool(),
        &context.actor,
        &file,
        FilePolicyAction::Read,
        query.resource_id,
    )
    .await?;
    let grant = state
        .file_platform
        .private_download(&repository, file_id)
        .await
        .map_err(map_platform_error)?;
    audit_allowed(context.actor.user_id, &file, "download");

    let response = FileDownloadGrantResponse::try_from(grant).map_err(|()| {
        AppError::InternalServerError("file_stream_grant_not_supported".to_string())
    })?;
    Ok(Json(ApiResponse::ok(response)).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/files/{id}",
    operation_id = "deleteFile",
    tag = "files",
    params(
        ("id" = Uuid, Path, description = "Logical file ID"),
        FileAccessQuery
    ),
    responses(
        (status = 200, description = "Delivery revoked and deletion requested", body = ApiResponse<FileDeleteResult>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "File policy denied", body = ApiErrorResponse),
        (status = 404, description = "File or resource relationship not found", body = ApiErrorResponse)
    )
)]
pub async fn delete_file(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(file_id): Path<Uuid>,
    Query(query): Query<FileAccessQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let repository = SqlFileRepository::new(context.tenant.pool);
    let file = state
        .file_platform
        .metadata(&repository, file_id)
        .await
        .map_err(map_platform_error)?;
    let domain_delete_guard = match file.purpose {
        FilePurpose::CertificateTemplateBackground | FilePurpose::CertificateTemplateImage => Some(
            file_access_policy::authorize_certificate_template_delete_guard(
                repository.pool(),
                &context.actor,
                &file,
                query.resource_id,
            )
            .await?,
        ),
        FilePurpose::SchoolFont => Some(
            file_access_policy::authorize_school_font_delete_guard(
                repository.pool(),
                &context.actor,
                &file,
                query.resource_id,
            )
            .await?,
        ),
        _ => {
            file_access_policy::authorize_existing(
                repository.pool(),
                &context.actor,
                &file,
                FilePolicyAction::Delete,
                query.resource_id,
            )
            .await?;
            None
        }
    };
    let outcome = if let Some(mut guard) = domain_delete_guard {
        let work = repository
            .request_delete_in_transaction(&mut guard, file_id)
            .await
            .map_err(FilePlatformError::from)
            .map_err(map_platform_error)?;
        guard.commit().await?;
        state
            .file_platform
            .complete_prepared_delete(&repository, work)
            .await
            .map_err(map_platform_error)?
    } else {
        state
            .file_platform
            .request_delete(&repository, file_id)
            .await
            .map_err(map_platform_error)?
    };
    audit_allowed(context.actor.user_id, &file, "delete");

    Ok(Json(ApiResponse::ok(FileDeleteResult {
        pending_retry: outcome.pending_retry,
    }))
    .into_response())
}

#[utoipa::path(
    get,
    path = "/api/public/files/{id}/content",
    operation_id = "getPublicFileContent",
    tag = "files",
    params(("id" = Uuid, Path, description = "Logical public file ID")),
    responses(
        (status = 307, description = "Ready public file redirect"),
        (status = 404, description = "Public ready file not found", body = ApiErrorResponse)
    )
)]
pub async fn get_public_file_content(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(file_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let tenant = tenant_context(&state, &headers).await?;
    let repository = SqlFileRepository::new(tenant.pool);
    let delivery = state
        .file_platform
        .public_delivery(&repository, file_id)
        .await
        .map_err(map_public_platform_error)?;
    let mut response = Redirect::temporary(delivery.location.as_str()).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=300"),
    );
    Ok(response)
}

#[utoipa::path(
    get,
    path = "/api/public/files/{id}/delivery",
    operation_id = "getPublicFileDelivery",
    tag = "files",
    params(("id" = Uuid, Path, description = "Logical public file ID")),
    responses(
        (status = 200, description = "Browser-safe public file delivery", body = ApiResponse<PublicFileDeliveryResponse>),
        (status = 404, description = "Public ready file not found", body = ApiErrorResponse)
    )
)]
pub async fn get_public_file_delivery(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(file_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let tenant = tenant_context(&state, &headers).await?;
    let repository = SqlFileRepository::new(tenant.pool);
    let delivery = state
        .file_platform
        .public_delivery(&repository, file_id)
        .await
        .map_err(map_public_platform_error)?;

    Ok(Json(ApiResponse::ok(PublicFileDeliveryResponse::from(delivery))).into_response())
}

async fn read_control_field(mut field: Field<'_>) -> Result<String, AppError> {
    let mut bytes = BytesMut::new();
    while let Some(chunk) = field.chunk().await.map_err(|_| invalid_multipart())? {
        if bytes.len().saturating_add(chunk.len()) > MAX_CONTROL_FIELD_BYTES {
            return Err(invalid_multipart());
        }
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| invalid_multipart())
}

async fn read_file_field(mut field: Field<'_>, max_bytes: u64) -> Result<Bytes, AppError> {
    let max_bytes =
        usize::try_from(max_bytes).map_err(|_| AppError::BadRequest("ไฟล์ใหญ่เกินไป".to_string()))?;
    let mut bytes = BytesMut::with_capacity(max_bytes.min(1024 * 1024));
    while let Some(chunk) = field.chunk().await.map_err(|_| invalid_multipart())? {
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err(AppError::BadRequest("ไฟล์ใหญ่เกินขนาดที่กำหนด".to_string()));
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        return Err(AppError::BadRequest("ไฟล์ต้องไม่ว่าง".to_string()));
    }
    Ok(bytes.freeze())
}

fn map_public_platform_error(error: super::platform_service::FilePlatformError) -> AppError {
    match error {
        FilePlatformError::NotFound
        | FilePlatformError::NotReady
        | FilePlatformError::VisibilityMismatch => AppError::NotFound("ไม่พบไฟล์".to_string()),
        other => map_platform_error(other),
    }
}

fn invalid_multipart() -> AppError {
    AppError::BadRequest("multipart ไม่ถูกต้อง".to_string())
}

fn audit_allowed(
    actor_user_id: Uuid,
    file: &super::repository::PlatformFile,
    action: &'static str,
) {
    tracing::info!(
        file_id = %file.id,
        actor_user_id = %actor_user_id,
        purpose = file.purpose.code(),
        action,
        result = "allowed",
        "File Platform policy decision"
    );
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use async_trait::async_trait;
    use bytes::Bytes;
    use url::Url;

    use super::*;
    use crate::{
        modules::files::{
            consumer_service::tests::{
                file_lifecycle_status, insert_file, insert_template, school_font_upload_relations,
            },
            malware_scanner::{MalwareScanner, ScanOutcome},
            platform_service::FilePlatform,
            platform_types::DownloadGrant,
            storage_provider::{ObjectMetadata, StorageError, StorageProvider, StoredObject},
        },
        test_helpers::{create_named_test_pool, create_test_user, run_test_migrations},
    };

    struct UnexpectedStorageProvider;

    #[async_trait]
    impl StorageProvider for UnexpectedStorageProvider {
        async fn check_readiness(&self) -> Result<(), StorageError> {
            panic!("upload-relation orchestration must not check provider readiness")
        }

        async fn put(&self, _object: &StoredObject, _body: Bytes) -> Result<(), StorageError> {
            panic!("upload-relation orchestration must not store another object")
        }

        async fn get(
            &self,
            _object: &StoredObject,
            _max_bytes: u64,
        ) -> Result<Bytes, StorageError> {
            panic!("upload-relation orchestration must not read object bytes")
        }

        async fn head(
            &self,
            _object: &StoredObject,
        ) -> Result<Option<ObjectMetadata>, StorageError> {
            panic!("upload-relation orchestration must not inspect provider metadata")
        }

        async fn delete(&self, _object: &StoredObject) -> Result<(), StorageError> {
            panic!("a versionless failed upload must complete deletion without provider work")
        }

        async fn private_download_grant(
            &self,
            _object: &StoredObject,
            _filename: &str,
            _ttl: Duration,
        ) -> Result<DownloadGrant, StorageError> {
            panic!("upload-relation orchestration must not issue download grants")
        }

        fn public_location(&self, _object: &StoredObject) -> Result<Url, StorageError> {
            panic!("upload-relation orchestration must not expose public locations")
        }
    }

    struct UnexpectedScanner;

    #[async_trait]
    impl MalwareScanner for UnexpectedScanner {
        async fn scan(&self, _content: &[u8]) -> ScanOutcome {
            panic!("upload-relation orchestration runs after scanning")
        }
    }

    fn file_platform() -> FilePlatform {
        FilePlatform::new(
            Arc::new(UnexpectedStorageProvider),
            Arc::new(UnexpectedScanner),
        )
    }

    async fn test_context(test_name: &str) -> (SqlFileRepository, Uuid) {
        let pool = create_named_test_pool(test_name).await;
        run_test_migrations(&pool).await;
        let actor_id =
            create_test_user(&pool, &format!("{test_name}@example.test"), "test-password")
                .await
                .expect("actor fixture should insert");
        (SqlFileRepository::new(pool), actor_id)
    }

    #[tokio::test]
    async fn central_school_font_dispatch_records_only_central_staging() {
        let (repository, actor_id) = test_context("handler_central_school_font_dispatch").await;
        let file_id = insert_file(repository.pool(), actor_id).await;

        record_upload_relation_or_request_cleanup(
            &file_platform(),
            &repository,
            file_id,
            FilePurpose::SchoolFont,
            None,
            actor_id,
        )
        .await
        .expect("central school-font dispatch should persist its staging relation");

        assert_eq!(
            school_font_upload_relations(repository.pool(), file_id).await,
            (1, Vec::new())
        );
    }

    #[tokio::test]
    async fn template_school_font_dispatch_records_only_the_exact_template_staging() {
        let (repository, actor_id) = test_context("handler_template_school_font_dispatch").await;
        let template_id = insert_template(repository.pool(), actor_id).await;
        let file_id = insert_file(repository.pool(), actor_id).await;

        record_upload_relation_or_request_cleanup(
            &file_platform(),
            &repository,
            file_id,
            FilePurpose::SchoolFont,
            Some(template_id),
            actor_id,
        )
        .await
        .expect("template school-font dispatch should persist its staging relation");

        assert_eq!(
            school_font_upload_relations(repository.pool(), file_id).await,
            (0, vec![template_id])
        );
    }

    #[tokio::test]
    async fn relation_failure_requests_file_deletion_and_returns_the_relation_error() {
        let (repository, actor_id) = test_context("handler_school_font_relation_failure").await;
        let file_id = insert_file(repository.pool(), actor_id).await;

        let error = record_upload_relation_or_request_cleanup(
            &file_platform(),
            &repository,
            file_id,
            FilePurpose::SchoolFont,
            Some(Uuid::new_v4()),
            actor_id,
        )
        .await
        .expect_err("a missing exact template must reject relation recording");

        assert!(matches!(
            error,
            AppError::NotFound(message) if message == "ไม่พบแม่แบบเกียรติบัตร"
        ));
        assert_eq!(
            file_lifecycle_status(repository.pool(), file_id).await,
            "deleted"
        );
        assert_eq!(
            school_font_upload_relations(repository.pool(), file_id).await,
            (0, Vec::new())
        );
    }
}
