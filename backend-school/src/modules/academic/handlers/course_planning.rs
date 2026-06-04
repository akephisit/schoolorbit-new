use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::load_actor_context;
use crate::modules::academic::models::course_planning::{
    AddCourseInstructorRequest, AssignCoursesRequest, PlanQuery, UpdateCourseInstructorRoleRequest,
    UpdateCourseRequest,
};
use crate::modules::academic::services::course_planning_service;
use crate::modules::academic::websockets::TimetableEvent;
use crate::permissions::registry::codes;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    resolve_tenant_pool(state, headers).await
}

async fn broadcast_course_refresh(
    state: &AppState,
    headers: &HeaderMap,
    pool: &sqlx::PgPool,
    course_id: Uuid,
) {
    if let Some(sem_id) = course_planning_service::get_course_semester_id(pool, course_id).await {
        let user_id = crate::middleware::auth::extract_user_id(headers, pool)
            .await
            .ok();
        let subdomain =
            extract_subdomain_from_request(headers).unwrap_or_else(|_| "default".to_string());
        state.websocket_manager.broadcast_mutation(
            subdomain,
            sem_id,
            TimetableEvent::CourseTeamChanged {
                user_id: user_id.unwrap_or_default(),
                course_id,
            },
        );
    }
}

pub async fn list_classroom_courses(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PlanQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL) {
        return Ok(response);
    }
    let courses = course_planning_service::list_classroom_courses(&pool, &query).await?;
    Ok(Json(json!({ "success": true, "data": courses })).into_response())
}

pub async fn assign_courses(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AssignCoursesRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL) {
        return Ok(response);
    }
    let added = course_planning_service::assign_courses(&pool, payload).await?;
    Ok(Json(
        json!({ "success": true, "data": {}, "message": format!("Assigned {} courses", added) }),
    )
    .into_response())
}

pub async fn remove_course(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL) {
        return Ok(response);
    }
    course_planning_service::remove_course(&pool, id).await?;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

pub async fn update_course(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateCourseRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL) {
        return Ok(response);
    }
    course_planning_service::update_course(&pool, id, payload).await?;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

#[derive(Debug, serde::Deserialize)]
pub struct BatchListCourseInstructorsQuery {
    pub course_ids: String,
}

pub async fn batch_list_course_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<BatchListCourseInstructorsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL) {
        return Ok(response);
    }
    let ids: Vec<Uuid> = query
        .course_ids
        .split(',')
        .filter_map(|s| s.trim().parse::<Uuid>().ok())
        .collect();
    let grouped = course_planning_service::batch_list_course_instructors(&pool, &ids).await?;
    Ok(Json(json!({ "success": true, "data": grouped })).into_response())
}

pub async fn list_course_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(course_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL) {
        return Ok(response);
    }
    let rows = course_planning_service::list_course_instructors(&pool, course_id).await?;
    Ok(Json(json!({ "success": true, "data": rows })).into_response())
}

pub async fn add_course_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(course_id): Path<Uuid>,
    Json(body): Json<AddCourseInstructorRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL) {
        return Ok(response);
    }
    let role = body.role.unwrap_or_else(|| "secondary".to_string());
    course_planning_service::add_course_instructor(&pool, course_id, body.instructor_id, &role)
        .await?;
    broadcast_course_refresh(&state, &headers, &pool, course_id).await;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

pub async fn remove_course_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((course_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL) {
        return Ok(response);
    }
    course_planning_service::remove_course_instructor(&pool, course_id, instructor_id).await?;
    broadcast_course_refresh(&state, &headers, &pool, course_id).await;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

pub async fn update_course_instructor_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((course_id, instructor_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateCourseInstructorRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL) {
        return Ok(response);
    }
    course_planning_service::update_course_instructor_role(
        &pool,
        course_id,
        instructor_id,
        &body.role,
    )
    .await?;
    broadcast_course_refresh(&state, &headers, &pool, course_id).await;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct ClassroomActivity {
    pub slot_id: Uuid,
    pub activity_catalog_id: Uuid,
    pub name: String,
    pub activity_type: String,
    pub periods_per_week: i32,
    pub scheduling_mode: String,
    pub is_active: bool,
}

#[derive(serde::Deserialize)]
pub struct ClassroomActivityQuery {
    pub semester_id: Uuid,
}

pub async fn list_classroom_activities(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(classroom_id): Path<Uuid>,
    Query(query): Query<ClassroomActivityQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL) {
        return Ok(response);
    }
    let rows =
        course_planning_service::list_classroom_activities(&pool, classroom_id, query.semester_id)
            .await?;
    Ok(Json(json!({ "success": true, "data": rows })).into_response())
}

pub async fn remove_classroom_from_slot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((classroom_id, slot_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL) {
        return Ok(response);
    }
    course_planning_service::remove_classroom_from_slot(&pool, classroom_id, slot_id).await?;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}
