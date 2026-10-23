use axum::{
    extract::{
        rejection::{JsonRejection, QueryRejection},
        Extension, Path, Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::access_policy as student_access_policy;
use crate::http::state::StudentHttpState;
use crate::models::{
    CreateStudentRequest, ListStudentsQuery, StudentAcademicYearQuery, StudentRegisterQuery,
    UpdateOwnProfileRequest, UpdateStudentRequest,
};
#[cfg(feature = "api-docs")]
use crate::models::{StudentProfile, StudentRegisterProfile};
use crate::services as student_service;
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
#[cfg(feature = "api-docs")]
use school_http::ApiErrorResponse;
use school_http::ApiResponse;
use school_http::HttpError as AppError;
use school_permissions::registry::codes;

/// GET /api/student/profile - นักเรียนดูข้อมูลตนเอง
#[cfg_attr(feature = "api-docs", utoipa::path(
    get,
    path = "/api/student/profile",
    operation_id = "getStudentProfile",
    tag = "student",
    params(StudentAcademicYearQuery),
    responses(
        (status = 200, description = "Current student's scoped profile", body = ApiResponse<StudentProfile>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student profile access denied", body = ApiErrorResponse),
        (status = 404, description = "Student profile not found", body = ApiErrorResponse)
    )
))]
pub async fn get_own_profile(
    State(state): State<StudentHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<StudentAcademicYearQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let include_pii =
        student_access_policy::authorize_student_profile_read(&pool, &actor, actor.user_id).await?;
    let student =
        student_service::get_own_profile(&pool, actor.user_id, query.academic_year_id, include_pii)
            .await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(student))))
}

/// PUT /api/student/profile - นักเรียนแก้ไขข้อมูลตนเอง (จำกัดฟิลด์)
#[cfg_attr(feature = "api-docs", utoipa::path(
    put,
    path = "/api/student/profile",
    operation_id = "updateStudentProfile",
    tag = "student",
    request_body = UpdateOwnProfileRequest,
    responses(
        (status = 200, description = "Current student profile updated", body = ApiResponse<school_http::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student profile access denied", body = ApiErrorResponse)
    )
))]
pub async fn update_own_profile(
    State(state): State<StudentHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<UpdateOwnProfileRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    student_service::update_own_profile(&pool, actor.user_id, payload).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("อัพเดตข้อมูลสำเร็จ")),
    ))
}

/// GET /api/students - รายชื่อนักเรียนทั้งหมด
#[cfg_attr(feature = "api-docs", utoipa::path(
    get,
    path = "/api/students",
    operation_id = "listStudents",
    tag = "student",
    params(ListStudentsQuery),
    responses(
        (status = 200, description = "Permanent student register in the authorized scope", body = ApiResponse<crate::models::StudentListResponse>),
        (status = 400, description = "Invalid register filters or pagination", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student list access denied", body = ApiErrorResponse)
    )
))]
pub async fn list_students(
    State(state): State<StudentHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    query: Result<Query<ListStudentsQuery>, QueryRejection>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let access = student_access_policy::resolve_student_list_access(&actor)?;
    let Query(filter) =
        query.map_err(|_| AppError::BadRequest("ตัวกรองทะเบียนนักเรียนไม่ถูกต้อง".into()))?;

    let students = student_service::list_students(&pool, filter, access).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(students))))
}

/// POST /api/students - เพิ่มนักเรียนใหม่
#[cfg_attr(feature = "api-docs", utoipa::path(
    post,
    path = "/api/students",
    operation_id = "createStudent",
    tag = "student",
    request_body = CreateStudentRequest,
    responses(
        (status = 201, description = "Student created", body = ApiResponse<crate::models::CreateStudentResponse>),
        (status = 400, description = "Invalid or duplicate student data", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student creation permission denied", body = ApiErrorResponse),
        (status = 409, description = "Required default role or parent account is unavailable", body = ApiErrorResponse),
        (status = 503, description = "Password processing is temporarily at capacity", body = ApiErrorResponse)
    )
))]
pub async fn create_student(
    State(state): State<StudentHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    payload: Result<Json<CreateStudentRequest>, JsonRejection>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::STUDENT_CREATE_ALL)?;
    let Json(payload) = payload.map_err(|_| AppError::BadRequest("ข้อมูลนักเรียนไม่ถูกต้อง".into()))?;

    let created = student_service::create_student(&pool, actor.user_id, payload).await?;
    for parent_id in &created.parent_ids {
        state
            .auth_runtime
            .invalidate_permission_user(&context.tenant.subdomain, *parent_id);
    }
    for parent_id in &created.parent_ids {
        state
            .auth_runtime
            .notify_permission_changed(&context.tenant.subdomain, *parent_id);
    }
    let student = created.student;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::with_message(student, "เพิ่มนักเรียนสำเร็จ")),
    ))
}

/// GET /api/students/:id - ดูข้อมูลนักเรียน
#[cfg_attr(feature = "api-docs", utoipa::path(
    get,
    path = "/api/students/{id}",
    operation_id = "getStudent",
    tag = "student",
    params(
        ("id" = Uuid, Path, description = "Student user ID"),
        StudentRegisterQuery
    ),
    responses(
        (status = 200, description = "Permanent student profile with optional annual context", body = ApiResponse<StudentRegisterProfile>),
        (status = 400, description = "Invalid student profile context", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student profile access denied", body = ApiErrorResponse),
        (status = 404, description = "Student profile not found", body = ApiErrorResponse)
    )
))]
pub async fn get_student(
    State(state): State<StudentHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(student_id): Path<Uuid>,
    query: Result<Query<StudentRegisterQuery>, QueryRejection>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let include_pii =
        student_access_policy::authorize_student_profile_read(&pool, &actor, student_id).await?;
    let Query(query) = query.map_err(|_| AppError::BadRequest("บริบทข้อมูลนักเรียนไม่ถูกต้อง".into()))?;

    let student =
        student_service::get_student(&pool, student_id, query.academic_year_id, include_pii)
            .await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(student))))
}

/// PUT /api/students/:id - แก้ไขข้อมูลนักเรียน
#[cfg_attr(feature = "api-docs", utoipa::path(
    put,
    path = "/api/students/{id}",
    operation_id = "updateStudent",
    tag = "student",
    params(("id" = Uuid, Path, description = "Student user ID")),
    request_body = UpdateStudentRequest,
    responses(
        (status = 200, description = "Student updated", body = ApiResponse<school_http::EmptyData>),
        (status = 400, description = "Invalid or duplicate student profile data", body = ApiErrorResponse),
        (status = 409, description = "Permanent student record requires reconciliation", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Student not found", body = ApiErrorResponse)
    )
))]
pub async fn update_student(
    State(state): State<StudentHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(student_id): Path<Uuid>,
    payload: Result<Json<UpdateStudentRequest>, JsonRejection>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::STUDENT_UPDATE_ALL)?;
    let Json(payload) = payload.map_err(|_| AppError::BadRequest("ข้อมูลนักเรียนไม่ถูกต้อง".into()))?;

    student_service::update_student(&pool, student_id, actor.user_id, payload).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("อัพเดตข้อมูลนักเรียนสำเร็จ")),
    ))
}

/// DELETE /api/students/:id - ลบนักเรียน (soft delete)
#[cfg_attr(feature = "api-docs", utoipa::path(
    delete,
    path = "/api/students/{id}",
    operation_id = "deleteStudent",
    tag = "student",
    params(("id" = Uuid, Path, description = "Student user ID")),
    responses(
        (status = 200, description = "Student deactivated", body = ApiResponse<school_http::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student deletion permission denied", body = ApiErrorResponse),
        (status = 404, description = "Student not found", body = ApiErrorResponse)
    )
))]
pub async fn delete_student(
    State(state): State<StudentHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(student_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let tenant = context.tenant.subdomain.clone();
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::STUDENT_DELETE_ALL)?;

    student_service::delete_student(&pool, student_id, actor.user_id, state.placement_rosters)
        .await?;
    state
        .auth_runtime
        .invalidate_permission_user(&tenant, student_id);
    state
        .auth_runtime
        .notify_permission_changed(&tenant, student_id);

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("ลบนักเรียนสำเร็จ")),
    ))
}
