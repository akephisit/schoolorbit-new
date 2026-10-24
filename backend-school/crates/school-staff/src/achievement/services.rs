use school_database::DbSession;
use sqlx::{PgPool, Postgres};
use uuid::Uuid;

use crate::achievement::access_policy as achievement_access_policy;
use school_authorization::resource_access_policy::UserResourceListAccess;
use school_authorization::ActorContext;
use school_errors::AppError;

use super::models::{
    Achievement, AchievementListFilter, CreateAchievementRequest, UpdateAchievementRequest,
};

pub struct AchievementMutationResult {
    pub achievement: Achievement,
    pub replaced_file_id: Option<Uuid>,
}

pub async fn list_achievements(
    pool: &PgPool,
    actor: &ActorContext,
    filter: AchievementListFilter,
) -> Result<Vec<Achievement>, AppError> {
    let _preflight_access = achievement_access_policy::resolve_achievement_list_access(actor)?;

    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "staff.achievement.services.list_achievements",
    )
    .await?;
    list_achievements_on_connection(&mut connection, actor, filter).await
}

pub async fn list_achievements_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    filter: AchievementListFilter,
) -> Result<Vec<Achievement>, AppError> {
    let access = achievement_access_policy::resolve_achievement_list_access(actor)?;

    let mut query = String::from(
        "
        SELECT
            a.*,
            u.first_name as user_first_name,
            u.display_name as user_display_name,
            u.last_name as user_last_name,
            u.profile_image_file_id as user_profile_image_file_id
        FROM staff_achievements a
        LEFT JOIN user_display_names u ON a.user_id = u.id
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

    let mut db_session_query_builder = DbSession::new(connection);
    let mut query_builder = db_session_query_builder.query_as::<Achievement>(
        "staff.achievement.services.list_achievements_on_connection",
        sqlx::AssertSqlSafe(query),
    );
    if let Some(user_id) = scoped_user_id {
        query_builder = query_builder.bind(user_id);
    }
    if let Some(start) = filter.start_date {
        query_builder = query_builder.bind(start);
    }
    if let Some(end) = filter.end_date {
        query_builder = query_builder.bind(end);
    }

    query_builder.fetch_all().await.map_err(|e| {
        tracing::error!(
            "Failed to list achievements: {}",
            school_database::error_class(&e)
        );
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
    if payload.image_file_id.is_some() {
        let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
            pool,
            "staff.achievement.services.create_achievement.preflight",
        )
        .await?;
        validate_achievement_image(&mut connection, payload.image_file_id, target_user_id).await?;
    }

    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "staff.achievement.services.create_achievement",
    )
    .await
    .map_err(achievement_write_error)?;
    validate_achievement_image(&mut transaction, payload.image_file_id, target_user_id).await?;
    let achievement = DbSession::new(&mut *transaction).query_as::<Achievement>("staff.achievement.services.create_achievement",
        "INSERT INTO staff_achievements (user_id, title, description, achievement_date, image_file_id, created_by)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *,
         NULL::text as user_first_name,
         NULL::text as user_display_name,
         NULL::text as user_last_name,
         NULL::uuid as user_profile_image_file_id",
    )
    .bind(target_user_id)
    .bind(&payload.title)
    .bind(&payload.description)
    .bind(payload.achievement_date)
    .bind(&payload.image_file_id)
    .bind(actor.user_id)
    .fetch_one()
    .await
    .map_err(achievement_write_error)?;
    finalize_achievement_image(&mut transaction, payload.image_file_id).await?;
    transaction
        .commit()
        .await
        .map_err(achievement_write_error)?;
    Ok(achievement)
}

pub async fn update_achievement(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    payload: UpdateAchievementRequest,
) -> Result<AchievementMutationResult, AppError> {
    let (owner_id, old_file_id) = {
        let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
            pool,
            "staff.achievement.services.update_achievement.preflight",
        )
        .await?;
        let (owner_id, old_file_id) =
            get_achievement_owner_and_file_id(&mut connection, id).await?;
        achievement_access_policy::can_update_achievement(actor, owner_id)?;
        validate_achievement_image(&mut connection, payload.image_file_id, owner_id).await?;
        (owner_id, old_file_id)
    };

    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "staff.achievement.services.update_achievement",
    )
    .await
    .map_err(achievement_write_error)?;
    validate_achievement_image(&mut transaction, payload.image_file_id, owner_id).await?;
    let achievement = DbSession::new(&mut *transaction)
        .query_as::<Achievement>(
            "staff.achievement.services.update_achievement",
            "UPDATE staff_achievements SET
            title = COALESCE($1, title),
            description = COALESCE($2, description),
            achievement_date = COALESCE($3, achievement_date),
            image_file_id = COALESCE($4, image_file_id),
            updated_at = NOW()
         WHERE id = $5
         RETURNING *,
         NULL::text as user_first_name,
         NULL::text as user_display_name,
         NULL::text as user_last_name,
         NULL::uuid as user_profile_image_file_id",
        )
        .bind(payload.title)
        .bind(payload.description)
        .bind(payload.achievement_date)
        .bind(payload.image_file_id)
        .bind(id)
        .fetch_one()
        .await
        .map_err(achievement_write_error)?;
    finalize_achievement_image(&mut transaction, payload.image_file_id).await?;
    transaction
        .commit()
        .await
        .map_err(achievement_write_error)?;
    Ok(AchievementMutationResult {
        achievement,
        replaced_file_id: (payload.image_file_id.is_some() && old_file_id != payload.image_file_id)
            .then_some(old_file_id)
            .flatten(),
    })
}

pub async fn delete_achievement(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
) -> Result<Option<Uuid>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "staff.achievement.services.delete_achievement",
    )
    .await?;
    delete_achievement_on_connection(&mut connection, actor, id).await
}

pub async fn delete_achievement_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    id: Uuid,
) -> Result<Option<Uuid>, AppError> {
    let (owner_id, file_id) = get_achievement_owner_and_file_id(connection, id).await?;
    achievement_access_policy::can_delete_achievement(actor, owner_id)?;

    DbSession::new(connection)
        .query(
            "staff.achievement.services.delete_achievement_on_connection",
            "DELETE FROM staff_achievements WHERE id = $1",
        )
        .bind(id)
        .execute()
        .await
        .map_err(|e| {
            tracing::error!(
                "Failed to delete achievement: {}",
                school_database::error_class(&e)
            );
            AppError::InternalServerError("ลบข้อมูลไม่สำเร็จ".to_string())
        })?;
    Ok(file_id)
}

async fn get_achievement_owner_and_file_id(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<(Uuid, Option<Uuid>), AppError> {
    DbSession::new(connection)
        .query_as(
            "staff.achievement.services.get_achievement_owner_and_file_id",
            "SELECT user_id, image_file_id FROM staff_achievements WHERE id = $1",
        )
        .bind(id)
        .fetch_optional()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to get achievement file relationship: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("Database error".to_string())
        })?
        .ok_or(AppError::NotFound("ไม่พบข้อมูล".to_string()))
}

async fn validate_achievement_image(
    connection: &mut sqlx::PgConnection,
    file_id: Option<Uuid>,
    owner_user_id: Uuid,
) -> Result<(), AppError> {
    let Some(file_id) = file_id else {
        return Ok(());
    };
    let valid = DbSession::new(connection)
        .scalar::<bool>(
            "staff.achievement.services.validate_achievement_image",
            r#"
SELECT true
    FROM files
    WHERE id = $1
      AND owner_user_id = $2
      AND purpose_code = 'achievement_image'
      AND lifecycle_status = 'ready'
      AND deleted_at IS NULL
    FOR UPDATE
"#,
        )
        .bind(file_id)
        .bind(owner_user_id)
        .fetch_optional()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to validate achievement image: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("Database error".to_string())
        })?;
    if valid == Some(true) {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "ไฟล์รูปผลงานไม่พร้อมใช้งาน".to_string(),
        ))
    }
}

async fn finalize_achievement_image(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    file_id: Option<Uuid>,
) -> Result<(), AppError> {
    let Some(file_id) = file_id else {
        return Ok(());
    };
    DbSession::new(&mut **transaction).query("staff.achievement.services.finalize_achievement_image",
        "UPDATE files SET retention_class = 'standard', expires_at = NULL, updated_at = NOW() WHERE id = $1",
    )
    .bind(file_id)
    .execute()
    .await
    .map_err(achievement_write_error)?;
    Ok(())
}

fn achievement_write_error(error: sqlx::Error) -> AppError {
    tracing::error!(
        "Failed to write achievement file relationship: {}",
        school_database::error_class(&error)
    );
    AppError::InternalServerError("บันทึกข้อมูลไม่สำเร็จ".to_string())
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
