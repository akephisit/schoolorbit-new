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
    pub instructor_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub hard_unavailable_slots: Option<serde_json::Value>,
    pub max_periods_per_day: Option<i32>,
    pub min_periods_per_day: Option<i32>,
    pub assigned_room_id: Option<Uuid>,
    pub assigned_room_name: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateInstructorConstraintRequest {
    pub instructor_id: Uuid,
    pub hard_unavailable_slots: Option<serde_json::Value>,
    pub max_periods_per_day: Option<i32>,
    pub preferred_slots: Option<serde_json::Value>,
    pub assigned_room_id: Option<Uuid>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct SubjectConstraintView {
    pub subject_id: Uuid,
    pub code: String,
    pub name: String,
    pub min_consecutive_periods: i32,
    pub max_consecutive_periods: Option<i32>, // Can be null in DB
    pub allow_single_period: Option<bool>,
    pub required_room_type: Option<String>,
    pub preferred_time_of_day: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateSubjectConstraintRequest {
    pub subject_id: Uuid,
    pub min_consecutive_periods: Option<i32>,
    pub max_consecutive_periods: Option<i32>,
    pub allow_single_period: Option<bool>,
    pub required_room_type: Option<String>,
    pub preferred_time_of_day: Option<String>,
}

/// Helper
async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    
    let db_url = get_school_database_url(&state.admin_pool, &subdomain).await
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

    let instructors = sqlx::query_as::<_, InstructorConstraintView>(
        r#"
        SELECT 
            u.id as instructor_id,
            u.first_name,
            u.last_name,
            ip.hard_unavailable_slots,
            ip.max_periods_per_day,
            ip.min_periods_per_day,
            ra.room_id as assigned_room_id,
            r.name as assigned_room_name
        FROM users u
        JOIN user_roles ur ON u.id = ur.user_id
        JOIN roles rol ON ur.role_id = rol.id
        LEFT JOIN instructor_preferences ip ON u.id = ip.instructor_id AND ip.academic_year_id = $1
        LEFT JOIN instructor_room_assignments ra ON u.id = ra.instructor_id AND ra.academic_year_id = $1 AND ra.is_required = true
        LEFT JOIN rooms r ON ra.room_id = r.id
        WHERE rol.name = 'Teacher'
        ORDER BY u.first_name
        "#
    )
    .bind(year_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(ApiResponse::success(instructors)))
}

pub async fn update_instructor_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
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

    // Update preferences
    // Using simple query! macro for update/insert
    sqlx::query(
        r#"
        INSERT INTO instructor_preferences (
            instructor_id, academic_year_id, 
            hard_unavailable_slots, max_periods_per_day, preferred_slots
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (instructor_id, academic_year_id) 
        DO UPDATE SET 
            hard_unavailable_slots = EXCLUDED.hard_unavailable_slots,
            max_periods_per_day = EXCLUDED.max_periods_per_day,
            updated_at = NOW()
        "#
    )
    .bind(payload.instructor_id)
    .bind(year_id)
    .bind(payload.hard_unavailable_slots.unwrap_or(serde_json::json!([])))
    .bind(payload.max_periods_per_day)
    .bind(payload.preferred_slots.unwrap_or(serde_json::json!([])))
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Handle Room Assignment
    // If room_id is Some, insert/update. If None, we might want to delete?
    // Current logic: only insert if payload has room_id
    if let Some(room_id) = payload.assigned_room_id {
        // First, clear existing assignments for this instructor/year ??
        // Actually, constraint usually implies "Primary Room".
        // Let's delete old one first to be safe or upsert.
        
        sqlx::query(
            "DELETE FROM instructor_room_assignments WHERE instructor_id = $1 AND academic_year_id = $2 AND is_required = true"
        )
        .bind(payload.instructor_id)
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
        .bind(payload.instructor_id)
        .bind(year_id)
        .bind(room_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    } else {
        // If assigned_room_id is None (or not sent? Payload field is Option),
        // If explicit Null was sent, we might want to clear assignment.
        // But `Option<Uuid>` doesn't distinguish between "Missing" and "Null" easily in Axum Json without wrapper.
        // Let's assume if it's sent as None, we do nothing or clear?
        // Let's clear if it's None for now? Or maybe user just didn't select one.
        // UI sends null if cleared.
        
        // Let's try to remove assignment if we can confirm intent. 
        // For now, let's just leave it alone if None, or maybe the UI sends a specific Value?
        // Just Update: if User unselects room, UI should send null.
        // So we should delete.
        // But checking if field was present is hard.
        // Let's assume Update always sends current state.
         sqlx::query(
            "DELETE FROM instructor_room_assignments WHERE instructor_id = $1 AND academic_year_id = $2 AND is_required = true"
        )
        .bind(payload.instructor_id)
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
            id as subject_id,
            code,
            name_th as name,
            min_consecutive_periods,
            max_consecutive_periods,
            allow_single_period,
            required_room_type,
            preferred_time_of_day
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
    Json(payload): Json<UpdateSubjectConstraintRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    sqlx::query(
        r#"
        UPDATE subjects SET
            min_consecutive_periods = COALESCE($2, min_consecutive_periods),
            max_consecutive_periods = $3,
            allow_single_period = COALESCE($4, allow_single_period),
            required_room_type = $5,
            preferred_time_of_day = $6,
            updated_at = NOW()
        WHERE id = $1
        "#
    )
    .bind(payload.subject_id)
    .bind(payload.min_consecutive_periods)
    .bind(payload.max_consecutive_periods)
    .bind(payload.allow_single_period)
    .bind(payload.required_room_type)
    .bind(payload.preferred_time_of_day)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(Json(ApiResponse::success("Updated subject constraints".to_string())))
}
