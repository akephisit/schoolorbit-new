use axum::{
    extract::{Extension, Path, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
    Json,
};
use serde::Serialize;
use utoipa::ToSchema;

use super::models::UpdateSchoolSettingsRequest;
use super::services as school_service;
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::utils::tenant::tenant_context;
use crate::AppState;
use school_auth::session_service::AuthenticatedSession;
use school_http::HttpError as AppError;
use school_http::{ApiErrorResponse, ApiResponse};
use school_permissions::registry::codes;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicSchoolInfoData {
    #[schema(required = true)]
    pub logo_file_id: Option<uuid::Uuid>,
    #[schema(required = true)]
    pub school_name: Option<String>,
}

#[utoipa::path(
    get, path = "/api/school/public/statistics", operation_id = "getPublicSchoolStatistics", tag = "school",
    responses(
        (status = 200, description = "Public school statistics for the active academic year", body = ApiResponse<super::models::PublicSchoolStatistics>),
        (status = 400, description = "Invalid tenant context", body = ApiErrorResponse),
        (status = 404, description = "School not found", body = ApiErrorResponse)
    )
)]
pub async fn get_public_statistics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let tenant = tenant_context(&state, &headers).await?;
    let data = school_service::public::get_statistics(&tenant.pool).await?;
    Ok((
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Json(ApiResponse::ok(data)),
    ))
}

#[utoipa::path(
    get, path = "/api/school/public/organization", operation_id = "getPublicSchoolOrganization", tag = "school",
    responses(
        (status = 200, description = "Active organization units and all current members", body = ApiResponse<super::models::PublicSchoolOrganization>),
        (status = 400, description = "Invalid tenant context", body = ApiErrorResponse),
        (status = 404, description = "School not found", body = ApiErrorResponse)
    )
)]
pub async fn get_public_organization(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let tenant = tenant_context(&state, &headers).await?;
    let data = school_service::public::get_organization(&tenant.pool).await?;
    Ok((
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Json(ApiResponse::ok(data)),
    ))
}

#[utoipa::path(
    get, path = "/api/school/public/organization-members/{id}/avatar", operation_id = "getPublicOrganizationAvatar", tag = "school",
    params(("id" = uuid::Uuid, Path, description = "Current public organization membership")),
    responses((status = 307, description = "Short-lived profile image delivery"),
        (status = 404, description = "Published staff photo not found", body = ApiErrorResponse))
)]
pub async fn get_public_organization_avatar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(member_id): Path<uuid::Uuid>,
) -> Result<Response, AppError> {
    let tenant = tenant_context(&state, &headers).await?;
    let file_id = school_service::public::organization_avatar_file(&tenant.pool, member_id).await?;
    let repository = school_file_platform::repository::SqlFileRepository::new(tenant.pool);
    let grant = state
        .file_platform
        .private_download(&repository, file_id)
        .await
        .map_err(crate::modules::files::consumer_service::map_platform_error)?;
    let grant = crate::modules::files::models::FileDownloadGrantResponse::try_from(grant)
        .map_err(|_| AppError::ServiceUnavailable("ส่งรูปบุคลากรไม่สำเร็จ".into()))?;
    Ok((
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Redirect::temporary(&grant.url),
    )
        .into_response())
}

/// GET /api/school/settings — staff only (SETTINGS_READ_ALL)
#[utoipa::path(
    get,
    path = "/api/school/settings",
    operation_id = "getSchoolSettings",
    tag = "school",
    responses(
        (status = 200, description = "School settings", body = ApiResponse<crate::modules::school::models::SchoolSettingsResponse>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Settings read permission required", body = ApiErrorResponse)
    )
)]
pub async fn get_settings(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context.actor.require_permission(codes::SETTINGS_READ_ALL)?;

    let response = school_service::get_settings_response(&context.tenant.pool).await?;

    Ok(Json(ApiResponse::ok(response)).into_response())
}

/// PATCH /api/school/settings — staff only (SETTINGS_UPDATE_ALL)
pub async fn update_settings(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<UpdateSchoolSettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::SETTINGS_UPDATE_ALL)?;

    let old_file_id =
        school_service::replace_logo(&context.tenant.pool, payload.logo_file_id).await?;
    if let Some(old_file_id) = old_file_id {
        crate::modules::files::consumer_service::request_deletions(
            state.file_platform.as_ref(),
            &context.tenant.pool,
            [old_file_id],
        )
        .await?;
    }

    Ok(Json(ApiResponse::empty()).into_response())
}

/// GET /api/school/public — no auth required
/// Returns the public File Platform identity + schoolName (from backend-admin)
#[utoipa::path(
    get,
    path = "/api/school/public",
    operation_id = "getPublicSchoolInfo",
    tag = "school",
    responses(
        (status = 200, description = "Public school branding", body = ApiResponse<PublicSchoolInfoData>)
    )
)]
pub async fn get_public_info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let tenant = tenant_context(&state, &headers).await?;

    let logo_file_id = school_service::get_settings_response(&tenant.pool)
        .await?
        .logo_file_id;
    let school_name = state
        .admin_client
        .get_school_name(&tenant.subdomain)
        .await
        .ok();

    Ok(Json(ApiResponse::ok(PublicSchoolInfoData {
        logo_file_id,
        school_name,
    }))
    .into_response())
}

/// DELETE /api/school/settings/logo — staff only (SETTINGS_UPDATE_ALL)
/// Detach the logo and request durable File Platform deletion.
pub async fn delete_logo(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::SETTINGS_UPDATE_ALL)?;

    if let Some(file_id) = school_service::detach_logo(&context.tenant.pool).await? {
        crate::modules::files::consumer_service::request_deletions(
            state.file_platform.as_ref(),
            &context.tenant.pool,
            [file_id],
        )
        .await?;
    }

    Ok(Json(ApiResponse::empty()).into_response())
}
