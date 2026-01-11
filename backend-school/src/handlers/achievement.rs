use crate::db::school_mapping::get_school_database_url;
use crate::models::achievement::*;
use crate::models::auth::User;
use crate::models::staff::UserPermissions;
use crate::permissions::registry::codes;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use uuid::Uuid;

// ===================================================================
// Helper Functions
// ===================================================================

/// Extract user from request and check authentication
async fn get_authenticated_user(
    headers: &HeaderMap,
    pool: &sqlx::PgPool,
) -> Result<User, Response> {
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
    let token = match token_from_header.or(token_from_cookie) {
        Some(t) => t,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "success": false, "error": "กรุณาเข้าสู่ระบบ" })),
            ).into_response());
        }
    };
    
    // Verify token
    let claims = match crate::utils::jwt::JwtService::verify_token(&token) {
        Ok(c) => c,
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "success": false, "error": "Token ไม่ถูกต้อง" })),
            ).into_response());
        }
    };
    
    // Get user from database
    match sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = $1"
    )
    .bind(uuid::Uuid::parse_str(&claims.sub).unwrap())
    .fetch_one(pool)
    .await
    {
        Ok(u) => Ok(u),
        Err(_) => {
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "ไม่สามารถดึงข้อมูลผู้ใช้ได้" })),
            ).into_response())
        }
    }
}

async fn get_db_pool(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<sqlx::PgPool, Response> {
    let subdomain = match extract_subdomain_from_request(&headers) {
        Ok(s) => s,
        Err(response) => return Err(response),
    };

    let db_url = match get_school_database_url(&state.admin_pool, &subdomain).await {
        Ok(url) => url,
        Err(e) => {
            eprintln!("❌ Failed to get school database: {}", e);
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "error": "ไม่พบโรงเรียน" })),
            ).into_response());
        }
    };

    match state.pool_manager.get_pool(&db_url, &subdomain).await {
        Ok(p) => Ok(p),
        Err(e) => {
            eprintln!("❌ Failed to get database pool: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "ไม่สามารถเชื่อมต่อฐานข้อมูลได้" })),
            ).into_response())
        }
    }
}

// ===================================================================
// Handlers
// ===================================================================

pub async fn list_achievements(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<AchievementListFilter>,
) -> Response {
    let pool = match get_db_pool(&state, &headers).await {
        Ok(p) => p,
        Err(e) => return e,
    };

    let user = match get_authenticated_user(&headers, &pool).await {
        Ok(u) => u,
        Err(e) => return e,
    };

    // Check Permissions
    let can_read_all = user.has_permission(&pool, codes::ACHIEVEMENT_READ_ALL).await.unwrap_or(false);
    let can_read_own = user.has_permission(&pool, codes::ACHIEVEMENT_READ_OWN).await.unwrap_or(false);

    if !can_read_all && !can_read_own {
         return (
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "คุณไม่มีสิทธิ์ดูผลงาน" })),
        ).into_response();
    }

    // Prepare Query
    let mut query = String::from("
        SELECT 
            a.*,
            u.first_name as user_first_name,
            u.last_name as user_last_name,
            u.profile_image_url as user_profile_image_url
        FROM staff_achievements a
        LEFT JOIN users u ON a.user_id = u.id
        WHERE 1=1
    ");
    
    // Apply Ownership Filter
    if !can_read_all {
        // If cannot read all, force filter by own ID
        query.push_str(&format!(" AND a.user_id = '{}'", user.id));
    } else {
        // If can read all, check if user provided a specific filter
        if let Some(target_user_id) = filter.user_id {
            query.push_str(&format!(" AND a.user_id = '{}'", target_user_id));
        }
    }

    // Date Filters
    if let Some(start) = filter.start_date {
        query.push_str(&format!(" AND a.achievement_date >= '{}'", start));
    }
    if let Some(end) = filter.end_date {
        query.push_str(&format!(" AND a.achievement_date <= '{}'", end));
    }

    query.push_str(" ORDER BY a.achievement_date DESC, a.created_at DESC");

    let items = match sqlx::query_as::<_, Achievement>(&query)
        .fetch_all(&pool)
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "เกิดข้อผิดพลาดในการดึงข้อมูล" })),
            ).into_response();
        }
    };

    (StatusCode::OK, Json(json!({ "success": true, "data": items }))).into_response()
}

pub async fn create_achievement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAchievementRequest>,
) -> Response {
    let pool = match get_db_pool(&state, &headers).await {
        Ok(p) => p,
        Err(e) => return e,
    };

    let user = match get_authenticated_user(&headers, &pool).await {
        Ok(u) => u,
        Err(e) => return e,
    };

    // Determine target user
    let target_user_id = payload.user_id.unwrap_or(user.id);
    let is_own = target_user_id == user.id;

    // Check Permissions
    let allowed = if is_own {
        user.has_permission(&pool, codes::ACHIEVEMENT_CREATE_OWN).await.unwrap_or(false)
    } else {
        user.has_permission(&pool, codes::ACHIEVEMENT_CREATE_ALL).await.unwrap_or(false)
    };

    if !allowed {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "คุณไม่มีสิทธิ์เพิ่มผลงานนี้" })),
        ).into_response();
    }

    // Insert
    let result = sqlx::query_as::<_, Achievement>(
        "INSERT INTO staff_achievements (user_id, title, description, achievement_date, image_path, created_by)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *, 
         NULL::text as user_first_name, 
         NULL::text as user_last_name, 
         NULL::text as user_profile_image_url"
    )
    .bind(target_user_id)
    .bind(&payload.title)
    .bind(&payload.description)
    .bind(payload.achievement_date)
    .bind(&payload.image_path)
    .bind(user.id)
    .fetch_one(&pool)
    .await;

    match result {
        Ok(achievement) => (StatusCode::CREATED, Json(json!({ "success": true, "data": achievement }))).into_response(),
        Err(e) => {
            eprintln!("❌ Create achievement error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "บันทึกข้อมูลไม่สำเร็จ" })),
            ).into_response()
        }
    }
}

pub async fn update_achievement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAchievementRequest>,
) -> Response {
    let pool = match get_db_pool(&state, &headers).await {
        Ok(p) => p,
        Err(e) => return e,
    };

    let user = match get_authenticated_user(&headers, &pool).await {
        Ok(u) => u,
        Err(e) => return e,
    };

    // Get existing achievement owner to check permission
    #[derive(sqlx::FromRow)]
    struct AchievementOwnership {
        user_id: Uuid,
    }

    let existing = match sqlx::query_as::<_, AchievementOwnership>("SELECT user_id FROM staff_achievements WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
    {
        Ok(Some(a)) => a,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({ "success": false, "error": "ไม่พบข้อมูล" }))).into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "error": "Database error" }))).into_response(),
    };

    let is_own = existing.user_id == user.id;

    // Check Permissions
    let allowed = if is_own {
        user.has_permission(&pool, codes::ACHIEVEMENT_UPDATE_OWN).await.unwrap_or(false)
    } else {
        user.has_permission(&pool, codes::ACHIEVEMENT_UPDATE_ALL).await.unwrap_or(false)
    };

    if !allowed {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "คุณไม่มีสิทธิ์แก้ไขข้อมูลนี้" })),
        ).into_response();
    }

    // Build Update Query
    // Note: We use COALESCE to keep existing values if fields are None
    // But standard SQL doesn't work nicely with Rust Option in one go usually unless we build dynamic query or use COALESCE($1, column)
    
    // Simplest way: dynamic query or extensive binding
    
    let result = sqlx::query_as::<_, Achievement>(
        "UPDATE staff_achievements SET
            title = COALESCE($1, title),
            description = COALESCE($2, description),
            achievement_date = COALESCE($3, achievement_date),
            image_path = COALESCE($4, image_path),
            updated_at = NOW()
         WHERE id = $5
         RETURNING *, 
         NULL::text as user_first_name, 
         NULL::text as user_last_name, 
         NULL::text as user_profile_image_url"
    )
    .bind(payload.title)
    .bind(payload.description)
    .bind(payload.achievement_date)
    .bind(payload.image_path)
    .bind(id)
    .fetch_one(&pool)
    .await;

    match result {
        Ok(updated) => (StatusCode::OK, Json(json!({ "success": true, "data": updated }))).into_response(),
        Err(e) => {
            eprintln!("❌ Update achievement error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "แก้ไขข้อมูลไม่สำเร็จ" })),
            ).into_response()
        }
    }
}

pub async fn delete_achievement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Response {
    let pool = match get_db_pool(&state, &headers).await {
        Ok(p) => p,
        Err(e) => return e,
    };

    let user = match get_authenticated_user(&headers, &pool).await {
        Ok(u) => u,
        Err(e) => return e,
    };

    // Get existing achievement owner for permission check
    #[derive(sqlx::FromRow)]
    struct AchievementOwnership {
        user_id: Uuid,
    }

    let existing = match sqlx::query_as::<_, AchievementOwnership>("SELECT user_id FROM staff_achievements WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
    {
        Ok(Some(a)) => a,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({ "success": false, "error": "ไม่พบข้อมูล" }))).into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "error": "Database error" }))).into_response(),
    };

    let is_own = existing.user_id == user.id;

    // Check Permissions
    let allowed = if is_own {
        user.has_permission(&pool, codes::ACHIEVEMENT_DELETE_OWN).await.unwrap_or(false)
    } else {
        user.has_permission(&pool, codes::ACHIEVEMENT_DELETE_ALL).await.unwrap_or(false)
    };

    if !allowed {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "คุณไม่มีสิทธิ์ลบข้อมูลนี้" })),
        ).into_response();
    }

    match sqlx::query("DELETE FROM staff_achievements WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({ "success": true }))).into_response(),
        Err(e) => {
            eprintln!("❌ Delete achievement error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "ลบข้อมูลไม่สำเร็จ" })),
            ).into_response()
        }
    }
}
