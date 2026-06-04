use crate::db::school_mapping::get_school_database_url;
use crate::error::AppError;
use crate::middleware::permission::check_permission;
use crate::modules::academic::models::timetable::*;
use crate::modules::academic::services::{period_service, timetable_service};
use crate::modules::academic::websockets::TimetableEvent;
use crate::permissions::registry::codes;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

/// Helper
async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_client, &subdomain)
        .await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;

    state
        .pool_manager
        .get_pool(&db_url, &subdomain)
        .await
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

    if let Err(response) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_STRUCTURE_READ_ALL,
        &state.permission_cache,
    )
    .await
    {
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

    if let Err(response) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_STRUCTURE_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(response);
    }

    let period = period_service::create_period(&pool, payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": period })),
    )
        .into_response())
}

/// PUT /api/academic/periods/{id}
pub async fn update_period(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdatePeriodRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_STRUCTURE_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
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

    if let Err(response) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_STRUCTURE_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(response);
    }

    period_service::delete_period(&pool, id).await?;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

/// POST /api/academic/periods/reorder
pub async fn reorder_periods(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ReorderPeriodsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_STRUCTURE_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(response);
    }

    let updated = period_service::reorder_periods(&pool, payload).await?;
    Ok(Json(json!({ "success": true, "data": { "updated": updated } })).into_response())
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

    if let Err(response) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_READ_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(response);
    }

    let semester_id = query.academic_semester_id;
    let entries = timetable_service::list_entries(&pool, query.into()).await?;

    // current_seq ของ semester — client ใช้เป็นจุดเริ่มต้น tracking patch events
    let current_seq = if let Some(sem_id) = semester_id {
        let subdomain =
            extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        state.websocket_manager.current_seq(subdomain, sem_id)
    } else {
        0
    };

    Ok(
        Json(json!({ "success": true, "data": { "items": entries, "current_seq": current_seq } }))
            .into_response(),
    )
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
    if let Err(response) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_READ_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(response);
    }

    let subdomain =
        extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    let current_seq = state
        .websocket_manager
        .current_seq(subdomain.clone(), query.semester_id);

    match state.websocket_manager.replay(subdomain, query.semester_id, query.after_seq) {
        Some(events) => Ok(Json(json!({ "success": true, "data": { "events": events, "current_seq": current_seq, "needs_refetch": false } })).into_response()),
        None => Ok(Json(json!({ "success": true, "data": { "needs_refetch": true, "current_seq": current_seq } })).into_response()),
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
        "parent" => {
            return Err(AppError::BadRequest(
                "ผู้ปกครองต้องใช้ /api/parent/students/{id}/timetable".to_string(),
            ))
        }
        _ => return Err(AppError::Forbidden("ไม่รองรับ user_type นี้".to_string())),
    };

    let entries = timetable_service::list_entries(&pool, filter).await?;

    let current_seq = if let Some(sem_id) = query.academic_semester_id {
        let subdomain =
            extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        state.websocket_manager.current_seq(subdomain, sem_id)
    } else {
        0
    };

    Ok(
        Json(json!({ "success": true, "data": { "items": entries, "current_seq": current_seq } }))
            .into_response(),
    )
}

/// POST /api/academic/timetable
pub async fn create_timetable_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateTimetableEntryRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(response);
    }

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
        .await
        .ok();
    let client_temp_id = payload.client_temp_id.clone();
    let payload_semester_id = payload.academic_semester_id;
    let payload_course_id = payload.classroom_course_id;

    let outcome = timetable_service::create_entry(&pool, user_id, payload).await?;

    match outcome {
        timetable_service::CreateEntryOutcome::Conflict(conflicts) => {
            // Broadcast EntryRejected → ทุก client ลบ tempEntry
            if let Some(temp_id) = client_temp_id {
                let subdomain_for_reject = extract_subdomain_from_request(&headers)
                    .unwrap_or_else(|_| "default".to_string());
                let sem_for_reject: Option<Uuid> = if let Some(sem) = payload_semester_id {
                    Some(sem)
                } else if let Some(cc_id) = payload_course_id {
                    sqlx::query_scalar(
                        "SELECT academic_semester_id FROM classroom_courses WHERE id = $1",
                    )
                    .bind(cc_id)
                    .fetch_optional(&pool)
                    .await
                    .ok()
                    .flatten()
                } else {
                    None
                };
                if let Some(sem) = sem_for_reject {
                    let reason = conflicts
                        .iter()
                        .map(|c| c.message.as_str())
                        .collect::<Vec<_>>()
                        .join(" · ");
                    state.websocket_manager.broadcast_ephemeral(
                        subdomain_for_reject,
                        sem,
                        TimetableEvent::EntryRejected {
                            user_id: user_id.unwrap_or_default(),
                            temp_id,
                            reason: if reason.is_empty() {
                                "พบข้อขัดแย้ง".to_string()
                            } else {
                                reason
                            },
                        },
                    );
                }
            }
            Ok((
                StatusCode::CONFLICT,
                Json(json!({ "success": false, "error": "Timetable conflict detected", "message": "Timetable conflict detected", "data": { "conflicts": conflicts } }))
            ).into_response())
        }
        timetable_service::CreateEntryOutcome::Created(entry) => {
            let subdomain =
                extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
            let has_subs = state
                .websocket_manager
                .has_other_subscribers(subdomain.clone(), entry.academic_semester_id);

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

            Ok((
                StatusCode::CREATED,
                Json(json!({ "success": true, "data": entry })),
            )
                .into_response())
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

    if let Err(response) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(response);
    }

    let deleted_count = timetable_service::delete_entries_by_slot(
        &pool,
        payload.activity_slot_id,
        &payload.day_of_week,
        payload.academic_semester_id,
    )
    .await?;

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
        .await
        .ok();
    let subdomain =
        extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    state.websocket_manager.broadcast_mutation(
        subdomain,
        payload.academic_semester_id,
        TimetableEvent::TableRefresh {
            user_id: user_id.unwrap_or_default(),
        },
    );

    Ok(
        Json(json!({ "success": true, "data": { "deleted_count": deleted_count } }))
            .into_response(),
    )
}

/// DELETE /api/academic/timetable/{id}
pub async fn delete_timetable_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(response);
    }

    let semester_id = timetable_service::delete_entry(&pool, id).await?;

    if let Some(semester_id) = semester_id {
        let subdomain =
            extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
            .await
            .ok();
        state.websocket_manager.broadcast_mutation(
            subdomain,
            semester_id,
            TimetableEvent::EntryDeleted {
                user_id: user_id.unwrap_or_default(),
                entry_id: id,
            },
        );
    }

    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

/// DELETE /api/academic/timetable/batch-group/{batch_id}
pub async fn delete_batch_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(batch_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(response) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(response);
    }

    let (deleted_count, semester_id) =
        timetable_service::delete_batch_group(&pool, batch_id).await?;

    if let Some(sid) = semester_id {
        let subdomain =
            extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
            .await
            .ok();
        state.websocket_manager.broadcast_mutation(
            subdomain,
            sid,
            TimetableEvent::TableRefresh {
                user_id: user_id.unwrap_or_default(),
            },
        );
    }

    Ok(
        Json(json!({ "success": true, "data": { "deleted_count": deleted_count } }))
            .into_response(),
    )
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

    if let Err(response) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(response);
    }

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
        .await
        .ok();
    let outcome = timetable_service::update_entry(&pool, user_id, id, payload).await?;

    match outcome {
        timetable_service::UpdateEntryOutcome::Conflict {
            conflicts,
            existing,
        } => {
            // Broadcast DropRejected → ทุก client rollback optimistic state
            let subdomain_for_reject =
                extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
            let reason = conflicts
                .iter()
                .map(|c| c.message.as_str())
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
                    reason: if reason.is_empty() {
                        "พบข้อขัดแย้ง".to_string()
                    } else {
                        reason
                    },
                },
            );
            Ok((
                StatusCode::CONFLICT,
                Json(json!({ "success": false, "error": "Conflict detected", "message": "Conflict detected", "data": { "conflicts": conflicts } }))
            ).into_response())
        }
        timetable_service::UpdateEntryOutcome::Updated { updated, existing } => {
            // Broadcast patch event — re-fetch joined เฉพาะถ้ามี subscriber
            let subdomain =
                extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
            let has_subs = state
                .websocket_manager
                .has_other_subscribers(subdomain.clone(), existing.academic_semester_id);
            if has_subs {
                if let Some(full_entry) = fetch_entry_with_joins(&pool, updated.id).await {
                    let entry_json = serde_json::to_value(&full_entry).unwrap_or_default();
                    state.websocket_manager.broadcast_mutation(
                        subdomain,
                        existing.academic_semester_id,
                        TimetableEvent::EntryUpdated {
                            user_id: user_id.unwrap_or_default(),
                            entry: entry_json,
                        },
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

    if let Err(response) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(response);
    }

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
        .await
        .ok();
    let outcome = timetable_service::create_batch_entries(&pool, user_id, payload).await?;

    let subdomain =
        extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    state.websocket_manager.broadcast_mutation(
        subdomain,
        outcome.semester_id,
        TimetableEvent::TableRefresh {
            user_id: user_id.unwrap_or_default(),
        },
    );

    Ok(Json(json!({ "success": true, "data": { "summary": {
            "inserted_count": outcome.inserted_count,
            "skipped": outcome.skipped,
            "blocked": outcome.blocked,
            "deleted": outcome.deleted,
            "excluded_instructors": outcome.excluded_instructors
        } } }))
    .into_response())
}

/// GET /api/academic/timetable/{id}/my-activity
/// Returns the activity group the current user is enrolled in for a given timetable entry's slot
pub async fn get_my_activity_for_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(entry_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
        .await
        .map_err(|_| AppError::AuthError("Not authenticated".to_string()))?;

    let data = timetable_service::get_my_activity_for_entry(&pool, user_id, entry_id).await?;
    Ok(Json(json!({ "success": true, "data": data })).into_response())
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
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
        .await
        .ok();
    let role = body.role.clone().unwrap_or_else(|| "primary".to_string());

    let result =
        timetable_service::add_entry_instructor(&pool, entry_id, body.instructor_id, &role).await?;

    if let Some(sem_id) = result.semester_id {
        let subdomain =
            extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        if state
            .websocket_manager
            .has_other_subscribers(subdomain.clone(), sem_id)
        {
            state.websocket_manager.broadcast_mutation(
                subdomain,
                sem_id,
                TimetableEvent::EntryInstructorAdded {
                    user_id: user_id.unwrap_or_default(),
                    entry_id,
                    instructor_id: body.instructor_id,
                    instructor_name: result.instructor_name,
                    role,
                },
            );
        }
    }

    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

/// DELETE /api/academic/timetable/:id/instructors/:uid
pub async fn remove_entry_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((entry_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
        .await
        .ok();

    let result = timetable_service::remove_entry_instructor(&pool, entry_id, instructor_id).await?;

    if let Some(sem_id) = result.semester_id {
        let subdomain =
            extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        state.websocket_manager.broadcast_mutation(
            subdomain,
            sem_id,
            TimetableEvent::EntryInstructorRemoved {
                user_id: user_id.unwrap_or_default(),
                entry_id,
                instructor_id,
                entry_deleted: result.entry_deleted,
            },
        );
    }

    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

/// POST /api/academic/timetable/slots/:slot_id/instructors/:uid/restore
pub async fn restore_instructor_to_slot_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((slot_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let inserted =
        timetable_service::restore_instructor_to_slot(&pool, slot_id, instructor_id).await?;
    Ok(Json(json!({ "success": true, "data": { "inserted": inserted } })).into_response())
}

/// DELETE /api/academic/timetable/slots/:slot_id/instructors/:uid
pub async fn hide_instructor_from_slot_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((slot_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let (deleted, semester_id) =
        timetable_service::hide_instructor_from_slot(&pool, slot_id, instructor_id).await?;

    if let Some(sid) = semester_id {
        let subdomain =
            extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
            .await
            .ok();
        state.websocket_manager.broadcast_mutation(
            subdomain,
            sid,
            TimetableEvent::TableRefresh {
                user_id: user_id.unwrap_or_default(),
            },
        );
    }

    Ok(Json(json!({ "success": true, "data": { "deleted": deleted } })).into_response())
}

/// DELETE /api/academic/timetable/slots/:slot_id/instructors/:uid/period
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
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let (deleted, semester_id) = timetable_service::hide_instructor_from_slot_period(
        &pool,
        slot_id,
        instructor_id,
        &q.day_of_week,
        q.period_id,
    )
    .await?;

    if let Some(sid) = semester_id {
        let subdomain =
            extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
        let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
            .await
            .ok();
        state.websocket_manager.broadcast_mutation(
            subdomain,
            sid,
            TimetableEvent::TableRefresh {
                user_id: user_id.unwrap_or_default(),
            },
        );
    }

    Ok(Json(json!({ "success": true, "data": { "deleted": deleted } })).into_response())
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

    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
        .await
        .ok()
        .unwrap_or_default();
    let subdomain =
        extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());

    let outcome = timetable_service::swap_entries(&pool, body).await?;

    match outcome {
        timetable_service::SwapOutcome::Conflict(info) => {
            state.websocket_manager.broadcast_ephemeral(
                subdomain,
                info.semester_id,
                TimetableEvent::DropRejected {
                    user_id,
                    entry_id: info.a_id,
                    original_day: info.a_day,
                    original_period_id: info.a_period,
                    original_room_id: info.a_room,
                    partner_id: Some(info.b_id),
                    partner_original_day: Some(info.b_day),
                    partner_original_period_id: Some(info.b_period),
                    reason: info.reason.clone(),
                },
            );
            Err(AppError::BadRequest(info.reason))
        }
        timetable_service::SwapOutcome::Swapped { semester_id } => {
            state.websocket_manager.broadcast_mutation(
                subdomain,
                semester_id,
                TimetableEvent::TableRefresh { user_id },
            );
            Ok(Json(json!({ "success": true, "data": {}, "message": "Swapped" })).into_response())
        }
    }
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

    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_READ_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }

    let cells = timetable_service::validate_moves(&pool, body).await?;
    Ok(Json(json!({ "success": true, "data": cells })).into_response())
}

#[derive(serde::Deserialize)]
pub struct OccupancyQuery {
    pub semester_id: Uuid,
}

/// GET /api/academic/timetable/occupancy?semester_id=X
pub async fn get_timetable_occupancy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<OccupancyQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_READ_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }

    let rows = timetable_service::get_occupancy(&pool, q.semester_id).await?;
    Ok(Json(json!({ "success": true, "data": rows })).into_response())
}
