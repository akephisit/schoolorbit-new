use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::permissions::registry::codes;

pub async fn can_read_staff_profile(
    pool: &PgPool,
    actor: &ActorContext,
    target_user_id: Uuid,
) -> Result<(), AppError> {
    if actor.has_permission(codes::STAFF_PROFILE_READ_SCHOOL) {
        return Ok(());
    }

    if actor.user_id == target_user_id && actor.has_permission(codes::STAFF_PROFILE_READ_OWN) {
        return Ok(());
    }

    if actor.has_permission(codes::STAFF_PROFILE_READ_ORGANIZATION_UNIT)
        && shares_active_organization_unit(pool, actor.user_id, target_user_id).await?
    {
        return Ok(());
    }

    if actor.has_permission(codes::STAFF_PROFILE_READ_ORGANIZATION_TREE)
        && target_is_in_actor_organization_tree(pool, actor.user_id, target_user_id).await?
    {
        return Ok(());
    }

    Err(AppError::Forbidden("ไม่มีสิทธิ์ดูข้อมูลบุคลากรนี้".to_string()))
}

pub fn can_read_staff_pii(actor: &ActorContext, target_user_id: Uuid) -> bool {
    if actor.has_permission(codes::STAFF_PII_READ_SCHOOL) {
        return true;
    }

    actor.user_id == target_user_id && actor.has_permission(codes::STAFF_PII_READ_OWN)
}

async fn shares_active_organization_unit(
    pool: &PgPool,
    actor_user_id: Uuid,
    target_user_id: Uuid,
) -> Result<bool, AppError> {
    sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM organization_members actor_member
            JOIN organization_members target_member
              ON target_member.organization_unit_id = actor_member.organization_unit_id
            WHERE actor_member.user_id = $1
              AND target_member.user_id = $2
              AND (actor_member.ended_at IS NULL OR actor_member.ended_at > CURRENT_DATE)
              AND (target_member.ended_at IS NULL OR target_member.ended_at > CURRENT_DATE)
        )
        "#,
    )
    .bind(actor_user_id)
    .bind(target_user_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to check shared organization unit access: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบสิทธิ์หน่วยงานได้".to_string())
    })
}

async fn target_is_in_actor_organization_tree(
    pool: &PgPool,
    actor_user_id: Uuid,
    target_user_id: Uuid,
) -> Result<bool, AppError> {
    sqlx::query_scalar(
        r#"
        WITH RECURSIVE actor_roots AS (
            SELECT organization_unit_id
            FROM organization_members
            WHERE user_id = $1
              AND (ended_at IS NULL OR ended_at > CURRENT_DATE)
        ),
        organization_tree AS (
            SELECT organization_unit_id
            FROM actor_roots
            UNION
            SELECT child.id
            FROM organization_units child
            JOIN organization_tree parent_tree
              ON child.parent_unit_id = parent_tree.organization_unit_id
            WHERE child.is_active = true
        )
        SELECT EXISTS (
            SELECT 1
            FROM organization_members target_member
            WHERE target_member.user_id = $2
              AND target_member.organization_unit_id IN (
                  SELECT organization_unit_id FROM organization_tree
              )
              AND (target_member.ended_at IS NULL OR target_member.ended_at > CURRENT_DATE)
        )
        "#,
    )
    .bind(actor_user_id)
    .bind(target_user_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to check organization tree access: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบสิทธิ์สายงานได้".to_string())
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
    fn pii_read_school_allows_any_target() {
        let actor = actor(Uuid::new_v4(), &[codes::STAFF_PII_READ_SCHOOL]);

        let allowed = can_read_staff_pii(&actor, Uuid::new_v4());

        assert!(allowed);
    }

    #[test]
    fn pii_read_own_only_allows_same_user() {
        let user_id = Uuid::new_v4();
        let actor = actor(user_id, &[codes::STAFF_PII_READ_OWN]);

        let allowed = can_read_staff_pii(&actor, user_id);

        assert!(allowed);
    }
}
