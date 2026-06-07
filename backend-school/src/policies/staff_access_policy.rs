use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::permissions::registry::codes;
use crate::policies::resource_access_policy;

const STAFF_PROFILE_ACCESS: resource_access_policy::ResourceAccessPermissions =
    resource_access_policy::ResourceAccessPermissions {
        own: Some(codes::STAFF_PROFILE_READ_OWN),
        assigned: None,
        organization_unit: Some(codes::STAFF_PROFILE_READ_ORGANIZATION_UNIT),
        organization_tree: Some(codes::STAFF_PROFILE_READ_ORGANIZATION_TREE),
        school: Some(codes::STAFF_PROFILE_READ_SCHOOL),
    };

const STAFF_PII_ACCESS: resource_access_policy::ResourceAccessPermissions =
    resource_access_policy::ResourceAccessPermissions {
        own: Some(codes::STAFF_PII_READ_OWN),
        assigned: None,
        organization_unit: None,
        organization_tree: None,
        school: Some(codes::STAFF_PII_READ_SCHOOL),
    };

pub async fn can_read_staff_profile(
    pool: &PgPool,
    actor: &ActorContext,
    target_user_id: Uuid,
) -> Result<(), AppError> {
    resource_access_policy::require_user_resource_access(
        pool,
        actor,
        STAFF_PROFILE_ACCESS,
        target_user_id,
        "ไม่มีสิทธิ์ดูข้อมูลบุคลากรนี้",
    )
    .await
    .map(|_| ())
}

pub fn resolve_staff_profile_list_access(
    actor: &ActorContext,
) -> Result<resource_access_policy::UserResourceListAccess, AppError> {
    if actor.has_any_permission(&[codes::STAFF_READ_ALL, codes::ACHIEVEMENT_CREATE_ALL]) {
        return Ok(resource_access_policy::UserResourceListAccess::School);
    }

    resource_access_policy::resolve_user_resource_list_access(actor, STAFF_PROFILE_ACCESS)
        .ok_or_else(|| AppError::Forbidden("ไม่มีสิทธิ์ดูรายชื่อบุคลากร".to_string()))
}

pub fn can_read_staff_pii(actor: &ActorContext, target_user_id: Uuid) -> bool {
    let target = resource_access_policy::ResourceAccessTarget::owned_by(target_user_id);
    resource_access_policy::can_access_direct_resource(actor, STAFF_PII_ACCESS, &target).is_some()
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
