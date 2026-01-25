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
use serde_json::json;
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
    level_type: String,
    year: i32,
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
    
    // Determine target year for filtering
    let mut target_year_id = query.academic_year_id;
    
    // Default to current active year if not specified and current_year is not explicitly false
    // (This ensures dropdowns show only configured levels by default)
    if target_year_id.is_none() && query.current_year.unwrap_or(true) {
         let active_year_id: Option<Uuid> = sqlx::query_scalar(
             "SELECT id FROM academic_years WHERE is_active = true LIMIT 1"
         )
         .fetch_optional(&pool)
         .await
         .unwrap_or(None);
         
         target_year_id = active_year_id;
    }

    // Build SQL
    let mut sql = String::from("SELECT gl.id, gl.level_type, gl.year FROM grade_levels gl");
    
    // Apply Year Filter via Join
    if let Some(yid) = target_year_id {
        sql.push_str(" JOIN academic_year_grade_levels aygl ON gl.id = aygl.grade_level_id");
        // Use WHERE 1=1 trick to handle subsequent ANDs easily, but here update condition
        sql.push_str(&format!(" WHERE aygl.academic_year_id = '{}'", yid));
    } else {
        sql.push_str(" WHERE 1=1");
    }
    
    if query.active_only.unwrap_or(true) {
        sql.push_str(" AND gl.is_active = true");
    }

    if let Some(ltype) = &query.level_type {
        sql.push_str(&format!(" AND gl.level_type = '{}'", ltype));
    }
    
    sql.push_str(" ORDER BY CASE gl.level_type 
            WHEN 'kindergarten' THEN 1 
            WHEN 'primary' THEN 2 
            WHEN 'secondary' THEN 3 
            ELSE 4 
         END, gl.year ASC LIMIT 500");
    
    let rows = sqlx::query_as::<_, GradeLevelRow>(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;
    
    let data: Vec<GradeLevelLookupItem> = rows.into_iter().map(|r| {
        let (name, code, short_name) = match r.level_type.as_str() {
            "kindergarten" => (format!("อนุบาลปีที่ {}", r.year), format!("K{}", r.year), format!("อ.{}", r.year)),
            "primary" => (format!("ประถมศึกษาปีที่ {}", r.year), format!("P{}", r.year), format!("ป.{}", r.year)),
            "secondary" => (format!("มัธยมศึกษาปีที่ {}", r.year), format!("M{}", r.year), format!("ม.{}", r.year)),
            _ => (format!("Other {}", r.year), format!("O{}", r.year), format!("?{}", r.year)),
        };
        
        let order_base = match r.level_type.as_str() {
            "kindergarten" => 1,
            "primary" => 2,
            "secondary" => 3,
            _ => 4,
        };
        
        GradeLevelLookupItem {
            id: r.id,
            code,
            name,
            short_name: Some(short_name),
            level_order: order_base * 100 + r.year,
        }
    }).collect();
    
    // Filter by search in memory if needed
    let final_data = if let Some(search) = query.search {
         data.into_iter().filter(|i| i.name.contains(&search) || i.code.contains(&search)).take(limit as usize).collect()
    } else {
         data
    };
    
    Ok((StatusCode::OK, Json(LookupResponse { success: true, data: final_data })))
}

// ===================================================================
// Classrooms Lookup
// ===================================================================

#[derive(Debug, FromRow)]
struct ClassroomRow {
    id: Uuid,
    name: String,
    level_type: Option<String>,
    year: Option<i32>,
    grade_level_id: Option<Uuid>,
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
    // let active_only = query.active_only.unwrap_or(true); // class_rooms might not have is_active
    
    let mut sql = String::from(
        "SELECT c.id, c.name, g.level_type, g.year, c.grade_level_id
         FROM class_rooms c
         LEFT JOIN grade_levels g ON c.grade_level_id = g.id
         WHERE 1=1"
    );
    
    // Check if class_rooms has is_active? Assuming not for now based on create handler.
    // If it does, uncomment below:
    // if active_only { sql.push_str(" AND c.is_active = true"); }
    
    if let Some(ref search) = query.search {
        sql.push_str(&format!(" AND c.name ILIKE '%{}%'", search));
    }
    
    sql.push_str(&format!(" ORDER BY g.year, c.name LIMIT {}", limit));
    
    let rows = sqlx::query_as::<_, ClassroomRow>(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;
    
    let data: Vec<ClassroomLookupItem> = rows.into_iter().map(|r| {
        let grade_level_name = match (r.level_type.as_deref(), r.year) {
            (Some("kindergarten"), Some(y)) => Some(format!("อ.{}", y)),
            (Some("primary"), Some(y)) => Some(format!("ป.{}", y)),
            (Some("secondary"), Some(y)) => Some(format!("ม.{}", y)),
            _ => None,
        };
        
        ClassroomLookupItem {
            id: r.id,
            name: r.name,
            grade_level: grade_level_name,
            grade_level_id: r.grade_level_id,
        }
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
    year: i32,
    is_active: bool,
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
    
    let mut sql = String::from("SELECT id, name, year, is_active FROM academic_years WHERE 1=1");
    
    if active_only {
        sql.push_str(" AND is_active = true");
    }
    
    if let Some(ref search) = query.search {
        sql.push_str(&format!(" AND name ILIKE '%{}%'", search));
    }
    
    // Order by active first, then by year descending (latest year first)
    sql.push_str(&format!(" ORDER BY is_active DESC, year DESC LIMIT {}", limit));
    
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
        year: r.year,
        is_current: r.is_active, // Map is_active to is_current for API compatibility
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
/// Returns minimal student data for dropdowns (with student_id and class_room)
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
    
    // Query with student_id from student_info and current classroom from enrollments
    // Fix: Table name is 'student_class_enrollments' and 'class_rooms'
    let mut sql = String::from(
        "SELECT u.id, u.title, u.first_name, u.last_name, u.username,
                si.student_id,
                c.name as class_room
         FROM users u
         LEFT JOIN student_info si ON u.id = si.user_id
         LEFT JOIN student_class_enrollments e ON u.id = e.student_id AND e.status = 'active'
         LEFT JOIN class_rooms c ON e.class_room_id = c.id
         WHERE u.user_type = 'student'"
    );
    
    if active_only {
        sql.push_str(" AND u.status = 'active'");
    }
    
    if let Some(ref search) = query.search {
        sql.push_str(&format!(
            " AND (u.first_name ILIKE '%{}%' OR u.last_name ILIKE '%{}%' OR u.username ILIKE '%{}%' OR si.student_id ILIKE '%{}%')",
            search, search, search, search
        ));
    }
    
    sql.push_str(&format!(" ORDER BY u.first_name, u.last_name LIMIT {}", limit));
    
    #[derive(Debug, FromRow)]
    struct StudentWithInfoRow {
        id: Uuid,
        title: Option<String>,
        first_name: String,
        last_name: String,
        #[allow(dead_code)]
        username: String,
        student_id: Option<String>,
        class_room: Option<String>,
    }
    
    let rows = sqlx::query_as::<_, StudentWithInfoRow>(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;
    
    let data: Vec<StudentLookupItem> = rows.into_iter().map(|r| {
        let name = format!("{} {}", r.first_name, r.last_name);
        StudentLookupItem {
            id: r.id,
            name,
            title: r.title,
            student_id: r.student_id,
            class_room: r.class_room,
        }
    }).collect();
    
    Ok((StatusCode::OK, Json(LookupResponse { success: true, data })))
}

// ===================================================================
// Room Lookup
// ===================================================================

/// GET /api/lookup/rooms
/// Returns active rooms with building info
pub async fn lookup_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|_| AppError::NotFound("ไม่พบโรงเรียน".to_string()))?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|_| AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string()))?;

    // 1. Check authentication
    verify_authenticated(&headers, &pool).await?;

    // 2. Query
    use crate::modules::facility::models::Room;

    let rooms = sqlx::query_as::<_, Room>(
        r#"
        SELECT r.*, b.name_th as building_name
        FROM rooms r
        LEFT JOIN buildings b ON r.building_id = b.id
        WHERE r.status = 'ACTIVE'
        ORDER BY b.code NULLS LAST, r.floor NULLS FIRST, r.code ASC
        "#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("Lookup Rooms Error: {}", e);
        AppError::InternalServerError("Failed to fetch rooms".to_string())
    })?;

    Ok(Json(json!({ "success": true, "data": rooms })).into_response())
}


// ===================================================================
// Subjects Lookup
// ===================================================================

#[derive(Debug, FromRow)]
struct SubjectRow {
    id: Uuid,
    code: String,
    name_th: String,
}

/// GET /api/lookup/subjects
/// Returns minimal subject data for dropdowns
pub async fn lookup_subjects(
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
    
    let mut sql = String::from("SELECT id, code, name_th FROM subjects WHERE 1=1");
    
    if active_only {
        sql.push_str(" AND is_active = true");
    }
    
    if let Some(ref stype) = query.subject_type {
        sql.push_str(&format!(" AND type = '{}'", stype));
    }

    if let Some(ref search) = query.search {
        sql.push_str(&format!(" AND (name_th ILIKE '%{}%' OR code ILIKE '%{}%')", search, search));
    }
    
    sql.push_str(&format!(" ORDER BY code, name_th LIMIT {}", limit));
    
    let rows = sqlx::query_as::<_, SubjectRow>(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;
    
    let data: Vec<LookupItem> = rows.into_iter().map(|r| LookupItem {
        id: r.id,
        name: r.name_th,
        code: Some(r.code),
    }).collect();
    
    Ok((StatusCode::OK, Json(LookupResponse { success: true, data })))
}
