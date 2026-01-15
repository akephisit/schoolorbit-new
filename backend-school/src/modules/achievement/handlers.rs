use crate::db::school_mapping::get_school_database_url;
use crate::modules::achievement::models::*;
use crate::modules::auth::models::User;
use crate::modules::auth::permissions::UserPermissions;
use crate::permissions::registry::codes;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;
use crate::error::AppError;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
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
) -> Result<User, AppError> {
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
    
    // Get user from database
    sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = $1"
    )
    .bind(uuid::Uuid::parse_str(&claims.sub).unwrap())
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::InternalServerError("ไม่สามารถดึงข้อมูลผู้ใช้ได้".to_string()))
}

async fn get_db_pool(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })
}

// ===================================================================
// Handlers
// ===================================================================

pub async fn list_achievements(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<AchievementListFilter>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_db_pool(&state, &headers).await?;
    let user = get_authenticated_user(&headers, &pool).await?;

    // Check Permissions
    let can_read_all = user.has_permission(&pool, codes::ACHIEVEMENT_READ_ALL).await.unwrap_or(false);
    let can_read_own = user.has_permission(&pool, codes::ACHIEVEMENT_READ_OWN).await.unwrap_or(false);

    if !can_read_all && !can_read_own {
         return Err(AppError::Forbidden("คุณไม่มีสิทธิ์ดูผลงาน".to_string()));
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

    let items = sqlx::query_as::<_, Achievement>(&query)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
        })?;

    Ok((StatusCode::OK, Json(json!({ "success": true, "data": items }))))
}

pub async fn create_achievement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAchievementRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_db_pool(&state, &headers).await?;
    let user = get_authenticated_user(&headers, &pool).await?;

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
        return Err(AppError::Forbidden("คุณไม่มีสิทธิ์เพิ่มผลงานนี้".to_string()));
    }

    // Insert
    let achievement = sqlx::query_as::<_, Achievement>(
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
    .await
    .map_err(|e| {
        eprintln!("❌ Create achievement error: {}", e);
        AppError::InternalServerError("บันทึกข้อมูลไม่สำเร็จ".to_string())
    })?;

    Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": achievement }))))
}

pub async fn update_achievement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAchievementRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_db_pool(&state, &headers).await?;
    let user = get_authenticated_user(&headers, &pool).await?;

    // Get existing achievement owner to check permission
    #[derive(sqlx::FromRow)]
    struct AchievementOwnership {
        user_id: Uuid,
    }

    let existing = sqlx::query_as::<_, AchievementOwnership>("SELECT user_id FROM staff_achievements WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError("Database error".to_string()))?
        .ok_or(AppError::NotFound("ไม่พบข้อมูล".to_string()))?;

    let is_own = existing.user_id == user.id;

    // Check Permissions
    let allowed = if is_own {
        user.has_permission(&pool, codes::ACHIEVEMENT_UPDATE_OWN).await.unwrap_or(false)
    } else {
        user.has_permission(&pool, codes::ACHIEVEMENT_UPDATE_ALL).await.unwrap_or(false)
    };

    if !allowed {
        return Err(AppError::Forbidden("คุณไม่มีสิทธิ์แก้ไขข้อมูลนี้".to_string()));
    }

    // Build Update Query
    let updated = sqlx::query_as::<_, Achievement>(
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
    .await
    .map_err(|e| {
        eprintln!("❌ Update achievement error: {}", e);
        AppError::InternalServerError("แก้ไขข้อมูลไม่สำเร็จ".to_string())
    })?;

    Ok((StatusCode::OK, Json(json!({ "success": true, "data": updated }))))
}

pub async fn delete_achievement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_db_pool(&state, &headers).await?;
    let user = get_authenticated_user(&headers, &pool).await?;

    // Get existing achievement owner for permission check
    #[derive(sqlx::FromRow)]
    struct AchievementOwnership {
        user_id: Uuid,
    }

    let existing = sqlx::query_as::<_, AchievementOwnership>("SELECT user_id FROM staff_achievements WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| AppError::InternalServerError("Database error".to_string()))?
        .ok_or(AppError::NotFound("ไม่พบข้อมูล".to_string()))?;

    let is_own = existing.user_id == user.id;

    // Check Permissions
    let allowed = if is_own {
        user.has_permission(&pool, codes::ACHIEVEMENT_DELETE_OWN).await.unwrap_or(false)
    } else {
        user.has_permission(&pool, codes::ACHIEVEMENT_DELETE_ALL).await.unwrap_or(false)
    };

    if !allowed {
        return Err(AppError::Forbidden("คุณไม่มีสิทธิ์ลบข้อมูลนี้".to_string()));
    }

    sqlx::query("DELETE FROM staff_achievements WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Delete achievement error: {}", e);
            AppError::InternalServerError("ลบข้อมูลไม่สำเร็จ".to_string())
        })?;

    Ok((StatusCode::OK, Json(json!({ "success": true }))))
}
