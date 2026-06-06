use crate::api_response::{ApiResponse, IdData};
use crate::error::AppError;
use crate::modules::staff::models::*;
use crate::modules::staff::services::staff_service;
use crate::permissions::registry::codes;
use crate::policies::staff_access_policy;
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
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
struct StaffListData {
    items: Vec<StaffListItem>,
    total: i64,
    page: i64,
    page_size: i64,
    total_pages: i64,
}

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
    // staff_profile.read.school OR legacy staff.read.all OR achievement.create.all
    // (achievement.create.all is used by achievement flows that need a teacher picker)
    actor.require_any_permission(&[
        codes::STAFF_PROFILE_READ_SCHOOL,
        codes::STAFF_READ_ALL,
        codes::ACHIEVEMENT_CREATE_ALL,
    ])?;

    let (items, total, page, page_size) = staff_service::list_staff(&pool, filter).await?;
    let total_pages = (total as f64 / page_size as f64).ceil() as i64;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::ok(StaffListData {
            items,
            total,
            page,
            page_size,
            total_pages,
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
    staff_access_policy::can_read_staff_profile(&pool, &actor, staff_id).await?;
    let include_pii = staff_access_policy::can_read_staff_pii(&actor, staff_id);

    let profile = staff_service::get_staff_profile(&pool, staff_id, include_pii).await?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(profile))).into_response())
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
        Json(ApiResponse::with_message(
            IdData::new(user_id),
            "สร้างบุคลากรสำเร็จ",
        )),
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

    // Roles/organization memberships may have changed — invalidate this user's permission cache
    state.permission_cache.invalidate(&staff_id);
    state.notify_permission_changed(staff_id);

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("อัปเดตข้อมูลสำเร็จ")),
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
        Json(ApiResponse::empty_with_message("ลบบุคลากรสำเร็จ")),
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
    Ok((StatusCode::OK, Json(ApiResponse::ok(data))).into_response())
}
