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

/// Fetch 1 entry พร้อม joined fields (subject, classroom, room, period, instructors)
/// ใช้กับ patch events เพื่อให้ frontend ได้ entry ครบ, patch ได้ทันทีไม่ต้อง re-fetch
async fn fetch_entry_with_joins(pool: &sqlx::PgPool, entry_id: Uuid) -> Option<TimetableEntry> {
    sqlx::query_as::<_, TimetableEntry>(
        r#"
        SELECT
            te.*,
            s.code   AS subject_code,
            s.name_th AS subject_name_th,
            (SELECT ARRAY_AGG(concat(u2.first_name, ' ', u2.last_name) ORDER BY tei2.role, tei2.created_at)
             FROM timetable_entry_instructors tei2
             JOIN users u2 ON u2.id = tei2.instructor_id
             WHERE tei2.entry_id = te.id) AS instructor_names,
            (SELECT ARRAY_AGG(tei_id.instructor_id ORDER BY tei_id.role, tei_id.created_at)
             FROM timetable_entry_instructors tei_id
             WHERE tei_id.entry_id = te.id) AS instructor_ids,
            (SELECT concat(u3.first_name, ' ', u3.last_name)
             FROM timetable_entry_instructors tei3
             JOIN users u3 ON u3.id = tei3.instructor_id
             WHERE tei3.entry_id = te.id
             ORDER BY tei3.role, tei3.created_at
             LIMIT 1) AS instructor_name,
            cr.name  AS classroom_name,
            r.code   AS room_code,
            ap.name  AS period_name,
            ap.start_time,
            ap.end_time,
            asl_ac.name AS activity_slot_name,
            asl_ac.activity_type AS activity_type,
            asl_ac.scheduling_mode AS activity_scheduling_mode
        FROM academic_timetable_entries te
        LEFT JOIN classroom_courses cc ON te.classroom_course_id = cc.id
        LEFT JOIN subjects s ON cc.subject_id = s.id
        LEFT JOIN class_rooms cr ON te.classroom_id = cr.id
        JOIN academic_periods ap ON te.period_id = ap.id
        LEFT JOIN rooms r ON te.room_id = r.id
        LEFT JOIN activity_slots asl ON te.activity_slot_id = asl.id
        LEFT JOIN activity_catalog asl_ac ON asl.activity_catalog_id = asl_ac.id
        WHERE te.id = $1
        "#
    )
    .bind(entry_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
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

    // Auto-assign order_index = MAX + 1 ถ้าไม่ส่งมา
    let order_index = match payload.order_index {
        Some(idx) => idx,
        None => {
            let next: Option<i32> = sqlx::query_scalar(
                "SELECT COALESCE(MAX(order_index), 0) + 1 FROM academic_periods WHERE academic_year_id = $1"
            )
            .bind(payload.academic_year_id)
            .fetch_one(&pool)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to compute next order_index: {}", e)))?;
            next.unwrap_or(1)
        }
    };

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
    .bind(payload.name.filter(|s| !s.trim().is_empty()))
    .bind(start_time)
    .bind(end_time)
    .bind(order_index)
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

    // name: ถ้า field ไม่ส่งมา → คงเดิม; ถ้าส่ง "" → clear เป็น NULL; ส่งค่า → set
    // ใช้ flag separate เพราะ COALESCE แยก "ไม่ส่ง" กับ "ส่ง NULL" ไม่ได้
    let (name_set, name_value) = match payload.name {
        None => (false, None),
        Some(s) => (true, Some(s).filter(|s| !s.trim().is_empty())),
    };

    let period = sqlx::query_as::<_, AcademicPeriod>(
        r#"
        UPDATE academic_periods SET
            name = CASE WHEN $2 THEN $3 ELSE name END,
            start_time = COALESCE($4, start_time),
            end_time = COALESCE($5, end_time),
            order_index = COALESCE($6, order_index),
            applicable_days = COALESCE($7, applicable_days),
            is_active = COALESCE($8, is_active),
            updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#
    )
    .bind(id)
    .bind(name_set)
    .bind(name_value)
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

/// POST /api/academic/periods/reorder
/// Batch update order_index หลายแถวใน transaction เดียว
/// ใช้ SET CONSTRAINTS DEFERRED เพื่อเลี่ยง unique constraint ชนระหว่าง update
pub async fn reorder_periods(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ReorderPeriodsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_STRUCTURE_MANAGE_ALL, &state.permission_cache).await {
        return Ok(response);
    }

    if payload.items.is_empty() {
        return Ok(Json(json!({ "success": true, "updated": 0 })).into_response());
    }

    let mut tx = pool.begin().await
        .map_err(|e| AppError::InternalServerError(format!("Transaction failed: {}", e)))?;

    sqlx::query("SET CONSTRAINTS unique_period_per_year DEFERRED")
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to defer constraint: {}", e)))?;

    for item in &payload.items {
        sqlx::query(
            "UPDATE academic_periods SET order_index = $1 WHERE id = $2 AND academic_year_id = $3"
        )
        .bind(item.order_index)
        .bind(item.id)
        .bind(payload.academic_year_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to update period: {}", e)))?;
    }

    tx.commit().await
        .map_err(|e| {
            let msg = if e.to_string().contains("unique_period_per_year") {
                "ลำดับคาบซ้ำกัน — ตรวจสอบ payload".to_string()
            } else {
                format!("Failed to commit reorder: {}", e)
            };
            AppError::BadRequest(msg)
        })?;

    Ok(Json(json!({ "success": true, "updated": payload.items.len() })).into_response())
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
            (SELECT ARRAY_AGG(concat(u2.first_name, ' ', u2.last_name) ORDER BY tei2.role, tei2.created_at)
             FROM timetable_entry_instructors tei2
             JOIN users u2 ON u2.id = tei2.instructor_id
             WHERE tei2.entry_id = te.id) AS instructor_names,
            (SELECT ARRAY_AGG(tei_id.instructor_id ORDER BY tei_id.role, tei_id.created_at)
             FROM timetable_entry_instructors tei_id
             WHERE tei_id.entry_id = te.id) AS instructor_ids,
            (SELECT concat(u3.first_name, ' ', u3.last_name)
             FROM timetable_entry_instructors tei3
             JOIN users u3 ON u3.id = tei3.instructor_id
             WHERE tei3.entry_id = te.id
             ORDER BY tei3.role, tei3.created_at
             LIMIT 1) AS instructor_name,
            cr.name  AS classroom_name,
            r.code   AS room_code,
            ap.name  AS period_name,
            ap.start_time,
            ap.end_time,
            asl_ac.name AS activity_slot_name,
            asl_ac.activity_type AS activity_type,
            asl_ac.scheduling_mode AS activity_scheduling_mode
        FROM academic_timetable_entries te
        LEFT JOIN classroom_courses cc ON te.classroom_course_id = cc.id
        LEFT JOIN subjects s ON cc.subject_id = s.id
        LEFT JOIN class_rooms cr ON te.classroom_id = cr.id
        JOIN academic_periods ap ON te.period_id = ap.id
        LEFT JOIN rooms r ON te.room_id = r.id
        LEFT JOIN activity_slots asl ON te.activity_slot_id = asl.id
        LEFT JOIN activity_catalog asl_ac ON asl.activity_catalog_id = asl_ac.id
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
        if query.include_team_ghosts.unwrap_or(false) {
            // Ghost mode: entries ที่ instructor อยู่ในทีม
            //   COURSE    → match ผ่าน classroom_course_instructors (team-level; รวม ghost cell
            //              ที่คนอื่นสอนจริงใน tei แต่ user อยู่ในทีมของ course)
            //   ACTIVITY  → match ผ่าน timetable_entry_instructors (tei populate จาก
            //              activity_slot_classroom_assignments/activity_slot_instructors แล้ว)
            sql.push_str(&format!(
                " AND (EXISTS (SELECT 1 FROM classroom_course_instructors cci \
                       WHERE cci.classroom_course_id = te.classroom_course_id AND cci.instructor_id = ${idx}) \
                    OR EXISTS (SELECT 1 FROM timetable_entry_instructors tei \
                       WHERE tei.entry_id = te.id AND tei.instructor_id = ${idx}))"
            ));
        } else {
            // Normal mode: เฉพาะ cell ที่ instructor ถูก assign
            sql.push_str(&format!(
                " AND EXISTS (SELECT 1 FROM timetable_entry_instructors tei WHERE tei.entry_id = te.id AND tei.instructor_id = ${idx})"
            ));
        }
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

    // current_seq ของ semester — client ใช้เป็นจุดเริ่มต้น tracking patch events
    let current_seq = if let Some(sem_id) = query.academic_semester_id {
        let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        state.websocket_manager.current_seq(subdomain, sem_id)
    } else {
        0
    };

    Ok(Json(json!({ "success": true, "data": entries, "current_seq": current_seq })).into_response())
}

/// GET /api/academic/timetable/replay
/// Query: semester_id, after_seq
/// Return: { events: [...], current_seq } หรือ { needs_refetch: true, current_seq }
#[derive(Debug, serde::Deserialize)]
pub struct ReplayQuery {
    pub semester_id: Uuid,
    pub after_seq: u64,
}

pub async fn replay_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ReplayQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL, &state.permission_cache).await {
        return Ok(response);
    }

    let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    let current_seq = state.websocket_manager.current_seq(subdomain.clone(), query.semester_id);

    match state.websocket_manager.replay(subdomain, query.semester_id, query.after_seq) {
        Some(events) => Ok(Json(json!({
            "events": events,
            "current_seq": current_seq,
            "needs_refetch": false,
        })).into_response()),
        None => Ok(Json(json!({
            "needs_refetch": true,
            "current_seq": current_seq,
        })).into_response()),
    }
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
        // Phase 2: broadcast EntryRejected → ทุก client ลบ tempEntry
        if let Some(temp_id) = payload.client_temp_id.as_ref() {
            let subdomain_for_reject = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
            // ใช้ semester จาก payload หรือ lookup จาก course/slot
            let sem_for_reject: Option<Uuid> = if let Some(sem) = payload.academic_semester_id {
                Some(sem)
            } else if let Some(cc_id) = payload.classroom_course_id {
                sqlx::query_scalar("SELECT academic_semester_id FROM classroom_courses WHERE id = $1")
                    .bind(cc_id)
                    .fetch_optional(&pool)
                    .await
                    .ok()
                    .flatten()
            } else { None };
            if let Some(sem) = sem_for_reject {
                let reason = validation.conflicts.iter()
                    .map(|c| c.message.as_str())
                    .collect::<Vec<_>>()
                    .join(" · ");
                state.websocket_manager.broadcast_ephemeral(
                    subdomain_for_reject,
                    sem,
                    TimetableEvent::EntryRejected {
                        user_id: user_id.unwrap_or_default(),
                        temp_id: temp_id.clone(),
                        reason: if reason.is_empty() { "พบข้อขัดแย้ง".to_string() } else { reason },
                    },
                );
            }
        }
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

            // Validate: classroom must participate in this slot via activity_slot_classrooms junction.
            // Admin adds participation through Course Planning page — not here.
            let participates: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM activity_slot_classrooms
                 WHERE slot_id = $1 AND classroom_id = $2)"
            )
            .bind(slot_id)
            .bind(cls)
            .fetch_one(&pool)
            .await
            .unwrap_or(false);
            if !participates {
                return Err(AppError::BadRequest(
                    "ห้องนี้ไม่ได้อยู่ในกิจกรรมนี้ — เพิ่มห้องที่ Course Planning ก่อน".to_string()
                ));
            }

            // Validate: must have instructor before scheduling.
            // Independent mode → activity_slot_classroom_assignments per classroom
            // Synchronized mode → activity_slot_instructors at slot level
            let has_instructor: bool = sqlx::query_scalar(
                r#"SELECT CASE
                     WHEN ac.scheduling_mode = 'independent' THEN
                         EXISTS(SELECT 1 FROM activity_slot_classroom_assignments
                                WHERE slot_id = $1 AND classroom_id = $2)
                     ELSE
                         EXISTS(SELECT 1 FROM activity_slot_instructors WHERE slot_id = $1)
                   END
                   FROM activity_slots s
                   JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
                   WHERE s.id = $1"#
            )
            .bind(slot_id)
            .bind(cls)
            .fetch_one(&pool)
            .await
            .unwrap_or(false);
            if !has_instructor {
                return Err(AppError::BadRequest(
                    "กิจกรรมนี้ยังไม่ได้กำหนดครูผู้สอน — เพิ่มครูที่หน้า Activities ก่อน".to_string()
                ));
            }

            // Lookup slot name (from catalog via FK) for title
            let slot_name: Option<String> = sqlx::query_scalar(
                "SELECT ac.name FROM activity_slots s
                 JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
                 WHERE s.id = $1"
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

    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let entry = sqlx::query_as::<_, TimetableEntry>(
        r#"
        INSERT INTO academic_timetable_entries (
            id, classroom_course_id, day_of_week, period_id,
            room_id, note, classroom_id, academic_semester_id, entry_type, title, is_active,
            created_by, updated_by, activity_slot_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, true, $11, $11, $12)
        RETURNING *, NULL::TEXT AS subject_code, NULL::TEXT AS subject_name_th,
                  NULL::TEXT[] AS instructor_names,
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
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("Failed to create timetable entry: {}", e);
        if e.to_string().contains("unique_entry_per_slot") {
            AppError::BadRequest("This slot is already occupied".to_string())
        } else {
            AppError::InternalServerError("Failed to create timetable entry".to_string())
        }
    })?;

    // Populate junction from source tables (inline — transactional)
    if let Some(cc_id) = entry.classroom_course_id {
        sqlx::query(
            "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
             SELECT $1, instructor_id, role FROM classroom_course_instructors
             WHERE classroom_course_id = $2 ON CONFLICT DO NOTHING"
        )
        .bind(entry.id)
        .bind(cc_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    } else if let Some(slot_id) = entry.activity_slot_id {
        let mode: Option<String> = sqlx::query_scalar(
            "SELECT ac.scheduling_mode FROM activity_slots s JOIN activity_catalog ac ON ac.id = s.activity_catalog_id WHERE s.id = $1"
        )
        .bind(slot_id)
        .fetch_optional(&mut *tx)
        .await
        .ok()
        .flatten();

        if mode.as_deref() == Some("independent") {
            sqlx::query(
                "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                 SELECT $1, instructor_id, 'primary'
                 FROM activity_slot_classroom_assignments
                 WHERE slot_id = $2 AND classroom_id = $3 ON CONFLICT DO NOTHING"
            )
            .bind(entry.id)
            .bind(slot_id)
            .bind(entry.classroom_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        } else {
            sqlx::query(
                "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                 SELECT $1, user_id, 'primary' FROM activity_slot_instructors
                 WHERE slot_id = $2 ON CONFLICT DO NOTHING"
            )
            .bind(entry.id)
            .bind(slot_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        }
    }

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    let has_subs = state.websocket_manager.has_other_subscribers(subdomain.clone(), entry.academic_semester_id);

    // Re-fetch joined เฉพาะเมื่อต้อง broadcast (frontend caller เรียก loadTimetable ต่ออยู่แล้ว
    // ไม่ได้พึ่งพา joined fields ใน response)
    if has_subs {
        if let Some(full_entry) = fetch_entry_with_joins(&pool, entry.id).await {
            let entry_json = serde_json::to_value(&full_entry).unwrap_or_default();
            state.websocket_manager.broadcast_mutation(
                subdomain,
                full_entry.academic_semester_id,
                TimetableEvent::EntryCreated {
                    user_id: user_id.unwrap_or_default(),
                    entry: entry_json,
                    client_temp_id: payload.client_temp_id.clone(),
                },
            );
        }
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
    state.websocket_manager.broadcast_mutation(
        subdomain,
        payload.academic_semester_id,
        TimetableEvent::TableRefresh { user_id: user_id.unwrap_or_default() },
    );

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

    let semester_id: Option<Uuid> = sqlx::query_scalar(
        "DELETE FROM academic_timetable_entries WHERE id = $1 RETURNING academic_semester_id"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to delete entry".to_string()))?;

    if let Some(semester_id) = semester_id {
        let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();
        state.websocket_manager.broadcast_mutation(
            subdomain,
            semester_id,
            TimetableEvent::EntryDeleted { user_id: user_id.unwrap_or_default(), entry_id: id },
        );
    }

    Ok(Json(json!({ "success": true })).into_response())
}

/// DELETE /api/academic/timetable/batch-group/{batch_id}
/// ลบทุก entries ที่มี batch_id ตรงกัน (ใช้กับ entries ที่สร้างผ่าน /timetable/batch)
pub async fn delete_batch_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(batch_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(response);
    }

    // Grab semester_id ก่อนลบ เพื่อ broadcast
    let semester_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT academic_semester_id FROM academic_timetable_entries WHERE batch_id = $1 LIMIT 1"
    )
    .bind(batch_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let result = sqlx::query("DELETE FROM academic_timetable_entries WHERE batch_id = $1")
        .bind(batch_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to delete batch group {}: {}", batch_id, e);
            AppError::InternalServerError("Failed to delete batch group".to_string())
        })?;

    if let Some(sid) = semester_id {
        let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();
        state.websocket_manager.broadcast_mutation(
            subdomain,
            sid,
            TimetableEvent::TableRefresh { user_id: user_id.unwrap_or_default() },
        );
    }

    Ok(Json(json!({
        "success": true,
        "deleted_count": result.rows_affected()
    })).into_response())
}

// ============================================
// Conflict Detection Logic
// ============================================

async fn validate_timetable_entry(
    pool: &sqlx::PgPool,
    payload: &CreateTimetableEntryRequest,
) -> Result<TimetableValidationResponse, AppError> {
    let mut conflicts = Vec::new();

    // Unified instructor conflict check via junction
    let candidate_instructors: Vec<Uuid> = if let Some(cc_id) = payload.classroom_course_id {
        sqlx::query_scalar(
            "SELECT instructor_id FROM classroom_course_instructors WHERE classroom_course_id = $1"
        )
        .bind(cc_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else if let Some(slot_id) = payload.activity_slot_id {
        let mode: Option<String> = sqlx::query_scalar(
            "SELECT ac.scheduling_mode FROM activity_slots s JOIN activity_catalog ac ON ac.id = s.activity_catalog_id WHERE s.id = $1"
        ).bind(slot_id).fetch_optional(pool).await.ok().flatten();
        if mode.as_deref() == Some("independent") {
            if let Some(cls_id) = payload.classroom_id {
                sqlx::query_scalar(
                    "SELECT instructor_id FROM activity_slot_classroom_assignments
                     WHERE slot_id = $1 AND classroom_id = $2"
                ).bind(slot_id).bind(cls_id).fetch_all(pool).await.unwrap_or_default()
            } else { Vec::new() }
        } else {
            sqlx::query_scalar(
                "SELECT user_id FROM activity_slot_instructors WHERE slot_id = $1"
            ).bind(slot_id).fetch_all(pool).await.unwrap_or_default()
        }
    } else {
        Vec::new()
    };

    if !candidate_instructors.is_empty() {
        let conflict_instructors: Vec<(String,)> = sqlx::query_as(
            r#"SELECT DISTINCT concat(u.first_name, ' ', u.last_name)
               FROM academic_timetable_entries te
               JOIN timetable_entry_instructors tei ON tei.entry_id = te.id
               JOIN users u ON u.id = tei.instructor_id
               WHERE tei.instructor_id = ANY($1)
                 AND te.day_of_week = $2
                 AND te.period_id = $3
                 AND te.is_active = true"#
        )
        .bind(&candidate_instructors)
        .bind(&payload.day_of_week)
        .bind(payload.period_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        for (name,) in &conflict_instructors {
            conflicts.push(ConflictInfo {
                conflict_type: "INSTRUCTOR_CONFLICT".to_string(),
                message: format!("{} มีสอนในคาบนี้อยู่แล้ว", name),
                existing_entry: None,
            });
        }
    }

    // Classroom conflict check (resolves classroom_id from course if needed)
    if let Some(course_id) = payload.classroom_course_id {
        let classroom_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT classroom_id FROM classroom_courses WHERE id = $1"
        )
        .bind(course_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

        if classroom_id.is_none() {
             return Err(AppError::NotFound("Classroom course not found".to_string()));
        }

        if let Some(cls_id) = classroom_id {
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

    // Block: ถ้า entry สร้างจาก batch (pinned) → ไม่ให้ update/move/replace
    if existing_entry.batch_id.is_some() {
        return Err(AppError::BadRequest(
            "คาบที่สร้างจาก Batch ไม่สามารถย้าย/เปลี่ยนเนื้อหาได้ (ลบก่อนแล้ว batch ใหม่แทน)".to_string(),
        ));
    }

    // 2. Prepare mock CreateRequest for validation (using new values or fallback to existing)
    let target_classroom_id: Option<Uuid> = payload.classroom_id.or(existing_entry.classroom_id);
    let validation_payload = CreateTimetableEntryRequest {
        classroom_course_id: existing_entry.classroom_course_id,
        day_of_week: payload.day_of_week.clone().unwrap_or_else(|| existing_entry.day_of_week.clone()),
        period_id: payload.period_id.unwrap_or(existing_entry.period_id),
        room_id: payload.room_id.or(existing_entry.room_id),
        note: payload.note.clone().or(existing_entry.note),
        activity_slot_id: existing_entry.activity_slot_id,
        entry_type: Some(existing_entry.entry_type.clone()),
        title: existing_entry.title.clone(),
        classroom_id: target_classroom_id,
        academic_semester_id: Some(existing_entry.academic_semester_id),
        client_temp_id: None,
    };

    // 3. Validate conflicts (excluding current entry ID)
    let mut conflict_list: Vec<serde_json::Value> = Vec::new();

    // 3a. Classroom conflict — ใช้ target classroom (ใหม่ถ้าเปลี่ยน)
    let classroom_conflict: Option<(String,)> = sqlx::query_as(
        r#"SELECT cr.name
           FROM academic_timetable_entries te
           LEFT JOIN class_rooms cr ON cr.id = te.classroom_id
           WHERE te.classroom_id = $1
             AND te.day_of_week = $2
             AND te.period_id = $3
             AND te.is_active = true
             AND te.id <> $4
           LIMIT 1"#
    )
    .bind(target_classroom_id)
    .bind(&validation_payload.day_of_week)
    .bind(validation_payload.period_id)
    .bind(id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if let Some((cr_name,)) = classroom_conflict {
        conflict_list.push(json!({
            "conflict_type": "CLASSROOM_CONFLICT",
            "message": format!("{} มีตารางในคาบนี้อยู่แล้ว", cr_name)
        }));
    }

    // 3b. Room conflict — same room, day, period (exclude self)
    if let Some(room_id) = validation_payload.room_id {
        let room_conflict: Option<(String,)> = sqlx::query_as(
            r#"SELECT r.code
               FROM academic_timetable_entries te
               JOIN rooms r ON r.id = te.room_id
               WHERE te.room_id = $1
                 AND te.day_of_week = $2
                 AND te.period_id = $3
                 AND te.is_active = true
                 AND te.id <> $4
               LIMIT 1"#
        )
        .bind(room_id)
        .bind(&validation_payload.day_of_week)
        .bind(validation_payload.period_id)
        .bind(id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);

        if let Some((room_code,)) = room_conflict {
            conflict_list.push(json!({
                "conflict_type": "ROOM_CONFLICT",
                "message": format!("ห้อง {} มีการใช้งานในคาบนี้อยู่แล้ว", room_code)
            }));
        }
    }

    // 3c. Instructor conflict via junction (exclude self)
    let candidate_instructors: Vec<Uuid> = sqlx::query_scalar(
        "SELECT instructor_id FROM timetable_entry_instructors WHERE entry_id = $1"
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    if !candidate_instructors.is_empty() {
        let conflict_instructors: Vec<(String,)> = sqlx::query_as(
            r#"SELECT DISTINCT concat(u.first_name, ' ', u.last_name)
               FROM academic_timetable_entries te
               JOIN timetable_entry_instructors tei ON tei.entry_id = te.id
               JOIN users u ON u.id = tei.instructor_id
               WHERE tei.instructor_id = ANY($1)
                 AND te.day_of_week = $2
                 AND te.period_id = $3
                 AND te.is_active = true
                 AND te.id <> $4"#
        )
        .bind(&candidate_instructors)
        .bind(&validation_payload.day_of_week)
        .bind(validation_payload.period_id)
        .bind(id)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        for (name,) in &conflict_instructors {
            conflict_list.push(json!({
                "conflict_type": "INSTRUCTOR_CONFLICT",
                "message": format!("{} มีสอนในคาบนี้อยู่แล้ว", name)
            }));
        }
    }

    if !conflict_list.is_empty() {
        // Broadcast DropRejected → ทุก client rollback optimistic state
        // (เฉพาะถ้ามี subscriber อื่น ๆ ที่อาจรับ DropIntent ก่อนหน้านี้)
        let subdomain_for_reject = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        let reason = conflict_list
            .iter()
            .filter_map(|c| c.get("message").and_then(|m| m.as_str()))
            .collect::<Vec<_>>()
            .join(" · ");
        state.websocket_manager.broadcast_ephemeral(
            subdomain_for_reject,
            existing_entry.academic_semester_id,
            TimetableEvent::DropRejected {
                user_id: user_id.unwrap_or_default(),
                entry_id: id,
                original_day: existing_entry.day_of_week.clone(),
                original_period_id: existing_entry.period_id,
                original_room_id: existing_entry.room_id,
                partner_id: None,
                partner_original_day: None,
                partner_original_period_id: None,
                reason: if reason.is_empty() { "พบข้อขัดแย้ง".to_string() } else { reason },
            },
        );
        return Ok((
            StatusCode::CONFLICT,
            Json(json!({
                "success": false,
                "message": "Conflict detected",
                "conflicts": conflict_list
            }))
        ).into_response());
    }

    // 4. Update Entry (accept content change via classroom_course_id / activity_slot_id
    //    for drag-from-sidebar-onto-occupied "replace" flow, + classroom_id สำหรับ replace ข้ามห้อง)
    let updated_entry = sqlx::query_as::<_, TimetableEntry>(
        r#"
        UPDATE academic_timetable_entries SET
            day_of_week = COALESCE($2, day_of_week),
            period_id = COALESCE($3, period_id),
            room_id = COALESCE($4, room_id),
            note = COALESCE($5, note),
            classroom_course_id = COALESCE($7, classroom_course_id),
            activity_slot_id = COALESCE($8, activity_slot_id),
            classroom_id = COALESCE($9, classroom_id),
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
    .bind(payload.classroom_course_id)
    .bind(payload.activity_slot_id)
    .bind(payload.classroom_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to update entry: {}", e);
        let msg = e.to_string();
        if msg.contains("unique_entry_per_slot") {
            AppError::BadRequest("This slot is already occupied".to_string())
        } else if msg.contains("instructor double-book") || msg.contains("ไม่สามารถย้าย") {
            // Trigger-raised RAISE EXCEPTION surfaces as Rust string
            AppError::BadRequest(msg.split("ERROR:").last().unwrap_or(&msg).trim().to_string())
        } else {
            AppError::InternalServerError("Failed to update entry".to_string())
        }
    })?;

    // Broadcast patch event — re-fetch joined เฉพาะถ้ามี subscriber
    let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    let has_subs = state.websocket_manager.has_other_subscribers(subdomain.clone(), existing_entry.academic_semester_id);
    if has_subs {
        if let Some(full_entry) = fetch_entry_with_joins(&pool, updated_entry.id).await {
            let entry_json = serde_json::to_value(&full_entry).unwrap_or_default();
            state.websocket_manager.broadcast_mutation(
                subdomain,
                existing_entry.academic_semester_id,
                TimetableEvent::EntryUpdated { user_id: user_id.unwrap_or_default(), entry: entry_json },
            );
        }
    }

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
    let force = payload.force.unwrap_or(false);

    // ต้องเลือกห้องอย่างน้อย 1 หรือ ครูอย่างน้อย 1
    if payload.classroom_ids.is_empty() && payload.instructor_ids.is_empty() {
        return Err(AppError::BadRequest(
            "ต้องเลือกห้องเรียน หรือ ครู อย่างน้อย 1 อย่าง".to_string(),
        ));
    }

    // Validate slot participation + instructor exists (sync) — unchanged from before
    if let Some(slot_id) = payload.activity_slot_id {
        let non_participating: Vec<(String,)> = sqlx::query_as(
            r#"SELECT cr.name FROM class_rooms cr
               WHERE cr.id = ANY($1)
                 AND NOT EXISTS (SELECT 1 FROM activity_slot_classrooms
                                 WHERE slot_id = $2 AND classroom_id = cr.id)"#
        ).bind(&payload.classroom_ids).bind(slot_id)
        .fetch_all(&pool).await.unwrap_or_default();
        if !non_participating.is_empty() {
            let names: Vec<String> = non_participating.into_iter().map(|(n,)| n).collect();
            return Err(AppError::BadRequest(format!(
                "ห้องต่อไปนี้ยังไม่ได้อยู่ในกิจกรรม: {} — เพิ่มห้องที่ Course Planning ก่อน",
                names.join(", ")
            )));
        }
        let missing_teacher: Vec<(String,)> = sqlx::query_as(
            r#"SELECT cr.name
               FROM class_rooms cr, activity_slots s
               JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
               WHERE s.id = $2 AND cr.id = ANY($1)
                 AND CASE WHEN ac.scheduling_mode = 'independent' THEN
                          NOT EXISTS(SELECT 1 FROM activity_slot_classroom_assignments
                                     WHERE slot_id = $2 AND classroom_id = cr.id)
                         ELSE NOT EXISTS(SELECT 1 FROM activity_slot_instructors
                                         WHERE slot_id = $2) END"#
        ).bind(&payload.classroom_ids).bind(slot_id)
        .fetch_all(&pool).await.unwrap_or_default();
        if !missing_teacher.is_empty() {
            let names: Vec<String> = missing_teacher.into_iter().map(|(n,)| n).collect();
            return Err(AppError::BadRequest(format!(
                "กิจกรรมนี้ยังไม่ได้กำหนดครูผู้สอน (กระทบ: {}) — เพิ่มครูที่หน้า Activities ก่อน",
                names.join(", ")
            )));
        }
    }

    // ===== Determine batch type =====
    // BatchType::SyncActivity → slot.scheduling_mode = synchronized
    // BatchType::Other        → text หรือ independent activity (ใช้ rules เดียวกัน)
    let is_sync_batch = if let Some(slot_id) = payload.activity_slot_id {
        let mode: Option<String> = sqlx::query_scalar(
            "SELECT ac.scheduling_mode FROM activity_slots s
             JOIN activity_catalog ac ON ac.id = s.activity_catalog_id WHERE s.id = $1"
        ).bind(slot_id).fetch_optional(&pool).await.ok().flatten();
        mode.as_deref() == Some("synchronized")
    } else { false };

    // ===== Resolve candidate instructors (ครูที่จะติด tei กับ entries ใหม่) =====
    let mut candidate_instructors: Vec<Uuid> = if let Some(slot_id) = payload.activity_slot_id {
        if is_sync_batch {
            sqlx::query_scalar(
                "SELECT user_id FROM activity_slot_instructors WHERE slot_id = $1"
            ).bind(slot_id).fetch_all(&pool).await.unwrap_or_default()
        } else {
            // independent
            sqlx::query_scalar(
                "SELECT instructor_id FROM activity_slot_classroom_assignments
                 WHERE slot_id = $1 AND classroom_id = ANY($2)"
            ).bind(slot_id).bind(&payload.classroom_ids).fetch_all(&pool).await.unwrap_or_default()
        }
    } else if let Some(subject_id) = payload.subject_id {
        sqlx::query_scalar(
            "SELECT DISTINCT cci.instructor_id FROM classroom_course_instructors cci
             JOIN classroom_courses cc ON cc.id = cci.classroom_course_id
             WHERE cc.classroom_id = ANY($1) AND cc.subject_id = $2"
        ).bind(&payload.classroom_ids).bind(subject_id).fetch_all(&pool).await.unwrap_or_default()
    } else { Vec::new() };
    for id in &payload.instructor_ids {
        if !candidate_instructors.contains(id) { candidate_instructors.push(*id); }
    }

    // ===== Pre-fetch existing entries that COULD conflict =====
    // (ทุก entry ที่ active + อยู่ใน day×period ของ batch + ไม่ใช่ slot เดียวกัน)
    #[derive(sqlx::FromRow, Clone)]
    struct ExistingEntry {
        id: Uuid,
        classroom_id: Option<Uuid>,
        classroom_name: Option<String>,
        day_of_week: String,
        period_id: Uuid,
        period_name: Option<String>,
        room_id: Option<Uuid>,
        title: Option<String>,
        entry_type: String,
        activity_slot_id: Option<Uuid>,
        scheduling_mode: Option<String>,
        display_title: String,
        instructor_ids: Vec<Uuid>,
        instructor_names: Vec<String>,
    }

    let existing: Vec<ExistingEntry> = sqlx::query_as::<_, ExistingEntry>(
        r#"
        SELECT te.id, te.classroom_id, cr.name AS classroom_name,
               te.day_of_week, te.period_id,
               COALESCE(ap.name, 'คาบ ' || ap.order_index::text) AS period_name,
               te.room_id, te.title, te.entry_type,
               te.activity_slot_id, ac.scheduling_mode,
               COALESCE(s.name_th, te.title, '(ไม่ระบุ)') AS display_title,
               COALESCE(ARRAY_AGG(DISTINCT tei.instructor_id) FILTER (WHERE tei.instructor_id IS NOT NULL), '{}'::uuid[]) AS instructor_ids,
               COALESCE(ARRAY_AGG(DISTINCT concat(u.first_name, ' ', u.last_name)) FILTER (WHERE u.id IS NOT NULL), '{}'::text[]) AS instructor_names
          FROM academic_timetable_entries te
          LEFT JOIN class_rooms cr ON cr.id = te.classroom_id
          LEFT JOIN academic_periods ap ON ap.id = te.period_id
          LEFT JOIN classroom_courses cc ON cc.id = te.classroom_course_id
          LEFT JOIN subjects s ON s.id = cc.subject_id
          LEFT JOIN activity_slots aslot ON aslot.id = te.activity_slot_id
          LEFT JOIN activity_catalog ac ON ac.id = aslot.activity_catalog_id
          LEFT JOIN timetable_entry_instructors tei ON tei.entry_id = te.id
          LEFT JOIN users u ON u.id = tei.instructor_id
         WHERE te.is_active = true
           AND te.day_of_week = ANY($1)
           AND te.period_id = ANY($2)
           AND (te.activity_slot_id IS DISTINCT FROM $3 OR te.activity_slot_id IS NULL)
         GROUP BY te.id, cr.name, ap.name, ap.order_index, s.name_th, ac.scheduling_mode
        "#
    )
    .bind(&payload.days_of_week)
    .bind(&payload.period_ids)
    .bind(payload.activity_slot_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("fetch existing entries: {}", e)))?;

    // ===== Build summary collectors =====
    let mut skipped: Vec<serde_json::Value> = Vec::new();
    let mut blocked: Vec<serde_json::Value> = Vec::new();
    let mut deleted: Vec<serde_json::Value> = Vec::new();
    let mut excluded_instructors_map: std::collections::HashMap<Uuid, (String, Vec<serde_json::Value>)> = std::collections::HashMap::new();
    let mut entries_to_delete: Vec<Uuid> = Vec::new();

    // ===== Per-cell decision =====
    // เซตของ (classroom, day, period) ที่จะ INSERT จริง (filtered)
    let mut insert_tuples: Vec<(Uuid, String, Uuid)> = Vec::new();

    for cr_id in &payload.classroom_ids {
        for day in &payload.days_of_week {
            for p_id in &payload.period_ids {
                // หา entries ที่ "ทับ" cell นี้:
                //   1. classroom_id ตรง → classroom conflict
                //   2. room_id ตรง → room conflict
                //   3. instructor_id ใน candidate_instructors → instructor conflict
                let cell_conflicts: Vec<&ExistingEntry> = existing.iter().filter(|e| {
                    if e.day_of_week != *day || e.period_id != *p_id { return false; }
                    // Match by classroom OR room OR shared instructor
                    e.classroom_id == Some(*cr_id)
                    || (payload.room_id.is_some() && e.room_id == payload.room_id)
                    || e.instructor_ids.iter().any(|i| candidate_instructors.contains(i))
                }).collect();

                if cell_conflicts.is_empty() {
                    insert_tuples.push((*cr_id, day.clone(), *p_id));
                    continue;
                }

                // มี conflict — ตัดสินใจตาม batch_type + force
                let has_sync_conflict = cell_conflicts.iter().any(|e|
                    e.scheduling_mode.as_deref() == Some("synchronized")
                );

                if is_sync_batch {
                    // === SYNC batch decisions ===
                    // Classroom conflict → block (นักเรียนห้องนี้มีคาบอื่นอยู่)
                    let classroom_busy = cell_conflicts.iter().find(|e| e.classroom_id == Some(*cr_id));
                    if let Some(blocker) = classroom_busy {
                        if force {
                            // Force: ลบ entry เดิมแล้ว insert sync ใหม่
                            // (ถ้าเป็น sync อื่นอยู่แล้ว — block เพราะ sync vs sync ไม่ should override)
                            if blocker.scheduling_mode.as_deref() == Some("synchronized") {
                                blocked.push(json!({
                                    "classroom_id": cr_id, "classroom_name": blocker.classroom_name,
                                    "day_of_week": day, "period_id": p_id, "period_name": blocker.period_name,
                                    "reason": "SYNC_VS_SYNC",
                                    "message": format!("{} {}: ทับกับกิจกรรม sync '{}' — sync vs sync ทับไม่ได้",
                                                       blocker.classroom_name.as_deref().unwrap_or("?"), day, blocker.display_title)
                                }));
                                continue;
                            }
                            entries_to_delete.push(blocker.id);
                            deleted.push(json!({
                                "id": blocker.id, "classroom_name": blocker.classroom_name,
                                "day_of_week": day, "period_id": p_id, "period_name": blocker.period_name,
                                "title": blocker.display_title, "entry_type": blocker.entry_type,
                                "instructor_names": blocker.instructor_names
                            }));
                            insert_tuples.push((*cr_id, day.clone(), *p_id));
                        } else {
                            blocked.push(json!({
                                "classroom_id": cr_id, "classroom_name": blocker.classroom_name,
                                "day_of_week": day, "period_id": p_id, "period_name": blocker.period_name,
                                "reason": "STUDENT_BUSY",
                                "message": format!("{} {}: นักเรียนติด '{}' ลบของเดิมก่อน",
                                                   blocker.classroom_name.as_deref().unwrap_or("?"), day, blocker.display_title)
                            }));
                        }
                        continue;
                    }
                    // Room conflict → skip cell (force: delete + insert)
                    let room_busy = cell_conflicts.iter().find(|e|
                        payload.room_id.is_some() && e.room_id == payload.room_id && e.classroom_id != Some(*cr_id)
                    );
                    if let Some(blocker) = room_busy {
                        if force {
                            entries_to_delete.push(blocker.id);
                            deleted.push(json!({
                                "id": blocker.id, "classroom_name": blocker.classroom_name,
                                "day_of_week": day, "period_id": p_id, "period_name": blocker.period_name,
                                "title": blocker.display_title, "entry_type": blocker.entry_type,
                                "instructor_names": blocker.instructor_names
                            }));
                            insert_tuples.push((*cr_id, day.clone(), *p_id));
                        } else {
                            skipped.push(json!({
                                "classroom_id": cr_id, "day_of_week": day, "period_id": p_id,
                                "period_name": blocker.period_name,
                                "reason": "ROOM_BUSY",
                                "message": format!("{}: ห้องสอนถูกใช้โดย '{}' อยู่", day, blocker.display_title)
                            }));
                        }
                        continue;
                    }
                    // Instructor conflict only (no classroom/room) → exclude instructor (sync no-force) / delete (force)
                    // For sync, we collect excluded instructors globally; cell still gets inserted
                    let mut conflicting_instructors: Vec<(Uuid, String)> = Vec::new();
                    for e in &cell_conflicts {
                        for (idx, iid) in e.instructor_ids.iter().enumerate() {
                            if candidate_instructors.contains(iid) {
                                let name = e.instructor_names.get(idx).cloned().unwrap_or_default();
                                conflicting_instructors.push((*iid, name));
                                if force {
                                    if !entries_to_delete.contains(&e.id) {
                                        entries_to_delete.push(e.id);
                                        deleted.push(json!({
                                            "id": e.id, "classroom_name": e.classroom_name,
                                            "day_of_week": day, "period_id": p_id, "period_name": e.period_name,
                                            "title": e.display_title, "entry_type": e.entry_type,
                                            "instructor_names": e.instructor_names
                                        }));
                                    }
                                }
                            }
                        }
                    }
                    if !force {
                        for (iid, _name) in &conflicting_instructors {
                            // find existing entry that conflict with this instructor at this cell
                            let conf_entry = cell_conflicts.iter().find(|e| e.instructor_ids.contains(iid)).unwrap();
                            let entry_record = excluded_instructors_map.entry(*iid).or_insert_with(|| {
                                let nm = cell_conflicts.iter()
                                    .filter_map(|e| {
                                        e.instructor_ids.iter().position(|x| x == iid)
                                            .and_then(|idx| e.instructor_names.get(idx))
                                    })
                                    .next()
                                    .cloned()
                                    .unwrap_or_default();
                                (nm, Vec::new())
                            });
                            entry_record.1.push(json!({
                                "day_of_week": day, "period_id": p_id,
                                "period_name": conf_entry.period_name,
                                "existing_title": conf_entry.display_title
                            }));
                        }
                    }
                    insert_tuples.push((*cr_id, day.clone(), *p_id));
                } else {
                    // === TEXT/independent activity batch decisions ===
                    if has_sync_conflict {
                        // ทับ sync activity — block ทั้ง no-force และ force
                        let sync_blocker = cell_conflicts.iter().find(|e|
                            e.scheduling_mode.as_deref() == Some("synchronized")
                        ).unwrap();
                        blocked.push(json!({
                            "classroom_id": cr_id, "classroom_name": sync_blocker.classroom_name,
                            "day_of_week": day, "period_id": p_id, "period_name": sync_blocker.period_name,
                            "reason": "SYNC_ACTIVITY_PRESENT",
                            "message": format!("{} {}: มีกิจกรรม sync '{}' — ลบกิจกรรม sync ก่อน",
                                               sync_blocker.classroom_name.as_deref().unwrap_or("?"), day, sync_blocker.display_title)
                        }));
                        continue;
                    }
                    if force {
                        // Overwrite — delete conflicts + insert
                        for e in &cell_conflicts {
                            if !entries_to_delete.contains(&e.id) {
                                entries_to_delete.push(e.id);
                                deleted.push(json!({
                                    "id": e.id, "classroom_name": e.classroom_name,
                                    "day_of_week": day, "period_id": p_id, "period_name": e.period_name,
                                    "title": e.display_title, "entry_type": e.entry_type,
                                    "instructor_names": e.instructor_names
                                }));
                            }
                        }
                        insert_tuples.push((*cr_id, day.clone(), *p_id));
                    } else {
                        // No force: skip cell with detailed reason
                        let primary = &cell_conflicts[0];
                        let reason = if primary.classroom_id == Some(*cr_id) {
                            if primary.entry_type == "COURSE" { "CLASSROOM_COURSE" } else { "CLASSROOM_ACTIVITY" }
                        } else if primary.instructor_ids.iter().any(|i| candidate_instructors.contains(i)) {
                            "INSTRUCTOR_BUSY"
                        } else { "ROOM_BUSY" };
                        skipped.push(json!({
                            "classroom_id": cr_id, "classroom_name": primary.classroom_name,
                            "day_of_week": day, "period_id": p_id, "period_name": primary.period_name,
                            "reason": reason,
                            "message": format!("{} {}: ทับ '{}' ({})",
                                               primary.classroom_name.as_deref().unwrap_or("?"),
                                               day, primary.display_title,
                                               primary.instructor_names.join(", "))
                        }));
                    }
                }
            }
        }
    }

    // Compute effective instructors for tei attach (exclude conflicting for sync no-force)
    let effective_instructors: Vec<Uuid> = if is_sync_batch && !force {
        payload.instructor_ids.iter()
            .filter(|i| !excluded_instructors_map.contains_key(i))
            .copied().collect()
    } else {
        payload.instructor_ids.clone()
    };

    // ===== Execute transaction =====
    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;
    let batch_uuid = Uuid::new_v4();

    // DELETE for overwrite
    if !entries_to_delete.is_empty() {
        sqlx::query("DELETE FROM academic_timetable_entries WHERE id = ANY($1)")
            .bind(&entries_to_delete)
            .execute(&mut *tx).await
            .map_err(|e| AppError::InternalServerError(format!("delete overwrite: {}", e)))?;
    }

    // === CLASSROOM entries — bulk INSERT จาก insert_tuples (filtered) ===
    // ใช้ UNNEST ของ 3 arrays แบบ paired (ไม่ใช่ cross join) เพื่อ insert เฉพาะ cell ที่ผ่านเงื่อนไข
    let mut inserted_count: i64 = 0;
    if !insert_tuples.is_empty() {
        let cr_arr: Vec<Uuid> = insert_tuples.iter().map(|(c, _, _)| *c).collect();
        let d_arr: Vec<String> = insert_tuples.iter().map(|(_, d, _)| d.clone()).collect();
        let p_arr: Vec<Uuid> = insert_tuples.iter().map(|(_, _, p)| *p).collect();

        let result = sqlx::query(
            r#"
            WITH cc_map AS (
                SELECT cc.id AS cc_id, cc.classroom_id AS cr_id, s.name_th AS course_name
                FROM classroom_courses cc
                JOIN subjects s ON cc.subject_id = s.id
                WHERE $8::uuid IS NOT NULL
                  AND cc.subject_id = $8
                  AND cc.classroom_id = ANY($5)
            ),
            new_entries AS (
                INSERT INTO academic_timetable_entries (
                    id, classroom_id, academic_semester_id, day_of_week, period_id, room_id,
                    entry_type, title, is_active, created_by, updated_by,
                    classroom_course_id, note, activity_slot_id, batch_id
                )
                SELECT gen_random_uuid(), t.c, $1, t.d, t.p, $2,
                    CASE WHEN cc_map.cc_id IS NOT NULL THEN 'COURSE' ELSE $3 END,
                    COALESCE(cc_map.course_name, $4),
                    true, $9, $9,
                    cc_map.cc_id, $10, $11, $12
                FROM UNNEST($5::uuid[], $6::text[], $7::uuid[]) AS t(c, d, p)
                LEFT JOIN cc_map ON cc_map.cr_id = t.c
                ON CONFLICT DO NOTHING
                RETURNING id, classroom_id, classroom_course_id
            ),
            slot_mode AS (
                SELECT ac.scheduling_mode AS mode
                FROM activity_slots s
                JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
                WHERE $11::uuid IS NOT NULL AND s.id = $11
            ),
            tei_inserts AS (
                INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                SELECT ne.id, cci.instructor_id, cci.role
                    FROM new_entries ne
                    JOIN classroom_course_instructors cci ON cci.classroom_course_id = ne.classroom_course_id
                    WHERE ne.classroom_course_id IS NOT NULL
                UNION ALL
                SELECT ne.id, asca.instructor_id, 'primary'
                    FROM new_entries ne
                    JOIN activity_slot_classroom_assignments asca
                        ON asca.slot_id = $11 AND asca.classroom_id = ne.classroom_id
                    WHERE (SELECT mode FROM slot_mode) = 'independent'
                UNION ALL
                -- SLOT-sync: attach effective instructors (ตัด excluded ออกแล้ว)
                SELECT ne.id, i.v, 'primary'
                    FROM new_entries ne
                    CROSS JOIN UNNEST($13::uuid[]) AS i(v)
                    WHERE (SELECT mode FROM slot_mode) = 'synchronized'
                ON CONFLICT DO NOTHING
                RETURNING entry_id
            )
            SELECT COUNT(*) FROM new_entries
            "#
        )
        .bind(payload.academic_semester_id)   // $1
        .bind(payload.room_id)                // $2
        .bind(&payload.entry_type)            // $3
        .bind(&payload.title)                 // $4
        .bind(&cr_arr)                        // $5
        .bind(&d_arr)                         // $6
        .bind(&p_arr)                         // $7
        .bind(payload.subject_id)             // $8
        .bind(user_id)                        // $9
        .bind(&payload.note)                  // $10
        .bind(payload.activity_slot_id)       // $11
        .bind(batch_uuid)                     // $12
        .bind(&effective_instructors)         // $13
        .fetch_one(&mut *tx).await
        .map_err(|e| {
            eprintln!("Failed bulk classroom batch INSERT: {}", e);
            AppError::InternalServerError("Failed to batch create entries".to_string())
        })?;
        inserted_count = sqlx::Row::try_get::<i64, _>(&result, 0).unwrap_or(0);
    }

    // === INSTRUCTOR-only entries — bulk INSERT + bulk TEI INSERT ===
    // ข้าม ถ้าเป็น SLOT mode (ครูถูก attach เข้า classroom entries' tei แล้วผ่าน CTE ด้านบน)
    if !payload.instructor_ids.is_empty() && payload.activity_slot_id.is_none() {
        let total = payload.instructor_ids.len()
            * payload.days_of_week.len()
            * payload.period_ids.len();
        let mut entry_ids: Vec<Uuid> = Vec::with_capacity(total);
        let mut instr_ids: Vec<Uuid> = Vec::with_capacity(total);
        let mut days: Vec<String> = Vec::with_capacity(total);
        let mut periods: Vec<Uuid> = Vec::with_capacity(total);
        for i_id in &payload.instructor_ids {
            for d in &payload.days_of_week {
                for p_id in &payload.period_ids {
                    entry_ids.push(Uuid::new_v4());
                    instr_ids.push(*i_id);
                    days.push(d.clone());
                    periods.push(*p_id);
                }
            }
        }

        sqlx::query(
            r#"INSERT INTO academic_timetable_entries (
                id, classroom_id, academic_semester_id, day_of_week, period_id, room_id,
                entry_type, title, is_active, created_by, updated_by,
                classroom_course_id, note, activity_slot_id, batch_id
            )
            SELECT id, NULL, $1, day, period, $2, $3, $4, true, $5, $5, NULL, $6, NULL, $7
            FROM UNNEST($8::uuid[], $9::text[], $10::uuid[]) AS t(id, day, period)
            ON CONFLICT DO NOTHING"#
        )
        .bind(payload.academic_semester_id)
        .bind(payload.room_id)
        .bind(&payload.entry_type)
        .bind(&payload.title)
        .bind(user_id)
        .bind(&payload.note)
        .bind(batch_uuid)
        .bind(&entry_ids)
        .bind(&days)
        .bind(&periods)
        .execute(&mut *tx).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        sqlx::query(
            r#"INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
               SELECT id, instr, 'primary'
               FROM UNNEST($1::uuid[], $2::uuid[]) AS t(id, instr)
               ON CONFLICT DO NOTHING"#
        )
        .bind(&entry_ids)
        .bind(&instr_ids)
        .execute(&mut *tx).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Broadcast refresh event (batch create affects many entries — client full-refetch)
    let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    state.websocket_manager.broadcast_mutation(
        subdomain,
        payload.academic_semester_id,
        TimetableEvent::TableRefresh { user_id: user_id.unwrap_or_default() },
    );

    // Build excluded_instructors response (sync no-force only)
    let excluded_instructors: Vec<serde_json::Value> = excluded_instructors_map.into_iter()
        .map(|(iid, (name, conflicts))| json!({
            "instructor_id": iid,
            "instructor_name": name,
            "conflicting_at": conflicts
        }))
        .collect();

    Ok(Json(json!({
        "success": true,
        "summary": {
            "inserted_count": inserted_count,
            "skipped": skipped,
            "blocked": blocked,
            "deleted": deleted,
            "excluded_instructors": excluded_instructors
        }
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

/// POST /api/academic/timetable/:id/instructors
#[derive(Debug, serde::Deserialize)]
pub struct AddEntryInstructorRequest {
    pub instructor_id: Uuid,
    pub role: Option<String>,
}

pub async fn add_entry_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(entry_id): Path<Uuid>,
    Json(body): Json<AddEntryInstructorRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();
    let role = body.role.clone().unwrap_or_else(|| "primary".to_string());
    sqlx::query(
        "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"
    )
    .bind(entry_id)
    .bind(body.instructor_id)
    .bind(&role)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Broadcast patch event: EntryInstructorAdded — skip instructor_name fetch ถ้าไม่มีคนฟัง
    let semester_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT academic_semester_id FROM academic_timetable_entries WHERE id = $1"
    ).bind(entry_id).fetch_optional(&pool).await.ok().flatten();
    if let Some(sem_id) = semester_id {
        let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        if state.websocket_manager.has_other_subscribers(subdomain.clone(), sem_id) {
            let instructor_name: String = sqlx::query_scalar(
                "SELECT CONCAT(first_name, ' ', last_name) FROM users WHERE id = $1"
            ).bind(body.instructor_id).fetch_one(&pool).await.unwrap_or_default();

            state.websocket_manager.broadcast_mutation(
                subdomain,
                sem_id,
                TimetableEvent::EntryInstructorAdded {
                    user_id: user_id.unwrap_or_default(),
                    entry_id,
                    instructor_id: body.instructor_id,
                    instructor_name,
                    role,
                },
            );
        }
    }

    Ok(Json(json!({ "success": true })).into_response())
}

/// DELETE /api/academic/timetable/:id/instructors/:uid
pub async fn remove_entry_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((entry_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();

    // Capture semester before potential cascade delete
    let semester_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT academic_semester_id FROM academic_timetable_entries WHERE id = $1"
    ).bind(entry_id).fetch_optional(&pool).await.ok().flatten();

    sqlx::query("DELETE FROM timetable_entry_instructors WHERE entry_id = $1 AND instructor_id = $2")
        .bind(entry_id)
        .bind(instructor_id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // If entry has no instructors left AND it's a regular course entry, delete the entry too
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM timetable_entry_instructors WHERE entry_id = $1"
    ).bind(entry_id).fetch_one(&pool).await.unwrap_or(1);
    let mut entry_deleted = false;
    if remaining == 0 {
        let is_course: bool = sqlx::query_scalar(
            "SELECT classroom_course_id IS NOT NULL FROM academic_timetable_entries WHERE id = $1"
        ).bind(entry_id).fetch_optional(&pool).await.ok().flatten().unwrap_or(false);
        if is_course {
            sqlx::query("DELETE FROM academic_timetable_entries WHERE id = $1")
                .bind(entry_id).execute(&pool).await.ok();
            entry_deleted = true;
        }
    }

    // Broadcast patch event: EntryInstructorRemoved (+ entry_deleted flag)
    if let Some(sem_id) = semester_id {
        let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        state.websocket_manager.broadcast_mutation(
            subdomain,
            sem_id,
            TimetableEvent::EntryInstructorRemoved {
                user_id: user_id.unwrap_or_default(),
                entry_id,
                instructor_id,
                entry_deleted,
            },
        );
    }

    Ok(Json(json!({ "success": true })).into_response())
}

/// POST /api/academic/timetable/slots/:slot_id/instructors/:uid/restore
/// Adds the instructor back to every active entry of the slot.
pub async fn restore_instructor_to_slot_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((slot_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let affected = sqlx::query(
        "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
         SELECT te.id, $2, 'primary' FROM academic_timetable_entries te
         WHERE te.activity_slot_id = $1 AND te.is_active = true
         ON CONFLICT DO NOTHING"
    )
    .bind(slot_id)
    .bind(instructor_id)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(Json(json!({ "success": true, "inserted": affected.rows_affected() })).into_response())
}

/// DELETE /api/academic/timetable/slots/:slot_id/instructors/:uid
/// Removes the instructor from every entry of the given slot (current semester implied by the entries).
/// Opposite of restore_instructor_to_slot_entries.
pub async fn hide_instructor_from_slot_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((slot_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let affected = sqlx::query(
        "DELETE FROM timetable_entry_instructors
         WHERE instructor_id = $1
           AND entry_id IN (
               SELECT id FROM academic_timetable_entries
               WHERE activity_slot_id = $2 AND is_active = true
           )"
    )
    .bind(instructor_id)
    .bind(slot_id)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Broadcast TableRefresh เพื่อให้ client อื่น sync
    let semester_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT academic_semester_id FROM academic_timetable_entries
         WHERE activity_slot_id = $1 LIMIT 1"
    )
    .bind(slot_id)
    .fetch_optional(&pool).await.ok().flatten();
    if let Some(sid) = semester_id {
        let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();
        state.websocket_manager.broadcast_mutation(
            subdomain, sid,
            TimetableEvent::TableRefresh { user_id: user_id.unwrap_or_default() },
        );
    }

    Ok(Json(json!({ "success": true, "deleted": affected.rows_affected() })).into_response())
}

/// DELETE /api/academic/timetable/slots/:slot_id/instructors/:uid/period
/// Query: day_of_week, period_id
/// Removes the instructor from entries of the given slot for one specific (day, period) only.
/// Used when an instructor wants to hide themselves from a single period of a synchronized
/// activity (across all classrooms in that slot).
#[derive(Debug, serde::Deserialize)]
pub struct HideSlotPeriodQuery {
    pub day_of_week: String,
    pub period_id: Uuid,
}

pub async fn hide_instructor_from_slot_period_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((slot_id, instructor_id)): Path<(Uuid, Uuid)>,
    Query(q): Query<HideSlotPeriodQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let affected = sqlx::query(
        "DELETE FROM timetable_entry_instructors
         WHERE instructor_id = $1
           AND entry_id IN (
               SELECT id FROM academic_timetable_entries
               WHERE activity_slot_id = $2
                 AND day_of_week = $3
                 AND period_id = $4
                 AND is_active = true
           )"
    )
    .bind(instructor_id)
    .bind(slot_id)
    .bind(&q.day_of_week)
    .bind(q.period_id)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Broadcast TableRefresh
    let semester_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT academic_semester_id FROM academic_timetable_entries
         WHERE activity_slot_id = $1 AND day_of_week = $2 AND period_id = $3 LIMIT 1"
    )
    .bind(slot_id)
    .bind(&q.day_of_week)
    .bind(q.period_id)
    .fetch_optional(&pool).await.ok().flatten();
    if let Some(sid) = semester_id {
        let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();
        state.websocket_manager.broadcast_mutation(
            subdomain, sid,
            TimetableEvent::TableRefresh { user_id: user_id.unwrap_or_default() },
        );
    }

    Ok(Json(json!({ "success": true, "deleted": affected.rows_affected() })).into_response())
}

// ============================================
// Swap + Validate-Moves (drag-drop UX enhancements)
// ============================================

/// POST /api/academic/timetable/swap
/// Atomically swap day+period of two timetable entries. classroom_id and room_id stay put.
/// Validates both entries don't conflict at their new positions.
pub async fn swap_timetable_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<SwapTimetableEntriesRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    // Fetch both entries' current day/period/room (+ batch_id เพื่อเช็ก pinned)
    let entries: Vec<(Uuid, String, Uuid, Option<Uuid>, Option<Uuid>, Uuid, Option<Uuid>)> = sqlx::query_as(
        r#"SELECT id, day_of_week, period_id, room_id, classroom_id, academic_semester_id, batch_id
           FROM academic_timetable_entries
           WHERE id = ANY($1) AND is_active = true"#
    )
    .bind(&[body.entry_a_id, body.entry_b_id])
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    if entries.len() != 2 {
        return Err(AppError::NotFound("Entry not found or inactive".to_string()));
    }

    // Block: ถ้า entry ใด entry หนึ่งสร้างจาก batch (pinned) → ไม่ให้สลับ
    if entries.iter().any(|e| e.6.is_some()) {
        return Err(AppError::BadRequest(
            "คาบที่สร้างจาก Batch ไม่สามารถสลับได้ (ลบก่อนแล้ว batch ใหม่แทน)".to_string(),
        ));
    }

    let (a, b) = if entries[0].0 == body.entry_a_id {
        (&entries[0], &entries[1])
    } else {
        (&entries[1], &entries[0])
    };

    // Helper tuples: (id, day, period, room, classroom, semester)
    let (a_id, a_day, a_period, a_room, a_classroom, _a_sem, _a_batch) = a.clone();
    let (b_id, b_day, b_period, b_room, b_classroom, _b_sem, _b_batch) = b.clone();

    // === Phase 2: ส่ง DropRejected เมื่อ swap conflict → ทุก client rollback optimistic state ===
    let swap_subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    let swap_user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok().unwrap_or_default();
    let swap_semester = _a_sem;
    let swap_a_day = a_day.clone();
    let swap_a_period = a_period;
    let swap_a_room = a_room;
    let swap_b_day = b_day.clone();
    let swap_b_period = b_period;
    let do_swap_reject = |reason: String| -> AppError {
        state.websocket_manager.broadcast_ephemeral(
            swap_subdomain.clone(),
            swap_semester,
            TimetableEvent::DropRejected {
                user_id: swap_user_id,
                entry_id: a_id,
                original_day: swap_a_day.clone(),
                original_period_id: swap_a_period,
                original_room_id: swap_a_room,
                partner_id: Some(b_id),
                partner_original_day: Some(swap_b_day.clone()),
                partner_original_period_id: Some(swap_b_period),
                reason: reason.clone(),
            },
        );
        AppError::BadRequest(reason)
    };

    // Validate: each entry's classroom must be free at new position (excluding swap partner)
    let a_target_conflict: Option<(String,)> = sqlx::query_as(
        r#"SELECT cr.name FROM academic_timetable_entries te
           LEFT JOIN class_rooms cr ON cr.id = te.classroom_id
           WHERE te.classroom_id = $1 AND te.day_of_week = $2 AND te.period_id = $3
             AND te.is_active = true AND te.id NOT IN ($4, $5)
           LIMIT 1"#
    )
    .bind(a_classroom)
    .bind(&b_day)
    .bind(b_period)
    .bind(a_id)
    .bind(b_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if let Some((name,)) = a_target_conflict {
        return Err(do_swap_reject(format!(
            "ห้อง {} ไม่ว่างที่ตำแหน่งปลายทางของ entry A", name
        )));
    }

    let b_target_conflict: Option<(String,)> = sqlx::query_as(
        r#"SELECT cr.name FROM academic_timetable_entries te
           LEFT JOIN class_rooms cr ON cr.id = te.classroom_id
           WHERE te.classroom_id = $1 AND te.day_of_week = $2 AND te.period_id = $3
             AND te.is_active = true AND te.id NOT IN ($4, $5)
           LIMIT 1"#
    )
    .bind(b_classroom)
    .bind(&a_day)
    .bind(a_period)
    .bind(a_id)
    .bind(b_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if let Some((name,)) = b_target_conflict {
        return Err(do_swap_reject(format!(
            "ห้อง {} ไม่ว่างที่ตำแหน่งปลายทางของ entry B", name
        )));
    }

    // Room conflict (if rooms set): each room must be free at new position (excluding each other)
    if let Some(a_room_id) = a_room {
        let conflict: Option<(String,)> = sqlx::query_as(
            r#"SELECT r.code FROM academic_timetable_entries te
               JOIN rooms r ON r.id = te.room_id
               WHERE te.room_id = $1 AND te.day_of_week = $2 AND te.period_id = $3
                 AND te.is_active = true AND te.id NOT IN ($4, $5)
               LIMIT 1"#
        )
        .bind(a_room_id)
        .bind(&b_day)
        .bind(b_period)
        .bind(a_id)
        .bind(b_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);
        if let Some((code,)) = conflict {
            return Err(do_swap_reject(format!(
                "ห้อง {} ถูกใช้ที่ตำแหน่งปลายทางของ entry A", code
            )));
        }
    }
    if let Some(b_room_id) = b_room {
        let conflict: Option<(String,)> = sqlx::query_as(
            r#"SELECT r.code FROM academic_timetable_entries te
               JOIN rooms r ON r.id = te.room_id
               WHERE te.room_id = $1 AND te.day_of_week = $2 AND te.period_id = $3
                 AND te.is_active = true AND te.id NOT IN ($4, $5)
               LIMIT 1"#
        )
        .bind(b_room_id)
        .bind(&a_day)
        .bind(a_period)
        .bind(a_id)
        .bind(b_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);
        if let Some((code,)) = conflict {
            return Err(do_swap_reject(format!(
                "ห้อง {} ถูกใช้ที่ตำแหน่งปลายทางของ entry B", code
            )));
        }
    }

    // Instructor conflict: each entry's instructors must be free at new position (excluding partner)
    let a_instr_conflict: Option<(String,)> = sqlx::query_as(
        r#"SELECT concat(u.first_name, ' ', u.last_name)
           FROM timetable_entry_instructors tei_self
           JOIN users u ON u.id = tei_self.instructor_id
           WHERE tei_self.entry_id = $1
             AND EXISTS (
                 SELECT 1 FROM timetable_entry_instructors tei_other
                 JOIN academic_timetable_entries te_other ON te_other.id = tei_other.entry_id
                 WHERE tei_other.instructor_id = tei_self.instructor_id
                   AND te_other.day_of_week = $2 AND te_other.period_id = $3
                   AND te_other.is_active = true
                   AND te_other.id NOT IN ($1, $4)
             )
           LIMIT 1"#
    )
    .bind(a_id)
    .bind(&b_day)
    .bind(b_period)
    .bind(b_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if let Some((name,)) = a_instr_conflict {
        return Err(do_swap_reject(format!(
            "ครู {} จะติดคาบที่ตำแหน่งปลายทางของ entry A", name
        )));
    }

    let b_instr_conflict: Option<(String,)> = sqlx::query_as(
        r#"SELECT concat(u.first_name, ' ', u.last_name)
           FROM timetable_entry_instructors tei_self
           JOIN users u ON u.id = tei_self.instructor_id
           WHERE tei_self.entry_id = $1
             AND EXISTS (
                 SELECT 1 FROM timetable_entry_instructors tei_other
                 JOIN academic_timetable_entries te_other ON te_other.id = tei_other.entry_id
                 WHERE tei_other.instructor_id = tei_self.instructor_id
                   AND te_other.day_of_week = $2 AND te_other.period_id = $3
                   AND te_other.is_active = true
                   AND te_other.id NOT IN ($1, $4)
             )
           LIMIT 1"#
    )
    .bind(b_id)
    .bind(&a_day)
    .bind(a_period)
    .bind(a_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if let Some((name,)) = b_instr_conflict {
        return Err(do_swap_reject(format!(
            "ครู {} จะติดคาบที่ตำแหน่งปลายทางของ entry B", name
        )));
    }

    // Perform swap in 3 steps to bypass trigger race (see migration 097 notes):
    //   1. Deactivate A (trigger returns early when NOT NEW.is_active)
    //   2. Move B to A's original position
    //   3. Reactivate A at B's original position
    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query("UPDATE academic_timetable_entries SET is_active = false WHERE id = $1")
        .bind(a_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(format!("swap step 1: {}", e)))?;

    sqlx::query(
        "UPDATE academic_timetable_entries SET day_of_week = $1, period_id = $2, updated_at = NOW() WHERE id = $3"
    )
    .bind(&a_day)
    .bind(a_period)
    .bind(b_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(format!("swap step 2: {}", e)))?;

    sqlx::query(
        "UPDATE academic_timetable_entries SET day_of_week = $1, period_id = $2, is_active = true, updated_at = NOW() WHERE id = $3"
    )
    .bind(&b_day)
    .bind(b_period)
    .bind(a_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(format!("swap step 3: {}", e)))?;

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Broadcast refresh (swap ทำให้ 2 entries เปลี่ยน day+period — client full-refetch ง่ายกว่า)
    let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();
    state.websocket_manager.broadcast_mutation(
        subdomain,
        entries[0].5,
        TimetableEvent::TableRefresh { user_id: user_id.unwrap_or_default() },
    );

    Ok(Json(json!({ "success": true, "message": "Swapped" })).into_response())
}

/// POST /api/academic/timetable/validate-moves
/// For given entry_id, compute validity of moving to every (day, period) cell in that entry's semester.
/// Returns map so frontend can colorize drop targets before release.
pub async fn validate_timetable_moves(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ValidateMovesRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    // Fetch source entry details
    let src: Option<(String, Uuid, Option<Uuid>, Option<Uuid>, Uuid, Uuid)> = sqlx::query_as(
        r#"SELECT day_of_week, period_id, classroom_id, room_id, academic_semester_id, id
           FROM academic_timetable_entries WHERE id = $1 AND is_active = true"#
    )
    .bind(body.entry_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let (src_day, src_period, src_classroom, src_room, src_semester, _) = match src {
        Some(v) => v,
        None => return Err(AppError::NotFound("Entry not found".to_string())),
    };

    // Fetch all relevant entries in the same semester
    let all_entries: Vec<(Uuid, String, Uuid, Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        r#"SELECT id, day_of_week, period_id, classroom_id, room_id
           FROM academic_timetable_entries
           WHERE academic_semester_id = $1 AND is_active = true"#
    )
    .bind(src_semester)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Source entry's instructors
    let src_instructors: Vec<Uuid> = sqlx::query_scalar(
        "SELECT instructor_id FROM timetable_entry_instructors WHERE entry_id = $1"
    )
    .bind(body.entry_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // For each other entry, get its instructors (for swap conflict checks)
    let other_ids: Vec<Uuid> = all_entries.iter().map(|e| e.0).collect();
    let other_instructors_flat: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT entry_id, instructor_id FROM timetable_entry_instructors WHERE entry_id = ANY($1)"
    )
    .bind(&other_ids)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Group instructors by entry
    use std::collections::HashMap;
    let mut by_entry: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for (eid, iid) in &other_instructors_flat {
        by_entry.entry(*eid).or_default().push(*iid);
    }

    // Build a map: (day, period) → existing entries there
    let mut cell_entries: HashMap<(String, Uuid), Vec<&(Uuid, String, Uuid, Option<Uuid>, Option<Uuid>)>> = HashMap::new();
    for e in &all_entries {
        cell_entries.entry((e.1.clone(), e.2)).or_default().push(e);
    }

    // Fetch all periods for this semester's year (for iteration). Assume all days of week.
    let periods: Vec<(Uuid,)> = sqlx::query_as(
        r#"SELECT p.id FROM academic_periods p
           JOIN academic_semesters sem ON sem.academic_year_id = p.academic_year_id
           WHERE sem.id = $1
           ORDER BY p.order_index"#
    )
    .bind(src_semester)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let days = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
    let mut cells: Vec<MoveValidityCell> = Vec::new();

    for day in days.iter() {
        for (pid,) in &periods {
            let key = (day.to_string(), *pid);

            // Source itself
            if *day == src_day && *pid == src_period {
                cells.push(MoveValidityCell {
                    day_of_week: day.to_string(),
                    period_id: *pid,
                    state: "source".to_string(),
                    target_entry_id: None,
                    valid: false,
                    reason: String::new(),
                });
                continue;
            }

            let occupants: Vec<&(Uuid, String, Uuid, Option<Uuid>, Option<Uuid>)> =
                cell_entries.get(&key).cloned().unwrap_or_default();

            // Entries other than source at this cell
            let others: Vec<&(Uuid, String, Uuid, Option<Uuid>, Option<Uuid>)> =
                occupants.iter().filter(|e| e.0 != body.entry_id).copied().collect();

            if others.is_empty() {
                // Empty cell — check move validity (instructor/room conflicts of source at new pos)
                let mut valid = true;
                let mut reason = String::new();

                // Classroom conflict: source's classroom mustn't have another entry at this cell
                if all_entries.iter().any(|e| e.0 != body.entry_id && e.3 == src_classroom && e.1 == *day && e.2 == *pid) {
                    valid = false;
                    reason = "ห้องเรียนมี entry อื่น".to_string();
                }

                // Instructor conflict
                if valid {
                    for iid in &src_instructors {
                        if all_entries.iter().any(|e| {
                            e.0 != body.entry_id
                                && e.1 == *day
                                && e.2 == *pid
                                && by_entry.get(&e.0).map_or(false, |ids| ids.contains(iid))
                        }) {
                            valid = false;
                            reason = "ครูติดคาบ".to_string();
                            break;
                        }
                    }
                }

                // Room conflict
                if valid {
                    if let Some(r) = src_room {
                        if all_entries.iter().any(|e| e.0 != body.entry_id && e.4 == Some(r) && e.1 == *day && e.2 == *pid) {
                            valid = false;
                            reason = "ห้องถูกใช้".to_string();
                        }
                    }
                }

                cells.push(MoveValidityCell {
                    day_of_week: day.to_string(),
                    period_id: *pid,
                    state: "empty".to_string(),
                    target_entry_id: None,
                    valid,
                    reason,
                });
            } else {
                // Occupied — potential swap target (pick first other entry)
                let target = others[0];
                let target_id = target.0;

                // Swap: source → target's position, target → source's position
                // Validate source at target's pos + target at source's pos (pairwise, excluding each other)
                let mut valid = true;
                let mut reason = String::new();

                // Classroom: source's classroom at target pos — mustn't have 3rd entry
                if all_entries.iter().any(|e| e.0 != body.entry_id && e.0 != target_id && e.3 == src_classroom && e.1 == *day && e.2 == *pid) {
                    valid = false;
                    reason = format!("ห้องของต้นทางถูกใช้ที่คาบนี้");
                }
                // Classroom: target's classroom at source pos — mustn't have 3rd entry
                if valid && all_entries.iter().any(|e| e.0 != body.entry_id && e.0 != target_id && e.3 == target.3 && e.1 == src_day && e.2 == src_period) {
                    valid = false;
                    reason = format!("ห้องของปลายทางถูกใช้ที่คาบต้นทาง");
                }

                // Instructor conflicts pairwise
                if valid {
                    for iid in &src_instructors {
                        if all_entries.iter().any(|e| {
                            e.0 != body.entry_id
                                && e.0 != target_id
                                && e.1 == *day
                                && e.2 == *pid
                                && by_entry.get(&e.0).map_or(false, |ids| ids.contains(iid))
                        }) {
                            valid = false;
                            reason = "ครูต้นทางติดคาบปลายทาง".to_string();
                            break;
                        }
                    }
                }
                if valid {
                    let target_instr: Vec<Uuid> = by_entry.get(&target_id).cloned().unwrap_or_default();
                    for iid in &target_instr {
                        if all_entries.iter().any(|e| {
                            e.0 != body.entry_id
                                && e.0 != target_id
                                && e.1 == src_day
                                && e.2 == src_period
                                && by_entry.get(&e.0).map_or(false, |ids| ids.contains(iid))
                        }) {
                            valid = false;
                            reason = "ครูปลายทางติดคาบต้นทาง".to_string();
                            break;
                        }
                    }
                }

                // Room conflicts (if either has room set)
                if valid {
                    if let Some(r) = src_room {
                        if all_entries.iter().any(|e| e.0 != body.entry_id && e.0 != target_id && e.4 == Some(r) && e.1 == *day && e.2 == *pid) {
                            valid = false;
                            reason = "ห้องต้นทางถูกใช้ที่คาบปลายทาง".to_string();
                        }
                    }
                }
                if valid {
                    if let Some(r) = target.4 {
                        if all_entries.iter().any(|e| e.0 != body.entry_id && e.0 != target_id && e.4 == Some(r) && e.1 == src_day && e.2 == src_period) {
                            valid = false;
                            reason = "ห้องปลายทางถูกใช้ที่คาบต้นทาง".to_string();
                        }
                    }
                }

                cells.push(MoveValidityCell {
                    day_of_week: day.to_string(),
                    period_id: *pid,
                    state: "occupied".to_string(),
                    target_entry_id: Some(target_id),
                    valid,
                    reason,
                });
            }
        }
    }

    Ok(Json(json!({ "data": cells })).into_response())
}

#[derive(serde::Deserialize)]
pub struct OccupancyQuery {
    pub semester_id: Uuid,
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct OccupancyRow {
    pub id: Uuid,
    pub classroom_id: Option<Uuid>,
    pub day_of_week: String,
    pub period_id: Uuid,
    pub room_id: Option<Uuid>,
    pub instructor_ids: Vec<Uuid>,
}

/// GET /api/academic/timetable/occupancy?semester_id=X
/// Returns all active entries with instructor_ids — frontend builds indexes locally
/// to compute drop validity without hitting backend per drag.
pub async fn get_timetable_occupancy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<OccupancyQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    // Single query — aggregate instructor_ids via array_agg
    let rows: Vec<OccupancyRow> = sqlx::query_as::<_, OccupancyRow>(
        r#"SELECT
            te.id,
            te.classroom_id,
            te.day_of_week,
            te.period_id,
            te.room_id,
            COALESCE(
                ARRAY_AGG(tei.instructor_id) FILTER (WHERE tei.instructor_id IS NOT NULL),
                '{}'::uuid[]
            ) AS instructor_ids
           FROM academic_timetable_entries te
           LEFT JOIN timetable_entry_instructors tei ON tei.entry_id = te.id
           WHERE te.academic_semester_id = $1 AND te.is_active = true
           GROUP BY te.id"#,
    )
    .bind(q.semester_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "data": rows })).into_response())
}
