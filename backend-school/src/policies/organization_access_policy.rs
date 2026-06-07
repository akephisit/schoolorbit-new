use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::staff::services::organization_delegation_service;
use crate::permissions::registry::codes;

pub async fn can_approve_organization_work(
    pool: &PgPool,
    actor: &ActorContext,
    organization_unit_id: Uuid,
) -> Result<(), AppError> {
    if has_school_wide_organization_authorization(actor) {
        return Ok(());
    }

    if !actor.has_permission(codes::ORGANIZATION_WORK_APPROVE_ORGANIZATION_UNIT) {
        return Err(AppError::Forbidden("ไม่มีสิทธิ์มอบหมายงานในกลุ่มนี้".to_string()));
    }

    if !organization_delegation_service::is_organization_unit_leader(
        pool,
        actor.user_id,
        organization_unit_id,
    )
    .await?
    {
        return Err(AppError::Forbidden(
            "เฉพาะหัวหน้าหรือรองหัวหน้าหน่วยงานเท่านั้นที่สามารถมอบหมายสิทธิ์ได้".to_string(),
        ));
    }

    Ok(())
}

fn has_school_wide_organization_authorization(actor: &ActorContext) -> bool {
    actor.has_any_permission(&[
        codes::WILDCARD,
        codes::ROLES_ASSIGN_ALL,
        codes::ROLES_UPDATE_ALL,
    ])
}

pub fn can_revoke_organization_delegation(actor: &ActorContext, from_user_id: Uuid) -> bool {
    actor.user_id == from_user_id || actor.has_permission(codes::WILDCARD)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    fn actor(user_id: Uuid, permissions: &[&str]) -> ActorContext {
        ActorContext {
            user_id,
            permissions: permissions
                .iter()
                .map(|permission| permission.to_string())
                .collect(),
        }
    }

    fn disconnected_pool() -> PgPool {
        PgPoolOptions::new()
            .connect_lazy("postgres://invalid:invalid@127.0.0.1:1/invalid")
            .expect("lazy pool should be constructed without connecting")
    }

    #[tokio::test]
    async fn approve_policy_allows_wildcard_without_unit_leader_lookup() {
        let pool = disconnected_pool();
        let actor = actor(Uuid::new_v4(), &[codes::WILDCARD]);

        let result = can_approve_organization_work(&pool, &actor, Uuid::new_v4()).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn approve_policy_allows_role_assignment_admin_without_unit_leader_lookup() {
        let pool = disconnected_pool();
        let actor = actor(Uuid::new_v4(), &[codes::ROLES_ASSIGN_ALL]);

        let result = can_approve_organization_work(&pool, &actor, Uuid::new_v4()).await;

        assert!(result.is_ok());
    }

    #[test]
    fn revoke_policy_allows_delegation_creator() {
        let user_id = Uuid::new_v4();
        let actor = actor(user_id, &[]);

        assert!(can_revoke_organization_delegation(&actor, user_id));
    }

    #[test]
    fn revoke_policy_allows_wildcard_actor() {
        let actor = actor(Uuid::new_v4(), &[codes::WILDCARD]);

        assert!(can_revoke_organization_delegation(&actor, Uuid::new_v4()));
    }

    #[test]
    fn revoke_policy_rejects_unrelated_actor_without_wildcard() {
        let actor = actor(Uuid::new_v4(), &[]);

        assert!(!can_revoke_organization_delegation(&actor, Uuid::new_v4()));
    }
}
