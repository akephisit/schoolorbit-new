use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
    response::IntoResponse,
    http::HeaderMap,
};
use serde_json::json;
use crate::middleware::permission::{check_permission, get_user_with_permissions};
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
    
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
        
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

/// Get the subject_group_id for a teacher based on their primary กลุ่มสาระ department membership.
/// Returns None if the teacher is not a member of any กลุ่มสาระ department.
async fn get_user_subject_group_id(user_id: Uuid, pool: &sqlx::PgPool) -> Option<Uuid> {
    sqlx::query_scalar(
        r#"
        SELECT d.subject_group_id
        FROM department_members dm
        JOIN departments d ON d.id = dm.department_id
        WHERE dm.user_id = $1
          AND d.subject_group_id IS NOT NULL
          AND (dm.ended_at IS NULL OR dm.ended_at > CURRENT_DATE)
        ORDER BY dm.is_primary_department DESC NULLS LAST
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

/// List all subject groups (Learning Areas)
pub async fn list_subject_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    // Accept: read.all (curriculum admin) OR manage.department (กลุ่มสาระ teacher)
    let (_, permissions) = match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
        Ok(r) => r,
        Err(resp) => return Ok(resp),
    };
    let has_access = permissions.contains(&"*".to_string())
        || permissions.contains(&codes::ACADEMIC_CURRICULUM_READ_ALL.to_string())
        || permissions.contains(&codes::ACADEMIC_CURRICULUM_MANAGE_DEPT.to_string());
    if !has_access {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": format!("ไม่มีสิทธิ์ {}", codes::ACADEMIC_CURRICULUM_READ_ALL) })),
        ).into_response());
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

    // Accept: read.all OR manage.department
    let (user_id, permissions) = match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
        Ok(r) => r,
        Err(resp) => return Ok(resp),
    };
    let has_all = permissions.contains(&"*".to_string())
        || permissions.contains(&codes::ACADEMIC_CURRICULUM_READ_ALL.to_string());
    let has_dept = permissions.contains(&codes::ACADEMIC_CURRICULUM_MANAGE_DEPT.to_string());
    if !has_all && !has_dept {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": format!("ไม่มีสิทธิ์ {}", codes::ACADEMIC_CURRICULUM_READ_ALL) })),
        ).into_response());
    }

    // dept-scope: auto-filter to teacher's กลุ่มสาระ, ignore any group_id from query
    let dept_group_id: Option<Uuid> = if !has_all && has_dept {
        match get_user_subject_group_id(user_id, &pool).await {
            Some(gid) => Some(gid),
            None => {
                return Ok((
                    StatusCode::FORBIDDEN,
                    Json(json!({ "success": false, "error": "ไม่พบกลุ่มสาระที่สังกัด" })),
                ).into_response());
            }
        }
    } else {
        None
    };

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

    // dept-scope overrides any user-supplied group_id filter
    if let Some(gid) = dept_group_id {
        query.push_str(&format!(" AND s.group_id = '{}'", gid));
    } else if let Some(gid) = filter.group_id {
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

    if let Some(year_id) = filter.start_academic_year_id {
        query.push_str(&format!(" AND s.start_academic_year_id = '{}'", year_id));
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

    // 1. Check Permission (create.all OR manage.department)
    let (user_id, permissions) = match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
        Ok(r) => r,
        Err(resp) => return Ok(resp),
    };
    let has_all = permissions.contains(&"*".to_string())
        || permissions.contains(&codes::ACADEMIC_CURRICULUM_CREATE_ALL.to_string());
    let has_dept = permissions.contains(&codes::ACADEMIC_CURRICULUM_MANAGE_DEPT.to_string());
    if !has_all && !has_dept {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": format!("ไม่มีสิทธิ์ {}", codes::ACADEMIC_CURRICULUM_CREATE_ALL) })),
        ).into_response());
    }

    // dept-scope: validate that the subject's group matches the teacher's กลุ่มสาระ
    if !has_all && has_dept {
        let teacher_group = get_user_subject_group_id(user_id, &pool).await
            .ok_or_else(|| AppError::BadRequest("ไม่พบกลุ่มสาระที่สังกัด".to_string()))?;
        if payload.group_id != Some(teacher_group) {
            return Err(AppError::BadRequest("ไม่สามารถเพิ่มวิชาในกลุ่มสาระอื่นได้".to_string()));
        }
    }

    // 2. Validate Code + Year Uniqueness (same code can exist in different start years)
    let exists: Option<bool> = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM subjects WHERE code = $1 AND start_academic_year_id = $2)"
    )
    .bind(&payload.code)
    .bind(payload.start_academic_year_id)
    .fetch_one(&pool)
    .await
    .unwrap_or(Some(false));

    if exists.unwrap_or(false) {
        let year_name: Option<String> = sqlx::query_scalar(
            "SELECT name FROM academic_years WHERE id = $1"
        )
        .bind(payload.start_academic_year_id)
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
            code, name_th, name_en,
            credit, hours_per_semester, type, group_id, level_scope, description,
            start_academic_year_id, term, default_instructor_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING *
        "#
    )
    .bind(&payload.code)
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

    // 1. Check Permission (update.all OR manage.department)
    let (user_id, permissions) = match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
        Ok(r) => r,
        Err(resp) => return Ok(resp),
    };
    let has_all = permissions.contains(&"*".to_string())
        || permissions.contains(&codes::ACADEMIC_CURRICULUM_UPDATE_ALL.to_string());
    let has_dept = permissions.contains(&codes::ACADEMIC_CURRICULUM_MANAGE_DEPT.to_string());
    if !has_all && !has_dept {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": format!("ไม่มีสิทธิ์ {}", codes::ACADEMIC_CURRICULUM_UPDATE_ALL) })),
        ).into_response());
    }

    // dept-scope: verify subject belongs to teacher's กลุ่มสาระ before updating
    if !has_all && has_dept {
        let teacher_group = get_user_subject_group_id(user_id, &pool).await
            .ok_or_else(|| AppError::BadRequest("ไม่พบกลุ่มสาระที่สังกัด".to_string()))?;
        let subject_group: Option<Uuid> = sqlx::query_scalar(
            "SELECT group_id FROM subjects WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError("Failed to fetch subject".to_string()))?
        .flatten();
        if subject_group != Some(teacher_group) {
            return Err(AppError::BadRequest("ไม่สามารถแก้ไขวิชาในกลุ่มสาระอื่นได้".to_string()));
        }
    }

    let mut tx = pool.begin().await.map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;
    
    // 2. Update
    let mut subject = sqlx::query_as::<_, Subject>(
        r#"
        UPDATE subjects SET
            code = COALESCE($1, code),
            name_th = COALESCE($2, name_th),
            name_en = COALESCE($3, name_en),
            credit = COALESCE($4, credit),
            hours_per_semester = COALESCE($5, hours_per_semester),
            type = COALESCE($6, type),
            group_id = COALESCE($7, group_id),
            level_scope = COALESCE($8, level_scope),
            description = COALESCE($9, description),
            is_active = COALESCE($10, is_active),
            start_academic_year_id = COALESCE($11, start_academic_year_id),
            term = COALESCE($13, term),
            default_instructor_id = COALESCE($14, default_instructor_id),
            updated_at = NOW()
        WHERE id = $12
        RETURNING *
        "#
    )
    .bind(&payload.code)
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

    // 1. Check Permission (delete.all OR manage.department)
    let (user_id, permissions) = match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
        Ok(r) => r,
        Err(resp) => return Ok(resp),
    };
    let has_all = permissions.contains(&"*".to_string())
        || permissions.contains(&codes::ACADEMIC_CURRICULUM_DELETE_ALL.to_string());
    let has_dept = permissions.contains(&codes::ACADEMIC_CURRICULUM_MANAGE_DEPT.to_string());
    if !has_all && !has_dept {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": format!("ไม่มีสิทธิ์ {}", codes::ACADEMIC_CURRICULUM_DELETE_ALL) })),
        ).into_response());
    }

    // dept-scope: verify subject belongs to teacher's กลุ่มสาระ before deleting
    if !has_all && has_dept {
        let teacher_group = get_user_subject_group_id(user_id, &pool).await
            .ok_or_else(|| AppError::BadRequest("ไม่พบกลุ่มสาระที่สังกัด".to_string()))?;
        let subject_group: Option<Uuid> = sqlx::query_scalar(
            "SELECT group_id FROM subjects WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError("Failed to fetch subject".to_string()))?
        .flatten();
        if subject_group != Some(teacher_group) {
            return Err(AppError::BadRequest("ไม่สามารถลบวิชาในกลุ่มสาระอื่นได้".to_string()));
        }
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
    if let Err(response) = check_permission(&headers, &pool, codes::ACADEMIC_CURRICULUM_CREATE_ALL, &state.permission_cache).await {
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
        "SELECT * FROM subjects WHERE start_academic_year_id = $1 AND is_active = true"
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
        // Check if code already exists for target start year
        let exists: Option<bool> = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM subjects WHERE code = $1 AND start_academic_year_id = $2)"
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

        // Insert new subject version effective from target year
        let _result = sqlx::query(
            r#"
            INSERT INTO subjects (
                code, name_th, name_en,
                credit, hours_per_semester, type, group_id, level_scope, description,
                start_academic_year_id, term, default_instructor_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#
        )
        .bind(&subject.code)
        .bind(&subject.name_th)
        .bind(&subject.name_en)
        .bind(subject.credit)
        .bind(subject.hours_per_semester)
        .bind(&subject.subject_type)
        .bind(subject.group_id)
        .bind(&subject.level_scope)
        .bind(&subject.description)
        .bind(payload.target_academic_year_id)
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
