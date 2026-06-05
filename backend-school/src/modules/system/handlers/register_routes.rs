use crate::error::AppError;
use crate::modules::menu::models::{RouteRegistration, RouteRegistrationResponse};
use crate::modules::system::services::route_registration_service;
use crate::utils::{
    request_context::tenant_context_by_subdomain, subdomain::extract_subdomain_from_request,
};
use crate::AppState;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json as JsonResponse,
};

/// Register routes from frontend build
/// This endpoint is called during frontend build to auto-sync menu items
pub async fn register_routes(
    State(state): State<AppState>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<RouteRegistration>,
) -> Result<impl IntoResponse, AppError> {
    // Validate deploy key
    let deploy_key = headers.get("X-Deploy-Key").and_then(|h| h.to_str().ok());

    let expected_key = std::env::var("DEPLOY_KEY").map_err(|_| {
        tracing::error!("DEPLOY_KEY environment variable not set");
        AppError::InternalServerError("Server configuration error".to_string())
    })?;

    if deploy_key != Some(expected_key.as_str()) {
        tracing::warn!("Invalid deploy key provided");
        return Err(AppError::AuthError("Invalid deploy key".to_string()));
    }

    let subdomain = extract_subdomain_from_request(&headers).map_err(|_| {
        tracing::warn!("No subdomain provided for route registration");
        AppError::BadRequest("No subdomain specified".to_string())
    })?;

    let pool = tenant_context_by_subdomain(&state, &subdomain).await?.pool;
    let outcome = route_registration_service::sync_routes(&pool, &data).await?;

    Ok((
        StatusCode::OK,
        JsonResponse(RouteRegistrationResponse {
            success: true,
            registered: outcome.registered,
            message: format!(
                "Synced {} routes (preserved user customizations)",
                outcome.registered
            ),
        }),
    ))
}
