use axum::{
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::auth::models::Claims;
use crate::modules::lookup::models::LookupQuery;
use crate::modules::lookup::services as lookup_service;
use crate::utils::request_context::{
    current_user_tenant_context_from_claims, CurrentUserTenantContext,
};
use crate::AppState;

async fn active_lookup_context(
    state: &AppState,
    headers: &HeaderMap,
    claims: &Claims,
) -> Result<CurrentUserTenantContext, AppError> {
    let context = current_user_tenant_context_from_claims(state, headers, claims).await?;
    lookup_service::verify_active_user(&context.tenant.pool, context.user_id).await?;

    Ok(context)
}

/// GET /api/lookup/staff
/// Returns minimal staff data for dropdowns (id, name, title)
pub async fn lookup_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = active_lookup_context(&state, &headers, &claims).await?;
    let data = lookup_service::lookup_staff(&context.tenant.pool, query).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(data))))
}

/// GET /api/lookup/roles
/// Returns minimal role data for dropdowns
pub async fn lookup_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = active_lookup_context(&state, &headers, &claims).await?;
    let data = lookup_service::lookup_roles(&context.tenant.pool, query).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(data))))
}

/// GET /api/lookup/departments
/// Returns department data. Supports ?member_only=true to filter to user's own depts.
pub async fn lookup_departments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = active_lookup_context(&state, &headers, &claims).await?;
    let data =
        lookup_service::lookup_departments(&context.tenant.pool, context.user_id, query).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(data))))
}

/// GET /api/lookup/departments/:id
/// Returns single department by ID (auth only, no permission required)
pub async fn lookup_department_by_id(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
) -> Result<impl IntoResponse, AppError> {
    let context = active_lookup_context(&state, &headers, &claims).await?;
    let department = lookup_service::lookup_department_by_id(&context.tenant.pool, id).await?;

    Ok(Json(ApiResponse::ok(department)).into_response())
}

/// GET /api/lookup/grade-levels
/// Returns minimal grade level data for dropdowns
pub async fn lookup_grade_levels(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = active_lookup_context(&state, &headers, &claims).await?;
    let data = lookup_service::lookup_grade_levels(&context.tenant.pool, query).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(data))))
}

/// GET /api/lookup/classrooms
/// Returns minimal classroom data for dropdowns
pub async fn lookup_classrooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = active_lookup_context(&state, &headers, &claims).await?;
    let data = lookup_service::lookup_classrooms(&context.tenant.pool, query).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(data))))
}

/// GET /api/lookup/academic-years
/// Returns minimal academic year data for dropdowns
pub async fn lookup_academic_years(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = active_lookup_context(&state, &headers, &claims).await?;
    let data = lookup_service::lookup_academic_years(&context.tenant.pool, query).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(data))))
}

/// GET /api/lookup/students
/// Returns minimal student data for dropdowns (with student_id and class_room)
pub async fn lookup_students(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = active_lookup_context(&state, &headers, &claims).await?;
    let data = lookup_service::lookup_students(&context.tenant.pool, query).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(data))))
}

/// GET /api/lookup/rooms
/// Returns active rooms with building info
pub async fn lookup_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
) -> Result<impl IntoResponse, AppError> {
    let context = active_lookup_context(&state, &headers, &claims).await?;
    let rooms = lookup_service::lookup_rooms(&context.tenant.pool).await?;

    Ok(Json(ApiResponse::ok(rooms)).into_response())
}

/// GET /api/lookup/subjects
/// Returns minimal subject data for dropdowns
pub async fn lookup_subjects(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = active_lookup_context(&state, &headers, &claims).await?;
    let data = lookup_service::lookup_subjects(&context.tenant.pool, query).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(data))))
}
