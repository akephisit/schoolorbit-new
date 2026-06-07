use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::permissions::registry::codes;
use crate::policies::resource_access_policy::{
    self, ResourceAccessPermissions, ResourceAccessTarget, UserResourceListAccess,
};

const ACTIVITY_MANAGE_ACCESS: ResourceAccessPermissions = ResourceAccessPermissions {
    own: Some(codes::ACTIVITY_MANAGE_OWN),
    assigned: Some(codes::ACTIVITY_MANAGE_OWN),
    organization_unit: None,
    organization_tree: None,
    school: Some(codes::ACTIVITY_MANAGE_ALL),
};

const ACTIVITY_READ_ACCESS: ResourceAccessPermissions = ResourceAccessPermissions {
    own: Some(codes::ACTIVITY_MANAGE_OWN),
    assigned: Some(codes::ACTIVITY_MANAGE_OWN),
    organization_unit: None,
    organization_tree: None,
    school: Some(codes::ACTIVITY_READ_ALL),
};

pub fn resolve_activity_list_access(
    actor: &ActorContext,
) -> Result<UserResourceListAccess, AppError> {
    if actor.has_permission(codes::ACTIVITY_MANAGE_ALL) {
        return Ok(UserResourceListAccess::School);
    }

    if actor.has_permission(codes::ACTIVITY_READ_ALL) {
        return Ok(UserResourceListAccess::School);
    }

    if actor.has_permission(codes::ACTIVITY_MANAGE_OWN) {
        return Ok(UserResourceListAccess::Own(actor.user_id));
    }

    Err(AppError::Forbidden("ไม่มีสิทธิ์ดูกิจกรรม".to_string()))
}

pub fn can_manage_all_activity(actor: &ActorContext) -> bool {
    actor.has_permission(codes::ACTIVITY_MANAGE_ALL)
}

pub fn can_create_activity_group_for(
    actor: &ActorContext,
    instructor_user_id: Uuid,
) -> Result<(), AppError> {
    let target = ResourceAccessTarget::owned_by(instructor_user_id);
    resource_access_policy::can_access_direct_resource(actor, ACTIVITY_MANAGE_ACCESS, &target)
        .map(|_| ())
        .ok_or_else(|| AppError::Forbidden("ไม่มีสิทธิ์สร้างกิจกรรมนี้".to_string()))
}

pub async fn can_read_activity_group(
    pool: &PgPool,
    actor: &ActorContext,
    group_id: Uuid,
) -> Result<(), AppError> {
    if can_manage_all_activity(actor) {
        return Ok(());
    }

    let target = activity_group_resource_target(pool, group_id).await?;
    resource_access_policy::require_resource_access(
        pool,
        actor,
        ACTIVITY_READ_ACCESS,
        &target,
        "ไม่มีสิทธิ์ดูกิจกรรมนี้",
    )
    .await
    .map(|_| ())
}

pub async fn can_manage_activity_group(
    pool: &PgPool,
    actor: &ActorContext,
    group_id: Uuid,
) -> Result<(), AppError> {
    let target = activity_group_resource_target(pool, group_id).await?;
    resource_access_policy::require_resource_access(
        pool,
        actor,
        ACTIVITY_MANAGE_ACCESS,
        &target,
        "ไม่มีสิทธิ์จัดการกิจกรรมนี้",
    )
    .await
    .map(|_| ())
}

async fn activity_group_resource_target(
    pool: &PgPool,
    group_id: Uuid,
) -> Result<ResourceAccessTarget, AppError> {
    let row: Option<(Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        r#"
        SELECT instructor_id, created_by
        FROM activity_groups
        WHERE id = $1
          AND is_active = true
        "#,
    )
    .bind(group_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load activity group target: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบกิจกรรมได้".to_string())
    })?;

    let Some((instructor_id, created_by)) = row else {
        return Err(AppError::NotFound("ไม่พบกลุ่มกิจกรรม".to_string()));
    };

    let assigned_user_ids = sqlx::query_scalar(
        r#"
        SELECT instructor_id
        FROM activity_group_instructors
        WHERE activity_group_id = $1
        "#,
    )
    .bind(group_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load activity group instructors: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบครูกิจกรรมได้".to_string())
    })?;

    Ok(ResourceAccessTarget {
        owner_user_id: instructor_id.or(created_by),
        assigned_user_ids,
        organization_unit_ids: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(user_id: Uuid, permissions: &[&str]) -> ActorContext {
        ActorContext {
            user_id,
            permissions: permissions
                .iter()
                .map(|permission| permission.to_string())
                .collect(),
        }
    }

    #[test]
    fn activity_list_access_prefers_manage_all_as_school_scope() {
        let actor = actor(
            Uuid::new_v4(),
            &[codes::ACTIVITY_MANAGE_OWN, codes::ACTIVITY_MANAGE_ALL],
        );

        let access = resolve_activity_list_access(&actor).expect("school access should resolve");

        assert_eq!(access, UserResourceListAccess::School);
    }

    #[test]
    fn activity_list_access_supports_manage_own_scope() {
        let actor_user_id = Uuid::new_v4();
        let actor = actor(actor_user_id, &[codes::ACTIVITY_MANAGE_OWN]);

        let access = resolve_activity_list_access(&actor).expect("own access should resolve");

        assert_eq!(access, UserResourceListAccess::Own(actor_user_id));
    }

    #[test]
    fn create_group_own_scope_requires_actor_as_instructor() {
        let actor_user_id = Uuid::new_v4();
        let actor = actor(actor_user_id, &[codes::ACTIVITY_MANAGE_OWN]);

        assert!(can_create_activity_group_for(&actor, actor_user_id).is_ok());
        assert!(can_create_activity_group_for(&actor, Uuid::new_v4()).is_err());
    }

    #[test]
    fn create_group_manage_all_allows_other_instructor() {
        let actor = actor(Uuid::new_v4(), &[codes::ACTIVITY_MANAGE_ALL]);

        assert!(can_create_activity_group_for(&actor, Uuid::new_v4()).is_ok());
    }
}
