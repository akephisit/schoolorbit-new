// Lookup Handlers
// These endpoints return minimal data for dropdowns
// Only require authentication, no specific permission needed

use crate::db::school_mapping::get_school_database_url;
use crate::modules::lookup::models::*;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;
use crate::error::AppError;
use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use sqlx::FromRow;
use uuid::Uuid;

// ===================================================================
// Helper: Verify user is authenticated (no permission check)
// ===================================================================

async fn verify_authenticated(
    headers: &HeaderMap,
    pool: &sqlx::PgPool,
) -> Result<Uuid, AppError> {
    // Try to extract token from Authorization header first
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());
    
    let token_from_header = auth_header
        .and_then(|h| {
            if h.starts_with("Bearer ") {
                Some(h[7..].to_string())
            } else {
                None
            }
        });

    // Fallback to cookie
    let token_from_cookie = headers
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .and_then(|cookie| crate::utils::jwt::JwtService::extract_token_from_cookie(Some(cookie)));

    // Use Authorization header first, then cookie
    let token = token_from_header.or(token_from_cookie)
        .ok_or(AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string()))?;

    // Verify token
    let claims = crate::utils::jwt::JwtService::verify_token(&token)
        .map_err(|_| AppError::AuthError("Token ไม่ถูกต้อง".to_string()))?;
    
    // Verify user exists in database
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::AuthError("Invalid user ID".to_string()))?;
    
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND status = 'active')"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap_or(false);
    
    if !exists {
        return Err(AppError::AuthError("ไม่พบผู้ใช้หรือบัญชีถูกระงับ".to_string()));
    }
    
    Ok(user_id)
}

// ===================================================================
// Staff Lookup
// ===================================================================

#[derive(Debug, FromRow)]
struct StaffRow {
    id: Uuid,
    title: Option<String>,
    first_name: String,
    last_name: String,
    username: String,
}

/// GET /api/lookup/staff
/// Returns minimal staff data for dropdowns (id, name, title)
pub async fn lookup_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;
    
    // Only requires authentication
    verify_authenticated(&headers, &pool).await?;

    let limit = query.limit.unwrap_or(100).min(500);
    let active_only = query.active_only.unwrap_or(true);
    
    let mut sql = String::from(
        "SELECT id, title, first_name, last_name, username 
         FROM users 
         WHERE user_type = 'staff'"
    );
    
    if active_only {
        sql.push_str(" AND status = 'active'");
    }
    
    if let Some(ref search) = query.search {
        sql.push_str(&format!(
            " AND (first_name ILIKE '%{}%' OR last_name ILIKE '%{}%' OR username ILIKE '%{}%')",
            search, search, search
        ));
    }
    
    sql.push_str(&format!(" ORDER BY first_name, last_name LIMIT {}", limit));
    
    let rows = sqlx::query_as::<_, StaffRow>(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;
    
    let data: Vec<StaffLookupItem> = rows.into_iter().map(|r| {
        let name = format!("{} {}", r.first_name, r.last_name);
        StaffLookupItem {
            id: r.id,
            name,
            title: r.title,
            username: Some(r.username),
        }
    }).collect();
    
    Ok((StatusCode::OK, Json(LookupResponse { success: true, data })))
}

// ===================================================================
// Roles Lookup
// ===================================================================

#[derive(Debug, FromRow)]
struct RoleRow {
    id: Uuid,
    code: String,
    name: String,
    user_type: String,
}

/// GET /api/lookup/roles
/// Returns minimal role data for dropdowns
pub async fn lookup_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|_| AppError::NotFound("ไม่พบโรงเรียน".to_string()))?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|_| AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string()))?;
    
    verify_authenticated(&headers, &pool).await?;

    let limit = query.limit.unwrap_or(100).min(500);
    let active_only = query.active_only.unwrap_or(true);
    
    let mut sql = String::from("SELECT id, code, name, user_type FROM roles WHERE 1=1");
    
    if active_only {
        sql.push_str(" AND is_active = true");
    }
    
    if let Some(ref search) = query.search {
        sql.push_str(&format!(" AND (name ILIKE '%{}%' OR code ILIKE '%{}%')", search, search));
    }
    
    sql.push_str(&format!(" ORDER BY level DESC, name LIMIT {}", limit));
    
    let rows = sqlx::query_as::<_, RoleRow>(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;
    
    let data: Vec<RoleLookupItem> = rows.into_iter().map(|r| RoleLookupItem {
        id: r.id,
        code: r.code,
        name: r.name,
        user_type: r.user_type,
    }).collect();
    
    Ok((StatusCode::OK, Json(LookupResponse { success: true, data })))
}

// ===================================================================
// Departments Lookup
// ===================================================================

#[derive(Debug, FromRow)]
struct DepartmentRow {
    id: Uuid,
    code: String,
    name: String,
}

/// GET /api/lookup/departments
/// Returns minimal department data for dropdowns
pub async fn lookup_departments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|_| AppError::NotFound("ไม่พบโรงเรียน".to_string()))?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|_| AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string()))?;
    
    verify_authenticated(&headers, &pool).await?;

    let limit = query.limit.unwrap_or(100).min(500);
    let active_only = query.active_only.unwrap_or(true);
    
    let mut sql = String::from("SELECT id, code, name FROM departments WHERE 1=1");
    
    if active_only {
        sql.push_str(" AND is_active = true");
    }
    
    if let Some(ref search) = query.search {
        sql.push_str(&format!(" AND (name ILIKE '%{}%' OR code ILIKE '%{}%')", search, search));
    }
    
    sql.push_str(&format!(" ORDER BY name LIMIT {}", limit));
    
    let rows = sqlx::query_as::<_, DepartmentRow>(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;
    
    let data: Vec<DepartmentLookupItem> = rows.into_iter().map(|r| DepartmentLookupItem {
        id: r.id,
        code: r.code,
        name: r.name,
    }).collect();
    
    Ok((StatusCode::OK, Json(LookupResponse { success: true, data })))
}

// ===================================================================
// Grade Levels Lookup
// ===================================================================

#[derive(Debug, FromRow)]
struct GradeLevelRow {
    id: Uuid,
    code: String,
    name: String,
    level_order: i32,
}

/// GET /api/lookup/grade-levels
/// Returns minimal grade level data for dropdowns
pub async fn lookup_grade_levels(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|_| AppError::NotFound("ไม่พบโรงเรียน".to_string()))?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|_| AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string()))?;
    
    verify_authenticated(&headers, &pool).await?;

    let limit = query.limit.unwrap_or(100).min(500);
    let active_only = query.active_only.unwrap_or(true);
    
    let mut sql = String::from("SELECT id, code, name, level_order FROM grade_levels WHERE 1=1");
    
    if active_only {
        sql.push_str(" AND is_active = true");
    }
    
    if let Some(ref search) = query.search {
        sql.push_str(&format!(" AND (name ILIKE '%{}%' OR code ILIKE '%{}%')", search, search));
    }
    
    sql.push_str(&format!(" ORDER BY level_order LIMIT {}", limit));
    
    let rows = sqlx::query_as::<_, GradeLevelRow>(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;
    
    let data: Vec<GradeLevelLookupItem> = rows.into_iter().map(|r| GradeLevelLookupItem {
        id: r.id,
        code: r.code,
        name: r.name,
        level_order: r.level_order,
    }).collect();
    
    Ok((StatusCode::OK, Json(LookupResponse { success: true, data })))
}

// ===================================================================
// Classrooms Lookup
// ===================================================================

#[derive(Debug, FromRow)]
struct ClassroomRow {
    id: Uuid,
    name: String,
    grade_level_name: Option<String>,
}

/// GET /api/lookup/classrooms
/// Returns minimal classroom data for dropdowns
pub async fn lookup_classrooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|_| AppError::NotFound("ไม่พบโรงเรียน".to_string()))?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|_| AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string()))?;
    
    verify_authenticated(&headers, &pool).await?;

    let limit = query.limit.unwrap_or(100).min(500);
    let active_only = query.active_only.unwrap_or(true);
    
    let mut sql = String::from(
        "SELECT c.id, c.name, g.name as grade_level_name 
         FROM classrooms c
         LEFT JOIN grade_levels g ON c.grade_level_id = g.id
         WHERE 1=1"
    );
    
    if active_only {
        sql.push_str(" AND c.is_active = true");
    }
    
    if let Some(ref search) = query.search {
        sql.push_str(&format!(" AND c.name ILIKE '%{}%'", search));
    }
    
    sql.push_str(&format!(" ORDER BY g.level_order, c.name LIMIT {}", limit));
    
    let rows = sqlx::query_as::<_, ClassroomRow>(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;
    
    let data: Vec<ClassroomLookupItem> = rows.into_iter().map(|r| ClassroomLookupItem {
        id: r.id,
        name: r.name,
        grade_level: r.grade_level_name,
    }).collect();
    
    Ok((StatusCode::OK, Json(LookupResponse { success: true, data })))
}

// ===================================================================
// Academic Years Lookup
// ===================================================================

#[derive(Debug, FromRow)]
struct AcademicYearRow {
    id: Uuid,
    name: String,
    is_current: bool,
}

/// GET /api/lookup/academic-years
/// Returns minimal academic year data for dropdowns
pub async fn lookup_academic_years(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|_| AppError::NotFound("ไม่พบโรงเรียน".to_string()))?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|_| AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string()))?;
    
    verify_authenticated(&headers, &pool).await?;

    let limit = query.limit.unwrap_or(100).min(500);
    let active_only = query.active_only.unwrap_or(true);
    
    let mut sql = String::from("SELECT id, name, is_current FROM academic_years WHERE 1=1");
    
    if active_only {
        sql.push_str(" AND is_active = true");
    }
    
    if let Some(ref search) = query.search {
        sql.push_str(&format!(" AND name ILIKE '%{}%'", search));
    }
    
    sql.push_str(&format!(" ORDER BY is_current DESC, name DESC LIMIT {}", limit));
    
    let rows = sqlx::query_as::<_, AcademicYearRow>(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;
    
    let data: Vec<AcademicYearLookupItem> = rows.into_iter().map(|r| AcademicYearLookupItem {
        id: r.id,
        name: r.name,
        is_current: r.is_current,
    }).collect();
    
    Ok((StatusCode::OK, Json(LookupResponse { success: true, data })))
}

// ===================================================================
// Students Lookup
// ===================================================================

#[derive(Debug, FromRow)]
struct StudentRow {
    id: Uuid,
    first_name: String,
    last_name: String,
    username: String,
}

/// GET /api/lookup/students
/// Returns minimal student data for dropdowns
pub async fn lookup_students(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LookupQuery>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|_| AppError::NotFound("ไม่พบโรงเรียน".to_string()))?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|_| AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string()))?;
    
    verify_authenticated(&headers, &pool).await?;

    let limit = query.limit.unwrap_or(100).min(500);
    let active_only = query.active_only.unwrap_or(true);
    
    let mut sql = String::from(
        "SELECT id, first_name, last_name, username 
         FROM users 
         WHERE user_type = 'student'"
    );
    
    if active_only {
        sql.push_str(" AND status = 'active'");
    }
    
    if let Some(ref search) = query.search {
        sql.push_str(&format!(
            " AND (first_name ILIKE '%{}%' OR last_name ILIKE '%{}%' OR username ILIKE '%{}%')",
            search, search, search
        ));
    }
    
    sql.push_str(&format!(" ORDER BY first_name, last_name LIMIT {}", limit));
    
    let rows = sqlx::query_as::<_, StudentRow>(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;
    
    let data: Vec<StaffLookupItem> = rows.into_iter().map(|r| {
        let name = format!("{} {}", r.first_name, r.last_name);
        StaffLookupItem {
            id: r.id,
            name,
            title: None,
            username: Some(r.username),
        }
    }).collect();
    
    Ok((StatusCode::OK, Json(LookupResponse { success: true, data })))
}
