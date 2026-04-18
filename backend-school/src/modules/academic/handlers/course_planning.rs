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
    ClassroomCourse, PlanQuery, AssignCoursesRequest, UpdateCourseRequest,
    CourseInstructor, AddCourseInstructorRequest, UpdateCourseInstructorRoleRequest
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
    
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
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

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL, &state.permission_cache).await {
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
            s.hours_per_semester as subject_hours,
            s.type as subject_type,
            concat(u.first_name, ' ', u.last_name) as instructor_name,
            cr.name as classroom_name
        FROM classroom_courses cc
        JOIN subjects s ON cc.subject_id = s.id
        LEFT JOIN users u ON cc.primary_instructor_id = u.id
        JOIN class_rooms cr ON cc.classroom_id = cr.id
        WHERE 1=1
        "#
    );

    let mut idx = 0u32;
    let mut has_filter = false;

    if let Some(_) = query.classroom_id {
        idx += 1;
        sql.push_str(&format!(" AND cc.classroom_id = ${idx}"));
        has_filter = true;
    }

    if let Some(_) = query.instructor_id {
        idx += 1;
        sql.push_str(&format!(" AND cc.primary_instructor_id = ${idx}"));
        has_filter = true;
    }

    if let Some(_) = query.academic_semester_id {
        idx += 1;
        sql.push_str(&format!(" AND cc.academic_semester_id = ${idx}"));
        has_filter = true;
    }

    if let Some(_) = query.subject_id {
        idx += 1;
        sql.push_str(&format!(" AND cc.subject_id = ${idx}"));
        has_filter = true;
    }

    if !has_filter {
        // Prevent loading absolutely everything if no filter provided.
        // For safety, return nothing if no main filter.
        return Ok(Json(json!({ "success": true, "data": [] })).into_response());
    }

    sql.push_str(" ORDER BY s.code ASC");

    let mut q = sqlx::query_as::<_, ClassroomCourse>(&sql);
    if let Some(classroom_id) = query.classroom_id { q = q.bind(classroom_id); }
    if let Some(instructor_id) = query.instructor_id { q = q.bind(instructor_id); }
    if let Some(term_id) = query.academic_semester_id { q = q.bind(term_id); }
    if let Some(subject_id) = query.subject_id { q = q.bind(subject_id); }
    let courses = q
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

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(response);
    }

    // Verify classroom exists
    let _exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM class_rooms WHERE id = $1)")
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
            INSERT INTO classroom_courses (
                classroom_id, academic_semester_id, subject_id, primary_instructor_id
            )
            SELECT $1, $2, s.id, s.default_instructor_id
            FROM subjects s
            WHERE s.id = $3
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

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
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

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
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

/// GET /api/academic/planning/courses/:id/instructors
pub async fn list_course_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(course_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let rows: Vec<CourseInstructor> = sqlx::query_as(
        r#"SELECT cci.*, concat(u.first_name, ' ', u.last_name) AS instructor_name
           FROM classroom_course_instructors cci
           JOIN users u ON u.id = cci.instructor_id
           WHERE cci.classroom_course_id = $1
           ORDER BY cci.role, cci.created_at"#
    )
    .bind(course_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(Json(json!({ "data": rows })).into_response())
}

/// POST /api/academic/planning/courses/:id/instructors
pub async fn add_course_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(course_id): Path<Uuid>,
    Json(body): Json<AddCourseInstructorRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let role = body.role.unwrap_or_else(|| "secondary".to_string());
    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // If inserting as primary, demote existing primary to secondary
    if role == "primary" {
        sqlx::query(
            "UPDATE classroom_course_instructors SET role = 'secondary'
             WHERE classroom_course_id = $1 AND role = 'primary'"
        ).bind(course_id).execute(&mut *tx).await
          .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }

    sqlx::query(
        "INSERT INTO classroom_course_instructors (classroom_course_id, instructor_id, role)
         VALUES ($1, $2, $3)
         ON CONFLICT (classroom_course_id, instructor_id) DO UPDATE SET role = EXCLUDED.role"
    )
    .bind(course_id)
    .bind(body.instructor_id)
    .bind(&role)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sync_primary_instructor(&mut *tx, course_id).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(Json(json!({ "success": true })).into_response())
}

/// DELETE /api/academic/planning/courses/:id/instructors/:uid
pub async fn remove_course_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((course_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;
    sqlx::query("DELETE FROM classroom_course_instructors WHERE classroom_course_id = $1 AND instructor_id = $2")
        .bind(course_id).bind(instructor_id).execute(&mut *tx).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    sync_primary_instructor(&mut *tx, course_id).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(Json(json!({ "success": true })).into_response())
}

/// PUT /api/academic/planning/courses/:id/instructors/:uid
pub async fn update_course_instructor_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((course_id, instructor_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateCourseInstructorRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;
    if body.role == "primary" {
        sqlx::query(
            "UPDATE classroom_course_instructors SET role = 'secondary'
             WHERE classroom_course_id = $1 AND role = 'primary' AND instructor_id <> $2"
        ).bind(course_id).bind(instructor_id).execute(&mut *tx).await
          .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }
    sqlx::query(
        "UPDATE classroom_course_instructors SET role = $3
         WHERE classroom_course_id = $1 AND instructor_id = $2"
    ).bind(course_id).bind(instructor_id).bind(&body.role).execute(&mut *tx).await
      .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    sync_primary_instructor(&mut *tx, course_id).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(Json(json!({ "success": true })).into_response())
}

/// Keep classroom_courses.primary_instructor_id in sync with the oldest primary (or oldest instructor) in the junction.
async fn sync_primary_instructor(
    tx: &mut sqlx::PgConnection,
    course_id: Uuid,
) -> Result<(), sqlx::Error> {
    let chosen: Option<Uuid> = sqlx::query_scalar(
        "SELECT instructor_id FROM classroom_course_instructors
         WHERE classroom_course_id = $1
         ORDER BY (role = 'primary') DESC, created_at ASC LIMIT 1"
    ).bind(course_id).fetch_optional(&mut *tx).await?;
    sqlx::query(
        "UPDATE classroom_courses SET primary_instructor_id = $1 WHERE id = $2"
    ).bind(chosen).bind(course_id).execute(&mut *tx).await?;
    Ok(())
}
