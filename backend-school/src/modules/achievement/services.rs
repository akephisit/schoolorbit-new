use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::policies::achievement_access_policy;
use crate::policies::resource_access_policy::UserResourceListAccess;

use super::models::{
    Achievement, AchievementListFilter, CreateAchievementRequest, UpdateAchievementRequest,
};

pub async fn list_achievements(
    pool: &PgPool,
    actor: &ActorContext,
    filter: AchievementListFilter,
) -> Result<Vec<Achievement>, AppError> {
    let access = achievement_access_policy::resolve_achievement_list_access(actor)?;

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

    let scoped_user_id = achievement_list_user_filter(filter.user_id, access);

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
        tracing::error!("Failed to list achievements: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
    })
}

pub async fn create_achievement(
    pool: &PgPool,
    actor: &ActorContext,
    payload: CreateAchievementRequest,
) -> Result<Achievement, AppError> {
    let target_user_id = target_achievement_user_id(payload.user_id, actor.user_id);
    achievement_access_policy::can_create_achievement_for(actor, target_user_id)?;

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
        tracing::error!("Failed to create achievement: {}", e);
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
    achievement_access_policy::can_update_achievement(actor, owner_id)?;

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
        tracing::error!("Failed to update achievement: {}", e);
        AppError::InternalServerError("แก้ไขข้อมูลไม่สำเร็จ".to_string())
    })
}

pub async fn delete_achievement(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
) -> Result<(), AppError> {
    let owner_id = get_achievement_owner_id(pool, id).await?;
    achievement_access_policy::can_delete_achievement(actor, owner_id)?;

    sqlx::query("DELETE FROM staff_achievements WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete achievement: {}", e);
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
            tracing::error!("Failed to get achievement owner: {}", e);
            AppError::InternalServerError("Database error".to_string())
        })?
        .ok_or(AppError::NotFound("ไม่พบข้อมูล".to_string()))
}

fn achievement_list_user_filter(
    requested_user_id: Option<Uuid>,
    access: UserResourceListAccess,
) -> Option<Uuid> {
    match access {
        UserResourceListAccess::School => requested_user_id,
        UserResourceListAccess::Own(actor_user_id)
        | UserResourceListAccess::Assigned(actor_user_id)
        | UserResourceListAccess::OrganizationUnit(actor_user_id)
        | UserResourceListAccess::OrganizationTree(actor_user_id) => Some(actor_user_id),
    }
}

fn target_achievement_user_id(requested_user_id: Option<Uuid>, actor_user_id: Uuid) -> Uuid {
    requested_user_id.unwrap_or(actor_user_id)
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
    fn achievement_list_user_filter_keeps_requested_filter_for_school_scope() {
        let requested_user_id = Uuid::new_v4();

        assert_eq!(
            achievement_list_user_filter(Some(requested_user_id), UserResourceListAccess::School),
            Some(requested_user_id)
        );
    }

    #[test]
    fn achievement_list_user_filter_forces_actor_for_own_scope() {
        let actor_user_id = Uuid::new_v4();

        assert_eq!(
            achievement_list_user_filter(
                Some(Uuid::new_v4()),
                UserResourceListAccess::Own(actor_user_id)
            ),
            Some(actor_user_id)
        );
    }
}
