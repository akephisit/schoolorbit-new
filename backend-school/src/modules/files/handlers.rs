use axum::{
    extract::{Extension, Multipart, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
};
use serde_json::json;
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    error::AppError,
    modules::{auth::models::Claims, files::models::DeleteFileResponse},
    utils::{
        subdomain::extract_subdomain_from_request, tenant::resolve_tenant_context_by_subdomain,
    },
    AppState,
};

use super::services as file_service;

fn user_id_from_claims(claims: &Claims) -> Result<Uuid, AppError> {
    Uuid::parse_str(&claims.sub).map_err(|_| {
        error!("Invalid user ID in claims: {}", claims.sub);
        AppError::AuthError("Invalid user authentication".to_string())
    })
}

async fn tenant_pool_by_subdomain(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(String, sqlx::PgPool), AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;
    let pool = resolve_tenant_context_by_subdomain(state, &subdomain)
        .await?
        .pool;

    Ok((subdomain, pool))
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
    let user_id = user_id_from_claims(&claims)?;
    info!("Uploading file for user: {}", user_id);

    let (subdomain, pool) = tenant_pool_by_subdomain(&state, &headers).await?;
    let file_response = file_service::upload_file(&pool, user_id, &subdomain, multipart).await?;

    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": { "file": file_response } })),
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
    let user_id = user_id_from_claims(&claims)?;
    info!("Deleting file: {} for user: {}", file_id, user_id);

    let (_subdomain, pool) = tenant_pool_by_subdomain(&state, &headers).await?;
    file_service::delete_file(&pool, user_id, file_id).await?;

    Ok((
        StatusCode::OK,
        Json(DeleteFileResponse {
            success: true,
            message: "File deleted successfully".to_string(),
        }),
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
    let user_id = user_id_from_claims(&claims)?;
    let (_subdomain, pool) = tenant_pool_by_subdomain(&state, &headers).await?;
    let response = file_service::list_user_files(&pool, user_id).await?;

    Ok((StatusCode::OK, Json(response)))
}
