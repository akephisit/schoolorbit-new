use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
    response::IntoResponse,
    http::HeaderMap,
};
use serde_json::json;
use crate::middleware::permission::check_permission;
use crate::modules::academic::models::curriculum::{
    Subject, SubjectGroup, CreateSubjectRequest, UpdateSubjectRequest, SubjectFilter
};
use uuid::Uuid;
use crate::permissions::registry::codes;
use crate::AppState;
use crate::error::AppError;
use crate::db::school_mapping::get_school_database_url;
use crate::utils::subdomain::extract_subdomain_from_request;

/// Helper function to get DB pool
async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    
    let db_url = get_school_database_url(&state.admin_pool, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
        
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

/// List all subject groups (Learning Areas)
pub async fn list_subject_groups(
    State(state): State<AppState>,
    headers: HeaderMap, 
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    // Check READ permission
    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_CURRICULUM_READ_ALL).await {
         return Ok(response);
    }

    let groups = sqlx::query_as::<_, SubjectGroup>(
        "SELECT * FROM subject_groups WHERE is_active = true ORDER BY display_order ASC"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch subject groups: {}", e);
        AppError::InternalServerError("Failed to fetch subject groups".to_string())
    })?;

    Ok(Json(json!({ "success": true, "data": groups })).into_response())
}

/// List subjects with filtering
pub async fn list_subjects(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<SubjectFilter>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    // Check READ permission
    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_CURRICULUM_READ_ALL).await {
        return Ok(response);
    }

    let mut query = String::from(
        r#"
        SELECT s.*, sg.name_th as group_name_th,
               (SELECT COALESCE(array_agg(sgl.grade_level_id), '{}') 
                FROM subject_grade_levels sgl 
                WHERE sgl.subject_id = s.id) as grade_level_ids,
               concat(u.first_name, ' ', u.last_name) as default_instructor_name
        FROM subjects s
        LEFT JOIN subject_groups sg ON s.group_id = sg.id
        LEFT JOIN users u ON s.default_instructor_id = u.id
        WHERE 1=1
        "#
    );

    // Apply Filters
    if let Some(active) = filter.active_only {
        if active {
            query.push_str(" AND s.is_active = true");
        }
    }

    if let Some(gid) = filter.group_id {
        query.push_str(&format!(" AND s.group_id = '{}'", gid));
    }

    if let Some(scope) = &filter.level_scope {
        query.push_str(&format!(" AND s.level_scope = '{}'", scope));
    }
    
    if let Some(stype) = &filter.subject_type {
        query.push_str(&format!(" AND s.type = '{}'", stype));
    }

    if let Some(search) = &filter.search {
        if !search.is_empty() {
            query.push_str(&format!(
                " AND (s.code ILIKE '%{}%' OR s.name_th ILIKE '%{}%' OR s.name_en ILIKE '%{}%')",
                search, search, search
            ));
        }
    }

    if let Some(year_id) = filter.academic_year_id {
        query.push_str(&format!(" AND s.academic_year_id = '{}'", year_id));
    }

    if let Some(term) = &filter.term {
        query.push_str(&format!(" AND (s.term = '{}' OR s.term IS NULL)", term));
    }

    query.push_str(" ORDER BY s.code ASC");

    let subjects = sqlx::query_as::<_, Subject>(&query)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to fetch subjects: {}", e);
            AppError::InternalServerError("Failed to fetch subjects".to_string())
        })?;

    Ok(Json(json!({ "success": true, "data": subjects })).into_response())
}

/// Create a new subject
pub async fn create_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateSubjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    // 1. Check Permission
    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_CURRICULUM_CREATE_ALL).await {
        return Ok(response);
    }

    // 2. Validate Code + Year Uniqueness (same code can exist in different years)
    let exists: Option<bool> = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM subjects WHERE code = $1 AND academic_year_id = $2)"
    )
    .bind(&payload.code)
    .bind(payload.academic_year_id)
    .fetch_one(&pool)
    .await
    .unwrap_or(Some(false));

    if exists.unwrap_or(false) {
        // Get year info for better error message
        let year_name: Option<String> = sqlx::query_scalar(
            "SELECT name FROM academic_years WHERE id = $1"
        )
        .bind(payload.academic_year_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);
        
        return Err(AppError::BadRequest(format!(
            "รหัสวิชา {} {} มีอยู่ในระบบแล้ว",
            payload.code,
            year_name.unwrap_or_else(|| "ในปีการศึกษานี้".to_string())
        )));
    }

    let mut tx = pool.begin().await.map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    // 3. Insert Subject
    let mut subject = sqlx::query_as::<_, Subject>(
        r#"
        INSERT INTO subjects (
            code, academic_year_id, name_th, name_en, 
            credit, hours_per_semester, type, group_id, level_scope, description, start_academic_year_id,
            term, default_instructor_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        RETURNING *
        "#
    )
    .bind(&payload.code)
    .bind(payload.academic_year_id)
    .bind(&payload.name_th)
    .bind(&payload.name_en)
    .bind(payload.credit.unwrap_or(0.0)) 
    .bind(payload.hours_per_semester)
    .bind(&payload.subject_type)
    .bind(payload.group_id)
    .bind(&payload.level_scope)
    .bind(&payload.description)
    .bind(payload.start_academic_year_id)
    .bind(&payload.term)
    .bind(payload.default_instructor_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("Failed to create subject: {}", e);
        AppError::InternalServerError("Failed to create subject".to_string())
    })?;

    // 4. Insert Grade Level Relations
    if let Some(level_ids) = &payload.grade_level_ids {
        for lid in level_ids {
            sqlx::query("INSERT INTO subject_grade_levels (subject_id, grade_level_id) VALUES ($1, $2)")
                .bind(subject.id)
                .bind(lid)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    eprintln!("Failed to link grade level: {}", e);
                    AppError::InternalServerError("Failed to save grade level links".to_string())
                })?;
        }
        // Update response object
        subject.grade_level_ids = Some(level_ids.clone());
    }

    tx.commit().await.map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": subject }))).into_response())
}

/// Update a subject
pub async fn update_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSubjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    // 1. Check Permission
    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_CURRICULUM_UPDATE_ALL).await {
        return Ok(response);
    }

    let mut tx = pool.begin().await.map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;
    
    // 2. Update
    let mut subject = sqlx::query_as::<_, Subject>(
        r#"
        UPDATE subjects SET 
            code = COALESCE($1, code),
            academic_year_id = COALESCE($2, academic_year_id),
            name_th = COALESCE($3, name_th),
            name_en = COALESCE($4, name_en),
            credit = COALESCE($5, credit),
            hours_per_semester = COALESCE($6, hours_per_semester),
            type = COALESCE($7, type),
            group_id = COALESCE($8, group_id),
            level_scope = COALESCE($9, level_scope),
            description = COALESCE($10, description),
            is_active = COALESCE($11, is_active),
            start_academic_year_id = COALESCE($12, start_academic_year_id),
            term = COALESCE($14, term),
            default_instructor_id = COALESCE($15, default_instructor_id),
            updated_at = NOW()
        WHERE id = $13
        RETURNING *
        "#
    )
    .bind(&payload.code)
    .bind(payload.academic_year_id)
    .bind(&payload.name_th)
    .bind(&payload.name_en)
    .bind(payload.credit)
    .bind(payload.hours_per_semester)
    .bind(&payload.subject_type)
    .bind(payload.group_id)
    .bind(&payload.level_scope)
    .bind(&payload.description)
    .bind(payload.is_active)
    .bind(payload.start_academic_year_id)
    .bind(id)
    .bind(&payload.term)
    .bind(payload.default_instructor_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("Failed to update subject {}: {}", id, e);
        AppError::InternalServerError("Failed to update subject".to_string())
    })?;

    // 3. Update Grade Levels if provided
    if let Some(level_ids) = &payload.grade_level_ids {
        // Clear existing
        sqlx::query("DELETE FROM subject_grade_levels WHERE subject_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to clear links: {}", e)))?;
            
        // Insert new
        for lid in level_ids {
            sqlx::query("INSERT INTO subject_grade_levels (subject_id, grade_level_id) VALUES ($1, $2)")
                .bind(id)
                .bind(lid)
                .execute(&mut *tx)
                .await
                .map_err(|e| AppError::InternalServerError(format!("Failed to link grade level: {}", e)))?;
        }
        
        subject.grade_level_ids = Some(level_ids.clone());
    }

    tx.commit().await.map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(Json(json!({ "success": true, "data": subject })).into_response())
}

/// Delete subject
pub async fn delete_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    // 1. Check Permission
    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_CURRICULUM_DELETE_ALL).await {
        return Ok(response);
    }

    sqlx::query("DELETE FROM subjects WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to delete subject {}: {}", id, e);
            AppError::BadRequest("ไม่สามารถลบรายวิชาได้ (อาจมีการใช้งานอยู่)".to_string())
        })?;

    Ok(Json(json!({ "success": true })).into_response())
}

/// POST /api/academic/subjects/bulk-copy
/// Copy all subjects from one academic year to another
pub async fn bulk_copy_subjects(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<crate::modules::academic::models::curriculum::BulkCopySubjectsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    // 1. Check Permission
    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_CURRICULUM_CREATE_ALL).await {
        return Ok(response);
    }

    // 1.5 Validate Years (Source must be older than Target)
    let source_year: Option<i32> = sqlx::query_scalar("SELECT year FROM academic_years WHERE id = $1")
        .bind(payload.source_academic_year_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    
    let target_year: Option<i32> = sqlx::query_scalar("SELECT year FROM academic_years WHERE id = $1")
        .bind(payload.target_academic_year_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let source_year = source_year.ok_or_else(|| AppError::BadRequest("ไม่พบปีการศึกษาต้นทาง".to_string()))?;
    let target_year = target_year.ok_or_else(|| AppError::BadRequest("ไม่พบปีการศึกษาปลายทาง".to_string()))?;

    if source_year >= target_year {
        return Err(AppError::BadRequest("ไม่สามารถคัดลอกรายวิชาจากปีที่ใหม่กว่าหรือปีเดียวกันได้".to_string()));
    }

    // 2. Fetch all subjects from source year
    let source_subjects = sqlx::query_as::<_, Subject>(
        "SELECT * FROM subjects WHERE academic_year_id = $1 AND is_active = true"
    )
    .bind(payload.source_academic_year_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch source subjects: {}", e);
        AppError::InternalServerError("Failed to fetch source subjects".to_string())
    })?;

    if source_subjects.is_empty() {
        return Ok((
            StatusCode::OK,
            Json(json!({
                "success": true,
                "data": {
                    "copied_count": 0,
                    "skipped_count": 0,
                    "message": "ไม่พบรายวิชาในปีต้นทาง"
                }
            }))
        ).into_response());
    }

    // 3. Copy subjects one by one (skip if code already exists in target year)
    let mut copied_count = 0;
    let mut skipped_count = 0;

    for subject in source_subjects {
        // Check if code already exists in target year
        let exists: Option<bool> = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM subjects WHERE code = $1 AND academic_year_id = $2)"
        )
        .bind(&subject.code)
        .bind(payload.target_academic_year_id)
        .fetch_one(&pool)
        .await
        .unwrap_or(Some(false));

        if exists.unwrap_or(false) {
            skipped_count += 1;
            continue;
        }

        // Insert new subject with target year
        let _result = sqlx::query(
            r#"
            INSERT INTO subjects (
                code, academic_year_id, name_th, name_en,
                credit, hours_per_semester, type, group_id, level_scope, description,
                start_academic_year_id, term, default_instructor_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#
        )
        .bind(&subject.code)
        .bind(payload.target_academic_year_id)
        .bind(&subject.name_th)
        .bind(&subject.name_en)
        .bind(subject.credit)
        .bind(subject.hours_per_semester)
        .bind(&subject.subject_type)
        .bind(subject.group_id)
        .bind(&subject.level_scope)
        .bind(&subject.description)
        .bind(subject.start_academic_year_id)
        .bind(&subject.term)
        .bind(subject.default_instructor_id)
        .execute(&pool)
        .await;

        match _result {
            Ok(_) => copied_count += 1,
            Err(_) => skipped_count += 1,
        }
    }

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": {
                "copied_count": copied_count,
                "skipped_count": skipped_count,
                "message": format!("คัดลอกสำเร็จ {} รายวิชา (ข้าม {} รายการ)", copied_count, skipped_count)
            }
        }))
    ).into_response())
}
