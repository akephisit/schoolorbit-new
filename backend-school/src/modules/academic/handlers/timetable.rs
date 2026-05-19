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
use crate::modules::academic::services::{timetable_service, period_service};

/// Helper
async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;

    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

/// Fetch 1 entry พร้อม joined fields — re-export จาก service สำหรับ patch events
async fn fetch_entry_with_joins(pool: &sqlx::PgPool, entry_id: Uuid) -> Option<TimetableEntry> {
    timetable_service::fetch_entry_by_id(pool, entry_id).await
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

    let periods = period_service::list_periods(&pool, query).await?;
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

    let period = period_service::create_period(&pool, payload).await?;
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

    let period = period_service::update_period(&pool, id, payload).await?;
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

    period_service::delete_period(&pool, id).await?;
    Ok(Json(json!({ "success": true })).into_response())
}

/// POST /api/academic/periods/reorder
pub async fn reorder_periods(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ReorderPeriodsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_STRUCTURE_MANAGE_ALL, &state.permission_cache).await {
        return Ok(response);
    }

    let updated = period_service::reorder_periods(&pool, payload).await?;
    Ok(Json(json!({ "success": true, "updated": updated })).into_response())
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

    let semester_id = query.academic_semester_id;
    let entries = timetable_service::list_entries(&pool, query.into()).await?;

    // current_seq ของ semester — client ใช้เป็นจุดเริ่มต้น tracking patch events
    let current_seq = if let Some(sem_id) = semester_id {
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

#[derive(Debug, serde::Deserialize)]
pub struct MyTimetableQuery {
    pub academic_semester_id: Option<Uuid>,
    pub day_of_week: Option<String>,
    pub include_team_ghosts: Option<bool>,
}

/// GET /api/me/timetable — ผู้ใช้ดูตารางของตัวเอง (student/staff)
/// - student: filter ตาม student_class_enrollments
/// - staff: filter ตาม timetable_entry_instructors (+ team ghosts ถ้าเลือก)
/// - parent: ใช้ /api/parent/students/{id}/timetable แทน
pub async fn get_my_timetable(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<MyTimetableQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user = crate::middleware::auth::get_current_user(&headers, &pool).await?;

    let filter = match user.user_type.as_str() {
        "student" => crate::modules::academic::services::timetable_service::TimetableFilter {
            student_id: Some(user.id),
            academic_semester_id: query.academic_semester_id,
            day_of_week: query.day_of_week,
            ..Default::default()
        },
        "staff" => crate::modules::academic::services::timetable_service::TimetableFilter {
            instructor_id: Some(user.id),
            academic_semester_id: query.academic_semester_id,
            day_of_week: query.day_of_week,
            include_team_ghosts: query.include_team_ghosts.unwrap_or(false),
            ..Default::default()
        },
        "parent" => return Err(AppError::BadRequest(
            "ผู้ปกครองต้องใช้ /api/parent/students/{id}/timetable".to_string(),
        )),
        _ => return Err(AppError::Forbidden("ไม่รองรับ user_type นี้".to_string())),
    };

    let entries = timetable_service::list_entries(&pool, filter).await?;

    let current_seq = if let Some(sem_id) = query.academic_semester_id {
        let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        state.websocket_manager.current_seq(subdomain, sem_id)
    } else {
        0
    };

    Ok(Json(json!({ "success": true, "data": entries, "current_seq": current_seq })).into_response())
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

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();
    let client_temp_id = payload.client_temp_id.clone();
    let payload_semester_id = payload.academic_semester_id;
    let payload_course_id = payload.classroom_course_id;

    let outcome = timetable_service::create_entry(&pool, user_id, payload).await?;

    match outcome {
        timetable_service::CreateEntryOutcome::Conflict(conflicts) => {
            // Broadcast EntryRejected → ทุก client ลบ tempEntry
            if let Some(temp_id) = client_temp_id {
                let subdomain_for_reject = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
                let sem_for_reject: Option<Uuid> = if let Some(sem) = payload_semester_id {
                    Some(sem)
                } else if let Some(cc_id) = payload_course_id {
                    sqlx::query_scalar("SELECT academic_semester_id FROM classroom_courses WHERE id = $1")
                        .bind(cc_id)
                        .fetch_optional(&pool)
                        .await
                        .ok()
                        .flatten()
                } else { None };
                if let Some(sem) = sem_for_reject {
                    let reason = conflicts.iter()
                        .map(|c| c.message.as_str())
                        .collect::<Vec<_>>()
                        .join(" · ");
                    state.websocket_manager.broadcast_ephemeral(
                        subdomain_for_reject,
                        sem,
                        TimetableEvent::EntryRejected {
                            user_id: user_id.unwrap_or_default(),
                            temp_id,
                            reason: if reason.is_empty() { "พบข้อขัดแย้ง".to_string() } else { reason },
                        },
                    );
                }
            }
            Ok((
                StatusCode::CONFLICT,
                Json(json!({
                    "success": false,
                    "message": "Timetable conflict detected",
                    "conflicts": conflicts
                }))
            ).into_response())
        }
        timetable_service::CreateEntryOutcome::Created(entry) => {
            let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
            let has_subs = state.websocket_manager.has_other_subscribers(subdomain.clone(), entry.academic_semester_id);

            // Re-fetch joined เฉพาะเมื่อต้อง broadcast
            if has_subs {
                if let Some(full_entry) = fetch_entry_with_joins(&pool, entry.id).await {
                    let entry_json = serde_json::to_value(&full_entry).unwrap_or_default();
                    state.websocket_manager.broadcast_mutation(
                        subdomain,
                        full_entry.academic_semester_id,
                        TimetableEvent::EntryCreated {
                            user_id: user_id.unwrap_or_default(),
                            entry: entry_json,
                            client_temp_id,
                        },
                    );
                }
            }

            Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": entry }))).into_response())
        }
    }
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

    let deleted_count = timetable_service::delete_entries_by_slot(
        &pool,
        payload.activity_slot_id,
        &payload.day_of_week,
        payload.academic_semester_id,
    ).await?;

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();
    let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    state.websocket_manager.broadcast_mutation(
        subdomain,
        payload.academic_semester_id,
        TimetableEvent::TableRefresh { user_id: user_id.unwrap_or_default() },
    );

    Ok(Json(json!({ "success": true, "deleted_count": deleted_count })).into_response())
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

    let semester_id = timetable_service::delete_entry(&pool, id).await?;

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
pub async fn delete_batch_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(batch_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(response);
    }

    let (deleted_count, semester_id) = timetable_service::delete_batch_group(&pool, batch_id).await?;

    if let Some(sid) = semester_id {
        let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();
        state.websocket_manager.broadcast_mutation(
            subdomain,
            sid,
            TimetableEvent::TableRefresh { user_id: user_id.unwrap_or_default() },
        );
    }

    Ok(Json(json!({ "success": true, "deleted_count": deleted_count })).into_response())
}

// Conflict detection ย้ายไป timetable_service::validate_entry

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

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();
    let outcome = timetable_service::update_entry(&pool, user_id, id, payload).await?;

    match outcome {
        timetable_service::UpdateEntryOutcome::Conflict { conflicts, existing } => {
            // Broadcast DropRejected → ทุก client rollback optimistic state
            let subdomain_for_reject = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
            let reason = conflicts
                .iter()
                .filter_map(|c| c.get("message").and_then(|m| m.as_str()))
                .collect::<Vec<_>>()
                .join(" · ");
            state.websocket_manager.broadcast_ephemeral(
                subdomain_for_reject,
                existing.academic_semester_id,
                TimetableEvent::DropRejected {
                    user_id: user_id.unwrap_or_default(),
                    entry_id: id,
                    original_day: existing.day_of_week.clone(),
                    original_period_id: existing.period_id,
                    original_room_id: existing.room_id,
                    partner_id: None,
                    partner_original_day: None,
                    partner_original_period_id: None,
                    reason: if reason.is_empty() { "พบข้อขัดแย้ง".to_string() } else { reason },
                },
            );
            Ok((
                StatusCode::CONFLICT,
                Json(json!({
                    "success": false,
                    "message": "Conflict detected",
                    "conflicts": conflicts
                }))
            ).into_response())
        }
        timetable_service::UpdateEntryOutcome::Updated { updated, existing } => {
            // Broadcast patch event — re-fetch joined เฉพาะถ้ามี subscriber
            let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
            let has_subs = state.websocket_manager.has_other_subscribers(subdomain.clone(), existing.academic_semester_id);
            if has_subs {
                if let Some(full_entry) = fetch_entry_with_joins(&pool, updated.id).await {
                    let entry_json = serde_json::to_value(&full_entry).unwrap_or_default();
                    state.websocket_manager.broadcast_mutation(
                        subdomain,
                        existing.academic_semester_id,
                        TimetableEvent::EntryUpdated { user_id: user_id.unwrap_or_default(), entry: entry_json },
                    );
                }
            }
            Ok(Json(json!({ "success": true, "data": updated })).into_response())
        }
    }
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
    let outcome = timetable_service::create_batch_entries(&pool, user_id, payload).await?;

    let subdomain = extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    state.websocket_manager.broadcast_mutation(
        subdomain,
        outcome.semester_id,
        TimetableEvent::TableRefresh { user_id: user_id.unwrap_or_default() },
    );

    Ok(Json(json!({
        "success": true,
        "summary": {
            "inserted_count": outcome.inserted_count,
            "skipped": outcome.skipped,
            "blocked": outcome.blocked,
            "deleted": outcome.deleted,
            "excluded_instructors": outcome.excluded_instructors
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
