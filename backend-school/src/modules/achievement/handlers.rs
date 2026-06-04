use crate::error::AppError;
use crate::middleware::permission::load_actor_context;
use crate::modules::achievement::models::*;
use crate::permissions::registry::codes;
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

// ===================================================================
// Helper Functions
// ===================================================================

async fn get_db_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    resolve_tenant_pool(state, headers).await
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
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };

    // Check Permissions
    let can_read_all = actor.has_permission(codes::ACHIEVEMENT_READ_ALL);
    let can_read_own = actor.has_permission(codes::ACHIEVEMENT_READ_OWN);

    if !can_read_all && !can_read_own {
        return Err(AppError::Forbidden("คุณไม่มีสิทธิ์ดูผลงาน".to_string()));
    }

    // Prepare Query
    let mut query = String::from(
        "
        SELECT 
            a.*,
            u.first_name as user_first_name,
            u.last_name as user_last_name,
            u.profile_image_url as user_profile_image_url
        FROM staff_achievements a
        LEFT JOIN users u ON a.user_id = u.id
        WHERE 1=1
    ",
    );

    let mut idx = 0u32;

    // Apply Ownership Filter
    if !can_read_all {
        idx += 1;
        query.push_str(&format!(" AND a.user_id = ${idx}"));
    } else if let Some(_) = filter.user_id {
        idx += 1;
        query.push_str(&format!(" AND a.user_id = ${idx}"));
    }

    // Date Filters
    if let Some(_) = filter.start_date {
        idx += 1;
        query.push_str(&format!(" AND a.achievement_date >= ${idx}"));
    }
    if let Some(_) = filter.end_date {
        idx += 1;
        query.push_str(&format!(" AND a.achievement_date <= ${idx}"));
    }

    query.push_str(" ORDER BY a.achievement_date DESC, a.created_at DESC");

    let mut q = sqlx::query_as::<_, Achievement>(&query);
    if !can_read_all {
        q = q.bind(actor.user_id);
    } else if let Some(target_user_id) = filter.user_id {
        q = q.bind(target_user_id);
    }
    if let Some(start) = filter.start_date {
        q = q.bind(start);
    }
    if let Some(end) = filter.end_date {
        q = q.bind(end);
    }

    let items = q.fetch_all(&pool).await.map_err(|e| {
        eprintln!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
    })?;

    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": items })),
    )
        .into_response())
}

pub async fn create_achievement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAchievementRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_db_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };

    // Determine target user
    let target_user_id = payload.user_id.unwrap_or(actor.user_id);
    let is_own = target_user_id == actor.user_id;

    // Check Permissions
    let allowed = if is_own {
        actor.has_permission(codes::ACHIEVEMENT_CREATE_OWN)
    } else {
        actor.has_permission(codes::ACHIEVEMENT_CREATE_ALL)
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
    .bind(actor.user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Create achievement error: {}", e);
        AppError::InternalServerError("บันทึกข้อมูลไม่สำเร็จ".to_string())
    })?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": achievement })),
    )
        .into_response())
}

pub async fn update_achievement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAchievementRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_db_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };

    // Get existing achievement owner to check permission
    #[derive(sqlx::FromRow)]
    struct AchievementOwnership {
        user_id: Uuid,
    }

    let existing = sqlx::query_as::<_, AchievementOwnership>(
        "SELECT user_id FROM staff_achievements WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?
    .ok_or(AppError::NotFound("ไม่พบข้อมูล".to_string()))?;

    let is_own = existing.user_id == actor.user_id;

    // Check Permissions
    let allowed = if is_own {
        actor.has_permission(codes::ACHIEVEMENT_UPDATE_OWN)
    } else {
        actor.has_permission(codes::ACHIEVEMENT_UPDATE_ALL)
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
         NULL::text as user_profile_image_url",
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

    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": updated })),
    )
        .into_response())
}

pub async fn delete_achievement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_db_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };

    // Get existing achievement owner for permission check
    #[derive(sqlx::FromRow)]
    struct AchievementOwnership {
        user_id: Uuid,
    }

    let existing = sqlx::query_as::<_, AchievementOwnership>(
        "SELECT user_id FROM staff_achievements WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?
    .ok_or(AppError::NotFound("ไม่พบข้อมูล".to_string()))?;

    let is_own = existing.user_id == actor.user_id;

    // Check Permissions
    let allowed = if is_own {
        actor.has_permission(codes::ACHIEVEMENT_DELETE_OWN)
    } else {
        actor.has_permission(codes::ACHIEVEMENT_DELETE_ALL)
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

    Ok((StatusCode::OK, Json(json!({ "success": true, "data": {} }))).into_response())
}
