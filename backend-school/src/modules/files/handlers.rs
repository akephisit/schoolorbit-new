use axum::{
    extract::{Extension, Multipart, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
};
use serde::Serialize;
use tracing::info;
use uuid::Uuid;

use crate::{
    api_response::ApiResponse, error::AppError, modules::auth::models::Claims,
    utils::request_context::current_user_tenant_context_from_claims, AppState,
};

use super::models::FileResponse;
use super::services as file_service;

#[derive(Debug, Serialize)]
struct UploadFileData {
    file: FileResponse,
}

#[derive(Debug, Serialize)]
struct UserFilesData {
    files: Vec<FileResponse>,
    total: i64,
}

/// Upload a file
///
/// POST /api/files/upload
pub async fn upload_file(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_claims(&state, &headers, &claims).await?;
    info!("Uploading file for user: {}", context.user_id);

    let file_response = file_service::upload_file(
        &context.tenant.pool,
        context.user_id,
        &context.tenant.subdomain,
        multipart,
    )
    .await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::ok(UploadFileData {
            file: file_response,
        })),
    ))
}

/// Delete a file
///
/// DELETE /api/files/:id
pub async fn delete_file(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    headers: HeaderMap,
    Path(file_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_claims(&state, &headers, &claims).await?;
    info!("Deleting file: {} for user: {}", file_id, context.user_id);

    file_service::delete_file(&context.tenant.pool, context.user_id, file_id).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("File deleted successfully")),
    ))
}

/// Get file list for current user
///
/// GET /api/files
pub async fn list_user_files(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_claims(&state, &headers, &claims).await?;
    let response = file_service::list_user_files(&context.tenant.pool, context.user_id).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::ok(UserFilesData {
            files: response.files,
            total: response.total,
        })),
    ))
}
