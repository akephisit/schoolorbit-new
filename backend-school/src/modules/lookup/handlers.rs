use axum::{
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::auth::models::Claims;
use crate::modules::lookup::models::{LookupQuery, LookupResponse};
use crate::modules::lookup::services as lookup_service;
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    resolve_tenant_pool(state, headers).await
}

async fn require_active_user(claims: &Claims, pool: &sqlx::PgPool) -> Result<Uuid, AppError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::AuthError("Invalid user ID".to_string()))?;
    lookup_service::verify_active_user(pool, user_id).await?;

    Ok(user_id)
}

/// GET /api/lookup/staff
/// Returns minimal staff data for dropdowns (id, name, title)
pub async fn lookup_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    require_active_user(&claims, &pool).await?;
    let data = lookup_service::lookup_staff(&pool, query).await?;

    Ok((
        StatusCode::OK,
        Json(LookupResponse {
            success: true,
            data,
        }),
    ))
}

/// GET /api/lookup/roles
/// Returns minimal role data for dropdowns
pub async fn lookup_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    require_active_user(&claims, &pool).await?;
    let data = lookup_service::lookup_roles(&pool, query).await?;

    Ok((
        StatusCode::OK,
        Json(LookupResponse {
            success: true,
            data,
        }),
    ))
}

/// GET /api/lookup/departments
/// Returns department data. Supports ?member_only=true to filter to user's own depts.
pub async fn lookup_departments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = require_active_user(&claims, &pool).await?;
    let data = lookup_service::lookup_departments(&pool, user_id, query).await?;

    Ok((
        StatusCode::OK,
        Json(LookupResponse {
            success: true,
            data,
        }),
    ))
}

/// GET /api/lookup/departments/:id
/// Returns single department by ID (auth only, no permission required)
pub async fn lookup_department_by_id(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    require_active_user(&claims, &pool).await?;
    let department = lookup_service::lookup_department_by_id(&pool, id).await?;

    Ok(Json(json!({ "success": true, "data": department })).into_response())
}

/// GET /api/lookup/grade-levels
/// Returns minimal grade level data for dropdowns
pub async fn lookup_grade_levels(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    require_active_user(&claims, &pool).await?;
    let data = lookup_service::lookup_grade_levels(&pool, query).await?;

    Ok((
        StatusCode::OK,
        Json(LookupResponse {
            success: true,
            data,
        }),
    ))
}

/// GET /api/lookup/classrooms
/// Returns minimal classroom data for dropdowns
pub async fn lookup_classrooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    require_active_user(&claims, &pool).await?;
    let data = lookup_service::lookup_classrooms(&pool, query).await?;

    Ok((
        StatusCode::OK,
        Json(LookupResponse {
            success: true,
            data,
        }),
    ))
}

/// GET /api/lookup/academic-years
/// Returns minimal academic year data for dropdowns
pub async fn lookup_academic_years(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    require_active_user(&claims, &pool).await?;
    let data = lookup_service::lookup_academic_years(&pool, query).await?;

    Ok((
        StatusCode::OK,
        Json(LookupResponse {
            success: true,
            data,
        }),
    ))
}

/// GET /api/lookup/students
/// Returns minimal student data for dropdowns (with student_id and class_room)
pub async fn lookup_students(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    require_active_user(&claims, &pool).await?;
    let data = lookup_service::lookup_students(&pool, query).await?;

    Ok((
        StatusCode::OK,
        Json(LookupResponse {
            success: true,
            data,
        }),
    ))
}

/// GET /api/lookup/rooms
/// Returns active rooms with building info
pub async fn lookup_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    require_active_user(&claims, &pool).await?;
    let rooms = lookup_service::lookup_rooms(&pool).await?;

    Ok(Json(json!({ "success": true, "data": rooms })).into_response())
}

/// GET /api/lookup/subjects
/// Returns minimal subject data for dropdowns
pub async fn lookup_subjects(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    require_active_user(&claims, &pool).await?;
    let data = lookup_service::lookup_subjects(&pool, query).await?;

    Ok((
        StatusCode::OK,
        Json(LookupResponse {
            success: true,
            data,
        }),
    ))
}
