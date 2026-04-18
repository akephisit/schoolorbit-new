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
use crate::modules::academic::websockets::TimetableEvent;
use chrono::NaiveTime;

/// Helper
async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
        
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

/// Populate timetable_entry_instructors from the source table for a newly-created entry.
/// Uses &PgPool only; callers inside transactions should inline the same INSERTs.
async fn populate_entry_instructors(
    pool: &sqlx::PgPool,
    entry_id: Uuid,
    classroom_course_id: Option<Uuid>,
    activity_slot_id: Option<Uuid>,
    classroom_id: Uuid,
) -> Result<(), sqlx::Error> {
    if let Some(cc_id) = classroom_course_id {
        sqlx::query(
            "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
             SELECT $1, instructor_id, role FROM classroom_course_instructors
             WHERE classroom_course_id = $2
             ON CONFLICT DO NOTHING"
        )
        .bind(entry_id)
        .bind(cc_id)
        .execute(pool)
        .await?;
        return Ok(());
    }

    if let Some(slot_id) = activity_slot_id {
        let mode: Option<String> = sqlx::query_scalar(
            "SELECT scheduling_mode FROM activity_slots WHERE id = $1"
        )
        .bind(slot_id)
        .fetch_optional(pool)
        .await?;

        match mode.as_deref() {
            Some("independent") => {
                sqlx::query(
                    "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                     SELECT $1, instructor_id, 'primary'
                     FROM activity_slot_classroom_assignments
                     WHERE slot_id = $2 AND classroom_id = $3
                     ON CONFLICT DO NOTHING"
                )
                .bind(entry_id)
                .bind(slot_id)
                .bind(classroom_id)
                .execute(pool)
                .await?;
            }
            _ => {
                sqlx::query(
                    "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                     SELECT $1, user_id, 'primary'
                     FROM activity_slot_instructors
                     WHERE slot_id = $2
                     ON CONFLICT DO NOTHING"
                )
                .bind(entry_id)
                .bind(slot_id)
                .execute(pool)
                .await?;
            }
        }
    }

    Ok(())
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

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_STRUCTURE_READ_ALL, &state.permission_cache).await {
        return Ok(response);
    }

    let mut sql = String::from("SELECT * FROM academic_periods WHERE 1=1");
    let mut idx = 0u32;

    if let Some(_) = query.academic_year_id {
        idx += 1;
        sql.push_str(&format!(" AND academic_year_id = ${idx}"));
    }

    if query.active_only.unwrap_or(false) {
        sql.push_str(" AND is_active = true");
    }

    sql.push_str(" ORDER BY order_index ASC");

    let mut q = sqlx::query_as::<_, AcademicPeriod>(&sql);
    if let Some(year_id) = query.academic_year_id {
        q = q.bind(year_id);
    }
    let periods = q
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

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_STRUCTURE_MANAGE_ALL, &state.permission_cache).await {
        return Ok(response);
    }

    // Parse time strings
    let start_time = NaiveTime::parse_from_str(&payload.start_time, "%H:%M")
        .map_err(|_| AppError::BadRequest("Invalid start_time format (use HH:MM)".to_string()))?;
    
    let end_time = NaiveTime::parse_from_str(&payload.end_time, "%H:%M")
        .map_err(|_| AppError::BadRequest("Invalid end_time format (use HH:MM)".to_string()))?;

    if end_time <= start_time {
        return Err(AppError::BadRequest("เวลาจบต้องมากกว่าเวลาเริ่ม".to_string()));
    }

    let period = sqlx::query_as::<_, AcademicPeriod>(
        r#"
        INSERT INTO academic_periods (
            academic_year_id, name, start_time, end_time, order_index, applicable_days
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#
    )
    .bind(payload.academic_year_id)
    .bind(payload.name)
    .bind(start_time)
    .bind(end_time)
    .bind(payload.order_index)
    .bind(payload.applicable_days)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to create period: {}", e);
        let msg = if e.to_string().contains("valid_time_range") {
            "เวลาจบต้องมากกว่าเวลาเริ่ม"
        } else if e.to_string().contains("unique_period_per_year") {
            "ลำดับคาบซ้ำกับที่มีอยู่แล้ว"
        } else {
            "ไม่สามารถสร้างคาบเรียนได้"
        };
        AppError::BadRequest(msg.to_string())
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

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_STRUCTURE_MANAGE_ALL, &state.permission_cache).await {
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
            order_index = COALESCE($5, order_index),
            applicable_days = COALESCE($6, applicable_days),
            is_active = COALESCE($7, is_active),
            updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#
    )
    .bind(id)
    .bind(payload.name)
    .bind(start_time)
    .bind(end_time)
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

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_STRUCTURE_MANAGE_ALL, &state.permission_cache).await {
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

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL, &state.permission_cache).await {
        return Ok(response);
    }

    let mut sql = String::from(
        r#"
        SELECT
            te.*,
            s.code   AS subject_code,
            s.name_th AS subject_name_th,
            CASE
                WHEN u.id IS NOT NULL THEN concat(u.first_name, ' ', u.last_name)
                WHEN u2.id IS NOT NULL THEN concat(u2.first_name, ' ', u2.last_name)
                ELSE NULL
            END AS instructor_name,
            cr.name  AS classroom_name,
            r.code   AS room_code,
            ap.name  AS period_name,
            ap.start_time,
            ap.end_time,
            ap.start_time,
            ap.end_time,
            asl.name AS activity_slot_name,
            asl.activity_type AS activity_type,
            asl.scheduling_mode AS activity_scheduling_mode
        FROM academic_timetable_entries te
        LEFT JOIN classroom_courses cc ON te.classroom_course_id = cc.id
        LEFT JOIN subjects s ON cc.subject_id = s.id
        JOIN class_rooms cr ON te.classroom_id = cr.id
        JOIN academic_periods ap ON te.period_id = ap.id
        LEFT JOIN users u ON cc.primary_instructor_id = u.id
        LEFT JOIN rooms r ON te.room_id = r.id
        LEFT JOIN activity_slots asl ON te.activity_slot_id = asl.id
        LEFT JOIN activity_slot_classroom_assignments asca ON asca.slot_id = te.activity_slot_id AND asca.classroom_id = te.classroom_id
        LEFT JOIN users u2 ON asca.instructor_id = u2.id
        WHERE te.is_active = true
        "#
    );

    let mut idx = 0u32;

    if let Some(_) = query.classroom_id {
        idx += 1;
        sql.push_str(&format!(" AND te.classroom_id = ${idx}"));
    }

    // student_id: ดึง classroom ที่นักเรียนสังกัด
    if let Some(_) = query.student_id {
        idx += 1;
        sql.push_str(&format!(" AND te.classroom_id IN (SELECT class_room_id FROM student_class_enrollments WHERE student_id = (SELECT user_id FROM student_info WHERE id = ${idx}) AND status = 'active')"));
    }

    if let Some(_) = query.instructor_id {
        idx += 1;
        sql.push_str(&format!(
            " AND (cc.primary_instructor_id = ${idx} OR te.activity_slot_id IN (SELECT activity_slot_id FROM activity_slot_instructors WHERE user_id = ${idx}) OR asca.instructor_id = ${idx})"
        ));
    }

    if let Some(_) = query.room_id {
        idx += 1;
        sql.push_str(&format!(" AND te.room_id = ${idx}"));
    }

    if let Some(_) = query.academic_semester_id {
        idx += 1;
        sql.push_str(&format!(" AND te.academic_semester_id = ${idx}"));
    }

    if let Some(ref _day) = query.day_of_week {
        idx += 1;
        sql.push_str(&format!(" AND te.day_of_week = ${idx}"));
    }

    if let Some(ref _entry_type) = query.entry_type {
        idx += 1;
        sql.push_str(&format!(" AND te.entry_type = ${idx}"));
    }

    sql.push_str(" ORDER BY te.day_of_week, ap.order_index");

    let mut q = sqlx::query_as::<_, TimetableEntry>(&sql);
    if let Some(classroom_id) = query.classroom_id { q = q.bind(classroom_id); }
    if let Some(student_id) = query.student_id { q = q.bind(student_id); }
    if let Some(instructor_id) = query.instructor_id { q = q.bind(instructor_id); }
    if let Some(room_id) = query.room_id { q = q.bind(room_id); }
    if let Some(semester_id) = query.academic_semester_id { q = q.bind(semester_id); }
    if let Some(ref day) = query.day_of_week { q = q.bind(day); }
    if let Some(ref entry_type) = query.entry_type { q = q.bind(entry_type); }
    let entries = q
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

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
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

    // Lookup IDs depending on entry type
    let (classroom_id_val, academic_semester_id, entry_type, title, activity_slot_id) =
        if let Some(course_id) = payload.classroom_course_id {
            let info: Option<(Uuid, Uuid)> = sqlx::query_as(
                "SELECT classroom_id, academic_semester_id FROM classroom_courses WHERE id = $1"
            )
            .bind(course_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;

            match info {
                Some((cls, sem)) => (cls, sem, "COURSE".to_string(), None::<String>, None::<Uuid>),
                None => return Err(AppError::NotFound("Classroom course not found".to_string()))
            }
        } else if let Some(slot_id) = payload.activity_slot_id {
            // Activity slot entry — require classroom_id + academic_semester_id from payload
            let cls = payload.classroom_id
                .ok_or_else(|| AppError::BadRequest("classroom_id required for activity entry".to_string()))?;
            let sem = payload.academic_semester_id
                .ok_or_else(|| AppError::BadRequest("academic_semester_id required for activity entry".to_string()))?;

            // Lookup slot name for title
            let slot_name: Option<String> = sqlx::query_scalar(
                "SELECT name FROM activity_slots WHERE id = $1"
            )
            .bind(slot_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;

            let title = payload.title.or(slot_name);
            let et = payload.entry_type.unwrap_or_else(|| "ACTIVITY".to_string());
            (cls, sem, et, title, Some(slot_id))
        } else {
            return Err(AppError::BadRequest("classroom_course_id or activity_slot_id required".to_string()));
        };

    let entry = sqlx::query_as::<_, TimetableEntry>(
        r#"
        INSERT INTO academic_timetable_entries (
            id, classroom_course_id, day_of_week, period_id,
            room_id, note, classroom_id, academic_semester_id, entry_type, title, is_active,
            created_by, updated_by, activity_slot_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, true, $11, $11, $12)
        RETURNING *, NULL::TEXT AS subject_code, NULL::TEXT AS subject_name_th,
                  NULL::TEXT AS instructor_name, NULL::TEXT AS classroom_name,
                  NULL::TEXT AS room_code, NULL::TEXT AS period_name,
                  NULL::TIME AS start_time, NULL::TIME AS end_time,
                  NULL::TEXT AS activity_slot_name, NULL::TEXT AS activity_type,
                  NULL::TEXT AS activity_scheduling_mode
        "#
    )
    .bind(Uuid::new_v4())
    .bind(payload.classroom_course_id)
    .bind(&payload.day_of_week)
    .bind(payload.period_id)
    .bind(payload.room_id)
    .bind(&payload.note)
    .bind(classroom_id_val)
    .bind(academic_semester_id)
    .bind(&entry_type)
    .bind(&title)
    .bind(user_id)
    .bind(activity_slot_id)
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

    // Populate junction from source tables (non-fatal; log only)
    if let Err(e) = populate_entry_instructors(
        &pool,
        entry.id,
        entry.classroom_course_id,
        entry.activity_slot_id,
        entry.classroom_id,
    ).await {
        eprintln!("Failed to populate entry instructors: {}", e);
    }

    Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": entry }))).into_response())
}

/// DELETE /api/academic/timetable/batch
/// Deletes all entries matching activity_slot_id + day_of_week + semester
pub async fn delete_batch_timetable_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DeleteBatchTimetableEntriesRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(response);
    }

    let result = sqlx::query(
        r#"
        DELETE FROM academic_timetable_entries
        WHERE activity_slot_id = $1
          AND day_of_week = $2
          AND academic_semester_id = $3
        "#
    )
    .bind(payload.activity_slot_id)
    .bind(&payload.day_of_week)
    .bind(payload.academic_semester_id)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to batch delete entries: {}", e);
        AppError::InternalServerError("Failed to batch delete entries".to_string())
    })?;

    // Broadcast refresh
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();
    let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    let event = TimetableEvent::TableRefresh {
        user_id: user_id.unwrap_or_default()
    };
    let _ = state.websocket_manager.get_or_create_room(subdomain, payload.academic_semester_id).send(event);

    Ok(Json(json!({
        "success": true,
        "deleted_count": result.rows_affected()
    })).into_response())
}

/// DELETE /api/academic/timetable/{id}
pub async fn delete_timetable_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
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
    } else if let Some(cls_id) = payload.classroom_id {
        // Activity entry: check classroom conflict using payload.classroom_id
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

        // Activity entry: check instructor conflict via classroom assignment
        if let Some(slot_id) = payload.activity_slot_id {
            let instr_id: Option<Uuid> = sqlx::query_scalar(
                "SELECT instructor_id FROM activity_slot_classroom_assignments WHERE slot_id = $1 AND classroom_id = $2"
            )
            .bind(slot_id)
            .bind(cls_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();

            if let Some(instr_id) = instr_id {
                let instructor_conflict: bool = sqlx::query_scalar(
                    r#"
                    SELECT EXISTS(
                        SELECT 1 FROM academic_timetable_entries te
                        LEFT JOIN classroom_courses cc ON te.classroom_course_id = cc.id
                        LEFT JOIN activity_slot_classroom_assignments asca ON asca.slot_id = te.activity_slot_id AND asca.classroom_id = te.classroom_id
                        WHERE (cc.primary_instructor_id = $1 OR asca.instructor_id = $1)
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

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(response);
    }
    
    // Get current user ID for audit
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();

    // 1. Fetch existing entry to get classroom_course_id for validation
    let existing_entry = sqlx::query_as::<_, TimetableEntry>(
        r#"
        SELECT te.*, NULL::TEXT as subject_code, NULL::TEXT as subject_name_th, NULL::TEXT as instructor_name,
               NULL::TEXT as classroom_name, NULL::TEXT as room_code, NULL::TEXT as period_name,
               NULL::TIME as start_time, NULL::TIME as end_time,
               NULL::TEXT as activity_slot_name, NULL::TEXT as activity_type
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
        activity_slot_id: existing_entry.activity_slot_id,
        entry_type: Some(existing_entry.entry_type.clone()),
        title: existing_entry.title.clone(),
        classroom_id: Some(existing_entry.classroom_id),
        academic_semester_id: Some(existing_entry.academic_semester_id),
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

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(response);
    }

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();

    // Pre-validate conflicts (unless force mode)
    if !payload.force.unwrap_or(false) {
        let mut conflicts: Vec<serde_json::Value> = Vec::new();

        // 1. Check classroom conflicts — single query with ANY
        // Skip entries that belong to the same activity_slot_id (re-batch scenario)
        let classroom_conflicts: Vec<(String,)> = sqlx::query_as(
            r#"SELECT DISTINCT cr.name
               FROM academic_timetable_entries te
               JOIN class_rooms cr ON cr.id = te.classroom_id
               WHERE te.classroom_id = ANY($1)
                 AND te.day_of_week = $2
                 AND te.period_id = ANY($3)
                 AND te.is_active = true
                 AND (te.activity_slot_id IS DISTINCT FROM $4 OR te.activity_slot_id IS NULL)"#
        )
        .bind(&payload.classroom_ids)
        .bind(&payload.day_of_week)
        .bind(&payload.period_ids)
        .bind(payload.activity_slot_id)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        for (name,) in &classroom_conflicts {
            conflicts.push(serde_json::json!({
                "conflict_type": "CLASSROOM_CONFLICT",
                "message": format!("{} มีตารางในคาบนี้อยู่แล้ว", name)
            }));
        }

        // 2. Check slot instructor conflicts — single query with ANY
        if let Some(slot_id) = payload.activity_slot_id {
            let instructor_ids: Vec<Uuid> = sqlx::query_scalar(
                "SELECT user_id FROM activity_slot_instructors WHERE slot_id = $1"
            )
            .bind(slot_id)
            .fetch_all(&pool)
            .await
            .unwrap_or_default();

            if !instructor_ids.is_empty() {
                let instructor_conflicts: Vec<(String, String)> = sqlx::query_as(
                    r#"SELECT DISTINCT concat(u.first_name, ' ', u.last_name), COALESCE(s.name_th, te.title, '')
                       FROM academic_timetable_entries te
                       LEFT JOIN classroom_courses cc ON te.classroom_course_id = cc.id
                       LEFT JOIN subjects s ON cc.subject_id = s.id
                       LEFT JOIN activity_slot_classroom_assignments asca ON asca.slot_id = te.activity_slot_id AND asca.classroom_id = te.classroom_id
                       JOIN users u ON u.id = ANY($1) AND (cc.primary_instructor_id = u.id OR asca.instructor_id = u.id)
                       WHERE te.day_of_week = $2
                         AND te.period_id = ANY($3)
                         AND te.is_active = true"#
                )
                .bind(&instructor_ids)
                .bind(&payload.day_of_week)
                .bind(&payload.period_ids)
                .fetch_all(&pool)
                .await
                .unwrap_or_default();

                for (teacher_name, existing_subject) in &instructor_conflicts {
                    conflicts.push(serde_json::json!({
                        "conflict_type": "INSTRUCTOR_CONFLICT",
                        "message": format!("{} มีสอน {} ในคาบนี้อยู่แล้ว", teacher_name, existing_subject)
                    }));
                }
            }
        }

        // 3. Check room conflict
        if let Some(room_id) = payload.room_id {
            let room_conflicts: Vec<(String, String)> = sqlx::query_as(
                r#"SELECT DISTINCT r.code, ap.name
                   FROM academic_timetable_entries te
                   JOIN rooms r ON r.id = te.room_id
                   JOIN academic_periods ap ON ap.id = te.period_id
                   WHERE te.room_id = $1
                     AND te.day_of_week = $2
                     AND te.period_id = ANY($3)
                     AND te.is_active = true"#
            )
            .bind(room_id)
            .bind(&payload.day_of_week)
            .bind(&payload.period_ids)
            .fetch_all(&pool)
            .await
            .unwrap_or_default();

            for (room_code, period_name) in &room_conflicts {
                conflicts.push(serde_json::json!({
                    "conflict_type": "ROOM_CONFLICT",
                    "message": format!("ห้อง {} ถูกใช้ในคาบ {} อยู่แล้ว", room_code, period_name)
                }));
            }
        }

        if !conflicts.is_empty() {
            return Ok((
                StatusCode::CONFLICT,
                Json(json!({
                    "success": false,
                    "message": "พบรายการที่ชนกัน",
                    "conflicts": conflicts
                }))
            ).into_response());
        }
    }

    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    for classroom_id in &payload.classroom_ids {
        let mut entry_type = payload.entry_type.clone();
        let mut classroom_course_id: Option<Uuid> = None;
        let mut title = payload.title.clone();

        // If subject_id is provided, try to find matching course mapping for this classroom
        if let Some(subject_id) = payload.subject_id {
            let course_info: Option<(Uuid, String)> = sqlx::query_as(
               "SELECT cc.id, s.name_th FROM classroom_courses cc
                JOIN subjects s ON cc.subject_id = s.id
                WHERE cc.classroom_id = $1 AND cc.subject_id = $2"
            )
            .bind(classroom_id)
            .bind(subject_id)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);

            if let Some((cc_id, s_name)) = course_info {
                classroom_course_id = Some(cc_id);
                entry_type = "COURSE".to_string();
                title = s_name;
            }
        }

        for period_id in &payload.period_ids {
            // FORCE OVERRIDE LOGIC
            if payload.force.unwrap_or(false) {
                // 1. Clear slot for this classroom
                let _ = sqlx::query("DELETE FROM academic_timetable_entries WHERE classroom_id = $1 AND day_of_week = $2 AND period_id = $3")
                    .bind(classroom_id)
                    .bind(&payload.day_of_week)
                    .bind(period_id)
                    .execute(&mut *tx)
                    .await;

                // 2. Clear slot for target room (if specified)
                if let Some(rid) = payload.room_id {
                     let _ = sqlx::query("DELETE FROM academic_timetable_entries WHERE room_id = $1 AND day_of_week = $2 AND period_id = $3")
                    .bind(rid)
                    .bind(&payload.day_of_week)
                    .bind(period_id)
                    .execute(&mut *tx)
                    .await;
                }

                // 3. Clear slot for instructor (if course mode)
                if let Some(cc_id) = classroom_course_id {
                     let instructor_id: Option<Uuid> = sqlx::query_scalar(
                         "SELECT primary_instructor_id FROM classroom_courses WHERE id = $1"
                     )
                     .bind(cc_id)
                     .fetch_optional(&mut *tx)
                     .await
                     .unwrap_or(None);

                     if let Some(inst_id) = instructor_id {
                          let _ = sqlx::query(r#"
                            DELETE FROM academic_timetable_entries
                            WHERE id IN (
                                SELECT te.id FROM academic_timetable_entries te
                                JOIN classroom_courses cc ON te.classroom_course_id = cc.id
                                WHERE cc.primary_instructor_id = $1
                                  AND te.day_of_week = $2
                                  AND te.period_id = $3
                            )
                          "#)
                          .bind(inst_id)
                          .bind(&payload.day_of_week)
                          .bind(period_id)
                          .execute(&mut *tx)
                          .await;
                     }
                }
            }

            let result = sqlx::query(
                r#"
                INSERT INTO academic_timetable_entries (
                    id, classroom_id, academic_semester_id, day_of_week, period_id, room_id,
                    entry_type, title, is_active, created_by, updated_by,
                    classroom_course_id, note, activity_slot_id
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, true, $9, $9, $10, $11, $12)
                ON CONFLICT DO NOTHING
                "#
            )
            .bind(Uuid::new_v4())
            .bind(classroom_id)
            .bind(payload.academic_semester_id)
            .bind(&payload.day_of_week)
            .bind(period_id)
            .bind(payload.room_id)
            .bind(&entry_type)
            .bind(&title)
            .bind(user_id)
            .bind(classroom_course_id)
            .bind(&payload.note)
            .bind(payload.activity_slot_id)
            .execute(&mut *tx)
            .await;

            if let Err(e) = result {
                 eprintln!("Failed to batch insert for classroom {}: {}", classroom_id, e);
                 return Err(AppError::InternalServerError("Failed to batch create entries".to_string()));
            }
        }
    }

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Broadcast refresh event
    let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    let event = TimetableEvent::TableRefresh { 
        user_id: user_id.unwrap_or_default() 
    };
    let _ = state.websocket_manager.get_or_create_room(subdomain, payload.academic_semester_id).send(event);

    Ok(Json(json!({
        "success": true,
        "message": "Batch entries created successfully"
    })).into_response())
}

/// GET /api/academic/timetable/{id}/my-activity
/// Returns the activity group the current user is enrolled in for a given timetable entry's slot
pub async fn get_my_activity_for_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(entry_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await
        .map_err(|_| AppError::AuthError("Not authenticated".to_string()))?;

    // 1. Get the timetable entry's activity_slot_id
    let slot_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT activity_slot_id FROM academic_timetable_entries WHERE id = $1"
    )
    .bind(entry_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Query failed".to_string()))?
    .flatten();

    let slot_id = match slot_id {
        Some(id) => id,
        None => return Ok(Json(json!({ "success": true, "data": null })).into_response()),
    };

    // 2. Find the activity group the user is enrolled in within this slot
    let group = sqlx::query_as::<_, (Uuid, String, Option<i32>, Option<String>)>(
        r#"
        SELECT ag.id, ag.name, ag.max_capacity,
               (SELECT concat(u.first_name, ' ', u.last_name)
                FROM activity_group_instructors agi
                JOIN users u ON agi.user_id = u.id
                WHERE agi.activity_group_id = ag.id
                LIMIT 1) AS instructor_name
        FROM activity_group_members agm
        JOIN activity_groups ag ON agm.activity_group_id = ag.id
        WHERE agm.student_id = $1 AND ag.slot_id = $2 AND ag.is_active = true
        LIMIT 1
        "#
    )
    .bind(user_id)
    .bind(slot_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch activity for entry: {}", e);
        AppError::InternalServerError("Query failed".to_string())
    })?;

    match group {
        Some((id, name, max_capacity, instructor_name)) => {
            // Get member count
            let member_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM activity_group_members WHERE activity_group_id = $1"
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap_or(0);

            // Get all instructors
            let instructors: Vec<(Uuid, String)> = sqlx::query_as(
                r#"
                SELECT u.id, concat(u.first_name, ' ', u.last_name) AS name
                FROM activity_group_instructors agi
                JOIN users u ON agi.user_id = u.id
                WHERE agi.activity_group_id = $1
                "#
            )
            .bind(id)
            .fetch_all(&pool)
            .await
            .unwrap_or_default();

            Ok(Json(json!({
                "success": true,
                "data": {
                    "enrolled": true,
                    "group_id": id,
                    "group_name": name,
                    "max_capacity": max_capacity,
                    "member_count": member_count,
                    "instructor_name": instructor_name,
                    "instructors": instructors.iter().map(|(id, name)| json!({ "id": id, "name": name })).collect::<Vec<_>>(),
                    "slot_id": slot_id
                }
            })).into_response())
        }
        None => {
            Ok(Json(json!({
                "success": true,
                "data": {
                    "enrolled": false,
                    "slot_id": slot_id
                }
            })).into_response())
        }
    }
}
