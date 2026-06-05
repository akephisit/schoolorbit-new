use crate::error::AppError;
use crate::modules::staff::models::*;
use crate::modules::staff::services::staff_service;
use crate::permissions::registry::codes;
use crate::utils::request_context::{
    actor_tenant_context, current_user_tenant_context_from_headers,
};
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

// ============================================
// Handlers
// ============================================

pub async fn list_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<StaffListFilter>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    // staff.read.all OR achievement.create.all (latter for non-staff viewing teacher list)
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
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
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
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
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
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
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
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
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
    let context = current_user_tenant_context_from_headers(&state, &headers).await?;
    let pool = context.tenant.pool;

    let data = staff_service::get_public_staff_profile(&pool, staff_id).await?;
    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": data })),
    )
        .into_response())
}
