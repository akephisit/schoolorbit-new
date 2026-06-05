use std::collections::HashMap;

use axum::{
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::auth::models::Claims;
use crate::modules::consent::models::CreateConsentRequest;
use crate::modules::consent::services::{self as consent_service, ConsentRequestContext};
use crate::utils::request_context::{current_user_tenant_context_from_claims, tenant_pool};
use crate::AppState;

fn request_context(headers: &HeaderMap) -> ConsentRequestContext {
    let ip_address = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let user_agent = headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    ConsentRequestContext {
        ip_address,
        user_agent,
    }
}

/// Get all consent types (filtered by user type)
/// GET /api/consent/types?user_type=student
pub async fn get_consent_types(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let user_type = query
        .get("user_type")
        .map(String::as_str)
        .unwrap_or("student");
    let responses = consent_service::list_consent_types(&pool, user_type).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "success": true, "data": responses })),
    ))
}

/// Get user's consent status
/// GET /api/consent/my-status
pub async fn get_my_consent_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_claims(&state, &headers, &claims).await?;
    let status =
        consent_service::get_user_consent_status(&context.tenant.pool, context.user_id).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "success": true, "data": status })),
    ))
}

/// Give consent
/// POST /api/consent
pub async fn create_consent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CreateConsentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_claims(&state, &headers, &claims).await?;
    let consent_id = consent_service::create_consent(
        &context.tenant.pool,
        context.user_id,
        payload,
        request_context(&headers),
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(
            serde_json::json!({ "success": true, "data": { "consent_id": consent_id }, "message": "บันทึกความยินยอมสำเร็จ" }),
        ),
    ))
}

/// Withdraw consent
/// POST /api/consent/:id/withdraw
pub async fn withdraw_consent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Path(consent_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_claims(&state, &headers, &claims).await?;
    consent_service::withdraw_consent(&context.tenant.pool, context.user_id, consent_id).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "success": true, "data": {}, "message": "ถอนความยินยอมสำเร็จ" })),
    ))
}

/// Get consent summary (Admin only)
/// GET /api/consent/summary
pub async fn get_consent_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let summary = consent_service::get_consent_summary(&pool).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "success": true, "data": summary })),
    ))
}
