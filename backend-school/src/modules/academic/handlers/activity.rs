use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
    response::IntoResponse,
};
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
// Activity Groups CRUD
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
            sem.name AS semester_name
        FROM activity_groups ag
        LEFT JOIN staff_info si ON si.id = ag.instructor_id
        LEFT JOIN users u ON u.id = si.user_id
        LEFT JOIN activity_group_members agm ON agm.activity_group_id = ag.id
        LEFT JOIN academic_semesters sem ON sem.id = ag.semester_id
        WHERE ag.is_active = true"#,
    );

    if let Some(semester_id) = filter.semester_id {
        sql.push_str(&format!(" AND ag.semester_id = '{semester_id}'"));
    }
    if let Some(ref activity_type) = filter.activity_type {
        sql.push_str(&format!(" AND ag.activity_type = '{activity_type}'"));
    }
    if let Some(instructor_id) = filter.instructor_id {
        sql.push_str(&format!(" AND ag.instructor_id = '{instructor_id}'"));
    }
    if let Some(open) = filter.registration_open {
        sql.push_str(&format!(" AND ag.registration_open = {open}"));
    }
    if let Some(ref search) = filter.search {
        let escaped = search.replace('\'', "''");
        sql.push_str(&format!(" AND ag.name ILIKE '%{escaped}%'"));
    }

    sql.push_str(" GROUP BY ag.id, u.first_name, u.last_name, sem.name ORDER BY ag.activity_type, ag.name");

    let groups: Vec<ActivityGroup> = sqlx::query_as(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ list_activity_groups error: {e}");
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

    // ต้องมี MANAGE_ALL หรือ MANAGE_OWN (ครูสร้างชุมนุมตัวเอง)
    let has_manage_all = check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_ALL, &state.permission_cache).await.is_ok();
    let has_manage_own = check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_OWN, &state.permission_cache).await.is_ok();
    if !has_manage_all && !has_manage_own {
        return Ok(axum::http::Response::builder()
            .status(403)
            .body(axum::body::Body::from(json!({"error": "ไม่มีสิทธิ์"}).to_string()))
            .unwrap());
    }

    let registration_type = body.registration_type.unwrap_or_else(|| "assigned".to_string());
    let registration_open = body.registration_open.unwrap_or(false);
    let allowed = body.allowed_grade_level_ids
        .map(|ids| serde_json::to_value(ids).unwrap_or(serde_json::Value::Null));

    let row: ActivityGroup = sqlx::query_as(
        r#"INSERT INTO activity_groups
            (name, description, activity_type, semester_id, instructor_id,
             registration_type, max_capacity, registration_open, allowed_grade_level_ids)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           RETURNING *, NULL::TEXT AS instructor_name, NULL::BIGINT AS member_count, NULL::TEXT AS semester_name"#,
    )
    .bind(&body.name)
    .bind(&body.description)
    .bind(&body.activity_type)
    .bind(body.semester_id)
    .bind(body.instructor_id)
    .bind(&registration_type)
    .bind(body.max_capacity)
    .bind(registration_open)
    .bind(&allowed)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ create_activity_group error: {e}");
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
        // Allow manage_own if this instructor owns the group
        if check_permission(&headers, &pool, codes::ACTIVITY_MANAGE_OWN, &state.permission_cache).await.is_err() {
            return Ok(r);
        }
    }

    // Build SET clause dynamically
    let mut parts: Vec<String> = vec!["updated_at = NOW()".to_string()];
    let mut idx = 1i32;

    if body.name.is_some()              { parts.push(format!("name = ${idx}")); idx += 1; }
    if body.description.is_some()       { parts.push(format!("description = ${idx}")); idx += 1; }
    if body.activity_type.is_some()     { parts.push(format!("activity_type = ${idx}")); idx += 1; }
    if body.instructor_id.is_some()     { parts.push(format!("instructor_id = ${idx}")); idx += 1; }
    if body.registration_type.is_some() { parts.push(format!("registration_type = ${idx}")); idx += 1; }
    if body.max_capacity.is_some()      { parts.push(format!("max_capacity = ${idx}")); idx += 1; }
    if body.registration_open.is_some() { parts.push(format!("registration_open = ${idx}")); idx += 1; }
    if body.allowed_grade_level_ids.is_some() { parts.push(format!("allowed_grade_level_ids = ${idx}")); idx += 1; }
    if body.is_active.is_some()         { parts.push(format!("is_active = ${idx}")); idx += 1; }

    if parts.len() == 1 {
        return Ok(Json(json!({ "message": "ไม่มีข้อมูลที่ต้องอัปเดต" })).into_response());
    }

    let id_idx = idx;
    let sql = format!(
        "UPDATE activity_groups SET {} WHERE id = ${} RETURNING *, NULL::TEXT AS instructor_name, NULL::BIGINT AS member_count, NULL::TEXT AS semester_name",
        parts.join(", "),
        id_idx
    );

    let mut q = sqlx::query_as::<_, ActivityGroup>(&sql);
    if let Some(ref v) = body.name               { q = q.bind(v); }
    if let Some(ref v) = body.description        { q = q.bind(v); }
    if let Some(ref v) = body.activity_type      { q = q.bind(v); }
    if let Some(v)     = body.instructor_id       { q = q.bind(v); }
    if let Some(ref v) = body.registration_type  { q = q.bind(v); }
    if let Some(v)     = body.max_capacity        { q = q.bind(v); }
    if let Some(v)     = body.registration_open   { q = q.bind(v); }
    if let Some(ref v) = body.allowed_grade_level_ids {
        q = q.bind(serde_json::to_value(v).unwrap_or(serde_json::Value::Null));
    }
    if let Some(v) = body.is_active               { q = q.bind(v); }
    q = q.bind(id);

    let row: ActivityGroup = q
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ update_activity_group error: {e}");
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
            eprintln!("❌ delete_activity_group error: {e}");
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
        eprintln!("❌ list_members error: {e}");
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(Json(json!({ "data": members })).into_response())
}

/// POST /api/academic/activities/:id/members  — ครู/admin เพิ่มสมาชิก (assigned)
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

    // Check capacity
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

/// POST /api/academic/activities/:id/enroll  — นักเรียน self-enroll (self)
pub async fn self_enroll(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    // ดึง student_id จาก session/token
    // สำหรับตอนนี้ใช้ permission check ว่า login แล้ว
    // TODO: ดึง student_id จาก JWT claims เมื่อ student portal พร้อม

    // Check group is open for self registration
    let row: Option<(bool, Option<i32>, String)> = sqlx::query_as(
        "SELECT registration_open, max_capacity, registration_type FROM activity_groups WHERE id = $1 AND is_active = true",
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

    // Check capacity
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

    Ok(Json(json!({ "message": "TODO: ต้องการ student_id จาก JWT" })).into_response())
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

/// PUT /api/academic/activities/members/:member_id/result  — บันทึกผล ผ/มผ
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
    pub role: Option<String>, // "primary" | "assistant"
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
           JOIN staff_info si ON si.id = agi.instructor_id
           JOIN users u ON u.id = si.user_id
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
