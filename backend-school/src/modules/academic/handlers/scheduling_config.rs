use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPool, Row};
use uuid::Uuid;
use crate::error::AppError;
use crate::AppState;
use crate::db::school_mapping::get_school_database_url;
use crate::utils::subdomain::extract_subdomain_from_request;

// Structs for Request/Response
#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self { success: true, data: Some(data), error: None }
    }
    pub fn error(msg: String) -> Self {
        Self { success: false, data: None, error: Some(msg) }
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub struct InstructorConstraintView {
    pub id: Uuid, // Changed from instructor_id to id
    pub first_name: String,
    pub last_name: String,
    pub hard_unavailable_slots: Option<serde_json::Value>,
    pub max_periods_per_day: Option<i32>,
    pub min_periods_per_day: Option<i32>,
    pub assigned_room_id: Option<Uuid>,
    pub assigned_room_name: Option<String>,
    pub priority: i32,
    pub primary_course_count: i64,
}

#[derive(Deserialize)]
pub struct UpdateInstructorConstraintRequest {
    // instructor_id removed, use Path param
    pub hard_unavailable_slots: Option<serde_json::Value>,
    pub max_periods_per_day: Option<i32>,
    pub preferred_slots: Option<serde_json::Value>,
    pub assigned_room_id: Option<Uuid>,
    pub priority: Option<i32>,
}

#[derive(Deserialize)]
pub struct ReorderInstructorPriorityRequest {
    /// Array of instructor_id ตามลำดับ — index 0 → priority=1, etc.
    pub instructor_ids: Vec<Uuid>,
}

#[derive(Serialize)]
pub struct SchoolSettingsView {
    pub default_max_consecutive: i32,
}

#[derive(Deserialize)]
pub struct UpdateSchoolSettingsRequest {
    pub default_max_consecutive: Option<i32>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct SubjectConstraintView {
    pub id: Uuid, // Changed from subject_id to id
    pub code: String,
    pub name: String,
    pub min_consecutive_periods: i32,
    pub max_consecutive_periods: Option<i32>, 
    pub allow_single_period: Option<bool>,
    pub periods_per_week: Option<i32>,
    pub allowed_period_ids: Option<serde_json::Value>, // JSONB array of period UUIDs
    pub allowed_days: Option<serde_json::Value>,       // JSONB array of days
}

#[derive(Deserialize)]
pub struct UpdateSubjectConstraintRequest {
    // subject_id removed, use Path param
    pub min_consecutive_periods: Option<i32>,
    pub max_consecutive_periods: Option<i32>,
    pub allow_single_period: Option<bool>,
    pub allowed_period_ids: Option<serde_json::Value>, // JSONB array
    pub allowed_days: Option<serde_json::Value>,       // JSONB array
}

// ==========================
// Classroom Course Constraints (Phase B)
// ==========================

#[derive(Serialize, sqlx::FromRow)]
pub struct ClassroomCourseConstraintView {
    pub id: Uuid,                 // classroom_course id
    pub classroom_id: Uuid,
    pub classroom_name: String,
    pub subject_id: Uuid,
    pub subject_code: String,
    pub subject_name: String,
    pub periods_per_week: Option<i32>,
    pub primary_instructor_id: Option<Uuid>,
    pub primary_instructor_name: Option<String>,
    pub consecutive_pattern: Option<serde_json::Value>,
    pub same_day_unique: bool,
    pub hard_unavailable_slots: serde_json::Value,
    /// คาบไม่ว่างของครูใน team (รวม primary + secondary) — readonly ฝั่ง UI
    pub team_unavailable_slots: serde_json::Value,
}

#[derive(Deserialize)]
pub struct UpdateClassroomCourseConstraintRequest {
    pub consecutive_pattern: Option<serde_json::Value>,
    pub same_day_unique: Option<bool>,
    pub hard_unavailable_slots: Option<serde_json::Value>,
}

/// GET /api/academic/scheduling/classroom-courses?instructor_id=...
/// List cc ที่ instructor นั้นเป็น primary — อยู่ในหน้า scheduling-config
pub async fn list_classroom_course_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Query(q): axum::extract::Query<ListCcConstraintsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    // Active academic year
    let year_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_years WHERE is_active = true LIMIT 1"
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?
    .ok_or_else(|| AppError::NotFound("Active academic year not found".to_string()))?;

    // Optional filter: instructor_id (ครูคนนั้นเป็น primary)
    // Effective unavailable = cc + union ของครูทุกคนใน team (จาก instructor_preferences)
    let mut sql = String::from(
        r#"
        WITH team_unavail AS (
            SELECT cci.classroom_course_id,
                   COALESCE(jsonb_agg(elem) FILTER (WHERE elem IS NOT NULL), '[]'::jsonb) AS slots
            FROM classroom_course_instructors cci
            JOIN classroom_courses cc2 ON cc2.id = cci.classroom_course_id
            JOIN academic_semesters sem2 ON sem2.id = cc2.academic_semester_id
            LEFT JOIN instructor_preferences ip2
                ON ip2.instructor_id = cci.instructor_id
                AND ip2.academic_year_id = sem2.academic_year_id
            LEFT JOIN LATERAL jsonb_array_elements(COALESCE(ip2.hard_unavailable_slots, '[]'::jsonb)) elem ON true
            WHERE sem2.academic_year_id = $1
            GROUP BY cci.classroom_course_id
        ),
        primary_instr AS (
            SELECT cci.classroom_course_id, cci.instructor_id
            FROM classroom_course_instructors cci
            WHERE cci.role = 'primary'
        )
        SELECT
            cc.id,
            cc.classroom_id,
            cls.name AS classroom_name,
            cc.subject_id,
            s.code AS subject_code,
            s.name_th AS subject_name,
            s.periods_per_week,
            pi.instructor_id AS primary_instructor_id,
            CASE WHEN u.id IS NOT NULL THEN u.first_name || ' ' || u.last_name ELSE NULL END
                AS primary_instructor_name,
            cc.consecutive_pattern,
            cc.same_day_unique,
            cc.hard_unavailable_slots,
            COALESCE(tu.slots, '[]'::jsonb) AS team_unavailable_slots
        FROM classroom_courses cc
        JOIN class_rooms cls ON cls.id = cc.classroom_id
        JOIN subjects s ON s.id = cc.subject_id
        JOIN academic_semesters sem ON sem.id = cc.academic_semester_id
        LEFT JOIN primary_instr pi ON pi.classroom_course_id = cc.id
        LEFT JOIN users u ON u.id = pi.instructor_id
        LEFT JOIN team_unavail tu ON tu.classroom_course_id = cc.id
        WHERE sem.academic_year_id = $1
        "#,
    );

    if q.instructor_id.is_some() {
        sql.push_str(" AND pi.instructor_id = $2");
    }
    sql.push_str(" ORDER BY cls.name, s.code");

    let rows = if let Some(iid) = q.instructor_id {
        sqlx::query_as::<_, ClassroomCourseConstraintView>(&sql)
            .bind(year_id)
            .bind(iid)
            .fetch_all(&pool)
            .await
    } else {
        sqlx::query_as::<_, ClassroomCourseConstraintView>(&sql)
            .bind(year_id)
            .fetch_all(&pool)
            .await
    }
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(ApiResponse::success(rows)))
}

#[derive(Deserialize)]
pub struct ListCcConstraintsQuery {
    pub instructor_id: Option<Uuid>,
}

/// PUT /api/academic/scheduling/classroom-courses/{id}
pub async fn update_classroom_course_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(cc_id): Path<Uuid>,
    Json(payload): Json<UpdateClassroomCourseConstraintRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    // Validate consecutive_pattern: array of int + sum == periods_per_week ของ subject ที่ผูก
    if let Some(ref pattern) = payload.consecutive_pattern {
        let arr = pattern
            .as_array()
            .ok_or_else(|| AppError::BadRequest("consecutive_pattern ต้องเป็น array".to_string()))?;
        let mut sum: i64 = 0;
        for v in arr {
            let n = v.as_i64().ok_or_else(|| AppError::BadRequest(
                "consecutive_pattern มีค่าที่ไม่ใช่ตัวเลข".to_string()
            ))?;
            if !(1..=20).contains(&n) {
                return Err(AppError::BadRequest(
                    "consecutive_pattern แต่ละค่าต้องอยู่ระหว่าง 1-20".to_string()
                ));
            }
            sum += n;
        }

        // เทียบกับ periods_per_week
        let pw: Option<i32> = sqlx::query_scalar(
            "SELECT s.periods_per_week
             FROM classroom_courses cc JOIN subjects s ON s.id = cc.subject_id
             WHERE cc.id = $1"
        )
        .bind(cc_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?
        .flatten();

        if let Some(ppw) = pw {
            if sum != ppw as i64 {
                return Err(AppError::BadRequest(format!(
                    "ผลรวมของ pattern ({}) ต้องเท่ากับ periods_per_week ของวิชา ({})",
                    sum, ppw
                )));
            }
        }
    }

    sqlx::query(
        r#"UPDATE classroom_courses SET
            consecutive_pattern = COALESCE($2, consecutive_pattern),
            same_day_unique     = COALESCE($3, same_day_unique),
            hard_unavailable_slots = COALESCE($4, hard_unavailable_slots),
            updated_at = NOW()
           WHERE id = $1"#
    )
    .bind(cc_id)
    .bind(payload.consecutive_pattern)
    .bind(payload.same_day_unique)
    .bind(payload.hard_unavailable_slots)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(ApiResponse::success("Updated classroom course constraints".to_string())))
}

// ==========================
// Phase D: Classroom Course Preferred Rooms
// ==========================

#[derive(Serialize, sqlx::FromRow)]
pub struct CcPreferredRoomView {
    pub id: Uuid,
    pub classroom_course_id: Uuid,
    pub room_id: Uuid,
    pub room_code: String,
    pub room_name: String,
    pub rank: i32,
    pub is_required: bool,
}

#[derive(Deserialize)]
pub struct SetCcRoomsRequest {
    /// ทั้ง list ของห้อง — replace ของเดิม
    pub rooms: Vec<CcRoomItem>,
}

#[derive(Deserialize)]
pub struct CcRoomItem {
    pub room_id: Uuid,
    pub rank: i32,
    pub is_required: Option<bool>,
}

/// GET /api/academic/scheduling/classroom-courses/{id}/rooms
pub async fn list_cc_preferred_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(cc_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let rows = sqlx::query_as::<_, CcPreferredRoomView>(
        r#"SELECT pr.id, pr.classroom_course_id, pr.room_id,
                  r.code AS room_code, r.name_th AS room_name,
                  pr.rank, pr.is_required
           FROM classroom_course_preferred_rooms pr
           JOIN rooms r ON r.id = pr.room_id
           WHERE pr.classroom_course_id = $1
           ORDER BY pr.rank ASC"#
    )
    .bind(cc_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(ApiResponse::success(rows)))
}

/// PUT /api/academic/scheduling/classroom-courses/{id}/rooms
/// Replace ทั้ง list — atomic transaction (delete + bulk insert)
pub async fn set_cc_preferred_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(cc_id): Path<Uuid>,
    Json(payload): Json<SetCcRoomsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let mut tx = pool.begin().await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query("DELETE FROM classroom_course_preferred_rooms WHERE classroom_course_id = $1")
        .bind(cc_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    if !payload.rooms.is_empty() {
        // Bulk insert ผ่าน UNNEST — 1 query
        let room_ids: Vec<Uuid> = payload.rooms.iter().map(|r| r.room_id).collect();
        let ranks: Vec<i32> = payload.rooms.iter().map(|r| r.rank).collect();
        let required: Vec<bool> = payload.rooms.iter()
            .map(|r| r.is_required.unwrap_or(false))
            .collect();

        sqlx::query(
            r#"INSERT INTO classroom_course_preferred_rooms
                   (classroom_course_id, room_id, rank, is_required)
               SELECT $1, room_id, rk, req
               FROM UNNEST($2::uuid[], $3::int[], $4::bool[]) AS t(room_id, rk, req)"#
        )
        .bind(cc_id)
        .bind(&room_ids)
        .bind(&ranks)
        .bind(&required)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(ApiResponse::success(format!("Updated {} rooms", payload.rooms.len()))))
}

/// GET /api/academic/scheduling/rooms — list ทุกห้อง (สำหรับ dropdown)
#[derive(Serialize, sqlx::FromRow)]
pub struct RoomView {
    pub id: Uuid,
    pub code: String,
    pub name_th: String,
    pub room_type: Option<String>,
}

pub async fn list_all_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let rows = sqlx::query_as::<_, RoomView>(
        "SELECT id, code, name_th, room_type FROM rooms WHERE status = 'ACTIVE' ORDER BY code"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(ApiResponse::success(rows)))
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

// Handlers

pub async fn list_instructor_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    
    // Get active academic year
    let academic_year = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM academic_years WHERE is_active = true LIMIT 1")
        .fetch_optional(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        
    let year_id = match academic_year {
        Some(y) => y.0,
        None => return Err(AppError::NotFound("Active academic year not found".to_string())),
    };

    // ดึงครู + priority + count จำนวน classroom_course ที่เป็น primary
    // (เพื่อให้ frontend filter ครูที่ไม่มีวิชาสอนเลยออกได้)
    // ใช้ subquery เดียวกัน aggregate ใน CTE — ไม่ใช่ N+1
    let instructors = sqlx::query_as::<_, InstructorConstraintView>(
        r#"
        WITH primary_counts AS (
            SELECT cci.instructor_id, COUNT(*)::bigint AS cnt
            FROM classroom_course_instructors cci
            JOIN classroom_courses cc ON cc.id = cci.classroom_course_id
            JOIN academic_semesters s ON s.id = cc.academic_semester_id
            WHERE cci.role = 'primary' AND s.academic_year_id = $1
            GROUP BY cci.instructor_id
        )
        SELECT
            u.id,
            u.first_name,
            u.last_name,
            ip.hard_unavailable_slots,
            ip.max_periods_per_day,
            ip.min_periods_per_day,
            ra.room_id AS assigned_room_id,
            r.name_th AS assigned_room_name,
            COALESCE(ip.priority, 100) AS priority,
            COALESCE(pc.cnt, 0) AS primary_course_count
        FROM users u
        LEFT JOIN instructor_preferences ip
            ON u.id = ip.instructor_id AND ip.academic_year_id = $1
        LEFT JOIN instructor_room_assignments ra
            ON u.id = ra.instructor_id AND ra.academic_year_id = $1 AND ra.is_required = true
        LEFT JOIN rooms r ON ra.room_id = r.id
        LEFT JOIN primary_counts pc ON pc.instructor_id = u.id
        WHERE u.user_type = 'staff' AND u.status = 'active'
        ORDER BY COALESCE(ip.priority, 100), u.first_name
        "#
    )
    .bind(year_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(ApiResponse::success(instructors)))
}

/// PUT /api/academic/scheduling/instructors/order
/// Bulk update priority — instructor_ids ตามลำดับ → priority = index + 1
pub async fn reorder_instructor_priority(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ReorderInstructorPriorityRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let academic_year = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM academic_years WHERE is_active = true LIMIT 1"
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let year_id = match academic_year {
        Some(y) => y.0,
        None => return Err(AppError::NotFound("Active academic year not found".to_string())),
    };

    if payload.instructor_ids.is_empty() {
        return Ok(Json(ApiResponse::success("No changes".to_string())));
    }

    // Build (instructor_id, priority) arrays
    let priorities: Vec<i32> = (1..=payload.instructor_ids.len() as i32).collect();

    // ใช้ INSERT ON CONFLICT แบบ batch — 1 query เดียว, ไม่ loop
    sqlx::query(
        r#"
        INSERT INTO instructor_preferences (instructor_id, academic_year_id, priority)
        SELECT instr_id, $2, prio
        FROM UNNEST($1::uuid[], $3::int[]) AS t(instr_id, prio)
        ON CONFLICT (instructor_id, academic_year_id)
        DO UPDATE SET priority = EXCLUDED.priority, updated_at = NOW()
        "#
    )
    .bind(&payload.instructor_ids)
    .bind(year_id)
    .bind(&priorities)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(ApiResponse::success(format!(
        "Reordered {} instructors", payload.instructor_ids.len()
    ))))
}

/// GET /api/academic/scheduling/settings
pub async fn get_school_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let rows = sqlx::query_as::<_, (String, serde_json::Value)>(
        "SELECT key, value FROM scheduler_settings"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let mut default_max_consecutive: i32 = 4;
    for (key, value) in rows {
        if key == "default_max_consecutive" {
            default_max_consecutive = value.as_i64().unwrap_or(4) as i32;
        }
    }

    Ok(Json(ApiResponse::success(SchoolSettingsView {
        default_max_consecutive,
    })))
}

/// PUT /api/academic/scheduling/settings
pub async fn update_school_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateSchoolSettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Some(v) = payload.default_max_consecutive {
        if !(1..=20).contains(&v) {
            return Err(AppError::BadRequest(
                "default_max_consecutive ต้องอยู่ระหว่าง 1-20".to_string()
            ));
        }
        sqlx::query(
            "INSERT INTO scheduler_settings (key, value) VALUES ('default_max_consecutive', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()"
        )
        .bind(serde_json::Value::from(v))
        .execute(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }

    Ok(Json(ApiResponse::success("Updated school settings".to_string())))
}

pub async fn update_instructor_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instructor_id): Path<Uuid>, // Extract from URL
    Json(payload): Json<UpdateInstructorConstraintRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    
    let mut tx = pool.begin().await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Get active academic year
    let academic_year = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM academic_years WHERE is_active = true LIMIT 1")
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let year_id = match academic_year {
        Some(y) => y.0,
        None => return Err(AppError::NotFound("Active academic year not found".to_string())),
    };

    // Update preferences — partial update: COALESCE เก็บค่าเดิมถ้า payload field เป็น None
    sqlx::query(
        r#"
        INSERT INTO instructor_preferences (
            instructor_id, academic_year_id,
            hard_unavailable_slots, max_periods_per_day, preferred_slots, priority
        )
        VALUES ($1, $2,
                COALESCE($3, '[]'::jsonb),
                $4,
                COALESCE($5, '[]'::jsonb),
                COALESCE($6, 100))
        ON CONFLICT (instructor_id, academic_year_id)
        DO UPDATE SET
            hard_unavailable_slots = COALESCE($3, instructor_preferences.hard_unavailable_slots),
            max_periods_per_day = COALESCE($4, instructor_preferences.max_periods_per_day),
            preferred_slots = COALESCE($5, instructor_preferences.preferred_slots),
            priority = COALESCE($6, instructor_preferences.priority),
            updated_at = NOW()
        "#
    )
    .bind(instructor_id)
    .bind(year_id)
    .bind(payload.hard_unavailable_slots)
    .bind(payload.max_periods_per_day)
    .bind(payload.preferred_slots)
    .bind(payload.priority)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Handle Room Assignment
    if let Some(room_id) = payload.assigned_room_id {
        sqlx::query(
            "DELETE FROM instructor_room_assignments WHERE instructor_id = $1 AND academic_year_id = $2 AND is_required = true"
        )
        .bind(instructor_id)
        .bind(year_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        
        sqlx::query(
            r#"
            INSERT INTO instructor_room_assignments (
                instructor_id, academic_year_id, room_id, is_required
            )
            VALUES ($1, $2, $3, true)
            "#
        )
        .bind(instructor_id)
        .bind(year_id)
        .bind(room_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    } else {
        // Clear assignment if None passed? Or assume no change? 
        // Let's check logic. If UI sends undefined, it's None.
        // If UI sends null, it's deserialized as None (if Option<Uuid>).
        // Standard JSON: null -> None.
        // If we want to support "Clear", we might need to know if it was explicitly null.
        // But for simplicity, let's assume if it is NOT in payload, we don't clear (Partial Update).
        // BUT `assigned_room_id` is Option<Uuid>. If field is missing => None.
        // We can't distinguish Missing vs Null easily without `Option<Option<T>>` or serde default.
        
        // Let's implement: If we want to clear, user must send specific cleared value?
        // Or actually, simple approach: Always clear if this endpoint is called? No.
        // Let's assume this endpoint is "Settings Save". It sends FULL state.
        // If user selected "No Room", frontend sends null.
        // So we should DELETE if room_id is None.
        
        sqlx::query(
            "DELETE FROM instructor_room_assignments WHERE instructor_id = $1 AND academic_year_id = $2 AND is_required = true"
        )
        .bind(instructor_id)
        .bind(year_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(ApiResponse::success("Updated instructor constraints".to_string())))
}

pub async fn list_subject_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    
    // Simple query for subjects
    let subjects = sqlx::query_as::<_, SubjectConstraintView>(
        r#"
        SELECT 
            id, -- map to id
            code,
            name_th as name,
            COALESCE(min_consecutive_periods, 1) as min_consecutive_periods,
            max_consecutive_periods,
            allow_single_period,
            periods_per_week,
            allowed_period_ids,
            allowed_days
        FROM subjects
        WHERE is_active = true
        ORDER BY code
        "#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(ApiResponse::success(subjects)))
}

pub async fn update_subject_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subject_id): Path<Uuid>, // Extract from URL
    Json(payload): Json<UpdateSubjectConstraintRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    sqlx::query(
        r#"
        UPDATE subjects SET
            min_consecutive_periods = COALESCE($2, min_consecutive_periods),
            max_consecutive_periods = $3,
            allow_single_period = COALESCE($4, allow_single_period),
            allowed_period_ids = $5,
            allowed_days = $6,
            updated_at = NOW()
        WHERE id = $1
        "#
    )
    .bind(subject_id)
    .bind(payload.min_consecutive_periods)
    .bind(payload.max_consecutive_periods)
    .bind(payload.allow_single_period)
    .bind(payload.allowed_period_ids)
    .bind(payload.allowed_days)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(ApiResponse::success("Updated subject constraints".to_string())))
}
