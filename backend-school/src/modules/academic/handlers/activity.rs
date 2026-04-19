use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
    response::IntoResponse,
};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;
use crate::db::school_mapping::get_school_database_url;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::middleware::permission::check_permission;
use crate::permissions::registry::codes;
use crate::modules::academic::models::activity::*;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

// ============================================
// Activity Slots CRUD (ช่องกิจกรรม — Admin)
// ============================================

/// GET /api/academic/activity-slots
pub async fn list_activity_slots(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<ActivitySlotFilter>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let mut sql = String::from(
        r#"SELECT
            s.*,
            ac.name AS name,
            ac.description AS description,
            ac.activity_type AS activity_type,
            ac.periods_per_week AS periods_per_week,
            ac.scheduling_mode AS scheduling_mode,
            ac.grade_level_ids AS allowed_grade_level_ids,
            sem.name AS semester_name,
            COUNT(DISTINCT ag.id) AS group_count,
            COUNT(DISTINCT agm.id) AS total_members
        FROM activity_slots s
        JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
        LEFT JOIN academic_semesters sem ON sem.id = s.semester_id
        LEFT JOIN activity_groups ag ON ag.slot_id = s.id AND ag.is_active = true
        LEFT JOIN activity_group_members agm ON agm.activity_group_id = ag.id
        WHERE s.is_active = true"#,
    );

    let mut idx = 0u32;

    if let Some(semester_id) = filter.semester_id {
        idx += 1;
        sql.push_str(&format!(" AND s.semester_id = ${idx}"));
    }
    if let Some(ref _activity_type) = filter.activity_type {
        idx += 1;
        sql.push_str(&format!(" AND ac.activity_type = ${idx}"));
    }
    if let Some(_open) = filter.teacher_reg_open {
        idx += 1;
        sql.push_str(&format!(" AND s.teacher_reg_open = ${idx}"));
    }
    if let Some(_open) = filter.student_reg_open {
        idx += 1;
        sql.push_str(&format!(" AND s.student_reg_open = ${idx}"));
    }

    sql.push_str(" GROUP BY s.id, ac.id, sem.name ORDER BY ac.activity_type, ac.name");

    let mut q = sqlx::query_as::<_, ActivitySlot>(&sql);
    if let Some(semester_id) = filter.semester_id { q = q.bind(semester_id); }
    if let Some(ref activity_type) = filter.activity_type { q = q.bind(activity_type); }
    if let Some(open) = filter.teacher_reg_open { q = q.bind(open); }
    if let Some(open) = filter.student_reg_open { q = q.bind(open); }

    let slots: Vec<ActivitySlot> = q
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("list_activity_slots error: {e}");
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;

    Ok(Json(json!({ "data": slots })).into_response())
}

/// PUT /api/academic/activity-slots/:id
/// Only semester-specific fields are editable here. Template fields
/// (name/activity_type/periods/mode/grade) live in activity_catalog.
pub async fn update_activity_slot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateActivitySlotRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let row: ActivitySlot = sqlx::query_as(
        r#"WITH upd AS (
            UPDATE activity_slots SET
                registration_type = COALESCE($2, registration_type),
                teacher_reg_open = COALESCE($3, teacher_reg_open),
                student_reg_open = COALESCE($4, student_reg_open),
                student_reg_start = COALESCE($5, student_reg_start),
                student_reg_end = COALESCE($6, student_reg_end),
                is_active = COALESCE($7, is_active),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
        )
        SELECT upd.*,
            ac.name AS name,
            ac.description AS description,
            ac.activity_type AS activity_type,
            ac.periods_per_week AS periods_per_week,
            ac.scheduling_mode AS scheduling_mode,
            ac.grade_level_ids AS allowed_grade_level_ids,
            NULL::TEXT AS semester_name,
            NULL::BIGINT AS group_count,
            NULL::BIGINT AS total_members
        FROM upd
        JOIN activity_catalog ac ON ac.id = upd.activity_catalog_id"#,
    )
    .bind(id)
    .bind(&body.registration_type)
    .bind(body.teacher_reg_open)
    .bind(body.student_reg_open)
    .bind(body.student_reg_start.as_ref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc)))
    .bind(body.student_reg_end.as_ref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc)))
    .bind(body.is_active)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("update_activity_slot error: {e}");
        AppError::NotFound("ไม่พบช่องกิจกรรม".to_string())
    })?;

    Ok(Json(json!({ "data": row })).into_response())
}

/// DELETE /api/academic/activity-slots/:id
pub async fn delete_activity_slot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    sqlx::query("DELETE FROM activity_slots WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| {
            eprintln!("delete_activity_slot error: {e}");
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;

    Ok(Json(json!({ "message": "ลบช่องกิจกรรมแล้ว" })).into_response())
}

// ============================================
// Activity Groups CRUD (ชุมนุม/กิจกรรมจริง)
// ============================================

/// GET /api/academic/activities
pub async fn list_activity_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<ActivityGroupFilter>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let mut sql = String::from(
        r#"SELECT
            ag.*,
            u.first_name || ' ' || u.last_name AS instructor_name,
            COUNT(agm.id) AS member_count,
            ac.name AS slot_name,
            ac.activity_type,
            sem.name AS semester_name
        FROM activity_groups ag
        LEFT JOIN users u ON u.id = ag.instructor_id
        LEFT JOIN activity_group_members agm ON agm.activity_group_id = ag.id
        LEFT JOIN activity_slots s ON s.id = ag.slot_id
        LEFT JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
        LEFT JOIN academic_semesters sem ON sem.id = s.semester_id
        WHERE ag.is_active = true"#,
    );

    let mut idx = 0u32;

    if let Some(_slot_id) = filter.slot_id {
        idx += 1;
        sql.push_str(&format!(" AND ag.slot_id = ${idx}"));
    }
    if let Some(_semester_id) = filter.semester_id {
        idx += 1;
        sql.push_str(&format!(" AND s.semester_id = ${idx}"));
    }
    if let Some(ref _activity_type) = filter.activity_type {
        idx += 1;
        sql.push_str(&format!(" AND ac.activity_type = ${idx}"));
    }
    if let Some(_instructor_id) = filter.instructor_id {
        idx += 1;
        sql.push_str(&format!(" AND ag.instructor_id = ${idx}"));
    }
    if let Some(_open) = filter.registration_open {
        idx += 1;
        sql.push_str(&format!(" AND ag.registration_open = ${idx}"));
    }
    if let Some(ref search) = filter.search {
        if !search.is_empty() {
            idx += 1;
            sql.push_str(&format!(" AND ag.name ILIKE ${idx}"));
        }
    }

    sql.push_str(" GROUP BY ag.id, u.first_name, u.last_name, ac.name, ac.activity_type, sem.name ORDER BY ac.activity_type, ag.name");

    let mut q = sqlx::query_as::<_, ActivityGroup>(&sql);
    if let Some(slot_id) = filter.slot_id { q = q.bind(slot_id); }
    if let Some(semester_id) = filter.semester_id { q = q.bind(semester_id); }
    if let Some(ref activity_type) = filter.activity_type { q = q.bind(activity_type); }
    if let Some(instructor_id) = filter.instructor_id { q = q.bind(instructor_id); }
    if let Some(open) = filter.registration_open { q = q.bind(open); }
    if let Some(ref search) = filter.search {
        if !search.is_empty() { q = q.bind(format!("%{search}%")); }
    }

    let groups: Vec<ActivityGroup> = q
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("list_activity_groups error: {e}");
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;

    Ok(Json(json!({ "data": groups })).into_response())
}

/// POST /api/academic/activities
pub async fn create_activity_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateActivityGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let has_manage_all = check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_ALL, &state.permission_cache).await.is_ok();
    let has_manage_own = check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_OWN, &state.permission_cache).await.is_ok();
    if !has_manage_all && !has_manage_own {
        return Err(AppError::Forbidden("ไม่มีสิทธิ์".to_string()));
    }

    // ตรวจว่า slot เปิดให้ครูลงทะเบียนอยู่
    let slot_open: Option<(bool,)> = sqlx::query_as(
        "SELECT teacher_reg_open FROM activity_slots WHERE id = $1 AND is_active = true"
    )
    .bind(body.slot_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    match slot_open {
        None => return Err(AppError::NotFound("ไม่พบช่องกิจกรรม".to_string())),
        Some((false,)) if !has_manage_all => {
            return Ok(Json(json!({ "error": "ช่องกิจกรรมนี้ยังไม่เปิดให้ลงทะเบียน" })).into_response());
        }
        _ => {}
    }

    // ตรวจว่าครูอยู่ใน slot (ยกเว้น admin)
    if let Some(instructor_id) = body.instructor_id {
        if !has_manage_all {
            let in_slot: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM activity_slot_instructors WHERE slot_id = $1 AND user_id = $2)"
            )
            .bind(body.slot_id)
            .bind(instructor_id)
            .fetch_one(&pool)
            .await
            .unwrap_or(false);

            if !in_slot {
                return Ok(Json(json!({ "error": "ครูคนนี้ไม่ได้อยู่ในรายชื่อครูของช่องกิจกรรมนี้" })).into_response());
            }
        }
    }

    let allowed = body.allowed_grade_level_ids
        .map(|ids| serde_json::to_value(ids).unwrap_or(serde_json::Value::Null));

    let row: ActivityGroup = sqlx::query_as(
        r#"INSERT INTO activity_groups
            (slot_id, name, description, instructor_id, max_capacity, allowed_grade_level_ids)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING *, NULL::TEXT AS instructor_name, NULL::BIGINT AS member_count,
                     NULL::TEXT AS slot_name, NULL::TEXT AS activity_type, NULL::TEXT AS semester_name"#,
    )
    .bind(body.slot_id)
    .bind(&body.name)
    .bind(&body.description)
    .bind(body.instructor_id)
    .bind(body.max_capacity)
    .bind(&allowed)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("create_activity_group error: {e}");
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(Json(json!({ "data": row })).into_response())
}

/// PUT /api/academic/activities/:id
pub async fn update_activity_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateActivityGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_ALL, &state.permission_cache).await {
        if check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_OWN, &state.permission_cache).await.is_err() {
            return Ok(r);
        }
    }

    let allowed = body.allowed_grade_level_ids.as_ref()
        .map(|ids| serde_json::to_value(ids).unwrap_or(serde_json::Value::Null));

    let row: ActivityGroup = sqlx::query_as(
        r#"UPDATE activity_groups SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            instructor_id = COALESCE($4, instructor_id),
            max_capacity = COALESCE($5, max_capacity),
            registration_open = COALESCE($6, registration_open),
            is_active = COALESCE($7, is_active),
            allowed_grade_level_ids = COALESCE($8, allowed_grade_level_ids),
            updated_at = NOW()
        WHERE id = $1
        RETURNING *, NULL::TEXT AS instructor_name, NULL::BIGINT AS member_count,
                  NULL::TEXT AS slot_name, NULL::TEXT AS activity_type, NULL::TEXT AS semester_name"#,
    )
    .bind(id)
    .bind(&body.name)
    .bind(&body.description)
    .bind(body.instructor_id)
    .bind(body.max_capacity)
    .bind(body.registration_open)
    .bind(body.is_active)
    .bind(&allowed)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("update_activity_group error: {e}");
        AppError::NotFound("ไม่พบกลุ่มกิจกรรม".to_string())
    })?;

    Ok(Json(json!({ "data": row })).into_response())
}

/// DELETE /api/academic/activities/:id
pub async fn delete_activity_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    sqlx::query("DELETE FROM activity_groups WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| {
            eprintln!("delete_activity_group error: {e}");
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;

    Ok(Json(json!({ "message": "ลบกลุ่มกิจกรรมแล้ว" })).into_response())
}

// ============================================
// Members API
// ============================================

/// GET /api/academic/activities/:id/members
pub async fn list_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let members: Vec<ActivityGroupMember> = sqlx::query_as(
        r#"SELECT
            agm.*,
            u.first_name || ' ' || u.last_name AS student_name,
            st.student_id AS student_code,
            cr.name AS classroom_name,
            CASE gl.level_type
                WHEN 'kindergarten' THEN 'อ.' || gl.year
                WHEN 'primary'      THEN 'ป.' || gl.year
                WHEN 'secondary'    THEN 'ม.' || gl.year
                ELSE gl.level_type || gl.year::TEXT
            END AS grade_level_name
        FROM activity_group_members agm
        JOIN student_info st ON st.id = agm.student_id
        JOIN users u ON u.id = st.user_id
        LEFT JOIN student_class_enrollments se ON se.student_id = st.user_id AND se.status = 'active'
        LEFT JOIN class_rooms cr ON cr.id = se.class_room_id
        LEFT JOIN grade_levels gl ON gl.id = cr.grade_level_id
        WHERE agm.activity_group_id = $1
        ORDER BY cr.name, u.first_name"#,
    )
    .bind(group_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("list_members error: {e}");
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(Json(json!({ "data": members })).into_response())
}

/// POST /api/academic/activities/:id/members
pub async fn add_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
    Json(body): Json<AddMembersRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_MEMBERS_MANAGE, &state.permission_cache).await {
        return Ok(r);
    }

    let (current_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM activity_group_members WHERE activity_group_id = $1",
    )
    .bind(group_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let (max_cap,): (Option<i32>,) = sqlx::query_as(
        "SELECT max_capacity FROM activity_groups WHERE id = $1",
    )
    .bind(group_id)
    .fetch_one(&pool)
    .await
    .map_err(|_| AppError::NotFound("ไม่พบกลุ่มกิจกรรม".to_string()))?;

    if let Some(cap) = max_cap {
        if current_count + body.student_ids.len() as i64 > cap as i64 {
            return Ok(Json(json!({
                "error": format!("จำนวนเกินที่รับได้ ({cap} คน)")
            }))
            .into_response());
        }
    }

    let mut inserted = 0usize;
    for student_id in &body.student_ids {
        let result = sqlx::query(
            "INSERT INTO activity_group_members (activity_group_id, student_id)
             VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(group_id)
        .bind(student_id)
        .execute(&pool)
        .await;

        if let Ok(r) = result {
            inserted += r.rows_affected() as usize;
        }
    }

    Ok(Json(json!({ "inserted": inserted })).into_response())
}

/// GET /api/academic/activities/my-enrollments — ดึง group_ids ที่นักเรียนลงทะเบียน
pub async fn my_enrollments(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await
        .map_err(|e| AppError::AuthError(e))?;

    let student_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM student_info WHERE user_id = $1"
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let sid = match student_id {
        Some(id) => id,
        None => return Ok(Json(json!({ "data": [] })).into_response()),
    };

    let group_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT activity_group_id FROM activity_group_members WHERE student_id = $1"
    )
    .bind(sid)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "data": group_ids })).into_response())
}

/// POST /api/academic/activities/:id/enroll  — นักเรียน self-enroll
pub async fn self_enroll(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    // Check slot is open for student registration
    let row: Option<(bool, Option<i32>, String)> = sqlx::query_as(
        r#"SELECT s.student_reg_open, ag.max_capacity, s.registration_type
           FROM activity_groups ag
           JOIN activity_slots s ON s.id = ag.slot_id
           WHERE ag.id = $1 AND ag.is_active = true"#,
    )
    .bind(group_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let (open, cap, reg_type) = row.ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มกิจกรรม".to_string()))?;

    if reg_type != "self" {
        return Ok(Json(json!({ "error": "กลุ่มนี้ไม่เปิดให้ลงทะเบียนด้วยตนเอง" })).into_response());
    }
    if !open {
        return Ok(Json(json!({ "error": "ยังไม่เปิดรับสมัคร" })).into_response());
    }

    // ดึง user_id จาก JWT
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await
        .map_err(|e| AppError::AuthError(e))?;

    // ดึง student_info.id จาก user_id
    let student_info_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM student_info WHERE user_id = $1"
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let student_id = student_info_id
        .ok_or_else(|| AppError::BadRequest("ไม่พบข้อมูลนักเรียน".to_string()))?;

    // เช็ค capacity
    if let Some(max) = cap {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM activity_group_members WHERE activity_group_id = $1",
        )
        .bind(group_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        if count >= max as i64 {
            return Ok(Json(json!({ "error": "กลุ่มเต็มแล้ว" })).into_response());
        }
    }

    // เช็คชั้นที่รับ (group level → slot level)
    let student_grade: Option<Uuid> = sqlx::query_scalar(
        r#"SELECT cr.grade_level_id FROM student_class_enrollments sce
           JOIN class_rooms cr ON cr.id = sce.class_room_id
           WHERE sce.student_id = $1 AND sce.status = 'active'
           LIMIT 1"#
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    if let Some(grade_id) = student_grade {
        // ตรวจ group allowed_grade_level_ids ก่อน แล้วค่อย catalog (ผ่าน slot)
        let allowed: Option<serde_json::Value> = sqlx::query_scalar(
            r#"SELECT COALESCE(ag.allowed_grade_level_ids, ac.grade_level_ids)
               FROM activity_groups ag
               LEFT JOIN activity_slots s ON s.id = ag.slot_id
               LEFT JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
               WHERE ag.id = $1"#
        )
        .bind(group_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?
        .flatten();

        if let Some(allowed_ids) = allowed {
            if let Some(arr) = allowed_ids.as_array() {
                let grade_str = grade_id.to_string();
                let is_allowed = arr.iter().any(|v| v.as_str() == Some(&grade_str));
                if !is_allowed {
                    return Ok(Json(json!({ "error": "ชั้นเรียนของคุณไม่อยู่ในชั้นที่รับ" })).into_response());
                }
            }
        }
    }

    // ลงทะเบียน
    let result = sqlx::query(
        "INSERT INTO activity_group_members (activity_group_id, student_id, enrolled_by)
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"
    )
    .bind(group_id)
    .bind(student_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    if result.rows_affected() > 0 {
        Ok(Json(json!({ "success": true, "message": "ลงทะเบียนสำเร็จ" })).into_response())
    } else {
        Ok(Json(json!({ "error": "ลงทะเบียนแล้วก่อนหน้านี้" })).into_response())
    }
}

/// DELETE /api/academic/activities/:id/enroll — นักเรียนยกเลิกลงทะเบียน
pub async fn self_unenroll(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await
        .map_err(|e| AppError::AuthError(e))?;

    let student_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM student_info WHERE user_id = $1"
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let sid = student_id.ok_or_else(|| AppError::BadRequest("ไม่พบข้อมูลนักเรียน".to_string()))?;

    sqlx::query("DELETE FROM activity_group_members WHERE activity_group_id = $1 AND student_id = $2")
        .bind(group_id)
        .bind(sid)
        .execute(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "success": true, "message": "ยกเลิกลงทะเบียนแล้ว" })).into_response())
}

/// DELETE /api/academic/activities/:id/members/:student_id
pub async fn remove_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((group_id, student_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_MEMBERS_MANAGE, &state.permission_cache).await {
        return Ok(r);
    }

    sqlx::query(
        "DELETE FROM activity_group_members WHERE activity_group_id = $1 AND student_id = $2",
    )
    .bind(group_id)
    .bind(student_id)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "message": "ลบสมาชิกแล้ว" })).into_response())
}

/// PUT /api/academic/activities/members/:member_id/result
pub async fn update_member_result(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(member_id): Path<Uuid>,
    Json(body): Json<UpdateMemberResultRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_MEMBERS_MANAGE, &state.permission_cache).await {
        return Ok(r);
    }

    if body.result != "pass" && body.result != "fail" {
        return Ok(Json(json!({ "error": "result ต้องเป็น pass หรือ fail" })).into_response());
    }

    sqlx::query("UPDATE activity_group_members SET result = $1 WHERE id = $2")
        .bind(&body.result)
        .bind(member_id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "message": "บันทึกผลแล้ว" })).into_response())
}

// ============================================
// Instructors API
// ============================================

#[derive(serde::Deserialize)]
pub struct InstructorRoleRequest {
    pub instructor_id: uuid::Uuid,
    pub role: Option<String>,
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct InstructorInfo {
    pub id: uuid::Uuid,
    pub instructor_id: uuid::Uuid,
    pub role: String,
    pub instructor_name: Option<String>,
}

/// GET /api/academic/activities/:id/instructors
pub async fn list_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let rows: Vec<InstructorInfo> = sqlx::query_as(
        r#"SELECT agi.id, agi.instructor_id, agi.role,
                  u.first_name || ' ' || u.last_name AS instructor_name
           FROM activity_group_instructors agi
           JOIN users u ON u.id = agi.instructor_id
           WHERE agi.activity_group_id = $1
           ORDER BY CASE agi.role WHEN 'primary' THEN 1 ELSE 2 END"#,
    )
    .bind(group_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "data": rows })).into_response())
}

/// POST /api/academic/activities/:id/instructors
pub async fn add_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
    Json(body): Json<InstructorRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_ALL, &state.permission_cache).await {
        if check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_OWN, &state.permission_cache).await.is_err() {
            return Ok(r);
        }
    }

    let role = body.role.unwrap_or_else(|| "assistant".to_string());

    sqlx::query(
        "INSERT INTO activity_group_instructors (activity_group_id, instructor_id, role)
         VALUES ($1, $2, $3) ON CONFLICT (activity_group_id, instructor_id) DO UPDATE SET role = $3"
    )
    .bind(group_id)
    .bind(body.instructor_id)
    .bind(&role)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "message": "เพิ่มครูแล้ว" })).into_response())
}

/// DELETE /api/academic/activities/:id/instructors/:instructor_id
pub async fn remove_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((group_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_ALL, &state.permission_cache).await {
        if check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_OWN, &state.permission_cache).await.is_err() {
            return Ok(r);
        }
    }

    sqlx::query(
        "DELETE FROM activity_group_instructors WHERE activity_group_id = $1 AND instructor_id = $2"
    )
    .bind(group_id)
    .bind(instructor_id)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "message": "ลบครูแล้ว" })).into_response())
}

// ============================================
// Slot Instructors API (ครูใน slot)
// ============================================

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct SlotInstructorInfo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub instructor_name: Option<String>,
}

/// GET /api/academic/activity-slots/:id/instructors
pub async fn list_slot_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let rows: Vec<SlotInstructorInfo> = sqlx::query_as(
        r#"SELECT asi.id, asi.user_id,
                  u.first_name || ' ' || u.last_name AS instructor_name
           FROM activity_slot_instructors asi
           JOIN users u ON u.id = asi.user_id
           WHERE asi.slot_id = $1
           ORDER BY u.first_name"#,
    )
    .bind(slot_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "data": rows })).into_response())
}

/// POST /api/academic/activity-slots/:id/instructors
pub async fn add_slot_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let user_id = body.get("user_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| AppError::BadRequest("user_id required".to_string()))?;

    sqlx::query(
        "INSERT INTO activity_slot_instructors (slot_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING"
    )
    .bind(slot_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "message": "เพิ่มครูแล้ว" })).into_response())
}

/// DELETE /api/academic/activity-slots/:id/instructors/:user_id
pub async fn remove_slot_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((slot_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    sqlx::query("DELETE FROM activity_slot_instructors WHERE slot_id = $1 AND user_id = $2")
        .bind(slot_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "message": "ลบครูแล้ว" })).into_response())
}

/// DELETE /api/academic/activity-slots/:id/timetable-entries
pub async fn delete_slot_timetable_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let result = sqlx::query("DELETE FROM academic_timetable_entries WHERE activity_slot_id = $1")
        .bind(slot_id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "message": "ลบรายการตารางสอนแล้ว", "deleted_count": result.rows_affected() })).into_response())
}

/// DELETE /api/academic/activity-slots/:id/groups
pub async fn delete_all_slot_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let result = sqlx::query("DELETE FROM activity_groups WHERE slot_id = $1")
        .bind(slot_id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "message": "ลบกิจกรรมทั้งหมดแล้ว", "deleted_count": result.rows_affected() })).into_response())
}

/// DELETE /api/academic/activity-slots/:id/instructors/all
pub async fn remove_all_slot_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let result = sqlx::query("DELETE FROM activity_slot_instructors WHERE slot_id = $1")
        .bind(slot_id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "message": "ลบครูทั้งหมดแล้ว", "deleted_count": result.rows_affected() })).into_response())
}

// ============================================
// Slot Classroom Assignments (ครูต่อห้อง — independent)
// ============================================

/// GET /api/academic/activity-slots/:id/classroom-assignments
pub async fn list_slot_classroom_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let rows = sqlx::query_as::<_, SlotClassroomAssignment>(
        r#"SELECT asca.*, cr.name AS classroom_name,
                  concat(u.first_name, ' ', u.last_name) AS instructor_name
           FROM activity_slot_classroom_assignments asca
           JOIN class_rooms cr ON cr.id = asca.classroom_id
           JOIN users u ON u.id = asca.instructor_id
           WHERE asca.slot_id = $1
           ORDER BY cr.name"#,
    )
    .bind(slot_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("list_slot_classroom_assignments error: {e}");
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(Json(json!({ "data": rows })).into_response())
}

/// POST /api/academic/activity-slots/:id/classroom-assignments
pub async fn batch_upsert_slot_classroom_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
    Json(body): Json<BatchUpsertSlotClassroomAssignmentsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    for a in &body.assignments {
        sqlx::query(
            r#"INSERT INTO activity_slot_classroom_assignments (slot_id, classroom_id, instructor_id)
               VALUES ($1, $2, $3)
               ON CONFLICT (slot_id, classroom_id)
               DO UPDATE SET instructor_id = EXCLUDED.instructor_id"#,
        )
        .bind(slot_id)
        .bind(a.classroom_id)
        .bind(a.instructor_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("upsert_slot_classroom_assignment error: {e}");
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;
    }

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "message": "บันทึกสำเร็จ", "count": body.assignments.len() })).into_response())
}

/// DELETE /api/academic/activity-slots/:id/classroom-assignments/all
pub async fn delete_all_slot_classroom_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let result = sqlx::query("DELETE FROM activity_slot_classroom_assignments WHERE slot_id = $1")
        .bind(slot_id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "message": "ลบครูประจำห้องทั้งหมดแล้ว", "deleted_count": result.rows_affected() })).into_response())
}

/// DELETE /api/academic/activity-slots/:id/classroom-assignments/:assignment_id
pub async fn delete_slot_classroom_assignment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((slot_id, assignment_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    sqlx::query("DELETE FROM activity_slot_classroom_assignments WHERE id = $1 AND slot_id = $2")
        .bind(assignment_id)
        .bind(slot_id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "message": "ลบสำเร็จ" })).into_response())
}
