use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
    response::IntoResponse,
};
use serde_json::json;
use crate::AppState;
use crate::middleware::permission::check_permission;
use crate::modules::academic::models::curriculum::{
    Subject, SubjectGroup, CreateSubjectRequest, UpdateSubjectRequest, SubjectFilter
};
use uuid::Uuid;

use crate::permissions::registry::codes;

/// List all subject groups (Learning Areas)
pub async fn list_subject_groups(
    headers: axum::http::HeaderMap, // Need headers for permission check? Usually yes for protected routes
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Check READ permission
    if let Err(e) = check_permission(&headers, &state.pool, codes::ACADEMIC_CURRICULUM_READ_ALL).await {
        return e;
    }

    let groups = sqlx::query_as::<_, SubjectGroup>(
        "SELECT * FROM subject_groups WHERE is_active = true ORDER BY display_order ASC"
    )
    .fetch_all(&state.pool)
    .await;

    match groups {
        Ok(data) => (StatusCode::OK, Json(json!({ "success": true, "data": data }))).into_response(),
        Err(e) => {
            eprintln!("Failed to fetch subject groups: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "Failed to fetch subject groups" })),
            ).into_response()
        }
    }
}

/// List subjects with filtering
pub async fn list_subjects(
    headers: axum::http::HeaderMap,
    State(state): State<AppState>,
    Query(filter): Query<SubjectFilter>,
) -> impl IntoResponse {
    // Check READ permission
    if let Err(e) = check_permission(&headers, &state.pool, codes::ACADEMIC_CURRICULUM_READ_ALL).await {
        return e;
    }

    let mut query = String::from(
        r#"
        SELECT s.*, sg.name_th as group_name_th 
        FROM subjects s
        LEFT JOIN subject_groups sg ON s.group_id = sg.id
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

    query.push_str(" ORDER BY s.code ASC");

    let subjects = sqlx::query_as::<_, Subject>(&query)
        .fetch_all(&state.pool)
        .await;

    match subjects {
        Ok(data) => (StatusCode::OK, Json(json!({ "success": true, "data": data }))).into_response(),
        Err(e) => {
            eprintln!("Failed to fetch subjects: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "Failed to fetch subjects" })),
            ).into_response()
        }
    }
}

/// Create a new subject
pub async fn create_subject(
    headers: axum::http::HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<CreateSubjectRequest>,
) -> impl IntoResponse {
    // 1. Check Permission
    if let Err(e) = check_permission(&headers, &state.pool, codes::ACADEMIC_CURRICULUM_CREATE_ALL).await {
        return e;
    }

    // 2. Validate Code Uniqueness
    let exists: Option<bool> = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM subjects WHERE code = $1)"
    )
    .bind(&payload.code)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(Some(false));

    if exists.unwrap_or(false) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "รหัสวิชานี้มีอยู่ในระบบแล้ว" })),
        ).into_response();
    }

    // 3. Insert
    let result = sqlx::query_as::<_, Subject>(
        r#"
        INSERT INTO subjects (
            code, academic_year_start, name_th, name_en, 
            credit, hours_per_semester, type, group_id, level_scope, description
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING *
        "#
    )
    .bind(&payload.code)
    .bind(payload.academic_year_start)
    .bind(&payload.name_th)
    .bind(&payload.name_en)
    .bind(rust_decimal::Decimal::from_f64_retain(payload.credit).unwrap_or_default()) 
    .bind(payload.hours_per_semester)
    .bind(&payload.subject_type)
    .bind(payload.group_id)
    .bind(&payload.level_scope)
    .bind(&payload.description)
    .fetch_one(&state.pool)
    .await;

    match result {
        Ok(subject) => (StatusCode::CREATED, Json(json!({ "success": true, "data": subject }))).into_response(),
        Err(e) => {
            eprintln!("Failed to create subject: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "Failed to create subject" })),
            ).into_response()
        }
    }
}

/// Update a subject
pub async fn update_subject(
    headers: axum::http::HeaderMap,
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateSubjectRequest>,
) -> impl IntoResponse {
    // 1. Check Permission
    if let Err(e) = check_permission(&headers, &state.pool, codes::ACADEMIC_CURRICULUM_UPDATE_ALL).await {
        return e;
    }

    // 2. Construct Query dynamically
    // Note: In real production code, we might use a query builder, but for now simple manual construction
    // Actually, SQLx doesn't support dynamic query building easily without query builder crate.
    // Let's use a simple update with all fields (null if not provided logic is tricky here)
    // For simplicity, we'll fetch existing first, update in memory, then save back? No, race condition.
    // Let's use COALESCE in SQL.

    let result = sqlx::query_as::<_, Subject>(
        r#"
        UPDATE subjects SET 
            code = COALESCE($1, code),
            academic_year_start = COALESCE($2, academic_year_start),
            name_th = COALESCE($3, name_th),
            name_en = COALESCE($4, name_en),
            credit = COALESCE($5, credit),
            hours_per_semester = COALESCE($6, hours_per_semester),
            type = COALESCE($7, type),
            group_id = COALESCE($8, group_id),
            level_scope = COALESCE($9, level_scope),
            description = COALESCE($10, description),
            is_active = COALESCE($11, is_active),
            updated_at = NOW()
        WHERE id = $12
        RETURNING *
        "#
    )
    .bind(&payload.code)
    .bind(payload.academic_year_start)
    .bind(&payload.name_th)
    .bind(&payload.name_en)
    // Handle Option<f64> -> Option<Decimal>
    .bind(payload.credit.map(|c| rust_decimal::Decimal::from_f64_retain(c).unwrap_or_default()))
    .bind(payload.hours_per_semester)
    .bind(&payload.subject_type)
    .bind(payload.group_id)
    .bind(&payload.level_scope)
    .bind(&payload.description)
    .bind(payload.is_active)
    .bind(id)
    .fetch_one(&state.pool)
    .await;

    match result {
        Ok(subject) => (StatusCode::OK, Json(json!({ "success": true, "data": subject }))).into_response(),
        Err(e) => {
            eprintln!("Failed to update subject {}: {}", id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "Failed to update subject" })),
            ).into_response()
        }
    }
}

/// Delete subject (Soft delete or Hard delete?)
/// Let's do Soft Delete by setting active=false (or actually delete if unused?)
/// For now, let's do REAL delete but check foreign keys constraints will stop it if used.
/// Actually, safe practice is check usage first. 
/// Let's stick to Soft Delete via update is_active in real usage, but here providing a delete endpoint usually means DELETE.
pub async fn delete_subject(
    headers: axum::http::HeaderMap,
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // 1. Check Permission
    if let Err(e) = check_permission(&headers, &state.pool, codes::ACADEMIC_CURRICULUM_DELETE_ALL).await {
        return e;
    }

    let result = sqlx::query("DELETE FROM subjects WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await;

    match result {
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))).into_response(),
        Err(e) => {
            eprintln!("Failed to delete subject {}: {}", id, e);
            // Likely Foreign Key violation
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "success": false, "error": "ไม่สามารถลบรายวิชาได้ (อาจมีการใช้งานอยู่)" })),
            ).into_response()
        }
    }
}
