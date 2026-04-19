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

    if query.active_only.unwrap_or(false) {
        sql.push_str(" AND is_active = true");
    }

    sql.push_str(" ORDER BY code");

    let q = sqlx::query_as::<_, StudyPlan>(&sql);
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
    
    let grade_ids = req.grade_level_ids
        .map(|ids| serde_json::to_value(ids).unwrap_or(serde_json::Value::Null));

    let plan = sqlx::query_as::<_, StudyPlan>(
        "INSERT INTO study_plans (code, name_th, name_en, description, grade_level_ids)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *"
    )
    .bind(&req.code)
    .bind(&req.name_th)
    .bind(&req.name_en)
    .bind(&req.description)
    .bind(&grade_ids)
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
    if req.grade_level_ids.is_some() {
        updates.push(format!("grade_level_ids = ${}", param_count));
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
    if let Some(ref grade_level_ids) = req.grade_level_ids {
        let grade_ids = serde_json::to_value(grade_level_ids).unwrap_or(serde_json::Value::Null);
        query = query.bind(grade_ids);
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
        "SELECT sps.id, sps.study_plan_version_id, sps.grade_level_id, sps.term,
                sps.subject_id,
                s.code as subject_code,
                sps.display_order, sps.metadata,
                sps.created_at, sps.updated_at,
                s.name_th as subject_name_th,
                s.name_en as subject_name_en,
                s.credit as subject_credit,
                s.type as subject_type,
                s.hours_per_semester as subject_hours,
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

    sql.push_str(" ORDER BY sps.display_order, s.code");

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
        sqlx::query(
            "INSERT INTO study_plan_subjects
             (study_plan_version_id, grade_level_id, term, subject_id, display_order)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (study_plan_version_id, grade_level_id, term, subject_id) DO NOTHING"
        )
        .bind(version_id)
        .bind(subject.grade_level_id)
        .bind(&subject.term)
        .bind(subject.subject_id)
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

    // 3. Resolve plan subjects + insert classroom_courses + copy team — all in one CTE:
    //    - plan_subjects: for each subject code in plan, pick the latest version effective by target year
    //    - inserted: INSERT classroom_courses with primary from subject_default_instructors
    //                (fallback subjects.default_instructor_id). ON CONFLICT DO NOTHING skips existing.
    //    - sec_copy: copy secondary defaults into classroom_course_instructors junction.
    //    Returns (total_plan_subjects, added_count) so we can report skipped = total - added.
    let counts: (i64, i64) = sqlx::query_as(
        r#"
        WITH plan_subjects AS (
            SELECT DISTINCT ON (original.code) s.id AS subject_id, s.default_instructor_id
            FROM study_plan_subjects sps
            JOIN subjects original ON original.id = sps.subject_id
            JOIN subjects s ON s.code = original.code
            JOIN academic_years ay ON ay.id = s.start_academic_year_id
            JOIN academic_years ay_target ON ay_target.id = $4
            WHERE sps.study_plan_version_id = $1
              AND sps.grade_level_id = $2
              AND sps.term = $3
              AND ay.year <= ay_target.year
            ORDER BY original.code, ay.year DESC
        ),
        inserted AS (
            INSERT INTO classroom_courses
                (classroom_id, subject_id, academic_semester_id, settings, primary_instructor_id)
            SELECT $5, ps.subject_id, $6, '{}'::jsonb,
                COALESCE(
                    (SELECT sdi.instructor_id FROM subject_default_instructors sdi
                     WHERE sdi.subject_id = ps.subject_id AND sdi.role = 'primary' LIMIT 1),
                    ps.default_instructor_id
                )
            FROM plan_subjects ps
            ON CONFLICT (classroom_id, subject_id, academic_semester_id) DO NOTHING
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
        SELECT
            (SELECT COUNT(*) FROM plan_subjects) AS total,
            (SELECT COUNT(*) FROM inserted) AS added
        "#
    )
    .bind(plan_version_id)
    .bind(grade_level_id)
    .bind(&semester_term)
    .bind(target_academic_year_id)
    .bind(req.classroom_id)
    .bind(req.academic_semester_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("generate_courses_from_plan failed: {}", e);
        AppError::InternalServerError("Failed to generate courses".to_string())
    })?;

    let added = counts.1 as i32;
    let skipped = (counts.0 - counts.1) as i32;
    let _ = req.skip_existing; // flag retained for API compat; ON CONFLICT always skips

    // ============================================
    // Also generate activity_slots from plan's activities for the same semester
    // ============================================
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();

    let plan_acts: Vec<(Uuid, Option<serde_json::Value>, String, Option<String>, String, i32, String)> = sqlx::query_as(
        r#"SELECT sva.id,
                  sva.allowed_grade_level_ids,
                  ac.name,
                  ac.description,
                  ac.activity_type,
                  ac.periods_per_week,
                  ac.scheduling_mode
           FROM study_plan_version_activities sva
           JOIN activity_catalog ac ON ac.id = sva.activity_catalog_id
           WHERE sva.study_plan_version_id = $1"#
    )
    .bind(plan_version_id)
    .fetch_all(&mut *tx)
    .await
    .unwrap_or_default();

    let mut activities_created = 0i32;
    let mut activities_skipped = 0i32;

    for (sva_id, allowed, name, description, activity_type, periods_per_week, scheduling_mode) in &plan_acts {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM activity_slots WHERE source_plan_activity_id = $1 AND semester_id = $2)"
        )
        .bind(sva_id)
        .bind(req.academic_semester_id)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(false);

        if exists {
            activities_skipped += 1;
            continue;
        }

        let res = sqlx::query(
            r#"INSERT INTO activity_slots
                (name, description, activity_type, semester_id, allowed_grade_level_ids,
                 registration_type, periods_per_week, scheduling_mode,
                 source_plan_activity_id, created_by)
               VALUES ($1, $2, $3, $4, $5,
                       'assigned', $6, $7,
                       $8, $9)"#
        )
        .bind(name)
        .bind(description)
        .bind(activity_type)
        .bind(req.academic_semester_id)
        .bind(allowed)
        .bind(periods_per_week)
        .bind(scheduling_mode)
        .bind(sva_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await;

        if res.is_ok() {
            activities_created += 1;
        }
    }

    tx.commit().await?;

    Ok(Json(json!({
        "success": true,
        "courses_created": added,
        "courses_skipped": skipped,
        "activities_created": activities_created,
        "activities_skipped": activities_skipped,
        "data": GenerateCoursesResponse {
            added_count: added,
            skipped_count: skipped,
            message: format!(
                "Added {} courses, skipped {} existing courses; Added {} activities, skipped {}",
                added, skipped, activities_created, activities_skipped
            ),
        }
    })))
}

// ============================================
// Study Plan Version Activities CRUD
// ============================================

/// GET /api/academic/study-plan-versions/:id/activities
pub async fn list_plan_activities(
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

    let rows: Vec<StudyPlanVersionActivity> = sqlx::query_as(
        "SELECT sva.*,
                ac.name AS catalog_name,
                ac.activity_type AS catalog_activity_type,
                ac.description AS catalog_description,
                ac.periods_per_week AS catalog_periods_per_week,
                ac.scheduling_mode AS catalog_scheduling_mode,
                ac.term AS catalog_term,
                ac.grade_level_ids AS catalog_grade_level_ids
         FROM study_plan_version_activities sva
         JOIN activity_catalog ac ON ac.id = sva.activity_catalog_id
         WHERE sva.study_plan_version_id = $1
         ORDER BY sva.display_order, ac.name"
    )
    .bind(version_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "success": true, "data": rows })))
}

/// POST /api/academic/study-plan-versions/:id/activities
pub async fn add_plan_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
    Json(req): Json<CreatePlanActivityRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    let allowed = req.allowed_grade_level_ids
        .map(|ids| serde_json::to_value(ids).unwrap_or(serde_json::Value::Null));

    let row: StudyPlanVersionActivity = sqlx::query_as(
        r#"INSERT INTO study_plan_version_activities
            (study_plan_version_id, activity_catalog_id, allowed_grade_level_ids,
             display_order)
           VALUES ($1, $2, $3,
                   COALESCE($4, 0))
           RETURNING *"#
    )
    .bind(version_id)
    .bind(req.activity_catalog_id)
    .bind(&allowed)
    .bind(req.display_order)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": row }))))
}

/// PUT /api/academic/study-plan-activities/:id
pub async fn update_plan_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePlanActivityRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    let allowed = req.allowed_grade_level_ids.as_ref()
        .map(|ids| serde_json::to_value(ids).unwrap_or(serde_json::Value::Null));

    let row: StudyPlanVersionActivity = sqlx::query_as(
        r#"UPDATE study_plan_version_activities SET
            allowed_grade_level_ids = COALESCE($2, allowed_grade_level_ids),
            display_order = COALESCE($3, display_order),
            updated_at = NOW()
           WHERE id = $1
           RETURNING *"#
    )
    .bind(id)
    .bind(&allowed)
    .bind(req.display_order)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::NotFound(e.to_string()))?;

    Ok(Json(json!({ "success": true, "data": row })))
}

/// DELETE /api/academic/study-plan-activities/:id
pub async fn delete_plan_activity(
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

    sqlx::query("DELETE FROM study_plan_version_activities WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "success": true })))
}

/// POST /api/academic/activities/generate-from-plan
/// Body: { study_plan_version_id, semester_id }
pub async fn generate_activities_from_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<GenerateActivitiesFromPlanRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool).await.ok();

    // Fetch all template activities for this plan version, joined with catalog data
    let templates: Vec<StudyPlanVersionActivity> = sqlx::query_as(
        "SELECT sva.*,
                ac.name AS catalog_name,
                ac.activity_type AS catalog_activity_type,
                ac.description AS catalog_description,
                ac.periods_per_week AS catalog_periods_per_week,
                ac.scheduling_mode AS catalog_scheduling_mode,
                ac.term AS catalog_term,
                ac.grade_level_ids AS catalog_grade_level_ids
         FROM study_plan_version_activities sva
         JOIN activity_catalog ac ON ac.id = sva.activity_catalog_id
         WHERE sva.study_plan_version_id = $1
         ORDER BY sva.display_order, ac.name"
    )
    .bind(req.study_plan_version_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let mut created = 0i32;
    let mut skipped = 0i32;

    for tpl in &templates {
        // Skip if activity_slot with same source_plan_activity_id already exists in this semester
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM activity_slots
                WHERE source_plan_activity_id = $1 AND semester_id = $2
            )"
        )
        .bind(tpl.id)
        .bind(req.semester_id)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(false);

        if exists {
            skipped += 1;
            continue;
        }

        // Pull activity fields from joined catalog (required — activity_catalog_id is NOT NULL)
        let catalog_name = tpl.catalog_name.as_deref().unwrap_or("");
        let catalog_activity_type = tpl.catalog_activity_type.as_deref().unwrap_or("other");
        let catalog_periods_per_week = tpl.catalog_periods_per_week.unwrap_or(1);
        let catalog_scheduling_mode = tpl.catalog_scheduling_mode.as_deref().unwrap_or("synchronized");

        sqlx::query(
            r#"INSERT INTO activity_slots
                (name, description, activity_type, semester_id, allowed_grade_level_ids,
                 registration_type, periods_per_week, scheduling_mode,
                 source_plan_activity_id, created_by)
               VALUES ($1, $2, $3, $4, $5,
                       'assigned', $6, $7,
                       $8, $9)"#
        )
        .bind(catalog_name)
        .bind(&tpl.catalog_description)
        .bind(catalog_activity_type)
        .bind(req.semester_id)
        .bind(&tpl.allowed_grade_level_ids)
        .bind(catalog_periods_per_week)
        .bind(catalog_scheduling_mode)
        .bind(tpl.id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        created += 1;
    }

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "created": created,
        "skipped": skipped,
        "total_templates": templates.len()
    })))
}

// ============================================
// Activity Catalog CRUD
// ============================================

/// GET /api/academic/activity-catalog
pub async fn list_activity_catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    let rows: Vec<ActivityCatalog> = sqlx::query_as(
        "SELECT * FROM activity_catalog WHERE is_active = true ORDER BY activity_type, name"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(json!({ "success": true, "data": rows })))
}

/// POST /api/academic/activity-catalog
pub async fn create_activity_catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateCatalogRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    let allowed = req.grade_level_ids
        .map(|ids| serde_json::to_value(ids).unwrap_or(serde_json::Value::Null));

    let row: ActivityCatalog = sqlx::query_as(
        r#"INSERT INTO activity_catalog (name, activity_type, description, periods_per_week, scheduling_mode, term, grade_level_ids)
           VALUES ($1, $2, $3, COALESCE($4, 1), COALESCE($5, 'synchronized'), $6, $7)
           RETURNING *"#
    )
    .bind(&req.name)
    .bind(&req.activity_type)
    .bind(&req.description)
    .bind(req.periods_per_week)
    .bind(&req.scheduling_mode)
    .bind(&req.term)
    .bind(&allowed)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::BadRequest(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": row }))))
}

/// PUT /api/academic/activity-catalog/:id
pub async fn update_activity_catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateCatalogRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    let allowed = req.grade_level_ids.as_ref()
        .map(|ids| serde_json::to_value(ids).unwrap_or(serde_json::Value::Null));

    let row: ActivityCatalog = sqlx::query_as(
        r#"UPDATE activity_catalog SET
            name = COALESCE($2, name),
            activity_type = COALESCE($3, activity_type),
            description = COALESCE($4, description),
            periods_per_week = COALESCE($5, periods_per_week),
            scheduling_mode = COALESCE($6, scheduling_mode),
            is_active = COALESCE($7, is_active),
            term = COALESCE($8, term),
            grade_level_ids = COALESCE($9, grade_level_ids),
            updated_at = NOW()
           WHERE id = $1
           RETURNING *"#
    )
    .bind(id)
    .bind(&req.name)
    .bind(&req.activity_type)
    .bind(&req.description)
    .bind(req.periods_per_week)
    .bind(&req.scheduling_mode)
    .bind(req.is_active)
    .bind(&req.term)
    .bind(&allowed)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::NotFound(e.to_string()))?;

    Ok(Json(json!({ "success": true, "data": row })))
}

/// DELETE /api/academic/activity-catalog/:id
pub async fn delete_activity_catalog(
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

    sqlx::query("DELETE FROM activity_catalog WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::BadRequest(
            if e.to_string().contains("foreign key") {
                "ไม่สามารถลบได้ มีหลักสูตรที่ใช้กิจกรรมนี้อยู่".to_string()
            } else {
                e.to_string()
            }
        ))?;

    Ok(Json(json!({ "success": true })))
}
