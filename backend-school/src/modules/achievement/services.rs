use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::permissions::registry::codes;

use super::models::{
    Achievement, AchievementListFilter, CreateAchievementRequest, UpdateAchievementRequest,
};

pub async fn list_achievements(
    pool: &PgPool,
    actor: &ActorContext,
    filter: AchievementListFilter,
) -> Result<Vec<Achievement>, AppError> {
    let can_read_all = actor.has_permission(codes::ACHIEVEMENT_READ_ALL);
    let can_read_own = actor.has_permission(codes::ACHIEVEMENT_READ_OWN);

    if !can_read_all && !can_read_own {
        return Err(AppError::Forbidden("คุณไม่มีสิทธิ์ดูผลงาน".to_string()));
    }

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

    let scoped_user_id = if can_read_all {
        filter.user_id
    } else {
        Some(actor.user_id)
    };

    if scoped_user_id.is_some() {
        idx += 1;
        query.push_str(&format!(" AND a.user_id = ${idx}"));
    }

    if filter.start_date.is_some() {
        idx += 1;
        query.push_str(&format!(" AND a.achievement_date >= ${idx}"));
    }
    if filter.end_date.is_some() {
        idx += 1;
        query.push_str(&format!(" AND a.achievement_date <= ${idx}"));
    }

    query.push_str(" ORDER BY a.achievement_date DESC, a.created_at DESC");

    let mut query_builder = sqlx::query_as::<_, Achievement>(&query);
    if let Some(user_id) = scoped_user_id {
        query_builder = query_builder.bind(user_id);
    }
    if let Some(start) = filter.start_date {
        query_builder = query_builder.bind(start);
    }
    if let Some(end) = filter.end_date {
        query_builder = query_builder.bind(end);
    }

    query_builder.fetch_all(pool).await.map_err(|e| {
        eprintln!("Failed to list achievements: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
    })
}

pub async fn create_achievement(
    pool: &PgPool,
    actor: &ActorContext,
    payload: CreateAchievementRequest,
) -> Result<Achievement, AppError> {
    let target_user_id = target_achievement_user_id(payload.user_id, actor.user_id);
    let is_own = target_user_id == actor.user_id;
    let allowed = achievement_scope_allowed(
        is_own,
        actor.has_permission(codes::ACHIEVEMENT_CREATE_OWN),
        actor.has_permission(codes::ACHIEVEMENT_CREATE_ALL),
    );

    if !allowed {
        return Err(AppError::Forbidden("คุณไม่มีสิทธิ์เพิ่มผลงานนี้".to_string()));
    }

    sqlx::query_as::<_, Achievement>(
        "INSERT INTO staff_achievements (user_id, title, description, achievement_date, image_path, created_by)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *,
         NULL::text as user_first_name,
         NULL::text as user_last_name,
         NULL::text as user_profile_image_url",
    )
    .bind(target_user_id)
    .bind(&payload.title)
    .bind(&payload.description)
    .bind(payload.achievement_date)
    .bind(&payload.image_path)
    .bind(actor.user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to create achievement: {}", e);
        AppError::InternalServerError("บันทึกข้อมูลไม่สำเร็จ".to_string())
    })
}

pub async fn update_achievement(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    payload: UpdateAchievementRequest,
) -> Result<Achievement, AppError> {
    let owner_id = get_achievement_owner_id(pool, id).await?;
    let allowed = achievement_scope_allowed(
        owner_id == actor.user_id,
        actor.has_permission(codes::ACHIEVEMENT_UPDATE_OWN),
        actor.has_permission(codes::ACHIEVEMENT_UPDATE_ALL),
    );

    if !allowed {
        return Err(AppError::Forbidden("คุณไม่มีสิทธิ์แก้ไขข้อมูลนี้".to_string()));
    }

    sqlx::query_as::<_, Achievement>(
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
    .fetch_one(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to update achievement: {}", e);
        AppError::InternalServerError("แก้ไขข้อมูลไม่สำเร็จ".to_string())
    })
}

pub async fn delete_achievement(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
) -> Result<(), AppError> {
    let owner_id = get_achievement_owner_id(pool, id).await?;
    let allowed = achievement_scope_allowed(
        owner_id == actor.user_id,
        actor.has_permission(codes::ACHIEVEMENT_DELETE_OWN),
        actor.has_permission(codes::ACHIEVEMENT_DELETE_ALL),
    );

    if !allowed {
        return Err(AppError::Forbidden("คุณไม่มีสิทธิ์ลบข้อมูลนี้".to_string()));
    }

    sqlx::query("DELETE FROM staff_achievements WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to delete achievement: {}", e);
            AppError::InternalServerError("ลบข้อมูลไม่สำเร็จ".to_string())
        })?;

    Ok(())
}

async fn get_achievement_owner_id(pool: &PgPool, id: Uuid) -> Result<Uuid, AppError> {
    sqlx::query_scalar("SELECT user_id FROM staff_achievements WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to get achievement owner: {}", e);
            AppError::InternalServerError("Database error".to_string())
        })?
        .ok_or(AppError::NotFound("ไม่พบข้อมูล".to_string()))
}

fn target_achievement_user_id(requested_user_id: Option<Uuid>, actor_user_id: Uuid) -> Uuid {
    requested_user_id.unwrap_or(actor_user_id)
}

fn achievement_scope_allowed(is_own: bool, can_own: bool, can_all: bool) -> bool {
    if is_own {
        can_own
    } else {
        can_all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_achievement_user_id_defaults_to_actor_for_own_create() {
        let actor_user_id = Uuid::new_v4();

        assert_eq!(
            target_achievement_user_id(None, actor_user_id),
            actor_user_id
        );
    }

    #[test]
    fn target_achievement_user_id_uses_requested_user_for_admin_create() {
        let actor_user_id = Uuid::new_v4();
        let requested_user_id = Uuid::new_v4();

        assert_eq!(
            target_achievement_user_id(Some(requested_user_id), actor_user_id),
            requested_user_id
        );
    }

    #[test]
    fn achievement_scope_permission_uses_own_permission_for_owned_records() {
        assert!(achievement_scope_allowed(true, true, false));
        assert!(!achievement_scope_allowed(true, false, false));
    }

    #[test]
    fn achievement_scope_permission_uses_all_permission_for_other_records() {
        assert!(achievement_scope_allowed(false, false, true));
        assert!(!achievement_scope_allowed(false, true, false));
    }
}
