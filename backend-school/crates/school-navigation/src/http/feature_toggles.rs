use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Json as JsonResponse},
};
use serde::{Deserialize, Serialize};
#[cfg(feature = "api-docs")]
use utoipa::ToSchema;
use uuid::Uuid;

use crate::http::state::NavigationHttpState;
use crate::models::FeatureToggle;
use crate::services::feature_toggle_service;
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
#[cfg(feature = "api-docs")]
use school_http::ApiErrorResponse;
use school_http::HttpError as AppError;
use school_permissions::registry::codes;

#[derive(Debug, Deserialize)]
pub struct UpdateFeatureRequest {
    pub is_enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct FeatureToggleResponse {
    pub success: bool,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub data: Option<FeatureToggle>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct FeatureListResponse {
    pub success: bool,
    pub data: Vec<FeatureToggle>,
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    get,
    path = "/api/admin/features",
    operation_id = "listFeatures",
    tag = "system",
    responses(
        (status = 200, description = "Feature toggles for authorized feature managers", body = FeatureListResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse)
    )
))]
pub async fn list_features(
    State(state): State<NavigationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;

    actor.require_permission(codes::FEATURES_READ_ALL)?;
    let features = feature_toggle_service::list_features(&pool).await?;

    Ok((
        StatusCode::OK,
        JsonResponse(FeatureListResponse {
            success: true,
            data: features,
        }),
    ))
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    get,
    path = "/api/admin/features/{id}",
    operation_id = "getFeature",
    tag = "system",
    params(("id" = Uuid, Path, description = "Feature toggle ID")),
    responses(
        (status = 200, description = "Feature toggle", body = FeatureToggleResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Feature management access denied", body = ApiErrorResponse),
        (status = 404, description = "Feature toggle not found", body = ApiErrorResponse)
    )
))]
pub async fn get_feature(
    State(state): State<NavigationHttpState>,
    Path(id): Path<Uuid>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::FEATURES_READ_ALL)?;
    let feature = feature_toggle_service::get_feature(&pool, id).await?;

    Ok((
        StatusCode::OK,
        JsonResponse(FeatureToggleResponse {
            success: true,
            data: Some(feature),
            message: None,
        }),
    ))
}

pub async fn update_feature(
    State(state): State<NavigationHttpState>,
    Path(id): Path<Uuid>,
    Extension(session): Extension<AuthenticatedSession>,
    JsonResponse(data): JsonResponse<UpdateFeatureRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::FEATURES_UPDATE_ALL)?;

    let feature = feature_toggle_service::update_feature(&pool, id, data.is_enabled).await?;
    Ok((
        StatusCode::OK,
        JsonResponse(FeatureToggleResponse {
            success: true,
            data: Some(feature),
            message: Some("Feature toggle updated successfully".to_string()),
        }),
    ))
}

pub async fn toggle_feature(
    State(state): State<NavigationHttpState>,
    Path(id): Path<Uuid>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::FEATURES_UPDATE_ALL)?;

    let feature = feature_toggle_service::toggle_feature(&pool, id).await?;
    let feature_code = feature.code.clone();
    let feature_enabled = feature.is_enabled;

    Ok((
        StatusCode::OK,
        JsonResponse(FeatureToggleResponse {
            success: true,
            data: Some(feature),
            message: Some(format!(
                "Feature {} {}",
                feature_code,
                if feature_enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            )),
        }),
    ))
}
