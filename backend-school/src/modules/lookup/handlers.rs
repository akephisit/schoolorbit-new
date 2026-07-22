use axum::{
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::auth::models::Claims;
use crate::modules::facility::models::Room;
use crate::modules::lookup::models::{
    AcademicYearLookupItem, ClassroomLookupItem, GradeLevelLookupItem, LookupItem, LookupQuery,
    OrganizationUnitLookupItem, RoleLookupItem, StaffLookupItem, StudentLookupItem,
};
use crate::modules::lookup::services as lookup_service;
use crate::permissions::registry::codes;
use crate::utils::request_context::{
    actor_tenant_context, current_user_tenant_context_from_claims, CurrentUserTenantContext,
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
#[utoipa::path(
    get,
    path = "/api/lookup/staff",
    operation_id = "lookupStaff",
    tag = "lookup",
    params(LookupQuery),
    responses(
        (status = 200, description = "Staff lookup items", body = ApiResponse<Vec<StaffLookupItem>>),
        (status = 401, description = "Authentication required or account inactive", body = ApiErrorResponse)
    )
)]
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
#[utoipa::path(
    get,
    path = "/api/lookup/roles",
    operation_id = "lookupRoles",
    tag = "lookup",
    params(LookupQuery),
    responses(
        (status = 200, description = "Role lookup items", body = ApiResponse<Vec<RoleLookupItem>>),
        (status = 401, description = "Authentication required or account inactive", body = ApiErrorResponse),
        (status = 403, description = "Role lookup permission denied", body = ApiErrorResponse)
    )
)]
pub async fn lookup_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    lookup_service::verify_active_user(&context.tenant.pool, context.actor.user_id).await?;
    context
        .actor
        .require_any_permission(&[codes::ROLES_READ_ALL, codes::ROLES_ASSIGN_ALL])?;
    let data = lookup_service::lookup_roles(&context.tenant.pool, query).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(data))))
}

/// GET /api/lookup/organization-units
/// Returns organization unit data. Supports ?member_only=true to filter to user's own units.
#[utoipa::path(
    get,
    path = "/api/lookup/organization-units",
    operation_id = "lookupOrganizationUnits",
    tag = "lookup",
    params(LookupQuery),
    responses(
        (status = 200, description = "Organization unit lookup items", body = ApiResponse<Vec<OrganizationUnitLookupItem>>),
        (status = 401, description = "Authentication required or account inactive", body = ApiErrorResponse)
    )
)]
pub async fn lookup_organization_units(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = active_lookup_context(&state, &headers, &claims).await?;
    let data =
        lookup_service::lookup_organization_units(&context.tenant.pool, context.user_id, query)
            .await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(data))))
}

/// GET /api/lookup/organization-units/:id
/// Returns single organization unit by ID (auth only, no permission required)
#[utoipa::path(
    get,
    path = "/api/lookup/organization-units/{id}",
    operation_id = "getLookupOrganizationUnit",
    tag = "lookup",
    params(("id" = Uuid, Path, description = "Organization unit ID")),
    responses(
        (status = 200, description = "Organization unit lookup item", body = ApiResponse<OrganizationUnitLookupItem>),
        (status = 401, description = "Authentication required or account inactive", body = ApiErrorResponse),
        (status = 404, description = "Organization unit not found", body = ApiErrorResponse)
    )
)]
pub async fn lookup_organization_unit_by_id(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Extension(claims): Extension<Claims>,
) -> Result<impl IntoResponse, AppError> {
    let context = active_lookup_context(&state, &headers, &claims).await?;
    let unit = lookup_service::lookup_organization_unit_by_id(&context.tenant.pool, id).await?;

    Ok(Json(ApiResponse::ok(unit)).into_response())
}

/// GET /api/lookup/grade-levels
/// Returns minimal grade level data for dropdowns
#[utoipa::path(
    get,
    path = "/api/lookup/grade-levels",
    operation_id = "lookupGradeLevels",
    tag = "lookup",
    params(LookupQuery),
    responses(
        (status = 200, description = "Grade level lookup items", body = ApiResponse<Vec<GradeLevelLookupItem>>),
        (status = 401, description = "Authentication required or account inactive", body = ApiErrorResponse)
    )
)]
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
#[utoipa::path(
    get,
    path = "/api/lookup/classrooms",
    operation_id = "lookupClassrooms",
    tag = "lookup",
    params(LookupQuery),
    responses(
        (status = 200, description = "Classroom lookup items", body = ApiResponse<Vec<ClassroomLookupItem>>),
        (status = 401, description = "Authentication required or account inactive", body = ApiErrorResponse)
    )
)]
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
#[utoipa::path(
    get,
    path = "/api/lookup/academic-years",
    operation_id = "lookupAcademicYears",
    tag = "lookup",
    params(LookupQuery),
    responses(
        (status = 200, description = "Academic year lookup items", body = ApiResponse<Vec<AcademicYearLookupItem>>),
        (status = 401, description = "Authentication required or account inactive", body = ApiErrorResponse)
    )
)]
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
#[utoipa::path(
    get,
    path = "/api/lookup/students",
    operation_id = "lookupStudents",
    tag = "lookup",
    params(LookupQuery),
    responses(
        (status = 200, description = "Student lookup items", body = ApiResponse<Vec<StudentLookupItem>>),
        (status = 401, description = "Authentication required or account inactive", body = ApiErrorResponse)
    )
)]
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
#[utoipa::path(
    get,
    path = "/api/lookup/rooms",
    operation_id = "lookupRooms",
    tag = "lookup",
    responses(
        (status = 200, description = "Active room lookup items", body = ApiResponse<Vec<Room>>),
        (status = 401, description = "Authentication required or account inactive", body = ApiErrorResponse)
    )
)]
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
#[utoipa::path(
    get,
    path = "/api/lookup/subjects",
    operation_id = "lookupSubjects",
    tag = "lookup",
    params(LookupQuery),
    responses(
        (status = 200, description = "Subject lookup items", body = ApiResponse<Vec<LookupItem>>),
        (status = 401, description = "Authentication required or account inactive", body = ApiErrorResponse)
    )
)]
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
