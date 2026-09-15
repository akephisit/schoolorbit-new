use crate::utils::request_context::current_user_tenant_context_from_session;
use crate::AppState;
use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use school_auth::{
    models::{ProfileResponse, UpdateProfileRequest},
    services,
    session_service::AuthenticatedSession,
};

use super::profile_orchestrator;
use school_http::HttpError as AppError;
use school_http::{ApiErrorResponse, ApiResponse};

/// Get full profile handler (GET /me/profile).
#[utoipa::path(
    get,
    path = "/api/auth/me/profile",
    operation_id = "getCurrentUserProfile",
    tag = "auth",
    responses(
        (status = 200, description = "Current user's full profile", body = ApiResponse<ProfileResponse>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 404, description = "User not found", body = ApiErrorResponse)
    )
)]
pub async fn get_profile(
    State(_state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_session(&session);
    let pool = context.tenant.pool;

    let user = services::find_user_by_id(&pool, context.user_id).await?;
    let primary_role_name = services::get_primary_role_name(&pool, user.id).await?;

    let mut profile_response = ProfileResponse::from(user);
    profile_response.primary_role_name = primary_role_name;

    Ok((StatusCode::OK, Json(ApiResponse::ok(profile_response))))
}

/// Update profile handler (PUT /me/profile).
#[utoipa::path(
    put,
    path = "/api/auth/me/profile",
    operation_id = "updateCurrentUserProfile",
    tag = "auth",
    request_body = UpdateProfileRequest,
    responses(
        (status = 200, description = "Updated current-user profile", body = ApiResponse<ProfileResponse>),
        (status = 400, description = "Invalid profile data", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 404, description = "User not found", body = ApiErrorResponse)
    )
)]
pub async fn update_profile(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<UpdateProfileRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_session(&session);
    let pool = context.tenant.pool;
    let user_id = context.user_id;

    let user =
        profile_orchestrator::update_profile(state.file_platform.as_ref(), &pool, user_id, payload)
            .await?;
    let primary_role_name = services::get_primary_role_name(&pool, user.id).await?;

    let mut profile_response = ProfileResponse::from(user);
    profile_response.primary_role_name = primary_role_name;

    Ok((StatusCode::OK, Json(ApiResponse::ok(profile_response))))
}
