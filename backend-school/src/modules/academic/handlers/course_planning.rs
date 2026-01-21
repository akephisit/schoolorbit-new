use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
    response::IntoResponse,
    http::HeaderMap,
};
use serde_json::json;
use crate::middleware::permission::check_permission;
use crate::modules::academic::models::course_planning::{
    ClassroomCourse, PlanQuery, AssignCoursesRequest, UpdateCourseRequest
};
use uuid::Uuid;
use crate::permissions::registry::codes;
use crate::AppState;
use crate::error::AppError;
use crate::db::school_mapping::get_school_database_url;
use crate::utils::subdomain::extract_subdomain_from_request;

/// Helper
async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    
    let db_url = get_school_database_url(&state.admin_pool, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
        
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

/// GET /api/academic/planning/courses
pub async fn list_classroom_courses(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PlanQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL).await {
        return Ok(response);
    }

    let mut sql = String::from(
        r#"
        SELECT 
            cc.*,
            s.code as subject_code,
            s.name_th as subject_name_th,
            s.name_en as subject_name_en,
            s.credit as subject_credit,
            concat(u.firstname, ' ', u.lastname) as instructor_name
        FROM classroom_courses cc
        JOIN subjects s ON cc.subject_id = s.id
        LEFT JOIN users u ON cc.primary_instructor_id = u.id
        WHERE cc.classroom_id = $1
        "#
    );

    if let Some(term_id) = query.academic_semester_id {
        sql.push_str(&format!(" AND cc.academic_semester_id = '{}'", term_id));
    }

    sql.push_str(" ORDER BY s.code ASC");

    let courses = sqlx::query_as::<_, ClassroomCourse>(&sql)
        .bind(query.classroom_id)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to fetch courses: {}", e);
            AppError::InternalServerError("Failed to fetch courses".to_string())
        })?;

    Ok(Json(json!({ "success": true, "data": courses })).into_response())
}

/// POST /api/academic/planning/courses
pub async fn assign_courses(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AssignCoursesRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL).await {
        return Ok(response);
    }

    // Verify classroom exists
    let _exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM classrooms WHERE id = $1)")
        .bind(payload.classroom_id)
        .fetch_one(&pool)
        .await
        .unwrap_or(false);
        
    if !_exists {
        return Err(AppError::NotFound("Classroom not found".to_string()));
    }

    let mut added_count = 0;

    for subject_id in payload.subject_ids {
        // Insert if not exists
        let result = sqlx::query(
            r#"
            INSERT INTO classroom_courses (classroom_id, academic_semester_id, subject_id)
            VALUES ($1, $2, $3)
            ON CONFLICT (classroom_id, academic_semester_id, subject_id) DO NOTHING
            "#
        )
        .bind(payload.classroom_id)
        .bind(payload.academic_semester_id)
        .bind(subject_id)
        .execute(&pool)
        .await;

        if let Ok(res) = result {
            if res.rows_affected() > 0 {
                added_count += 1;
            }
        }
    }

    Ok(Json(json!({ 
        "success": true, 
        "message": format!("Assigned {} courses", added_count) 
    })).into_response())
}

/// DELETE /api/academic/planning/courses/:id
pub async fn remove_course(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL).await {
        return Ok(response);
    }

    sqlx::query("DELETE FROM classroom_courses WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|_| AppError::InternalServerError("Failed to remove course".to_string()))?;

    Ok(Json(json!({ "success": true })).into_response())
}

/// PUT /api/academic/planning/courses/:id
pub async fn update_course(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateCourseRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL).await {
        return Ok(response);
    }
    
    // Use simple query with COALESCE for now

    
    // Simple update query
    let result = sqlx::query(
        r#"
        UPDATE classroom_courses SET 
            primary_instructor_id = COALESCE($1, primary_instructor_id),
            settings = COALESCE($2, settings),
            updated_at = NOW()
        WHERE id = $3
        RETURNING *
        "#
    )
    .bind(payload.primary_instructor_id)
    .bind(payload.settings)
    .bind(id)
    .execute(&pool)
    .await;
    
    match result {
        Ok(_) => Ok(Json(json!({ "success": true })).into_response()),
        Err(e) => {
             eprintln!("Update error: {}", e);
             Err(AppError::InternalServerError("Failed to update course".to_string()))
        }
    }
}
