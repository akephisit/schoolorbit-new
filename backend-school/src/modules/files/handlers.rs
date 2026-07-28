use axum::{
    extract::{multipart::Field, Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use bytes::{Bytes, BytesMut};
use uuid::Uuid;

use crate::{
    api_response::{ApiErrorResponse, ApiResponse},
    error::AppError,
    policies::file_access_policy::{self, FilePolicyAction},
    utils::request_context::{actor_tenant_context, tenant_context},
    AppState,
};

use super::{
    models::{FileAccessQuery, FileDeleteResult, FileMetadata, FileUploadMultipart},
    platform_service::{FilePlatformError, UploadCommand},
    platform_types::DownloadGrant,
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
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
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
                upload = Some((purpose, owner_user_id, filename, bytes));
            }
            _ => return Err(invalid_multipart()),
        }
    }

    let (purpose, owner_user_id, display_filename, bytes) =
        upload.ok_or_else(|| AppError::BadRequest("ไม่พบ file".to_string()))?;
    let repository = SqlFileRepository::new(context.tenant.pool);
    let file = state
        .file_platform
        .upload(
            &repository,
            UploadCommand {
                tenant_id: context.tenant.tenant_id,
                actor_user_id: context.actor.user_id,
                owner_user_id,
                purpose,
                display_filename,
                bytes,
            },
        )
        .await
        .map_err(map_platform_error)?;

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
    headers: HeaderMap,
    Path(file_id): Path<Uuid>,
    Query(query): Query<FileAccessQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
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
        (status = 303, description = "Short-lived private download redirect"),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "File policy denied", body = ApiErrorResponse),
        (status = 404, description = "File or resource relationship not found", body = ApiErrorResponse),
        (status = 409, description = "File is not ready", body = ApiErrorResponse)
    )
)]
pub async fn download_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(file_id): Path<Uuid>,
    Query(query): Query<FileAccessQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
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

    match grant {
        DownloadGrant::Redirect { location, .. } => Ok(Redirect::to(&location).into_response()),
        DownloadGrant::Stream { .. } => Err(AppError::InternalServerError(
            "file_stream_grant_not_supported".to_string(),
        )),
    }
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
    headers: HeaderMap,
    Path(file_id): Path<Uuid>,
    Query(query): Query<FileAccessQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
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
        FilePolicyAction::Delete,
        query.resource_id,
    )
    .await?;
    let outcome = state
        .file_platform
        .request_delete(&repository, file_id)
        .await
        .map_err(map_platform_error)?;
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

fn map_platform_error(error: FilePlatformError) -> AppError {
    match error {
        FilePlatformError::InspectionRejected => {
            AppError::BadRequest("ชนิดหรือโครงสร้างไฟล์ไม่ถูกต้อง".to_string())
        }
        FilePlatformError::MalwareDetected => {
            AppError::BadRequest("ไฟล์ไม่ผ่านการตรวจสอบความปลอดภัย".to_string())
        }
        FilePlatformError::NotFound => AppError::NotFound("ไม่พบไฟล์".to_string()),
        FilePlatformError::NotReady => AppError::Conflict("ไฟล์ยังไม่พร้อมใช้งาน".to_string()),
        FilePlatformError::VisibilityMismatch => {
            AppError::Forbidden("ไม่อนุญาตให้ส่งไฟล์ด้วยช่องทางนี้".to_string())
        }
        FilePlatformError::ScannerUnavailable
        | FilePlatformError::StorageUnavailable
        | FilePlatformError::RequiredDerivativeUnavailable => {
            AppError::ServiceUnavailable(error.log_safe_code().to_string())
        }
        FilePlatformError::MetadataUnavailable => {
            AppError::InternalServerError(error.log_safe_code().to_string())
        }
    }
}

fn map_public_platform_error(error: FilePlatformError) -> AppError {
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
