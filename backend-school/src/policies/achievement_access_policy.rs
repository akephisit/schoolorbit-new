use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::permissions::registry::codes;
use crate::policies::resource_access_policy::{
    self, ResourceAccessPermissions, ResourceAccessTarget, UserResourceListAccess,
};

const ACHIEVEMENT_READ_ACCESS: ResourceAccessPermissions = ResourceAccessPermissions {
    own: Some(codes::ACHIEVEMENT_READ_OWN),
    assigned: None,
    organization_unit: None,
    organization_tree: None,
    school: Some(codes::ACHIEVEMENT_READ_ALL),
};

const ACHIEVEMENT_CREATE_ACCESS: ResourceAccessPermissions = ResourceAccessPermissions {
    own: Some(codes::ACHIEVEMENT_CREATE_OWN),
    assigned: None,
    organization_unit: None,
    organization_tree: None,
    school: Some(codes::ACHIEVEMENT_CREATE_ALL),
};

const ACHIEVEMENT_UPDATE_ACCESS: ResourceAccessPermissions = ResourceAccessPermissions {
    own: Some(codes::ACHIEVEMENT_UPDATE_OWN),
    assigned: None,
    organization_unit: None,
    organization_tree: None,
    school: Some(codes::ACHIEVEMENT_UPDATE_ALL),
};

const ACHIEVEMENT_DELETE_ACCESS: ResourceAccessPermissions = ResourceAccessPermissions {
    own: Some(codes::ACHIEVEMENT_DELETE_OWN),
    assigned: None,
    organization_unit: None,
    organization_tree: None,
    school: Some(codes::ACHIEVEMENT_DELETE_ALL),
};

pub fn resolve_achievement_list_access(
    actor: &ActorContext,
) -> Result<UserResourceListAccess, AppError> {
    resource_access_policy::resolve_user_resource_list_access(actor, ACHIEVEMENT_READ_ACCESS)
        .ok_or_else(|| AppError::Forbidden("คุณไม่มีสิทธิ์ดูผลงาน".to_string()))
}

pub fn can_create_achievement_for(
    actor: &ActorContext,
    target_user_id: Uuid,
) -> Result<(), AppError> {
    require_direct_user_resource_access(
        actor,
        target_user_id,
        ACHIEVEMENT_CREATE_ACCESS,
        "คุณไม่มีสิทธิ์เพิ่มผลงานนี้",
    )
}

pub fn can_update_achievement(actor: &ActorContext, owner_user_id: Uuid) -> Result<(), AppError> {
    require_direct_user_resource_access(
        actor,
        owner_user_id,
        ACHIEVEMENT_UPDATE_ACCESS,
        "คุณไม่มีสิทธิ์แก้ไขข้อมูลนี้",
    )
}

pub fn can_delete_achievement(actor: &ActorContext, owner_user_id: Uuid) -> Result<(), AppError> {
    require_direct_user_resource_access(
        actor,
        owner_user_id,
        ACHIEVEMENT_DELETE_ACCESS,
        "คุณไม่มีสิทธิ์ลบข้อมูลนี้",
    )
}

fn require_direct_user_resource_access(
    actor: &ActorContext,
    owner_user_id: Uuid,
    permissions: ResourceAccessPermissions,
    forbidden_message: &'static str,
) -> Result<(), AppError> {
    let target = ResourceAccessTarget::owned_by(owner_user_id);
    resource_access_policy::can_access_direct_resource(actor, permissions, &target)
        .map(|_| ())
        .ok_or_else(|| AppError::Forbidden(forbidden_message.to_string()))
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
    fn achievement_list_access_prefers_school_scope() {
        let actor = actor(
            Uuid::new_v4(),
            &[codes::ACHIEVEMENT_READ_OWN, codes::ACHIEVEMENT_READ_ALL],
        );

        let access = resolve_achievement_list_access(&actor).expect("school access should resolve");

        assert_eq!(access, UserResourceListAccess::School);
    }

    #[test]
    fn achievement_list_access_supports_own_scope() {
        let user_id = Uuid::new_v4();
        let actor = actor(user_id, &[codes::ACHIEVEMENT_READ_OWN]);

        let access = resolve_achievement_list_access(&actor).expect("own access should resolve");

        assert_eq!(access, UserResourceListAccess::Own(user_id));
    }

    #[test]
    fn achievement_update_own_requires_owned_resource() {
        let actor_user_id = Uuid::new_v4();
        let actor = actor(actor_user_id, &[codes::ACHIEVEMENT_UPDATE_OWN]);

        assert!(can_update_achievement(&actor, actor_user_id).is_ok());
        assert!(can_update_achievement(&actor, Uuid::new_v4()).is_err());
    }

    #[test]
    fn achievement_update_all_allows_other_owner() {
        let actor = actor(Uuid::new_v4(), &[codes::ACHIEVEMENT_UPDATE_ALL]);

        assert!(can_update_achievement(&actor, Uuid::new_v4()).is_ok());
    }
}
