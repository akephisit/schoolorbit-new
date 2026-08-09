use axum::{
    extract::{Extension, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use utoipa::ToSchema;

use super::models::UpdateSchoolSettingsRequest;
use super::services as school_service;
use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::utils::tenant::tenant_context;
use crate::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicSchoolInfoData {
    #[schema(required = true)]
    pub logo_file_id: Option<uuid::Uuid>,
    #[schema(required = true)]
    pub school_name: Option<String>,
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
