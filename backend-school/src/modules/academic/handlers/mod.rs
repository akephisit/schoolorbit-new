use crate::db::school_mapping::get_school_database_url;
use crate::error::AppError;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;
// ==========================================
// Handlers Module
// ==========================================
pub mod subjects;
pub mod course_planning;
pub mod timetable;
pub mod study_plans;
pub mod scheduling;
pub mod scheduling_config;
pub mod activity;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;
use super::models::*;

// ==========================================
// Academic Structure Handlers (Years, Semesters, Levels)
// ==========================================

pub async fn list_academic_structure(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    // Fetch Years
    let years = sqlx::query_as::<_, AcademicYear>(
        "SELECT * FROM academic_years ORDER BY year DESC"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Fetch Semesters
    let semesters = sqlx::query_as::<_, Semester>(
        "SELECT * FROM academic_semesters ORDER BY start_date DESC"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Fetch Grade Levels (with computed fields)
    let levels_raw = sqlx::query_as::<_, GradeLevel>(
        "SELECT * FROM grade_levels ORDER BY 
         CASE level_type 
            WHEN 'kindergarten' THEN 1 
            WHEN 'primary' THEN 2 
            WHEN 'secondary' THEN 3 
            ELSE 4 
         END, 
         year ASC"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();
    
    // Convert to response format with computed fields
    let levels: Vec<GradeLevelResponse> = levels_raw.into_iter().map(|l| l.into()).collect();

    Ok(Json(json!({
        "success": true,
        "data": {
            "years": years,
            "semesters": semesters,
            "levels": levels
        }
    })))
}

pub async fn create_academic_year(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAcademicYearRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    // If setting as active, deactivate others
    if payload.is_active.unwrap_or(false) {
        let _ = sqlx::query("UPDATE academic_years SET is_active = false").execute(&pool).await;
    }

    let school_days = payload.school_days.unwrap_or_else(|| "MON,TUE,WED,THU,FRI".to_string());

    let result = sqlx::query_as::<_, AcademicYear>(
        "INSERT INTO academic_years (year, name, start_date, end_date, is_active, school_days)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *"
    )
    .bind(payload.year)
    .bind(payload.name)
    .bind(payload.start_date)
    .bind(payload.end_date)
    .bind(payload.is_active.unwrap_or(false))
    .bind(&school_days)
    .fetch_one(&pool)
    .await;

    match result {
        Ok(year) => Ok((StatusCode::CREATED, Json(json!({"success": true, "data": year})))),
        Err(e) => {
            eprintln!("Failed to create year: {}", e);
            Err(AppError::InternalServerError("Failed to create academic year".to_string()))
        }
    }
}

pub async fn update_academic_year(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAcademicYearRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    let result = sqlx::query_as::<_, AcademicYear>(
        r#"UPDATE academic_years SET
            year = COALESCE($2, year),
            name = COALESCE($3, name),
            start_date = COALESCE($4, start_date),
            end_date = COALESCE($5, end_date),
            school_days = COALESCE($6, school_days),
            updated_at = NOW()
        WHERE id = $1
        RETURNING *"#
    )
    .bind(id)
    .bind(payload.year)
    .bind(&payload.name)
    .bind(payload.start_date)
    .bind(payload.end_date)
    .bind(&payload.school_days)
    .fetch_one(&pool)
    .await;

    match result {
        Ok(year) => Ok(Json(json!({"success": true, "data": year})).into_response()),
        Err(e) => {
            eprintln!("Failed to update year: {}", e);
            Err(AppError::NotFound("Academic year not found".to_string()))
        }
    }
}

pub async fn toggle_active_year(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    let mut tx = pool.begin().await.map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    // Deactivate all
    sqlx::query("UPDATE academic_years SET is_active = false")
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::InternalServerError("Failed to reset active year".to_string()))?;

    // Activate target
    sqlx::query("UPDATE academic_years SET is_active = true WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::InternalServerError("Failed to set active year".to_string()))?;

    tx.commit().await.map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(Json(json!({"success": true, "message": "Updated active year"})))
}

// ==========================================
// Semester Handlers
// ==========================================

pub async fn create_semester(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateSemesterRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    // If setting as active, deactivate others globally
    if payload.is_active.unwrap_or(false) {
        sqlx::query("UPDATE academic_semesters SET is_active = false")
            .execute(&pool)
            .await
            .map_err(|_| AppError::InternalServerError("Failed to reset active semester".to_string()))?;
    }

    let result = sqlx::query_as::<_, Semester>(
        "INSERT INTO academic_semesters (academic_year_id, term, name, start_date, end_date, is_active) 
         VALUES ($1, $2, $3, $4, $5, $6) 
         RETURNING *"
    )
    .bind(payload.academic_year_id)
    .bind(payload.term)
    .bind(payload.name)
    .bind(payload.start_date)
    .bind(payload.end_date)
    .bind(payload.is_active.unwrap_or(false))
    .fetch_one(&pool)
    .await;

    match result {
        Ok(semester) => Ok((StatusCode::CREATED, Json(json!({"success": true, "data": semester})))),
        Err(e) => {
            eprintln!("Failed to create semester: {}", e);
            Err(AppError::InternalServerError("Failed to create semester".to_string()))
        }
    }
}

pub async fn update_semester(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSemesterRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    // If setting as active, deactivate others globally
    if payload.is_active.unwrap_or(false) {
        sqlx::query("UPDATE academic_semesters SET is_active = false")
            .execute(&pool)
            .await
            .map_err(|_| AppError::InternalServerError("Failed to reset active semester".to_string()))?;
    }

    let result = sqlx::query_as::<_, Semester>(
        "UPDATE academic_semesters SET 
            term = COALESCE($1, term),
            name = COALESCE($2, name),
            start_date = COALESCE($3, start_date),
            end_date = COALESCE($4, end_date),
            is_active = COALESCE($5, is_active),
            updated_at = NOW()
         WHERE id = $6
         RETURNING *"
    )
    .bind(payload.term)
    .bind(payload.name)
    .bind(payload.start_date)
    .bind(payload.end_date)
    .bind(payload.is_active)
    .bind(id)
    .fetch_one(&pool)
    .await;

    match result {
        Ok(semester) => Ok(Json(json!({"success": true, "data": semester}))),
        Err(e) => {
             eprintln!("Failed to update semester: {}", e);
             Err(AppError::InternalServerError("Failed to update semester".to_string()))
        }
    }
}

pub async fn delete_semester(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    let result = sqlx::query("DELETE FROM academic_semesters WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await;

    match result {
        Ok(_) => Ok(Json(json!({"success": true, "message": "Semester deleted"}))),
        Err(e) => {
            eprintln!("Failed to delete semester: {}", e);
             if e.to_string().contains("foreign key constraint") {
                Err(AppError::BadRequest("ไม่สามารถลบภาคเรียนที่มีการใช้งานได้".to_string()))
             } else {
                Err(AppError::InternalServerError("Failed to delete semester".to_string()))
             }
        }
    }
}

// ==========================================
// Classrooms Handlers
// ==========================================

pub async fn list_classrooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<ClassroomQuery>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    let year_id_filter = filter.year_id;

    let mut query = String::from(
        "SELECT c.*,
                CASE gl.level_type
                    WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                    WHEN 'primary' THEN CONCAT('ป.', gl.year)
                    WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                    ELSE CONCAT('?.', gl.year)
                END as grade_level_name,
                ay.name as academic_year_label,
                (SELECT COUNT(*) FROM student_class_enrollments ske WHERE ske.class_room_id = c.id AND ske.status = 'active') as student_count,
                COALESCE((
                    SELECT jsonb_agg(
                        jsonb_build_object(
                            'user_id', ca.user_id,
                            'role', ca.role,
                            'name', CONCAT(COALESCE(u.title, ''), u.first_name, ' ', u.last_name)
                        ) ORDER BY ca.role, u.first_name
                    )
                    FROM classroom_advisors ca
                    JOIN users u ON u.id = ca.user_id
                    WHERE ca.classroom_id = c.id
                ), '[]'::jsonb) as advisors
         FROM class_rooms c
         JOIN grade_levels gl ON c.grade_level_id = gl.id
         JOIN academic_years ay ON c.academic_year_id = ay.id
         WHERE 1=1"
    );

    let mut idx = 0u32;

    if year_id_filter.is_some() {
        idx += 1;
        query.push_str(&format!(" AND c.academic_year_id = ${idx}"));
    }

    query.push_str(" ORDER BY
         CASE gl.level_type
            WHEN 'kindergarten' THEN 1
            WHEN 'primary' THEN 2
            WHEN 'secondary' THEN 3
            ELSE 4
         END,
         gl.year ASC,
         c.room_number ASC");

    let mut q = sqlx::query_as::<_, Classroom>(&query);
    if let Some(yid) = year_id_filter { q = q.bind(yid); }

    let classrooms = q
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    Ok(Json(json!({
        "success": true,
        "data": classrooms
    })))
}

pub async fn create_classroom(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateClassroomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    // 1. Get Grade Level Info for Name Generation
    let grade_level = sqlx::query_as::<_, GradeLevel>(
        "SELECT * FROM grade_levels WHERE id = $1"
    )
    .bind(payload.grade_level_id)
    .fetch_one(&pool)
    .await
    .map_err(|_| AppError::BadRequest("Invalid grade level".to_string()))?;

    // 2. Get Year Info for Code Generation
    let year = sqlx::query_as::<_, AcademicYear>(
        "SELECT * FROM academic_years WHERE id = $1"
    )
    .bind(payload.academic_year_id)
    .fetch_one(&pool)
    .await
    .map_err(|_| AppError::BadRequest("Invalid academic year".to_string()))?;

    // 3. Validate advisors (roles + staff check)
    let advisors = validate_advisors(&payload.advisors, &pool).await?;

    // 4. Generate Name and Code
    // Name: "ม.1/2" or "ม.1/EP"
    let full_name = format!("{}/{}", grade_level.short_name(), payload.room_number);

    // Code: "67-M1-2" (Year-Level-Room)
    let short_year = year.year % 100;
    let code = format!("{}-{}-{}", short_year, grade_level.code(), payload.room_number.replace(" ", ""));

    let mut tx = pool.begin().await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    // 5. Insert classroom
    let classroom_id: Uuid = sqlx::query_scalar(
        "INSERT INTO class_rooms (code, name, academic_year_id, grade_level_id, room_number, study_plan_version_id, capacity)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id"
    )
    .bind(code)
    .bind(full_name)
    .bind(payload.academic_year_id)
    .bind(payload.grade_level_id)
    .bind(&payload.room_number)
    .bind(payload.study_plan_version_id)
    .bind(payload.capacity.unwrap_or(40))
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("Failed to create classroom: {}", e);
        if e.to_string().contains("unique constraint") {
            AppError::BadRequest("ห้องเรียนนี้มีอยู่แล้วในระบบ".to_string())
        } else if e.to_string().contains("violates foreign key constraint") {
            AppError::BadRequest("ข้อมูลอ้างอิงไม่ถูกต้อง (FK Violation)".to_string())
        } else {
            AppError::InternalServerError("Failed to create classroom".to_string())
        }
    })?;

    // 6. Insert advisors (if any)
    insert_advisors(&mut tx, classroom_id, &advisors).await?;

    tx.commit().await.map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    // Re-fetch with joined fields
    let classroom = fetch_classroom_full(&pool, classroom_id).await?;

    Ok((StatusCode::CREATED, Json(json!({"success": true, "data": classroom}))))
}

/// Validate advisor list: non-empty roles, max 1 primary, all are staff users
async fn validate_advisors(
    advisors: &Option<Vec<ClassroomAdvisorInput>>,
    pool: &sqlx::PgPool,
) -> Result<Vec<ClassroomAdvisorInput>, AppError> {
    let Some(list) = advisors else { return Ok(vec![]) };
    let primary_count = list.iter().filter(|a| a.role == "primary").count();
    if primary_count > 1 {
        return Err(AppError::BadRequest("ครูที่ปรึกษาหลักต้องมีได้ไม่เกิน 1 คน".to_string()));
    }
    for a in list {
        if a.role != "primary" && a.role != "secondary" {
            return Err(AppError::BadRequest("role ต้องเป็น 'primary' หรือ 'secondary' เท่านั้น".to_string()));
        }
        let is_staff: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND user_type = 'staff')"
        )
        .bind(a.user_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);
        if !is_staff {
            return Err(AppError::BadRequest("ครูที่ปรึกษาต้องเป็นบุคลากร (Staff)".to_string()));
        }
    }
    Ok(list.clone())
}

/// Insert advisor junction rows (atomic, used by both create + update-replace)
async fn insert_advisors(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    classroom_id: Uuid,
    advisors: &[ClassroomAdvisorInput],
) -> Result<(), AppError> {
    for a in advisors {
        sqlx::query(
            "INSERT INTO classroom_advisors (classroom_id, user_id, role)
             VALUES ($1, $2, $3)
             ON CONFLICT (classroom_id, user_id) DO UPDATE SET role = EXCLUDED.role"
        )
        .bind(classroom_id)
        .bind(a.user_id)
        .bind(&a.role)
        .execute(&mut **tx)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to save advisor: {}", e)))?;
    }
    Ok(())
}

/// Fetch classroom with joined fields (grade name, year label, student count, advisors)
async fn fetch_classroom_full(pool: &sqlx::PgPool, id: Uuid) -> Result<Classroom, AppError> {
    sqlx::query_as::<_, Classroom>(
        "SELECT c.*,
                CASE gl.level_type
                    WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                    WHEN 'primary' THEN CONCAT('ป.', gl.year)
                    WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                    ELSE CONCAT('?.', gl.year)
                END as grade_level_name,
                ay.name as academic_year_label,
                (SELECT COUNT(*) FROM student_class_enrollments ske WHERE ske.class_room_id = c.id AND ske.status = 'active') as student_count,
                COALESCE((
                    SELECT jsonb_agg(
                        jsonb_build_object(
                            'user_id', ca.user_id,
                            'role', ca.role,
                            'name', CONCAT(COALESCE(u.title, ''), u.first_name, ' ', u.last_name)
                        ) ORDER BY ca.role, u.first_name
                    )
                    FROM classroom_advisors ca
                    JOIN users u ON u.id = ca.user_id
                    WHERE ca.classroom_id = c.id
                ), '[]'::jsonb) as advisors
         FROM class_rooms c
         JOIN grade_levels gl ON c.grade_level_id = gl.id
         JOIN academic_years ay ON c.academic_year_id = ay.id
         WHERE c.id = $1"
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to fetch classroom".to_string()))
}

pub async fn update_classroom(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateClassroomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    // Validate advisors list if provided
    let advisors_opt = if payload.advisors.is_some() {
        Some(validate_advisors(&payload.advisors, &pool).await?)
    } else {
        None
    };

    let mut tx = pool.begin().await.map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    // If room_number changes, update name and code
    if let Some(ref new_room) = payload.room_number {
         // ดึงเฉพาะ field ที่ต้องใช้ (grade_level_id, academic_year_id)
         let current: (Uuid, Uuid) = sqlx::query_as(
            "SELECT grade_level_id, academic_year_id FROM class_rooms WHERE id = $1"
         )
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| AppError::NotFound("Classroom not found".to_string()))?;

         let grade_level = sqlx::query_as::<_, GradeLevel>(
            "SELECT id, level_type, year, next_grade_level_id, is_active FROM grade_levels WHERE id = $1"
         )
            .bind(current.0)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| AppError::InternalServerError("Failed to fetch grade level".to_string()))?;

         let year = sqlx::query_as::<_, AcademicYear>(
            "SELECT id, year, name, start_date, end_date, is_active, school_days, metadata, created_at, updated_at FROM academic_years WHERE id = $1"
         )
            .bind(current.1)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| AppError::InternalServerError("Failed to fetch year".to_string()))?;

        let full_name = format!("{}/{}", grade_level.short_name(), new_room);
        let short_year = year.year % 100;
        let code = format!("{}-{}-{}", short_year, grade_level.code(), new_room.replace(" ", ""));

        sqlx::query("UPDATE class_rooms SET name = $1, code = $2, room_number = $3 WHERE id = $4")
            .bind(full_name)
            .bind(code)
            .bind(new_room)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                if e.to_string().contains("unique") {
                    AppError::BadRequest("ชื่อ/รหัสห้องเรียนซ้ำ".to_string())
                } else {
                    AppError::InternalServerError("Failed to update room number".to_string())
                }
            })?;
    }

    // Update scalar fields (advisors handled via junction below)
    sqlx::query(
        "UPDATE class_rooms SET
            study_plan_version_id = COALESCE($1, study_plan_version_id),
            capacity = COALESCE($2, capacity),
            is_active = COALESCE($3, is_active),
            updated_at = NOW()
         WHERE id = $4"
    )
    .bind(payload.study_plan_version_id)
    .bind(payload.capacity)
    .bind(payload.is_active)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
         eprintln!("Failed to update classroom: {}", e);
         AppError::InternalServerError("Failed to update classroom".to_string())
    })?;

    // Replace advisors atomically (DELETE + INSERT) only when caller provided list.
    // Sending [] = ลบครูที่ปรึกษาทั้งหมด; ไม่ส่ง = คงเดิม
    if let Some(advisors) = advisors_opt {
        sqlx::query("DELETE FROM classroom_advisors WHERE classroom_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to clear advisors: {}", e)))?;
        insert_advisors(&mut tx, id, &advisors).await?;
    }

    tx.commit().await.map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    let result = fetch_classroom_full(&pool, id).await?;
    Ok(Json(json!({"success": true, "data": result})))
}

// ==========================================
// Grade Level Handlers
// ==========================================

pub async fn create_grade_level(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateGradeLevelRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    // Validate level_type
    if !["kindergarten", "primary", "secondary"].contains(&payload.level_type.as_str()) {
        return Err(AppError::BadRequest("ประเภทระดับชั้นไม่ถูกต้อง".to_string()));
    }

    let result = sqlx::query_as::<_, GradeLevel>(
        "INSERT INTO grade_levels (level_type, year, next_grade_level_id, is_active)
         VALUES ($1, $2, $3, $4)
         RETURNING *"
    )
    .bind(&payload.level_type)
    .bind(payload.year)
    .bind(payload.next_grade_level_id)
    .bind(payload.is_active.unwrap_or(true))
    .fetch_one(&pool)
    .await;

    match result {
        Ok(level) => {
            let response: GradeLevelResponse = level.into();
            Ok((StatusCode::CREATED, Json(json!({"success": true, "data": response}))))
        },
        Err(e) => {
            eprintln!("Failed to create grade level: {}", e);
            if e.to_string().contains("unique") {
                Err(AppError::BadRequest("ระดับชั้นนี้มีอยู่แล้ว".to_string()))
            } else {
                Err(AppError::InternalServerError("Failed to create grade level".to_string()))
            }
        }
    }
}

pub async fn delete_grade_level(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    // Check usage
    let usage_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM class_rooms WHERE grade_level_id = $1"
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    if usage_count > 0 {
        return Err(AppError::BadRequest(format!("ไม่สามารถลบระดับชั้นได้เนื่องจากมีการใช้งานอยู่ {} ห้องเรียน", usage_count)));
    }

    let result = sqlx::query("DELETE FROM grade_levels WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await;

    match result {
        Ok(_) => Ok(Json(json!({"success": true, "message": "Grade level deleted"}))),
        Err(e) => {
            eprintln!("Failed to delete grade level: {}", e);
            Err(AppError::InternalServerError("Failed to delete grade level".to_string()))
        }
    }
}

// ==========================================
// Enrollment Handlers
// ==========================================

pub async fn enroll_students(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<EnrollStudentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    // Validate Classroom
    let classroom = sqlx::query_as::<_, Classroom>(
        "SELECT c.*, 
                CASE gl.level_type 
                    WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                    WHEN 'primary' THEN CONCAT('ป.', gl.year)
                    WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                    ELSE CONCAT('?.', gl.year)
                END as grade_level_name,
                NULL::text as academic_year_label,
                NULL::bigint as student_count
         FROM class_rooms c
         JOIN grade_levels gl ON c.grade_level_id = gl.id
         WHERE c.id = $1"
    )
    .bind(payload.class_room_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?
    .ok_or(AppError::NotFound("Classroom not found".to_string()))?;

    let enroll_date = payload.enrollment_date.unwrap_or(chrono::Local::now().date_naive());

    let mut tx = pool.begin().await.map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    let mut enrolled_count = 0;
    
    for student_id in &payload.student_ids {
        // Deactivate old active enrollments for this student (if any)
        sqlx::query(
            "UPDATE student_class_enrollments SET status = 'moved_out', updated_at = NOW() 
             WHERE student_id = $1 AND status = 'active'"
        )
        .bind(student_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("Failed to update old enrollment: {}", e);
            AppError::InternalServerError("Failed to process enrollment".to_string())
        })?;

        // Create new enrollment
        sqlx::query(
            "INSERT INTO student_class_enrollments (student_id, class_room_id, enrollment_date, status)
             VALUES ($1, $2, $3, 'active')
             ON CONFLICT (student_id, class_room_id) 
             DO UPDATE SET status = 'active', enrollment_date = $3, updated_at = NOW()"
        )
        .bind(student_id)
        .bind(payload.class_room_id)
        .bind(enroll_date)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("Failed to enroll: {}", e);
            AppError::InternalServerError("Failed to enroll student".to_string())
        })?;



        enrolled_count += 1;
    }

    tx.commit().await.map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    // Handle class numbering based on method
    let numbering_method = payload.numbering_method.as_deref().unwrap_or("append");
    
    match numbering_method {
        "none" => {
            // Do nothing - no class numbers assigned
        }
        "append" => {
            // Get max class_number in the classroom
            let max_number: Option<i32> = sqlx::query_scalar(
                "SELECT MAX(class_number) FROM student_class_enrollments 
                 WHERE class_room_id = $1 AND status = 'active'"
            )
            .bind(payload.class_room_id)
            .fetch_one(&pool)
            .await
            .unwrap_or(None);

            let start_number = max_number.unwrap_or(0) + 1;

            // Assign numbers to newly enrolled students
            for (index, student_id) in payload.student_ids.iter().enumerate() {
                let class_number = start_number + index as i32;
                
                sqlx::query(
                    "UPDATE student_class_enrollments 
                     SET class_number = $1, updated_at = NOW()
                     WHERE student_id = $2 AND class_room_id = $3 AND status = 'active'"
                )
                .bind(class_number)
                .bind(student_id)
                .bind(payload.class_room_id)
                .execute(&pool)
                .await
                .map_err(|e| {
                    eprintln!("Failed to update class number: {}", e);
                    AppError::InternalServerError("Failed to update class number".to_string())
                })?;
            }
        }
        "student_code" | "name" | "gender_name" => {
            // Re-sort entire classroom using existing auto_assign logic
            #[derive(sqlx::FromRow)]
            struct StudentForNumbering {
                id: Uuid,
                student_code: String,
                first_name: String,
                title: Option<String>,
            }

            let mut students = sqlx::query_as::<_, StudentForNumbering>(
                "SELECT ske.id, s.student_id as student_code, u.first_name, u.title
                 FROM student_class_enrollments ske
                 LEFT JOIN users u ON ske.student_id = u.id
                 LEFT JOIN student_info s ON u.id = s.user_id
                 WHERE ske.class_room_id = $1 AND ske.status = 'active'"
            )
            .bind(payload.class_room_id)
            .fetch_all(&pool)
            .await
            .map_err(|e| {
                eprintln!("Failed to fetch students: {}", e);
                AppError::InternalServerError("Failed to fetch students".to_string())
            })?;

            // Sort based on method
            match numbering_method {
                "student_code" => {
                    students.sort_by(|a, b| a.student_code.cmp(&b.student_code));
                }
                "name" => {
                    students.sort_by(|a, b| a.first_name.cmp(&b.first_name));
                }
                "gender_name" => {
                    let is_male = |title: &Option<String>| -> bool {
                        if let Some(t) = title {
                            matches!(t.as_str(), "เด็กชาย" | "นาย")
                        } else {
                            false
                        }
                    };

                    students.sort_by(|a, b| {
                        let a_male = is_male(&a.title);
                        let b_male = is_male(&b.title);

                        match (a_male, b_male) {
                            (true, false) => std::cmp::Ordering::Less,
                            (false, true) => std::cmp::Ordering::Greater,
                            _ => a.first_name.cmp(&b.first_name)
                        }
                    });
                }
                _ => {}
            }

            // Update all class numbers
            for (index, student) in students.iter().enumerate() {
                let class_number = (index + 1) as i32;
                
                sqlx::query(
                    "UPDATE student_class_enrollments 
                     SET class_number = $1, updated_at = NOW() 
                     WHERE id = $2"
                )
                .bind(class_number)
                .bind(student.id)
                .execute(&pool)
                .await
                .map_err(|e| {
                    eprintln!("Failed to update class number: {}", e);
                    AppError::InternalServerError("Failed to update class number".to_string())
                })?;
            }
        }
        _ => {
            // Invalid method - do nothing
        }
    }

    Ok(Json(json!({
        "success": true,
        "message": format!("Enrolled {} students successfully", enrolled_count)
    })))
}

pub async fn get_class_enrollments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(class_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    let enrollments = sqlx::query_as::<_, StudentEnrollment>(
        "SELECT ske.*, 
                CONCAT(u.first_name, ' ', u.last_name) as student_name,
                c.name as class_name,
                s.student_id as student_code
         FROM student_class_enrollments ske
         LEFT JOIN users u ON ske.student_id = u.id
         LEFT JOIN student_info s ON u.id = s.user_id
         LEFT JOIN class_rooms c ON ske.class_room_id = c.id
         WHERE ske.class_room_id = $1 AND ske.status = 'active'
         ORDER BY ske.class_number ASC NULLS LAST, s.student_id ASC"
    )
    .bind(class_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    Ok(Json(json!({
        "success": true,
        "data": enrollments
    })))
}

pub async fn remove_enrollment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    let mut tx = pool.begin().await.map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    // Define temporary struct for fetching student_id
    #[derive(sqlx::FromRow)]
    struct EnrollmentRecord {
        student_id: Uuid,
    }

    // Get student ID before deleting to update denormalized data
    let enrollment = sqlx::query_as::<_, EnrollmentRecord>(
        "SELECT student_id FROM student_class_enrollments WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    if let Some(record) = enrollment {
        // Soft delete (set status to removed or just delete?) -> Let's hard delete for mistaken enrollment, 
        // OR better: set status to 'cancelled' so we keep history?
        // Let's hard delete for now if it's "removing from class" without moving to another
        sqlx::query("DELETE FROM student_class_enrollments WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|_| AppError::InternalServerError("Failed to delete enrollment".to_string()))?;


    }

    tx.commit().await.map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(Json(json!({"success": true, "message": "Enrollment removed"})))
}

#[derive(serde::Deserialize)]
pub struct UpdateEnrollmentNumberRequest {
    pub class_number: Option<i32>,
}

pub async fn update_enrollment_number(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateEnrollmentNumberRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    // Check duplicate number in same class? optional, but good practice.
    // However, it might be annoying during re-ordering. Let's allow simple update first.
    
    let result = sqlx::query(
        "UPDATE student_class_enrollments SET class_number = $1, updated_at = NOW() WHERE id = $2"
    )
    .bind(payload.class_number)
    .bind(id)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => Ok(Json(json!({"success": true, "message": "Class number updated"}))),
        Err(e) => {
            eprintln!("Failed to update class number: {}", e);
            Err(AppError::InternalServerError("Failed to update class number".to_string()))
        }
    }
}

#[derive(serde::Deserialize)]
pub struct AutoAssignClassNumbersRequest {
    pub sort_by: String, // "student_code" | "name" | "gender_name"
}

pub async fn auto_assign_class_numbers(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(class_id): Path<Uuid>,
    Json(payload): Json<AutoAssignClassNumbersRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    // Define struct for fetching student data with title
    #[derive(sqlx::FromRow)]
    struct StudentForNumbering {
        id: Uuid,
        student_code: String,
        first_name: String,
        title: Option<String>,
    }

    // Fetch all active enrollments with student info
    let mut students = sqlx::query_as::<_, StudentForNumbering>(
        "SELECT ske.id, s.student_id as student_code, u.first_name, u.title
         FROM student_class_enrollments ske
         LEFT JOIN users u ON ske.student_id = u.id
         LEFT JOIN student_info s ON u.id = s.user_id
         WHERE ske.class_room_id = $1 AND ske.status = 'active'"
    )
    .bind(class_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch students: {}", e);
        AppError::InternalServerError("Failed to fetch students".to_string())
    })?;

    // Sort based on requested method
    match payload.sort_by.as_str() {
        "student_code" => {
            students.sort_by(|a, b| a.student_code.cmp(&b.student_code));
        }
        "name" => {
            students.sort_by(|a, b| a.first_name.cmp(&b.first_name));
        }
        "gender_name" => {
            // Helper function to determine if title is male
            let is_male = |title: &Option<String>| -> bool {
                if let Some(t) = title {
                    matches!(t.as_str(), "เด็กชาย" | "นาย")
                } else {
                    false
                }
            };

            students.sort_by(|a, b| {
                let a_male = is_male(&a.title);
                let b_male = is_male(&b.title);

                match (a_male, b_male) {
                    (true, false) => std::cmp::Ordering::Less,    // Male before female
                    (false, true) => std::cmp::Ordering::Greater, // Female after male
                    _ => a.first_name.cmp(&b.first_name)          // Same gender, sort by name
                }
            });
        }
        _ => {
            return Err(AppError::BadRequest("Invalid sort_by parameter".to_string()));
        }
    }

    // Update class numbers in transaction
    let mut tx = pool.begin().await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    for (index, student) in students.iter().enumerate() {
        let class_number = (index + 1) as i32;
        
        sqlx::query(
            "UPDATE student_class_enrollments SET class_number = $1, updated_at = NOW() 
             WHERE id = $2"
        )
        .bind(class_number)
        .bind(student.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("Failed to update class number: {}", e);
            AppError::InternalServerError("Failed to update class number".to_string())
        })?;
    }

    tx.commit().await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(Json(json!({
        "success": true,
        "message": format!("เรียงเลขที่สำหรับ {} คนเรียบร้อยแล้ว", students.len())
    })))
}



// ==========================================
// Year-Level Configuration Handlers
// ==========================================

pub async fn get_year_levels(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(year_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    // Return list of grade_level_ids that are active for this year
    let level_ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT grade_level_id FROM academic_year_grade_levels WHERE academic_year_id = $1"
    )
    .bind(year_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    Ok(Json(json!({
        "success": true,
        "data": level_ids
    })))
}

pub async fn update_year_levels(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(year_id): Path<Uuid>,
    Json(payload): Json<UpdateYearLevelsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    let mut tx = pool.begin().await.map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    // 1. Clear existing mappings for this year
    sqlx::query("DELETE FROM academic_year_grade_levels WHERE academic_year_id = $1")
        .bind(year_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::InternalServerError("Failed to clear existing mappings".to_string()))?;

    // 2. Insert new mappings
    for level_id in payload.grade_level_ids {
        sqlx::query("INSERT INTO academic_year_grade_levels (academic_year_id, grade_level_id) VALUES ($1, $2)")
            .bind(year_id)
            .bind(level_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| AppError::InternalServerError("Failed to insert mapping".to_string()))?;
    }

    tx.commit().await.map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(Json(json!({"success": true, "message": "Year levels updated"})))
}
