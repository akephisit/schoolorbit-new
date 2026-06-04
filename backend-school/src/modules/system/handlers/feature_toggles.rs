use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json as JsonResponse},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::{get_actor_context_or_error, ActorContext};
use crate::modules::menu::models::FeatureToggle;
use crate::modules::system::services::feature_toggle_service;
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct UpdateFeatureRequest {
    pub is_enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct FeatureToggleResponse {
    pub success: bool,
    pub data: Option<FeatureToggle>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FeatureListResponse {
    pub success: bool,
    pub data: Vec<FeatureToggle>,
}

pub async fn list_features(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (pool, actor) = get_pool_and_actor(&state, &headers).await?;

    let all_features = feature_toggle_service::list_features(&pool).await?;
    let features: Vec<FeatureToggle> = all_features
        .into_iter()
        .filter(|f| match f.module {
            Some(ref module) => actor.has_module_permission(module),
            None => true,
        })
        .collect();

    Ok((
        StatusCode::OK,
        JsonResponse(FeatureListResponse {
            success: true,
            data: features,
        }),
    ))
}

pub async fn get_feature(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (pool, actor) = get_pool_and_actor(&state, &headers).await?;
    let feature = feature_toggle_service::get_feature(&pool, id).await?;

    if let Some(ref module) = feature.module {
        if !actor.has_module_permission(module) {
            return Err(AppError::Forbidden(format!(
                "No permission for module '{}'",
                module
            )));
        }
    }

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
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<UpdateFeatureRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (pool, actor) = get_pool_and_actor(&state, &headers).await?;
    let existing = feature_toggle_service::get_feature(&pool, id).await?;

    if let Some(ref module) = existing.module {
        if !actor.has_module_permission(module) {
            return Err(AppError::Forbidden(format!(
                "No permission for module '{}'",
                module
            )));
        }
    }

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
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (pool, actor) = get_pool_and_actor(&state, &headers).await?;
    let existing = feature_toggle_service::get_feature(&pool, id).await?;

    if let Some(ref module) = existing.module {
        if !actor.has_module_permission(module) {
            return Err(AppError::Forbidden(format!(
                "No permission for module '{}'",
                module
            )));
        }
    }

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

async fn get_pool_and_actor(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(sqlx::PgPool, ActorContext), AppError> {
    let pool = resolve_tenant_pool(state, headers).await?;
    let actor = get_actor_context_or_error(headers, &pool, &state.permission_cache).await?;
    Ok((pool, actor))
}
