use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, HeaderMap},
    Json,
    response::IntoResponse,
};
use serde_json::json;
use crate::middleware::permission::check_permission;
use crate::modules::academic::models::timetable::*;
use uuid::Uuid;
use crate::permissions::registry::codes;
use crate::AppState;
use crate::error::AppError;
use crate::db::school_mapping::get_school_database_url;
use crate::utils::subdomain::extract_subdomain_from_request;
use chrono::NaiveTime;

/// Helper
async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    
    let db_url = get_school_database_url(&state.admin_pool, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
        
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

// ============================================
// Academic Periods API
// ============================================

/// GET /api/academic/periods
pub async fn list_periods(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PeriodQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_STRUCTURE_READ_ALL).await {
        return Ok(response);
    }

    let mut sql = String::from("SELECT * FROM academic_periods WHERE 1=1");
    let mut conditions = Vec::new();

    if let Some(year_id) = query.academic_year_id {
        conditions.push(format!("academic_year_id = '{}'", year_id));
    }

    if let Some(ptype) = &query.period_type {
        conditions.push(format!("type = '{}'", ptype));
    }

    if query.active_only.unwrap_or(false) {
        conditions.push("is_active = true".to_string());
    }

    if !conditions.is_empty() {
        sql.push_str(&format!(" AND {}", conditions.join(" AND ")));
    }

    sql.push_str(" ORDER BY order_index ASC");

    let periods = sqlx::query_as::<_, AcademicPeriod>(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to fetch periods: {}", e);
            AppError::InternalServerError("Failed to fetch periods".to_string())
        })?;

    Ok(Json(json!({ "success": true, "data": periods })).into_response())
}

/// POST /api/academic/periods
pub async fn create_period(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreatePeriodRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_STRUCTURE_MANAGE_ALL).await {
        return Ok(response);
    }

    // Parse time strings
    let start_time = NaiveTime::parse_from_str(&payload.start_time, "%H:%M")
        .map_err(|_| AppError::BadRequest("Invalid start_time format (use HH:MM)".to_string()))?;
    
    let end_time = NaiveTime::parse_from_str(&payload.end_time, "%H:%M")
        .map_err(|_| AppError::BadRequest("Invalid end_time format (use HH:MM)".to_string()))?;

    let period = sqlx::query_as::<_, AcademicPeriod>(
        r#"
        INSERT INTO academic_periods (
            academic_year_id, name, start_time, end_time, type, order_index, applicable_days
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *
        "#
    )
    .bind(payload.academic_year_id)
    .bind(payload.name)
    .bind(start_time)
    .bind(end_time)
    .bind(payload.period_type)
    .bind(payload.order_index)
    .bind(payload.applicable_days)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to create period: {}", e);
        AppError::InternalServerError("Failed to create period".to_string())
    })?;

    Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": period }))).into_response())
}

/// PUT /api/academic/periods/{id}
pub async fn update_period(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdatePeriodRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_STRUCTURE_MANAGE_ALL).await {
        return Ok(response);
    }

    // Parse time if provided
    let start_time = if let Some(ref st) = payload.start_time {
        Some(NaiveTime::parse_from_str(st, "%H:%M")
            .map_err(|_| AppError::BadRequest("Invalid start_time format".to_string()))?)
    } else {
        None
    };

    let end_time = if let Some(ref et) = payload.end_time {
        Some(NaiveTime::parse_from_str(et, "%H:%M")
            .map_err(|_| AppError::BadRequest("Invalid end_time format".to_string()))?)
    } else {
        None
    };

    let period = sqlx::query_as::<_, AcademicPeriod>(
        r#"
        UPDATE academic_periods SET
            name = COALESCE($2, name),
            start_time = COALESCE($3, start_time),
            end_time = COALESCE($4, end_time),
            type = COALESCE($5, type),
            order_index = COALESCE($6, order_index),
            applicable_days = COALESCE($7, applicable_days),
            is_active = COALESCE($8, is_active),
            updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#
    )
    .bind(id)
    .bind(payload.name)
    .bind(start_time)
    .bind(end_time)
    .bind(payload.period_type)
    .bind(payload.order_index)
    .bind(payload.applicable_days)
    .bind(payload.is_active)
    .fetch_one(&pool)
    .await
    .map_err(|_| AppError::NotFound("Period not found".to_string()))?;

    Ok(Json(json!({ "success": true, "data": period })).into_response())
}

/// DELETE /api/academic/periods/{id}
pub async fn delete_period(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_STRUCTURE_MANAGE_ALL).await {
        return Ok(response);
    }

    sqlx::query("DELETE FROM academic_periods WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("foreign key constraint") {
                AppError::BadRequest("Cannot delete period that is used in timetable".to_string())
            } else {
                AppError::InternalServerError("Failed to delete period".to_string())
            }
        })?;

    Ok(Json(json!({ "success": true })).into_response())
}

// ============================================
// Timetable Entries API
// ============================================

/// GET /api/academic/timetable
pub async fn list_timetable_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<TimetableQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL).await {
        return Ok(response);
    }

    let mut sql = String::from(
        r#"
        SELECT 
            te.*,
            s.code as subject_code,
            s.name_th as subject_name_th,
            concat(u.first_name, ' ', u.last_name) as instructor_name,
            cr.name as classroom_name,
            r.code as room_code,
            ap.name as period_name,
            ap.start_time,
            ap.end_time
        FROM academic_timetable_entries te
        JOIN classroom_courses cc ON te.classroom_course_id = cc.id
        JOIN subjects s ON cc.subject_id = s.id
        JOIN class_rooms cr ON cc.classroom_id = cr.id
        JOIN academic_periods ap ON te.period_id = ap.id
        LEFT JOIN users u ON cc.primary_instructor_id = u.id
        LEFT JOIN rooms r ON te.room_id = r.id
        WHERE te.is_active = true
        "#
    );

    let mut conditions = Vec::new();

    if let Some(classroom_id) = query.classroom_id {
        conditions.push(format!("cc.classroom_id = '{}'", classroom_id));
    }

    if let Some(instructor_id) = query.instructor_id {
        conditions.push(format!("cc.primary_instructor_id = '{}'", instructor_id));
    }

    if let Some(room_id) = query.room_id {
        conditions.push(format!("te.room_id = '{}'", room_id));
    }

    if let Some(semester_id) = query.academic_semester_id {
        conditions.push(format!("cc.academic_semester_id = '{}'", semester_id));
    }

    if let Some(ref day) = query.day_of_week {
        conditions.push(format!("te.day_of_week = '{}'", day));
    }

    if !conditions.is_empty() {
        sql.push_str(&format!(" AND {}", conditions.join(" AND ")));
    }

    sql.push_str(" ORDER BY te.day_of_week, ap.order_index");

    let entries = sqlx::query_as::<_, TimetableEntry>(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to fetch timetable: {}", e);
            AppError::InternalServerError("Failed to fetch timetable".to_string())
        })?;

    Ok(Json(json!({ "success": true, "data": entries })).into_response())
}

/// POST /api/academic/timetable
pub async fn create_timetable_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateTimetableEntryRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL).await {
        return Ok(response);
    }

    // Get current user ID for audit
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();

    // Validate conflicts before inserting
    let validation = validate_timetable_entry(&pool, &payload).await?;
    if !validation.is_valid {
        return Ok((
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "message": "Timetable conflict detected",
                "conflicts": validation.conflicts
            }))
        ).into_response());
    }

    let entry = sqlx::query_as::<_, TimetableEntry>(
        r#"
        INSERT INTO academic_timetable_entries (
            classroom_course_id, day_of_week, period_id, room_id, note, created_by, updated_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $6)
        RETURNING *
        "#
    )
    .bind(payload.classroom_course_id)
    .bind(payload.day_of_week)
    .bind(payload.period_id)
    .bind(payload.room_id)
    .bind(payload.note)
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to create timetable entry: {}", e);
        if e.to_string().contains("unique_entry_per_slot") {
            AppError::BadRequest("This slot is already occupied".to_string())
        } else {
            AppError::InternalServerError("Failed to create timetable entry".to_string())
        }
    })?;

    Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": entry }))).into_response())
}

/// DELETE /api/academic/timetable/{id}
pub async fn delete_timetable_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL).await {
        return Ok(response);
    }

    sqlx::query("DELETE FROM academic_timetable_entries WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|_| AppError::InternalServerError("Failed to delete entry".to_string()))?;

    Ok(Json(json!({ "success": true })).into_response())
}

// ============================================
// Conflict Detection Logic
// ============================================

async fn validate_timetable_entry(
    pool: &sqlx::PgPool,
    payload: &CreateTimetableEntryRequest,
) -> Result<TimetableValidationResponse, AppError> {
    let mut conflicts = Vec::new();

    // Get classroom_course info
    let course_info: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT classroom_id, primary_instructor_id FROM classroom_courses WHERE id = $1"
    )
    .bind(payload.classroom_course_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    if course_info.is_none() {
        return Err(AppError::NotFound("Classroom course not found".to_string()));
    }

    let (classroom_id, instructor_id) = course_info.unwrap();

    // 1. Check instructor conflict
    if let Some(instr_id) = instructor_id {
        let has_conflict: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM academic_timetable_entries te
                JOIN classroom_courses cc ON te.classroom_course_id = cc.id
                WHERE cc.primary_instructor_id = $1
                  AND te.day_of_week = $2
                  AND te.period_id = $3
                  AND te.is_active = true
            )
            "#
        )
        .bind(instr_id)
        .bind(&payload.day_of_week)
        .bind(payload.period_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if has_conflict {
            conflicts.push(ConflictInfo {
                conflict_type: "INSTRUCTOR_CONFLICT".to_string(),
                message: "ครูมีตารางสอนในคาบนี้อยู่แล้ว".to_string(),
                existing_entry: None,
            });
        }
    }

    // 2. Check room conflict (if room is specified)
    if let Some(room_id) = payload.room_id {
        let has_conflict: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM academic_timetable_entries
                WHERE room_id = $1
                  AND day_of_week = $2
                  AND period_id = $3
                  AND is_active = true
            )
            "#
        )
        .bind(room_id)
        .bind(&payload.day_of_week)
        .bind(payload.period_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if has_conflict {
            conflicts.push(ConflictInfo {
                conflict_type: "ROOM_CONFLICT".to_string(),
                message: "ห้องเรียนถูกใช้ในคาบนี้อยู่แล้ว".to_string(),
                existing_entry: None,
            });
        }
    }

    Ok(TimetableValidationResponse {
        is_valid: conflicts.is_empty(),
        conflicts,
    })
}
