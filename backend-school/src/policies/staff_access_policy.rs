use sqlx::PgPool;
use uuid::Uuid;

use crate::policies::resource_access_policy;
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;

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
    resource_access_policy::resolve_user_resource_list_access(actor, STAFF_PROFILE_ACCESS)
        .ok_or_else(|| AppError::Forbidden("ไม่มีสิทธิ์ดูรายชื่อบุคลากร".to_string()))
}

pub fn can_read_staff_pii(actor: &ActorContext, target_user_id: Uuid) -> bool {
    let target = resource_access_policy::ResourceAccessTarget::owned_by(target_user_id);
    resource_access_policy::can_access_direct_resource(actor, STAFF_PII_ACCESS, &target).is_some()
}

pub fn require_reference_read(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_any_permission(&[
        codes::STAFF_PROFILE_READ_OWN,
        codes::STAFF_PROFILE_READ_ORGANIZATION_UNIT,
        codes::STAFF_PROFILE_READ_ORGANIZATION_TREE,
        codes::STAFF_PROFILE_READ_SCHOOL,
        codes::STAFF_CREATE_ALL,
        codes::STAFF_UPDATE_ALL,
    ])
}

pub fn require_reference_write(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_permission(codes::STAFF_UPDATE_ALL)
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

    #[test]
    fn reference_mutation_requires_staff_update() {
        assert!(require_reference_write(&actor(
            Uuid::new_v4(),
            &[codes::STAFF_PROFILE_READ_SCHOOL]
        ))
        .is_err());
        assert!(
            require_reference_write(&actor(Uuid::new_v4(), &[codes::STAFF_CREATE_ALL])).is_err()
        );
        assert!(
            require_reference_write(&actor(Uuid::new_v4(), &[codes::STAFF_UPDATE_ALL])).is_ok()
        );
        assert!(
            require_reference_read(&actor(Uuid::new_v4(), &[codes::STAFF_PROFILE_READ_OWN]))
                .is_ok()
        );
        assert!(require_reference_read(&actor(Uuid::new_v4(), &[])).is_err());
    }

    #[test]
    fn personnel_directory_scope_survives_unrelated_permissions() {
        let id = Uuid::new_v4();
        let a = actor(
            id,
            &[
                codes::STAFF_PROFILE_READ_OWN,
                codes::ACHIEVEMENT_CREATE_ALL,
                codes::STAFF_READ_ALL,
            ],
        );
        assert!(
            matches!(resolve_staff_profile_list_access(&a).unwrap(), resource_access_policy::UserResourceListAccess::Own(user) if user == id)
        );
        assert!(
            resolve_staff_profile_list_access(&actor(id, &[codes::ACHIEVEMENT_CREATE_ALL]))
                .is_err()
        );
    }
}
