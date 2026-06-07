use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::academic::models::curriculum::{
    AddSubjectDefaultInstructorRequest, CreateSubjectRequest, SubjectFilter,
    UpdateSubjectDefaultInstructorRoleRequest, UpdateSubjectRequest,
};
use crate::modules::academic::services::subject_service;
use crate::permissions::registry::codes;
use crate::policies::curriculum_access_policy;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

pub async fn list_subject_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    if curriculum_access_policy::resolve_subject_read_access(&actor, &pool)
        .await?
        .is_none()
    {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(ApiErrorResponse::new(format!(
                "ไม่มีสิทธิ์ {}",
                codes::ACADEMIC_CURRICULUM_READ_ALL
            ))),
        )
            .into_response());
    }

    let groups = subject_service::list_subject_groups(&pool).await?;
    Ok(Json(ApiResponse::ok(groups)).into_response())
}

pub async fn list_subjects(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<SubjectFilter>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let Some(access) = curriculum_access_policy::resolve_subject_read_access(&actor, &pool).await?
    else {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(ApiErrorResponse::new(format!(
                "ไม่มีสิทธิ์ {}",
                codes::ACADEMIC_CURRICULUM_READ_ALL
            ))),
        )
            .into_response());
    };

    let subjects = subject_service::list_subjects(&pool, filter, access).await?;
    Ok(Json(ApiResponse::ok(subjects)).into_response())
}

pub async fn create_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateSubjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let Some(access) = curriculum_access_policy::resolve_subject_manage_access(
        &actor,
        &pool,
        codes::ACADEMIC_CURRICULUM_CREATE_ALL,
    )
    .await?
    else {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(ApiErrorResponse::new(format!(
                "ไม่มีสิทธิ์ {}",
                codes::ACADEMIC_CURRICULUM_CREATE_ALL
            ))),
        )
            .into_response());
    };

    if !subject_service::subject_group_access_allows(&access, payload.group_id) {
        return Err(AppError::BadRequest(
            "ไม่สามารถเพิ่มวิชาในกลุ่มสาระอื่นได้".to_string(),
        ));
    }

    let subject = subject_service::create_subject(&pool, payload).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(subject))).into_response())
}

pub async fn update_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSubjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let Some(access) = curriculum_access_policy::resolve_subject_manage_access(
        &actor,
        &pool,
        codes::ACADEMIC_CURRICULUM_UPDATE_ALL,
    )
    .await?
    else {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(ApiErrorResponse::new(format!(
                "ไม่มีสิทธิ์ {}",
                codes::ACADEMIC_CURRICULUM_UPDATE_ALL
            ))),
        )
            .into_response());
    };

    let subject_group = subject_service::get_subject_group_id(&pool, id).await?;
    if !subject_service::subject_group_access_allows(&access, subject_group)
        || payload.group_id.is_some_and(|group_id| {
            !subject_service::subject_group_access_allows(&access, Some(group_id))
        })
    {
        return Err(AppError::BadRequest(
            "ไม่สามารถแก้ไขวิชาในกลุ่มสาระอื่นได้".to_string(),
        ));
    }

    let subject = subject_service::update_subject(&pool, id, payload).await?;
    Ok(Json(ApiResponse::ok(subject)).into_response())
}

pub async fn delete_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let Some(access) = curriculum_access_policy::resolve_subject_manage_access(
        &actor,
        &pool,
        codes::ACADEMIC_CURRICULUM_DELETE_ALL,
    )
    .await?
    else {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(ApiErrorResponse::new(format!(
                "ไม่มีสิทธิ์ {}",
                codes::ACADEMIC_CURRICULUM_DELETE_ALL
            ))),
        )
            .into_response());
    };

    let subject_group = subject_service::get_subject_group_id(&pool, id).await?;
    if !subject_service::subject_group_access_allows(&access, subject_group) {
        return Err(AppError::BadRequest(
            "ไม่สามารถลบวิชาในกลุ่มสาระอื่นได้".to_string(),
        ));
    }

    subject_service::delete_subject(&pool, id).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

pub async fn list_subject_default_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subject_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_subject_manage(
        &actor,
        &pool,
        subject_id,
        codes::ACADEMIC_CURRICULUM_UPDATE_ALL,
        true,
    )
    .await?;
    let rows = subject_service::list_subject_default_instructors(&pool, subject_id).await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

pub async fn add_subject_default_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subject_id): Path<Uuid>,
    Json(body): Json<AddSubjectDefaultInstructorRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_subject_manage(
        &actor,
        &pool,
        subject_id,
        codes::ACADEMIC_CURRICULUM_UPDATE_ALL,
        false,
    )
    .await?;
    subject_service::add_subject_default_instructor(&pool, subject_id, body).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

pub async fn remove_subject_default_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((subject_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_subject_manage(
        &actor,
        &pool,
        subject_id,
        codes::ACADEMIC_CURRICULUM_UPDATE_ALL,
        false,
    )
    .await?;
    subject_service::remove_subject_default_instructor(&pool, subject_id, instructor_id).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

pub async fn update_subject_default_instructor_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((subject_id, instructor_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateSubjectDefaultInstructorRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_subject_manage(
        &actor,
        &pool,
        subject_id,
        codes::ACADEMIC_CURRICULUM_UPDATE_ALL,
        false,
    )
    .await?;
    subject_service::update_subject_default_instructor_role(
        &pool,
        subject_id,
        instructor_id,
        &body.role,
    )
    .await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

#[derive(Debug, serde::Deserialize)]
pub struct BatchListSubjectDefaultInstructorsQuery {
    pub subject_ids: String,
}

pub async fn batch_list_subject_default_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<BatchListSubjectDefaultInstructorsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let Some(access) = curriculum_access_policy::resolve_subject_read_access(&actor, &pool).await?
    else {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(ApiErrorResponse::new(format!(
                "ไม่มีสิทธิ์ {}",
                codes::ACADEMIC_CURRICULUM_READ_ALL
            ))),
        )
            .into_response());
    };

    let ids: Vec<Uuid> = query
        .subject_ids
        .split(',')
        .filter_map(|s| s.trim().parse::<Uuid>().ok())
        .collect();

    let grouped =
        subject_service::batch_list_subject_default_instructors(&pool, ids, &access).await?;
    Ok(Json(ApiResponse::ok(grouped)).into_response())
}
