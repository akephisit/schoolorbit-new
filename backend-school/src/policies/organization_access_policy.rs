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

pub fn can_revoke_organization_delegation(actor: &ActorContext, from_user_id: Uuid) -> bool {
    actor.user_id == from_user_id || actor.has_permission(codes::WILDCARD)
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
