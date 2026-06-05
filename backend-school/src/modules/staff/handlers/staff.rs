use crate::error::AppError;
use crate::middleware::permission::load_actor_context;
use crate::modules::staff::models::*;
use crate::modules::staff::services::staff_service;
use crate::permissions::registry::codes;
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    resolve_tenant_pool(state, headers).await
}

// ============================================
// Handlers
// ============================================

pub async fn list_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<StaffListFilter>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    // staff.read.all OR achievement.create.all (latter for non-staff viewing teacher list)
    let actor = load_actor_context(&headers, &pool, &state.permission_cache).await?;
    actor.require_any_permission(&[codes::STAFF_READ_ALL, codes::ACHIEVEMENT_CREATE_ALL])?;

    let (items, total, page, page_size) = staff_service::list_staff(&pool, filter).await?;
    let total_pages = (total as f64 / page_size as f64).ceil() as i64;

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": {
                "items": items,
                "total": total,
                "page": page,
                "page_size": page_size,
                "total_pages": total_pages
            }
        })),
    )
        .into_response())
}

pub async fn get_staff_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(staff_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = load_actor_context(&headers, &pool, &state.permission_cache).await?;
    actor.require_permission(codes::STAFF_READ_ALL)?;

    let profile = staff_service::get_staff_profile(&pool, staff_id).await?;
    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": profile })),
    )
        .into_response())
}

pub async fn create_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateStaffRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = load_actor_context(&headers, &pool, &state.permission_cache).await?;
    actor.require_permission(codes::STAFF_CREATE_ALL)?;

    let user_id = staff_service::create_staff(&pool, payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": { "id": user_id }, "message": "สร้างบุคลากรสำเร็จ" })),
    )
        .into_response())
}

pub async fn update_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(staff_id): Path<Uuid>,
    Json(payload): Json<UpdateStaffRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = load_actor_context(&headers, &pool, &state.permission_cache).await?;
    actor.require_permission(codes::STAFF_UPDATE_ALL)?;

    staff_service::update_staff(&pool, staff_id, payload).await?;

    // Roles/departments may have changed — invalidate this user's permission cache
    state.permission_cache.invalidate(&staff_id);
    state.notify_permission_changed(staff_id);

    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": {}, "message": "อัปเดตข้อมูลสำเร็จ" })),
    )
        .into_response())
}

pub async fn delete_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(staff_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = load_actor_context(&headers, &pool, &state.permission_cache).await?;
    actor.require_permission(codes::STAFF_DELETE_ALL)?;

    staff_service::soft_delete_staff(&pool, staff_id).await?;
    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": {}, "message": "ลบบุคลากรสำเร็จ" })),
    )
        .into_response())
}

/// Public profile — authentication required but no permission check (any logged-in user)
pub async fn get_public_staff_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(staff_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    // Authentication only — verify token without permission check
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());
    let token_from_header = auth_header.and_then(|h| h.strip_prefix("Bearer ").map(str::to_string));
    let token_from_cookie = headers
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .and_then(|cookie| crate::utils::jwt::JwtService::extract_token_from_cookie(Some(cookie)));

    let token = token_from_header
        .or(token_from_cookie)
        .ok_or(AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string()))?;
    crate::utils::jwt::JwtService::verify_token(&token)
        .map_err(|_| AppError::AuthError("Token ไม่ถูกต้อง".to_string()))?;

    let data = staff_service::get_public_staff_profile(&pool, staff_id).await?;
    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": data })),
    )
        .into_response())
}
