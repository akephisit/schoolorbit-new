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
use crate::modules::academic::websockets::TimetableEvent;

/// Broadcast: ทีมครูของ course เปลี่ยน → client re-fetch entries ของ course นั้น
async fn broadcast_course_refresh(
    state: &AppState,
    headers: &HeaderMap,
    pool: &sqlx::PgPool,
    course_id: Uuid,
) {
    let semester_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT academic_semester_id FROM classroom_courses WHERE id = $1"
    ).bind(course_id).fetch_optional(pool).await.ok().flatten();
    if let Some(sem_id) = semester_id {
        let user_id = crate::middleware::auth::extract_user_id(headers, pool).await.ok();
        let subdomain = extract_subdomain_from_request(headers).unwrap_or_else(|_| "default".to_string());
        state.websocket_manager.broadcast_mutation(
            subdomain,
            sem_id,
            TimetableEvent::CourseTeamChanged { user_id: user_id.unwrap_or_default(), course_id },
        );
    }
}

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
        // ใช้ junction (รองรับทั้ง primary + secondary) ไม่ filter แค่ primary
        sql.push_str(&format!(
            " AND EXISTS (SELECT 1 FROM classroom_course_instructors cci \
               WHERE cci.classroom_course_id = cc.id AND cci.instructor_id = ${idx})"
        ));
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

    // Single round-trip for any N subjects:
    //   CTE 1: insert classroom_courses with primary resolved from subject_default_instructors
    //          (fallback to legacy subjects.default_instructor_id). The cc_sync_junction trigger
    //          (migration 078) auto-inserts the primary into classroom_course_instructors.
    //   CTE 2: copy secondary defaults from subject_default_instructors into the junction.
    //   SELECT: return the number of newly-inserted courses.
    let added_count: i64 = sqlx::query_scalar(
        r#"
        WITH inserted AS (
            INSERT INTO classroom_courses (
                classroom_id, academic_semester_id, subject_id, primary_instructor_id
            )
            SELECT $1, $2, s.id,
                COALESCE(
                    (SELECT sdi.instructor_id FROM subject_default_instructors sdi
                     WHERE sdi.subject_id = s.id AND sdi.role = 'primary' LIMIT 1),
                    s.default_instructor_id
                )
            FROM subjects s
            WHERE s.id = ANY($3)
            ON CONFLICT (classroom_id, academic_semester_id, subject_id) DO NOTHING
            RETURNING id, subject_id
        ),
        sec_copy AS (
            INSERT INTO classroom_course_instructors (classroom_course_id, instructor_id, role)
            SELECT i.id, sdi.instructor_id, sdi.role
            FROM inserted i
            JOIN subject_default_instructors sdi
              ON sdi.subject_id = i.subject_id AND sdi.role = 'secondary'
            ON CONFLICT (classroom_course_id, instructor_id) DO NOTHING
            RETURNING 1
        )
        SELECT COUNT(*) FROM inserted
        "#
    )
    .bind(payload.classroom_id)
    .bind(payload.academic_semester_id)
    .bind(&payload.subject_ids)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("assign_courses failed: {}", e);
        AppError::InternalServerError("Failed to assign courses".to_string())
    })?;

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

    if let Err(e) = result {
        eprintln!("Update error: {}", e);
        return Err(AppError::InternalServerError("Failed to update course".to_string()));
    }

    // Trigger cc_sync_junction (migration 078) upserts the junction and demotes other primaries
    // automatically when classroom_courses.primary_instructor_id changes.

    Ok(Json(json!({ "success": true })).into_response())
}

#[derive(Debug, serde::Deserialize)]
pub struct BatchListCourseInstructorsQuery {
    /// Comma-separated UUIDs
    pub course_ids: String,
}

/// GET /api/academic/planning/courses/instructors?course_ids=uuid1,uuid2,...
/// Returns instructors for multiple courses in one query. Output is an object keyed by course_id.
pub async fn batch_list_course_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Query(query): axum::extract::Query<BatchListCourseInstructorsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let ids: Vec<Uuid> = query.course_ids.split(',')
        .filter_map(|s| s.trim().parse::<Uuid>().ok())
        .collect();

    if ids.is_empty() {
        return Ok(Json(json!({ "data": {} })).into_response());
    }

    let rows: Vec<CourseInstructor> = sqlx::query_as(
        r#"SELECT cci.*, concat(u.first_name, ' ', u.last_name) AS instructor_name
           FROM classroom_course_instructors cci
           JOIN users u ON u.id = cci.instructor_id
           WHERE cci.classroom_course_id = ANY($1)
           ORDER BY cci.classroom_course_id, cci.role, cci.created_at"#
    )
    .bind(&ids)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Group by classroom_course_id
    let mut grouped: std::collections::HashMap<Uuid, Vec<CourseInstructor>> = std::collections::HashMap::new();
    for row in rows {
        grouped.entry(row.classroom_course_id).or_default().push(row);
    }

    Ok(Json(json!({ "data": grouped })).into_response())
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

    let mut tx = pool.begin().await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Trigger cci_sync_primary (migration 078) demotes any existing primary when a new primary
    // is inserted and refreshes classroom_courses.primary_instructor_id from the junction.
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

    // Propagate ไปยัง timetable_entry_instructors ของ entries ที่มีอยู่
    // ครูที่เพิ่งเข้าทีม → ได้เข้า tei เฉพาะคาบที่ยังว่าง (ไม่ชนกับคาบที่สอนอยู่แล้วที่อื่น)
    // คาบที่ชน → ยังเป็น ghost (ครูอยู่ในทีมของ course แต่ไม่อยู่ใน tei) ให้ครูมาเลือกเองว่า
    // จะสลับมาสอนคาบนี้ไหม
    sqlx::query(
        r#"INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
           SELECT te.id, $2, $3
           FROM academic_timetable_entries te
           WHERE te.classroom_course_id = $1
             AND NOT EXISTS (
                 SELECT 1 FROM academic_timetable_entries te2
                 JOIN timetable_entry_instructors tei2 ON tei2.entry_id = te2.id
                 WHERE tei2.instructor_id = $2
                   AND te2.day_of_week = te.day_of_week
                   AND te2.period_id = te.period_id
                   AND te2.id <> te.id
             )
           ON CONFLICT (entry_id, instructor_id) DO UPDATE SET role = EXCLUDED.role"#
    )
    .bind(course_id)
    .bind(body.instructor_id)
    .bind(&role)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit().await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    broadcast_course_refresh(&state, &headers, &pool, course_id).await;

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
    let mut tx = pool.begin().await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Trigger cci_sync_primary (migration 078) refreshes classroom_courses.primary_instructor_id
    // from the remaining junction rows after delete.
    sqlx::query("DELETE FROM classroom_course_instructors WHERE classroom_course_id = $1 AND instructor_id = $2")
        .bind(course_id).bind(instructor_id).execute(&mut *tx).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Propagate: ลบครูออกจาก timetable_entry_instructors ของ entries ทั้งหมดของ course นี้
    sqlx::query(
        "DELETE FROM timetable_entry_instructors tei
         USING academic_timetable_entries te
         WHERE tei.entry_id = te.id
           AND te.classroom_course_id = $1
           AND tei.instructor_id = $2"
    )
    .bind(course_id).bind(instructor_id).execute(&mut *tx).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit().await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    broadcast_course_refresh(&state, &headers, &pool, course_id).await;

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
    let mut tx = pool.begin().await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Trigger cci_sync_primary (migration 078) demotes other primaries and refreshes
    // classroom_courses.primary_instructor_id when role changes to/from 'primary'.
    sqlx::query(
        "UPDATE classroom_course_instructors SET role = $3
         WHERE classroom_course_id = $1 AND instructor_id = $2"
    ).bind(course_id).bind(instructor_id).bind(&body.role).execute(&mut *tx).await
      .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Propagate role change → timetable_entry_instructors ของ entries ทั้งหมด
    sqlx::query(
        "UPDATE timetable_entry_instructors SET role = $3
         FROM academic_timetable_entries te
         WHERE timetable_entry_instructors.entry_id = te.id
           AND te.classroom_course_id = $1
           AND timetable_entry_instructors.instructor_id = $2"
    ).bind(course_id).bind(instructor_id).bind(&body.role).execute(&mut *tx).await
      .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit().await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    broadcast_course_refresh(&state, &headers, &pool, course_id).await;

    Ok(Json(json!({ "success": true })).into_response())
}

// ==========================================
// Classroom Activities (กิจกรรมพัฒนาผู้เรียน ต่อห้อง)
// อ่าน/ลบ การเข้าร่วม slot ผ่าน junction activity_slot_classrooms
// ==========================================

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct ClassroomActivity {
    pub slot_id: Uuid,
    pub activity_catalog_id: Uuid,
    pub name: String,
    pub activity_type: String,
    pub periods_per_week: i32,
    pub scheduling_mode: String,
    pub is_active: bool,
}

#[derive(serde::Deserialize)]
pub struct ClassroomActivityQuery {
    pub semester_id: Uuid,
}

/// GET /api/academic/planning/classrooms/:classroom_id/activities?semester_id=...
pub async fn list_classroom_activities(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(classroom_id): Path<Uuid>,
    Query(query): Query<ClassroomActivityQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let rows: Vec<ClassroomActivity> = sqlx::query_as(
        r#"SELECT s.id AS slot_id,
                  s.activity_catalog_id,
                  ac.name,
                  ac.activity_type,
                  ac.periods_per_week,
                  ac.scheduling_mode,
                  s.is_active
           FROM activity_slot_classrooms asc_row
           JOIN activity_slots s ON s.id = asc_row.slot_id
           JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
           WHERE asc_row.classroom_id = $1
             AND s.semester_id = $2
           ORDER BY ac.activity_type, ac.name"#,
    )
    .bind(classroom_id)
    .bind(query.semester_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("list_classroom_activities error: {e}");
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(Json(json!({ "data": rows })).into_response())
}

/// DELETE /api/academic/planning/classrooms/:classroom_id/activities/:slot_id
/// ลบห้องออกจาก slot — ถ้าเป็นห้องสุดท้าย trigger จะลบ slot ต่อ
pub async fn remove_classroom_from_slot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((classroom_id, slot_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    sqlx::query(
        "DELETE FROM activity_slot_classrooms WHERE classroom_id = $1 AND slot_id = $2"
    )
    .bind(classroom_id)
    .bind(slot_id)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "success": true })).into_response())
}

