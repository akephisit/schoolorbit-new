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

// ==================== Common Response ====================

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
        }
    }
    
    // Unused but kept for completeness
    pub fn error(msg: String) -> Self {
        Self {
            success: false,
            data: None,
            message: Some(msg),
        }
    }
}

// Helper to get pool
async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    
    let db_url = get_school_database_url(&state.admin_pool, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
        
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

// ==================== Instructor Constraints ====================

#[derive(Debug, Serialize, Deserialize)]
pub struct InstructorConstraintView {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub short_name: Option<String>,
    
    // Preferences
    pub max_periods_per_day: Option<i32>,
    pub hard_unavailable_slots: Option<serde_json::Value>, 
    pub preferred_slots: Option<serde_json::Value>,
    
    // Room Assignment
    pub assigned_room_id: Option<Uuid>,
    pub assigned_room_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateInstructorConstraintRequest {
    pub max_periods_per_day: Option<i32>,
    pub hard_unavailable_slots: Option<serde_json::Value>,
    pub preferred_slots: Option<serde_json::Value>,
    pub assigned_room_id: Option<Uuid>,
}

pub async fn list_instructor_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    
    let academic_year = sqlx::query!("SELECT id FROM academic_years WHERE is_active = true LIMIT 1")
        .fetch_optional(&pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
    let year_id = match academic_year {
        Some(y) => y.id,
        None => return Err(AppError::NotFound("No active academic year found".into())),
    };

    let instructors = sqlx::query_as!(
        InstructorConstraintView,
        r#"
        SELECT 
            u.id,
            u.first_name,
            u.last_name,
            s.short_name,
            ip.max_periods_per_day,
            ip.hard_unavailable_slots,
            ip.preferred_slots,
            ira.room_id as assigned_room_id,
            r.name as assigned_room_name
        FROM users u
        JOIN staff s ON u.id = s.user_id
        LEFT JOIN instructor_preferences ip ON u.id = ip.instructor_id AND ip.academic_year_id = $1
        LEFT JOIN instructor_room_assignments ira ON u.id = ira.instructor_id AND ira.academic_year_id = $1
        LEFT JOIN rooms r ON ira.room_id = r.id
        WHERE (u.role = 'TEACHER' OR u.role = 'ADMIN')
        ORDER BY u.first_name
        "#,
        year_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(ApiResponse::success(instructors)))
}

pub async fn update_instructor_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instructor_id): Path<Uuid>,
    Json(payload): Json<UpdateInstructorConstraintRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let mut tx = pool.begin().await.map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let academic_year = sqlx::query!("SELECT id FROM academic_years WHERE is_active = true LIMIT 1")
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let year_id = match academic_year {
        Some(y) => y.id,
        None => return Err(AppError::NotFound("Active academic year not found".into())),
    };

    sqlx::query!(
        r#"
        INSERT INTO instructor_preferences (
            instructor_id, academic_year_id, 
            max_periods_per_day, hard_unavailable_slots, preferred_slots
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (instructor_id, academic_year_id) DO UPDATE SET
            max_periods_per_day = EXCLUDED.max_periods_per_day,
            hard_unavailable_slots = EXCLUDED.hard_unavailable_slots,
            preferred_slots = EXCLUDED.preferred_slots
        "#,
        instructor_id,
        year_id,
        payload.max_periods_per_day.unwrap_or(7),
        payload.hard_unavailable_slots.unwrap_or(serde_json::json!([])),
        payload.preferred_slots.unwrap_or(serde_json::json!([]))
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    if let Some(room_id) = payload.assigned_room_id {
        sqlx::query!(
            r#"
            INSERT INTO instructor_room_assignments (instructor_id, room_id, academic_year_id)
            VALUES ($1, $2, $3)
            ON CONFLICT (instructor_id, academic_year_id) DO UPDATE SET room_id = EXCLUDED.room_id
            "#,
            instructor_id,
            room_id,
            year_id
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    }

    tx.commit().await.map_err(|e| AppError::DatabaseError(e.to_string()))?;
    
    // Return empty success object? Or match ApiResponse structure
    // Since ApiResponse expects "data", we can pass a simple message struct or String
    Ok(Json(ApiResponse::success("Updated successfully".to_string())))
}


// ==================== Subject Constraints ====================

#[derive(Debug, Serialize, Deserialize)]
pub struct SubjectConstraintView {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    
    pub min_consecutive_periods: Option<i32>,
    pub max_consecutive_periods: Option<i32>,
    pub preferred_time_of_day: Option<String>,
    pub required_room_type: Option<String>,
    pub periods_per_week: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSubjectConstraintRequest {
    pub min_consecutive_periods: Option<i32>,
    pub max_consecutive_periods: Option<i32>,
    pub preferred_time_of_day: Option<String>,
    pub required_room_type: Option<String>,
    pub periods_per_week: Option<i32>,
}


pub async fn list_subject_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let subjects = sqlx::query_as!(
        SubjectConstraintView,
        r#"
        SELECT 
            id, code, name,
            min_consecutive_periods,
            max_consecutive_periods,
            preferred_time_of_day,
            required_room_type,
            periods_per_week
        FROM subjects
        ORDER BY code
        "#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(ApiResponse::success(subjects)))
}

pub async fn update_subject_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subject_id): Path<Uuid>,
    Json(payload): Json<UpdateSubjectConstraintRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    
    sqlx::query!(
        r#"
        UPDATE subjects SET
            min_consecutive_periods = COALESCE($2, min_consecutive_periods),
            max_consecutive_periods = COALESCE($3, max_consecutive_periods),
            preferred_time_of_day = COALESCE($4, preferred_time_of_day),
            required_room_type = COALESCE($5, required_room_type),
            periods_per_week = COALESCE($6, periods_per_week)
        WHERE id = $1
        "#,
        subject_id,
        payload.min_consecutive_periods,
        payload.max_consecutive_periods,
        payload.preferred_time_of_day,
        payload.required_room_type,
        payload.periods_per_week
    )
    .execute(&pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(ApiResponse::success("Updated subject constraints".to_string())))
}
