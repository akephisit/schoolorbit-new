use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;
use crate::db::school_mapping::get_school_database_url;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::middleware::permission::check_permission;
use crate::permissions::registry::codes;
use crate::modules::academic::models::scheduling::*;
use crate::modules::academic::services::scheduler_data::SchedulerDataLoader;
use crate::modules::academic::services::SchedulerBuilder;

/// Helper
async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    
    let db_url = get_school_database_url(&state.admin_pool, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
        
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

/// Auto-schedule timetable
pub async fn auto_schedule_timetable(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateSchedulingJobRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    
    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL).await {
        return Ok(response);
    }
    
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();
    
    // Create job record
    let job_id = Uuid::new_v4();
    let algorithm = payload.algorithm.unwrap_or(SchedulingAlgorithm::Backtracking);
    let config = payload.config.unwrap_or_default();
    
    let classroom_ids_json = serde_json::to_value(&payload.classroom_ids)
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    
    let config_json = serde_json::to_value(&config)
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    
    sqlx::query(
        r#"
        INSERT INTO timetable_scheduling_jobs 
            (id, academic_semester_id, classroom_ids, algorithm, config, status, progress, created_by)
        VALUES ($1, $2, $3, $4::scheduling_algorithm, $5, 'PENDING'::scheduling_job_status, 0, $6)
        "#
    )
    .bind(job_id)
    .bind(payload.academic_semester_id)
    .bind(&classroom_ids_json)
    .bind(match algorithm {
        SchedulingAlgorithm::Greedy => "GREEDY",
        SchedulingAlgorithm::Backtracking => "BACKTRACKING",
        SchedulingAlgorithm::Hybrid => "HYBRID",
    })
    .bind(&config_json)
    .bind(user_id)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    
    // Spawn background task
    let pool_clone = pool.clone();
    let semester_id = payload.academic_semester_id;
    let classrooms = payload.classroom_ids.clone();
    
    // Convert models::SchedulingAlgorithm to services::SchedulingAlgorithm
    let scheduler_algorithm = match algorithm {
        SchedulingAlgorithm::Greedy => crate::modules::academic::services::SchedulingAlgorithm::Greedy,
        SchedulingAlgorithm::Backtracking => crate::modules::academic::services::SchedulingAlgorithm::Backtracking,
        SchedulingAlgorithm::Hybrid => crate::modules::academic::services::SchedulingAlgorithm::Hybrid,
    };
    
    tokio::spawn(async move {
        if let Err(e) = run_scheduling_job(
            job_id,
            semester_id,
            classrooms,
            scheduler_algorithm,
            config,
            &pool_clone,
        ).await {
            eprintln!("Scheduling job {} failed: {}", job_id, e);
            
            // Update job status to failed
            let _ = sqlx::query(
                "UPDATE timetable_scheduling_jobs 
                 SET status = 'FAILED', error_message = $1, updated_at = NOW()
                 WHERE id = $2"
            )
            .bind(e.to_string())
            .bind(job_id)
            .execute(&pool_clone)
            .await;
        }
    });
    
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "job_id": job_id,
            "status": "PENDING",
            "message": "Scheduling job started"
        }))
    ).into_response())
}

/// Get scheduling job status
pub async fn get_scheduling_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(job_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    
    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL).await {
        return Ok(response);
    }
    
    let job = sqlx::query_as::<_, TimetableSchedulingJob>(
        r#"
        SELECT * FROM timetable_scheduling_jobs
        WHERE id = $1
        "#
    )
    .bind(job_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?
    .ok_or(AppError::NotFound("Job not found".to_string()))?;
    
    // Parse and return response
    let classroom_ids: Vec<Uuid> = serde_json::from_value(job.classroom_ids.clone())
        .unwrap_or_default();
    
    let failed_courses: Vec<FailedCourseInfo> = serde_json::from_value(job.failed_courses.clone())
        .unwrap_or_default();
    
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
    
    Ok(Json(response).into_response())
}

/// List scheduling jobs
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
    
    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL).await {
        return Ok(response);
    }
    
    let mut sql = "SELECT * FROM timetable_scheduling_jobs WHERE 1=1".to_string();
    
    if query.semester_id.is_some() {
        sql.push_str(" AND academic_semester_id = $1");
    }
    
    sql.push_str(" ORDER BY created_at DESC LIMIT $2");
    
    let limit = query.limit.unwrap_or(50).min(100);
    
    let jobs = if let Some(semester_id) = query.semester_id {
        sqlx::query_as::<_, TimetableSchedulingJob>(&sql)
            .bind(semester_id)
            .bind(limit)
            .fetch_all(&pool)
            .await
    } else {
        sqlx::query_as::<_, TimetableSchedulingJob>(
            "SELECT * FROM timetable_scheduling_jobs ORDER BY created_at DESC LIMIT $1"
        )
        .bind(limit)
        .fetch_all(&pool)
        .await
    }
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    
    Ok(Json(jobs).into_response())
}

/// Background scheduling task
async fn run_scheduling_job(
    job_id: Uuid,
    semester_id: Uuid,
    classroom_ids: Vec<Uuid>,
    algorithm: crate::modules::academic::services::SchedulingAlgorithm,
    config: SchedulingConfig,
    pool: &PgPool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Update status to RUNNING
    sqlx::query(
        "UPDATE timetable_scheduling_jobs 
         SET status = 'RUNNING', started_at = NOW(), updated_at = NOW()
         WHERE id = $1"
    )
    .bind(job_id)
    .execute(pool)
    .await?;
    
    // Load data
    let loader = SchedulerDataLoader::new(pool);
    
    let courses = loader.load_courses(&classroom_ids, semester_id).await?;
    let available_slots = loader.load_available_slots().await?;
    let periods = loader.load_periods().await?;
    let locked_slots = loader.load_locked_slots(semester_id, &classroom_ids).await?;
    
    // Get academic year for instructor prefs
    let academic_year_id: Uuid = sqlx::query_scalar(
        "SELECT academic_year_id FROM academic_semesters WHERE id = $1"
    )
    .bind(semester_id)
    .fetch_one(pool)
    .await?;
    
    let instructor_prefs = loader.load_instructor_preferences(academic_year_id).await?;
    
    // Update progress
    sqlx::query(
        "UPDATE timetable_scheduling_jobs SET progress = 10, updated_at = NOW() WHERE id = $1"
    )
    .bind(job_id)
    .execute(pool)
    .await?;
    
    // Build scheduler
    let mut scheduler_config = crate::modules::academic::services::scheduler::types::SchedulerConfig::default();
    scheduler_config.algorithm = algorithm.clone();
    scheduler_config.timeout_seconds = config.timeout_seconds.unwrap_or(300);
    scheduler_config.min_quality_score = config.min_quality_score.unwrap_or(70.0);
    scheduler_config.allow_partial = config.allow_partial.unwrap_or(false);
    scheduler_config.force_overwrite = config.force_overwrite.unwrap_or(false);
    
    let scheduler = SchedulerBuilder::new()
        .algorithm(algorithm)
        .timeout_seconds(scheduler_config.timeout_seconds)
        .min_quality_score(scheduler_config.min_quality_score)
        .allow_partial(scheduler_config.allow_partial)
        .build();
    
    // Run scheduling
    sqlx::query(
        "UPDATE timetable_scheduling_jobs SET progress = 20, updated_at = NOW() WHERE id = $1"
    )
    .bind(job_id)
    .execute(pool)
    .await?;
    
    let result = scheduler.schedule(
        courses,
        available_slots,
        locked_slots,
        instructor_prefs,
        periods,
    );
    
    // Save assignments to database
    if config.force_overwrite.unwrap_or(false) {
        // Delete existing entries for these classrooms
        sqlx::query(
            "DELETE FROM academic_timetable_entries 
             WHERE classroom_course_id IN (
                 SELECT id FROM classroom_courses 
                 WHERE classroom_id = ANY($1) AND academic_semester_id = $2
             )"
        )
        .bind(&classroom_ids)
        .bind(semester_id)
        .execute(pool)
        .await?;
    }
    
    // Insert new assignments
    for assignment in &result.assignments {
        sqlx::query(
            r#"
            INSERT INTO academic_timetable_entries 
                (id, classroom_course_id, day_of_week, period_id, room_id)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (classroom_course_id, day_of_week, period_id) DO UPDATE
            SET room_id = EXCLUDED.room_id, updated_at = NOW()
            "#
        )
        .bind(assignment.id)
        .bind(assignment.classroom_course_id)
        .bind(&assignment.time_slot.day)
        .bind(assignment.time_slot.period_id)
        .bind(assignment.room_id)
        .execute(pool)
        .await?;
    }
    
    // Update job with results
    let failed_courses_json = serde_json::to_value(&result.failed_courses)?;
    
    sqlx::query(
        r#"
        UPDATE timetable_scheduling_jobs
        SET status = 'COMPLETED',
            progress = 100,
            quality_score = $1,
            scheduled_courses = $2,
            total_courses = $3,
            failed_courses = $4,
            completed_at = NOW(),
            duration_seconds = $5,
            updated_at = NOW()
        WHERE id = $6
        "#
    )
    .bind(result.quality_score as f32)
    .bind(result.scheduled_courses as i32)
    .bind(result.total_courses as i32)
    .bind(failed_courses_json)
    .bind((result.duration_ms / 1000) as i32)
    .bind(job_id)
    .execute(pool)
    .await?;
    
    Ok(())
}

// ==================== Instructor Preferences ====================

pub async fn create_instructor_preference(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateInstructorPreferenceRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    
    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL).await {
        return Ok(response);
    }
    
    let hard_slots = serde_json::to_value(&payload.hard_unavailable_slots.unwrap_or_default())
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    
    let pref_slots = serde_json::to_value(&payload.preferred_slots.unwrap_or_default())
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    
    let pref_days = serde_json::to_value(&payload.preferred_days.unwrap_or_default())
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    
    let avoid_days = serde_json::to_value(&payload.avoid_days.unwrap_or_default())
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    
    let pref = sqlx::query_as::<_, InstructorPreference>(
        r#"
        INSERT INTO instructor_preferences 
            (instructor_id, academic_year_id, hard_unavailable_slots, preferred_slots,
             max_periods_per_day, min_periods_per_day, preferred_days, avoid_days, notes)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (instructor_id, academic_year_id) DO UPDATE
        SET hard_unavailable_slots = EXCLUDED.hard_unavailable_slots,
            preferred_slots = EXCLUDED.preferred_slots,
            max_periods_per_day = EXCLUDED.max_periods_per_day,
            min_periods_per_day = EXCLUDED.min_periods_per_day,
            preferred_days = EXCLUDED.preferred_days,
            avoid_days = EXCLUDED.avoid_days,
            notes = EXCLUDED.notes,
            updated_at = NOW()
        RETURNING *
        "#
    )
    .bind(payload.instructor_id)
    .bind(payload.academic_year_id)
    .bind(hard_slots)
    .bind(pref_slots)
    .bind(payload.max_periods_per_day)
    .bind(payload.min_periods_per_day)
    .bind(pref_days)
    .bind(avoid_days)
    .bind(payload.notes)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    
    Ok((StatusCode::CREATED, Json(pref)).into_response())
}

// ==================== Instructor Room Assignments ====================

pub async fn create_instructor_room_assignment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateInstructorRoomAssignmentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    
    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL).await {
        return Ok(response);
    }
    
    let for_subjects = serde_json::to_value(&payload.for_subjects.unwrap_or_default())
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    
    let assignment = sqlx::query_as::<_, InstructorRoomAssignment>(
        r#"
        INSERT INTO instructor_room_assignments 
            (instructor_id, room_id, academic_year_id, is_preferred, is_required, for_subjects, reason)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *
        "#
    )
    .bind(payload.instructor_id)
    .bind(payload.room_id)
    .bind(payload.academic_year_id)
    .bind(payload.is_preferred)
    .bind(payload.is_required)
    .bind(for_subjects)
    .bind(payload.reason)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    
    Ok((StatusCode::CREATED, Json(assignment)).into_response())
}

// ==================== Locked Slots ====================

pub async fn create_locked_slot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateLockedSlotRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    
    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL).await {
        return Ok(response);
    }
    
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();
    
    let scope_type = match payload.scope_type {
        LockedSlotScope::Classroom => "CLASSROOM",
        LockedSlotScope::GradeLevel => "GRADE_LEVEL",
        LockedSlotScope::AllSchool => "ALL_SCHOOL",
    };
    
    let scope_ids = serde_json::to_value(&payload.scope_ids)
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    
    let period_ids = serde_json::to_value(&payload.period_ids)
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    
    let locked = sqlx::query_as::<_, TimetableLockedSlot>(
        r#"
        INSERT INTO timetable_locked_slots 
            (academic_semester_id, scope_type, scope_ids, subject_id, day_of_week,
             period_ids, room_id, instructor_id, reason, locked_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING *
        "#
    )
    .bind(payload.academic_semester_id)
    .bind(scope_type)
    .bind(scope_ids)
    .bind(payload.subject_id)
    .bind(payload.day_of_week)
    .bind(period_ids)
    .bind(payload.room_id)
    .bind(payload.instructor_id)
    .bind(payload.reason)
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    
    Ok((StatusCode::CREATED, Json(locked)).into_response())
}

pub async fn list_locked_slots(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListJobsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    
    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL).await {
        return Ok(response);
    }
    
    let slots = if let Some(semester_id) = query.semester_id {
        sqlx::query_as::<_, TimetableLockedSlot>(
            "SELECT * FROM timetable_locked_slots WHERE academic_semester_id = $1"
        )
        .bind(semester_id)
        .fetch_all(&pool)
        .await
    } else {
        sqlx::query_as::<_, TimetableLockedSlot>(
            "SELECT * FROM timetable_locked_slots"
        )
        .fetch_all(&pool)
        .await
    }
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    
    Ok(Json(slots).into_response())
}

pub async fn delete_locked_slot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    
    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL).await {
        return Ok(response);
    }
    
    sqlx::query("DELETE FROM timetable_locked_slots WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    
    Ok(StatusCode::NO_CONTENT.into_response())
}
