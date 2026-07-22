use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};
use serde::Serialize;
use utoipa::ToSchema;

use super::models::UpdateSchoolSettingsRequest;
use super::services as school_service;
use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::permissions::registry::codes;
use crate::utils::request_context::{actor_tenant_context, tenant_context};
use crate::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicSchoolInfoData {
    #[schema(required = true)]
    pub logo_url: Option<String>,
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
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context.actor.require_permission(codes::SETTINGS_READ_ALL)?;

    let response = school_service::get_settings_response(&context.tenant.pool).await?;

    Ok(Json(ApiResponse::ok(response)).into_response())
}

/// PATCH /api/school/settings — staff only (SETTINGS_UPDATE_ALL)
pub async fn update_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateSchoolSettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::SETTINGS_UPDATE_ALL)?;

    school_service::update_settings(&context.tenant.pool, payload).await?;

    Ok(Json(ApiResponse::empty()).into_response())
}

/// GET /api/school/public — no auth required
/// Returns logoUrl (built from logo_path) + schoolName (from backend-admin)
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

    let logo_url = school_service::get_settings_response(&tenant.pool)
        .await?
        .logo_url;
    let school_name = state
        .admin_client
        .get_school_name(&tenant.subdomain)
        .await
        .ok();

    Ok(Json(ApiResponse::ok(PublicSchoolInfoData {
        logo_url,
        school_name,
    }))
    .into_response())
}

/// DELETE /api/school/settings/logo — staff only (SETTINGS_UPDATE_ALL)
/// ลบ logo จาก R2 และล้าง logo_path/logo_file_id ใน school_settings
pub async fn delete_logo(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::SETTINGS_UPDATE_ALL)?;

    school_service::delete_logo(&context.tenant.pool).await?;

    Ok(Json(ApiResponse::empty()).into_response())
}
