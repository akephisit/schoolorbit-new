use std::collections::HashMap;

use axum::{
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use super::models::UserConsentStatus;
use crate::modules::consent::models::CreateConsentRequest;
use crate::modules::consent::services::{self as consent_service, ConsentRequestContext};
use crate::utils::request_context::current_user_tenant_context_from_session;
use crate::utils::tenant::tenant_pool;
use crate::AppState;
use school_auth::session_service::AuthenticatedSession;
use school_http::HttpError as AppError;
use school_http::{ApiErrorResponse, ApiResponse, EmptyData};

#[derive(Debug, Serialize)]
struct CreateConsentData {
    consent_id: Uuid,
}

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

    Ok((StatusCode::OK, Json(ApiResponse::ok(responses))))
}

/// Get user's consent status
/// GET /api/consent/my-status
#[utoipa::path(
    get, path = "/api/consent/my-status", operation_id = "getMyConsentStatus", tag = "consent",
    responses(
        (status = 200, description = "Owned consent status", body = ApiResponse<UserConsentStatus>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse)
    )
)]
pub async fn get_my_consent_status(
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_session(&session);
    let status =
        consent_service::get_user_consent_status(&context.tenant.pool, context.user_id).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(status))))
}

/// Give consent
/// POST /api/consent
pub async fn create_consent(
    headers: HeaderMap,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<CreateConsentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_session(&session);
    let consent_id = consent_service::create_consent(
        &context.tenant.pool,
        context.user_id,
        payload,
        request_context(&headers),
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::with_message(
            CreateConsentData { consent_id },
            "บันทึกความยินยอมสำเร็จ",
        )),
    ))
}

/// Withdraw consent
/// POST /api/consent/:id/withdraw
#[utoipa::path(
    post, path = "/api/consent/{id}/withdraw", operation_id = "withdrawOwnConsent", tag = "consent",
    params(("id" = Uuid, Path, description = "Owned consent record")),
    responses(
        (status = 200, description = "Owned optional consent withdrawn", body = ApiResponse<EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 400, description = "Required consent cannot be withdrawn", body = ApiErrorResponse),
        (status = 404, description = "Owned consent not found", body = ApiErrorResponse)
    )
)]
pub async fn withdraw_consent(
    Extension(session): Extension<AuthenticatedSession>,
    Path(consent_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_session(&session);
    consent_service::withdraw_consent(&context.tenant.pool, context.user_id, consent_id).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("ถอนความยินยอมสำเร็จ")),
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

    Ok((StatusCode::OK, Json(ApiResponse::ok(summary))))
}
