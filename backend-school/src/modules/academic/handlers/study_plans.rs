use crate::db::school_mapping::get_school_database_url;
use crate::error::AppError;
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

use super::super::models::study_plans::*;

// ============================================
// Study Plans CRUD
// ============================================

pub async fn list_study_plans(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<StudyPlanQuery>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;
    
    let mut sql = String::from("SELECT * FROM study_plans WHERE 1=1");
    let mut idx = 0u32;

    if query.active_only.unwrap_or(false) {
        sql.push_str(" AND is_active = true");
    }

    if let Some(ref _level_scope) = query.level_scope {
        idx += 1;
        sql.push_str(&format!(" AND level_scope = ${idx}"));
    }

    sql.push_str(" ORDER BY code");

    let mut q = sqlx::query_as::<_, StudyPlan>(&sql);
    if let Some(ref level_scope) = query.level_scope { q = q.bind(level_scope); }
    let plans = q.fetch_all(&pool).await.unwrap_or_default();
    
    Ok(Json(json!({"success": true, "data": plans})))
}

pub async fn get_study_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(plan_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;
    
    let plan = sqlx::query_as::<_, StudyPlan>(
        "SELECT * FROM study_plans WHERE id = $1"
    )
    .bind(plan_id)
    .fetch_one(&pool)
    .await?;
    
    Ok(Json(json!({"success": true, "data": plan})))
}

pub async fn create_study_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateStudyPlanRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;
    
    let plan = sqlx::query_as::<_, StudyPlan>(
        "INSERT INTO study_plans (code, name_th, name_en, description, level_scope)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *"
    )
    .bind(&req.code)
    .bind(&req.name_th)
    .bind(&req.name_en)
    .bind(&req.description)
    .bind(&req.level_scope)
    .fetch_one(&pool)
    .await?;
    
    Ok((StatusCode::CREATED, Json(json!({"success": true, "data": plan}))))
}

pub async fn update_study_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(plan_id): Path<Uuid>,
    Json(req): Json<UpdateStudyPlanRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;
    
    let mut updates = Vec::new();
    let mut param_count = 1;
    
    if req.code.is_some() {
        updates.push(format!("code = ${}", param_count));
        param_count += 1;
    }
    if req.name_th.is_some() {
        updates.push(format!("name_th = ${}", param_count));
        param_count += 1;
    }
    if req.name_en.is_some() {
        updates.push(format!("name_en = ${}", param_count));
        param_count += 1;
    }
    if req.description.is_some() {
        updates.push(format!("description = ${}", param_count));
        param_count += 1;
    }
    if req.level_scope.is_some() {
        updates.push(format!("level_scope = ${}", param_count));
        param_count += 1;
    }
    if req.is_active.is_some() {
        updates.push(format!("is_active = ${}", param_count));
        param_count += 1;
    }
    
    if updates.is_empty() {
        return Err(AppError::BadRequest("No fields to update".to_string()));
    }
    
    let sql = format!(
        "UPDATE study_plans SET {} WHERE id = ${} RETURNING *",
        updates.join(", "),
        param_count
    );
    
    let mut query = sqlx::query_as::<_, StudyPlan>(&sql);
    
    if let Some(ref code) = req.code {
        query = query.bind(code);
    }
    if let Some(ref name_th) = req.name_th {
        query = query.bind(name_th);
    }
    if let Some(ref name_en) = req.name_en {
        query = query.bind(name_en);
    }
    if let Some(ref description) = req.description {
        query = query.bind(description);
    }
    if let Some(ref level_scope) = req.level_scope {
        query = query.bind(level_scope);
    }
    if let Some(is_active) = req.is_active {
        query = query.bind(is_active);
    }
    
    query = query.bind(plan_id);
    
    let plan = query.fetch_one(&pool).await?;
    
    Ok(Json(json!({"success": true, "data": plan})))
}

pub async fn delete_study_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(plan_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;
    
    sqlx::query("DELETE FROM study_plans WHERE id = $1")
        .bind(plan_id)
        .execute(&pool)
        .await?;
    
    Ok((StatusCode::OK, Json(json!({"success": true}))))
}

// ============================================
// Study Plan Versions CRUD
// ============================================

pub async fn list_study_plan_versions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<StudyPlanVersionQuery>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;
    
    let mut sql = String::from(
        "SELECT spv.*, 
                sp.name_th as study_plan_name_th,
                ay.name as start_year_name
         FROM study_plan_versions spv
         LEFT JOIN study_plans sp ON sp.id = spv.study_plan_id
         LEFT JOIN academic_years ay ON ay.id = spv.start_academic_year_id
         WHERE 1=1"
    );
    
    let mut idx = 0u32;

    if let Some(_study_plan_id) = query.study_plan_id {
        idx += 1;
        sql.push_str(&format!(" AND spv.study_plan_id = ${idx}"));
    }

    if query.active_only.unwrap_or(false) {
        sql.push_str(" AND spv.is_active = true");
    }

    sql.push_str(" ORDER BY spv.created_at DESC");

    let mut q = sqlx::query_as::<_, StudyPlanVersion>(&sql);
    if let Some(study_plan_id) = query.study_plan_id { q = q.bind(study_plan_id); }
    let versions = q.fetch_all(&pool).await.unwrap_or_default();
    
    Ok(Json(json!({"success": true, "data": versions})))
}

pub async fn get_study_plan_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;
    
    let version = sqlx::query_as::<_, StudyPlanVersion>(
        "SELECT spv.*, 
                sp.name_th as study_plan_name_th,
                ay.name as start_year_name
         FROM study_plan_versions spv
         LEFT JOIN study_plans sp ON sp.id = spv.study_plan_id
         LEFT JOIN academic_years ay ON ay.id = spv.start_academic_year_id
         WHERE spv.id = $1"
    )
    .bind(version_id)
    .fetch_one(&pool)
    .await?;
    
    Ok(Json(json!({"success": true, "data": version})))
}

pub async fn create_study_plan_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateStudyPlanVersionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;
    
    let version = sqlx::query_as::<_, StudyPlanVersion>(
        "INSERT INTO study_plan_versions 
         (study_plan_id, version_name, start_academic_year_id, end_academic_year_id, description)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *"
    )
    .bind(&req.study_plan_id)
    .bind(&req.version_name)
    .bind(&req.start_academic_year_id)
    .bind(&req.end_academic_year_id)
    .bind(&req.description)
    .fetch_one(&pool)
    .await?;
    
    Ok((StatusCode::CREATED, Json(json!({"success": true, "data": version}))))
}

pub async fn update_study_plan_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
    Json(req): Json<UpdateStudyPlanVersionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;
    
    // Similar UPDATE logic as study_plan...
    let sql = "UPDATE study_plan_versions SET 
                version_name = COALESCE($1, version_name),
                start_academic_year_id = COALESCE($2, start_academic_year_id),
                end_academic_year_id = COALESCE($3, end_academic_year_id),
                description = COALESCE($4, description),
                is_active = COALESCE($5, is_active)
              WHERE id = $6 RETURNING *";
    
    let version = sqlx::query_as::<_, StudyPlanVersion>(sql)
        .bind(&req.version_name)
        .bind(&req.start_academic_year_id)
        .bind(&req.end_academic_year_id)
        .bind(&req.description)
        .bind(&req.is_active)
        .bind(version_id)
        .fetch_one(&pool)
        .await?;
    
    Ok(Json(json!({"success": true, "data": version})))
}

pub async fn delete_study_plan_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;
    
    sqlx::query("DELETE FROM study_plan_versions WHERE id = $1")
        .bind(version_id)
        .execute(&pool)
        .await?;
    
    Ok((StatusCode::OK, Json(json!({"success": true}))))
}

// ============================================
// Study Plan Subjects Management
// ============================================

pub async fn list_study_plan_subjects(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<StudyPlanSubjectQuery>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;
    
    let mut sql = String::from(
        "SELECT sps.*,
                s.name_th as subject_name_th,
                s.name_en as subject_name_en,
                s.credit as subject_credit,
                s.type as subject_type,
                CASE gl.level_type 
                    WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                    WHEN 'primary' THEN CONCAT('ป.', gl.year)
                    WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                    ELSE CONCAT('?.', gl.year)
                END as grade_level_name
         FROM study_plan_subjects sps
         LEFT JOIN subjects s ON s.id = sps.subject_id
         LEFT JOIN grade_levels gl ON gl.id = sps.grade_level_id
         WHERE 1=1"
    );
    
    let mut idx = 0u32;

    if let Some(_version_id) = query.study_plan_version_id {
        idx += 1;
        sql.push_str(&format!(" AND sps.study_plan_version_id = ${idx}"));
    }

    if let Some(_grade_id) = query.grade_level_id {
        idx += 1;
        sql.push_str(&format!(" AND sps.grade_level_id = ${idx}"));
    }

    if let Some(ref _term) = query.term {
        idx += 1;
        sql.push_str(&format!(" AND sps.term = ${idx}"));
    }

    sql.push_str(" ORDER BY sps.display_order, sps.subject_code");

    let mut q = sqlx::query_as::<_, StudyPlanSubject>(&sql);
    if let Some(version_id) = query.study_plan_version_id { q = q.bind(version_id); }
    if let Some(grade_id) = query.grade_level_id { q = q.bind(grade_id); }
    if let Some(ref term) = query.term { q = q.bind(term); }
    let subjects = q.fetch_all(&pool).await.unwrap_or_default();
    
    Ok(Json(json!({"success": true, "data": subjects})))
}

pub async fn add_subjects_to_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
    Json(req): Json<AddSubjectsToVersionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;
    
    let mut tx = pool.begin().await?;
    
    for subject in &req.subjects {
        // Get subject code
        let subject_code: (String,) = sqlx::query_as(
            "SELECT code FROM subjects WHERE id = $1"
        )
        .bind(subject.subject_id)
        .fetch_one(&mut *tx)
        .await?;
        
        sqlx::query(
            "INSERT INTO study_plan_subjects 
             (study_plan_version_id, grade_level_id, term, subject_id, subject_code, is_required, display_order)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (study_plan_version_id, grade_level_id, term, subject_id) DO NOTHING"
        )
        .bind(version_id)
        .bind(subject.grade_level_id)
        .bind(&subject.term)
        .bind(subject.subject_id)
        .bind(&subject_code.0)
        .bind(subject.is_required.unwrap_or(true))
        .bind(subject.display_order.unwrap_or(0))
        .execute(&mut *tx)
        .await?;
    }
    
    tx.commit().await?;
    
    Ok(Json(json!({
        "success": true,
        "message": "Subjects added successfully",
        "count": req.subjects.len()
    })))
}

pub async fn delete_study_plan_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subject_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;
    
    sqlx::query("DELETE FROM study_plan_subjects WHERE id = $1")
        .bind(subject_id)
        .execute(&pool)
        .await?;
    
    Ok((StatusCode::OK, Json(json!({"success": true}))))
}

// ============================================
// Bulk Operations: Generate Courses from Plan
// ============================================

pub async fn generate_courses_from_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<GenerateCoursesFromPlanRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;
    
    let mut tx = pool.begin().await?;
    
    // 1. Get classroom info (plan version, grade level)
    let classroom: (Option<Uuid>, Uuid) = sqlx::query_as(
        "SELECT study_plan_version_id, grade_level_id FROM class_rooms WHERE id = $1"
    )
    .bind(req.classroom_id)
    .fetch_one(&mut *tx)
    .await?;
    
    let plan_version_id = classroom.0.ok_or_else(|| {
        AppError::BadRequest("Classroom does not have a study plan assigned".to_string())
    })?;
    
    let grade_level_id = classroom.1;
    
    // 2. Get semester term and academic year
    let (semester_term, target_academic_year_id): (String, Uuid) = sqlx::query_as(
        "SELECT term, academic_year_id FROM academic_semesters WHERE id = $1"
    )
    .bind(req.academic_semester_id)
    .fetch_one(&mut *tx)
    .await?;

    // 3. Resolve subjects from plan for this grade + term, using effective-from versioning:
    //    for each subject_code in the plan, find the latest version where start_academic_year_id <= target year
    let plan_subjects: Vec<(Uuid, Option<Uuid>)> = sqlx::query_as(
        r#"
        SELECT DISTINCT ON (sps.subject_code) s.id, s.default_instructor_id
        FROM study_plan_subjects sps
        JOIN subjects s ON s.code = sps.subject_code
        JOIN academic_years ay ON ay.id = s.start_academic_year_id
        JOIN academic_years ay_target ON ay_target.id = $4
        WHERE sps.study_plan_version_id = $1
          AND sps.grade_level_id = $2
          AND sps.term = $3
          AND ay.year <= ay_target.year
        ORDER BY sps.subject_code, ay.year DESC
        "#
    )
    .bind(plan_version_id)
    .bind(grade_level_id)
    .bind(&semester_term)
    .bind(target_academic_year_id)
    .fetch_all(&mut *tx)
    .await?;

    let mut added = 0;
    let mut skipped = 0;

    for (subject_id, default_instructor_id) in plan_subjects {
        // Check if already exists
        if req.skip_existing.unwrap_or(true) {
            let exists: (bool,) = sqlx::query_as(
                "SELECT EXISTS(
                    SELECT 1 FROM classroom_courses 
                    WHERE classroom_id = $1 
                    AND subject_id = $2 
                    AND academic_semester_id = $3
                )"
            )
            .bind(req.classroom_id)
            .bind(subject_id)
            .bind(req.academic_semester_id)
            .fetch_one(&mut *tx)
            .await?;
            
            if exists.0 {
                skipped += 1;
                continue;
            }
        }
        
        // Insert, return new course id so we can populate the instructor junction
        let inserted: Option<(Uuid,)> = sqlx::query_as(
            "INSERT INTO classroom_courses
             (classroom_id, subject_id, academic_semester_id, settings, primary_instructor_id)
             VALUES ($1, $2, $3, '{}'::jsonb, $4)
             ON CONFLICT (classroom_id, subject_id, academic_semester_id) DO NOTHING
             RETURNING id"
        )
        .bind(req.classroom_id)
        .bind(subject_id)
        .bind(req.academic_semester_id)
        .bind(default_instructor_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
             eprintln!("Failed to generate course: {}", e);
             AppError::InternalServerError("Database error".to_string())
        })?;

        if let Some((course_id,)) = inserted {
            if let Some(instructor_id) = default_instructor_id {
                sqlx::query(
                    "INSERT INTO classroom_course_instructors (classroom_course_id, instructor_id, role)
                     VALUES ($1, $2, 'primary')
                     ON CONFLICT (classroom_course_id, instructor_id)
                     DO UPDATE SET role = 'primary'"
                )
                .bind(course_id)
                .bind(instructor_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    eprintln!("Failed to insert course instructor junction: {}", e);
                    AppError::InternalServerError("Database error".to_string())
                })?;

                sqlx::query(
                    "UPDATE classroom_course_instructors SET role = 'secondary'
                     WHERE classroom_course_id = $1 AND role = 'primary' AND instructor_id <> $2"
                )
                .bind(course_id)
                .bind(instructor_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    eprintln!("Failed to demote other primary instructors: {}", e);
                    AppError::InternalServerError("Database error".to_string())
                })?;
            }
        }

        added += 1;
    }
    
    tx.commit().await?;
    
    Ok(Json(json!({
        "success": true,
        "data": GenerateCoursesResponse {
            added_count: added,
            skipped_count: skipped,
            message: format!("Added {} courses, skipped {} existing courses", added, skipped),
        }
    })))
}
