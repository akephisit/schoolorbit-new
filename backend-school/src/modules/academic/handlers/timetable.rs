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
        LEFT JOIN classroom_courses cc ON te.classroom_course_id = cc.id
        LEFT JOIN subjects s ON cc.subject_id = s.id
        JOIN class_rooms cr ON te.classroom_id = cr.id
        JOIN academic_periods ap ON te.period_id = ap.id
        LEFT JOIN users u ON cc.primary_instructor_id = u.id
        LEFT JOIN rooms r ON te.room_id = r.id
        WHERE te.is_active = true
        "#
    );

    let mut conditions = Vec::new();

    if let Some(classroom_id) = query.classroom_id {
        conditions.push(format!("te.classroom_id = '{}'", classroom_id));
    }

    if let Some(instructor_id) = query.instructor_id {
        conditions.push(format!("cc.primary_instructor_id = '{}'", instructor_id));
    }

    if let Some(room_id) = query.room_id {
        conditions.push(format!("te.room_id = '{}'", room_id));
    }

    if let Some(semester_id) = query.academic_semester_id {
        conditions.push(format!("te.academic_semester_id = '{}'", semester_id));
    }

    if let Some(ref day) = query.day_of_week {
        conditions.push(format!("te.day_of_week = '{}'", day));
    }

    if let Some(ref entry_type) = query.entry_type {
        conditions.push(format!("te.entry_type = '{}'", entry_type));
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

    // Lookup classroom_id and semester_id from course
    let (classroom_id, academic_semester_id) = if let Some(course_id) = payload.classroom_course_id {
         let info: Option<(Uuid, Uuid)> = sqlx::query_as(
            "SELECT classroom_id, academic_semester_id FROM classroom_courses WHERE id = $1"
         )
         .bind(course_id)
         .fetch_optional(&pool)
         .await
         .map_err(|e| AppError::InternalServerError(e.to_string()))?;
         
         match info {
             Some(i) => i,
             None => return Err(AppError::NotFound("Classroom course not found".to_string()))
         }
    } else {
         return Err(AppError::BadRequest("Course ID is required for regular timetable entry".to_string()));
    };

    let entry = sqlx::query_as::<_, TimetableEntry>(
        r#"
        INSERT INTO academic_timetable_entries (
            id, classroom_course_id, day_of_week, period_id, room_id, note, 
            classroom_id, academic_semester_id, entry_type, is_active,
            created_by, updated_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'COURSE', true, $9, $9)
        RETURNING *
        "#
    )
    .bind(Uuid::new_v4())
    .bind(payload.classroom_course_id)
    .bind(payload.day_of_week)
    .bind(payload.period_id)
    .bind(payload.room_id)
    .bind(payload.note)
    .bind(classroom_id)
    .bind(academic_semester_id)
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

    // 1. Check instructor conflict (only if attached to a course)
    if let Some(course_id) = payload.classroom_course_id {
        // Get classroom_course info
        let course_info: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
            "SELECT classroom_id, primary_instructor_id FROM classroom_courses WHERE id = $1"
        )
        .bind(course_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

        if course_info.is_none() {
             return Err(AppError::NotFound("Classroom course not found".to_string()));
        }

        if let Some((cls_id, Some(instr_id))) = course_info {
            // 1.1 Check INSTRUCTOR conflict
            let instructor_conflict: bool = sqlx::query_scalar(
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

            if instructor_conflict {
                conflicts.push(ConflictInfo {
                    conflict_type: "INSTRUCTOR_CONFLICT".to_string(),
                    message: "ครูมีตารางสอนในคาบนี้อยู่แล้ว".to_string(),
                    existing_entry: None,
                });
            }

            // 1.2 Check CLASSROOM (Student) conflict
            // Check if this classroom already has a class (course, activity, etc) in this slot
            // Note: entry can be COURSE (linked to classroom_course -> classroom_id) 
            // OR explicit classroom_id (for non-course entries).
            // Our DB schema update ensures all entries usually have classroom_id populated.
            // But let's check robustly.
            
            let classroom_conflict: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM academic_timetable_entries te
                    LEFT JOIN classroom_courses cc ON te.classroom_course_id = cc.id
                    WHERE (te.classroom_id = $1 OR cc.classroom_id = $1)
                      AND te.day_of_week = $2
                      AND te.period_id = $3
                      AND te.is_active = true
                )
                "#
            )
            .bind(cls_id)
            .bind(&payload.day_of_week)
            .bind(payload.period_id)
            .fetch_one(pool)
            .await
            .unwrap_or(false);

            if classroom_conflict {
                conflicts.push(ConflictInfo {
                    conflict_type: "CLASSROOM_CONFLICT".to_string(),
                    message: "ห้องเรียนนี้มีตารางในคาบนี้อยู่แล้ว".to_string(),
                    existing_entry: None,
                });
            }
        } else if let Some((cls_id, None)) = course_info {
             // Case: Course has no instructor, but we stil need to check Classroom Conflict
            let classroom_conflict: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM academic_timetable_entries te
                    LEFT JOIN classroom_courses cc ON te.classroom_course_id = cc.id
                    WHERE (te.classroom_id = $1 OR cc.classroom_id = $1)
                      AND te.day_of_week = $2
                      AND te.period_id = $3
                      AND te.is_active = true
                )
                "#
            )
            .bind(cls_id)
            .bind(&payload.day_of_week)
            .bind(payload.period_id)
            .fetch_one(pool)
            .await
            .unwrap_or(false);

            if classroom_conflict {
                conflicts.push(ConflictInfo {
                    conflict_type: "CLASSROOM_CONFLICT".to_string(),
                    message: "ห้องเรียนนี้มีตารางในคาบนี้อยู่แล้ว".to_string(),
                    existing_entry: None,
                });
            }
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

/// PUT /api/academic/timetable/{id}
pub async fn update_timetable_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTimetableEntryRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL).await {
        return Ok(response);
    }
    
    // Get current user ID for audit
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();

    // 1. Fetch existing entry to get classroom_course_id for validation
    let existing_entry = sqlx::query_as::<_, TimetableEntry>(
        r#"
        SELECT te.*, NULL as subject_code, NULL as subject_name_th, NULL as instructor_name,
               NULL as classroom_name, NULL as room_code, NULL as period_name,
               NULL as start_time, NULL as end_time
        FROM academic_timetable_entries te WHERE id = $1
        "#
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|_| AppError::NotFound("Entry not found".to_string()))?;

    // 2. Prepare mock CreateRequest for validation (using new values or fallback to existing)
    let validation_payload = CreateTimetableEntryRequest {
        classroom_course_id: existing_entry.classroom_course_id,
        day_of_week: payload.day_of_week.clone().unwrap_or(existing_entry.day_of_week),
        period_id: payload.period_id.unwrap_or(existing_entry.period_id),
        room_id: payload.room_id.or(existing_entry.room_id),
        note: payload.note.clone().or(existing_entry.note),
    };

    // 3. Validate conflicts (BUT need to exclude current entry ID)
    // NOTE: validation logic needs to support exclusion or we check manually
    // For now, let's implement a manual check specific for update to avoid big refactor
    
    // Check Instructor Conflict (Manual)
    let course_info: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT classroom_id, primary_instructor_id FROM classroom_courses WHERE id = $1"
    )
    .bind(existing_entry.classroom_course_id)
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();

    if let Some((_, Some(instr_id))) = course_info {
        let has_conflict: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM academic_timetable_entries te
                JOIN classroom_courses cc ON te.classroom_course_id = cc.id
                WHERE cc.primary_instructor_id = $1
                  AND te.day_of_week = $2
                  AND te.period_id = $3
                  AND te.is_active = true
                  AND te.id != $4  -- Exclude current entry
            )
            "#
        )
        .bind(instr_id)
        .bind(&validation_payload.day_of_week)
        .bind(validation_payload.period_id)
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap_or(false);

        if has_conflict {
            return Ok((
                StatusCode::CONFLICT,
                Json(json!({
                    "success": false,
                    "message": "Instructor conflict detected",
                    "conflicts": [{
                        "conflict_type": "INSTRUCTOR_CONFLICT",
                        "message": "ครูมีตารางสอนในคาบนี้อยู่แล้ว"
                    }]
                }))
            ).into_response());
        }
    }

    // 4. Update Entry
    let updated_entry = sqlx::query_as::<_, TimetableEntry>(
        r#"
        UPDATE academic_timetable_entries SET
            day_of_week = COALESCE($2, day_of_week),
            period_id = COALESCE($3, period_id),
            room_id = COALESCE($4, room_id),
            note = COALESCE($5, note),
            updated_at = NOW(),
            updated_by = $6
        WHERE id = $1
        RETURNING *
        "#
    )
    .bind(id)
    .bind(payload.day_of_week)
    .bind(payload.period_id)
    .bind(payload.room_id)
    .bind(payload.note)
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to update entry: {}", e);
        if e.to_string().contains("unique_entry_per_slot") {
            AppError::BadRequest("This slot is already occupied".to_string())
        } else {
            AppError::InternalServerError("Failed to update entry".to_string())
        }
    })?;

    Ok(Json(json!({ "success": true, "data": updated_entry })).into_response())
}

/// POST /api/academic/timetable/batch
pub async fn create_batch_timetable_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateBatchTimetableEntriesRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL).await {
        return Ok(response);
    }

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();
    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    for classroom_id in payload.classroom_ids {
        let result = sqlx::query(
            r#"
            INSERT INTO academic_timetable_entries (
                id, classroom_id, academic_semester_id, day_of_week, period_id, room_id, 
                entry_type, title, is_active, created_by, updated_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, true, $9, $9)
            ON CONFLICT DO NOTHING
            "#
        )
        .bind(Uuid::new_v4())
        .bind(classroom_id)
        .bind(payload.academic_semester_id)
        .bind(&payload.day_of_week)
        .bind(payload.period_id)
        .bind(payload.room_id)
        .bind(&payload.entry_type)
        .bind(&payload.title)
        .bind(user_id)
        .execute(&mut *tx)
        .await;

        if let Err(e) = result {
             eprintln!("Failed to batch insert for classroom {}: {}", classroom_id, e);
             return Err(AppError::InternalServerError("Failed to batch create entries".to_string()));
        }
    }

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ 
        "success": true, 
        "message": "Batch entries created successfully" 
    })).into_response())
}
