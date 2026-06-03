use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::db::school_mapping::get_school_database_url;
use crate::error::AppError;
use crate::middleware::permission::check_permission;
use crate::modules::academic::models::scheduling::*;
use crate::modules::academic::services::scheduling_service;
use crate::permissions::registry::codes;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

pub async fn auto_schedule_timetable(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateSchedulingJobRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();
    let job_id = Uuid::new_v4();
    let algorithm = payload.algorithm.unwrap_or(SchedulingAlgorithm::Backtracking);
    let config = payload.config.unwrap_or_default();

    let label = match algorithm {
        SchedulingAlgorithm::Greedy => "GREEDY",
        SchedulingAlgorithm::Backtracking => "BACKTRACKING",
        SchedulingAlgorithm::Hybrid => "HYBRID",
    };

    scheduling_service::create_scheduling_job(
        &pool, job_id, payload.academic_semester_id, &payload.classroom_ids, label, &config, user_id
    ).await?;

    let scheduler_algorithm = match algorithm {
        SchedulingAlgorithm::Greedy => crate::modules::academic::services::SchedulingAlgorithm::Greedy,
        SchedulingAlgorithm::Backtracking => crate::modules::academic::services::SchedulingAlgorithm::Backtracking,
        SchedulingAlgorithm::Hybrid => crate::modules::academic::services::SchedulingAlgorithm::Hybrid,
    };

    let pool_clone = pool.clone();
    let semester_id = payload.academic_semester_id;
    let classrooms = payload.classroom_ids.clone();

    tokio::spawn(async move {
        if let Err(e) = scheduling_service::run_scheduling_job(
            job_id, semester_id, classrooms, scheduler_algorithm, config, &pool_clone
        ).await {
            eprintln!("Scheduling job {} failed: {}", job_id, e);
            scheduling_service::mark_job_failed(&pool_clone, job_id, e.to_string()).await;
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "success": true, "data": { "job_id": job_id, "status": "PENDING" }, "message": "Scheduling job started" }))
    ).into_response())
}

pub async fn get_scheduling_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(job_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let job = scheduling_service::get_scheduling_job(&pool, job_id).await?;

    let classroom_ids: Vec<Uuid> = serde_json::from_value(job.classroom_ids.clone()).unwrap_or_default();
    let failed_courses: Vec<FailedCourseInfo> = serde_json::from_value(job.failed_courses.clone()).unwrap_or_default();

    let response = SchedulingJobResponse {
        id: job.id,
        academic_semester_id: job.academic_semester_id,
        classroom_ids,
        algorithm: match job.algorithm.as_str() {
            "GREEDY" => SchedulingAlgorithm::Greedy,
            "BACKTRACKING" => SchedulingAlgorithm::Backtracking,
            "HYBRID" => SchedulingAlgorithm::Hybrid,
            _ => SchedulingAlgorithm::Backtracking,
        },
        status: match job.status.as_str() {
            "PENDING" => SchedulingStatus::Pending,
            "RUNNING" => SchedulingStatus::Running,
            "COMPLETED" => SchedulingStatus::Completed,
            "FAILED" => SchedulingStatus::Failed,
            "CANCELLED" => SchedulingStatus::Cancelled,
            _ => SchedulingStatus::Pending,
        },
        progress: job.progress.unwrap_or(0),
        quality_score: job.quality_score.map(|f| f as f64),
        scheduled_courses: job.scheduled_courses.unwrap_or(0),
        total_courses: job.total_courses.unwrap_or(0),
        failed_courses,
        started_at: job.started_at,
        completed_at: job.completed_at,
        duration_seconds: job.duration_seconds,
        error_message: job.error_message,
        created_by: job.created_by,
        created_at: job.created_at,
    };

    Ok(Json(serde_json::json!({ "success": true, "data": response })).into_response())
}

pub async fn undo_scheduling_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(job_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let (semester_id, deleted) = scheduling_service::undo_scheduling_job(&pool, job_id).await?;

    if let Some(sid) = semester_id {
        let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();
        state.websocket_manager.broadcast_mutation(
            subdomain, sid,
            crate::modules::academic::websockets::TimetableEvent::TableRefresh {
                user_id: user_id.unwrap_or_default()
            },
        );
    }

    Ok(Json(serde_json::json!({ "success": true, "data": { "deleted": deleted } })).into_response())
}

#[derive(Deserialize)]
pub struct ListJobsQuery {
    semester_id: Option<Uuid>,
    limit: Option<i64>,
}

pub async fn list_scheduling_jobs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListJobsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let limit = query.limit.unwrap_or(50).min(100);
    let jobs = scheduling_service::list_scheduling_jobs(&pool, query.semester_id, limit).await?;
    Ok(Json(serde_json::json!({ "success": true, "data": jobs })).into_response())
}

pub async fn create_instructor_preference(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateInstructorPreferenceRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let pref = scheduling_service::create_instructor_preference(&pool, payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "success": true, "data": pref })),
    )
        .into_response())
}

pub async fn create_instructor_room_assignment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateInstructorRoomAssignmentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let a = scheduling_service::create_instructor_room_assignment(&pool, payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "success": true, "data": a })),
    )
        .into_response())
}

pub async fn create_locked_slot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateLockedSlotRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();
    let locked = scheduling_service::create_locked_slot(&pool, payload, user_id).await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "success": true, "data": locked })),
    )
        .into_response())
}

pub async fn list_locked_slots(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListJobsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let slots = scheduling_service::list_locked_slots(&pool, query.semester_id).await?;
    Ok(Json(serde_json::json!({ "success": true, "data": slots })).into_response())
}

pub async fn delete_locked_slot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    scheduling_service::delete_locked_slot(&pool, id).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
