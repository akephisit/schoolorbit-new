use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::permissions::registry::codes;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum WorkflowWindowManageAccess {
    All,
    Permissions(Vec<String>),
}

pub fn resolve_workflow_window_manage_access(actor: &ActorContext) -> WorkflowWindowManageAccess {
    if actor.has_permission(codes::WILDCARD) {
        return WorkflowWindowManageAccess::All;
    }

    WorkflowWindowManageAccess::Permissions(actor.permissions.clone())
}

pub fn require_workflow_window_manage_permission(
    actor: &ActorContext,
    managed_by_permission: &str,
) -> Result<(), AppError> {
    if actor.has_permission(managed_by_permission) {
        return Ok(());
    }

    Err(AppError::Forbidden("ไม่มีสิทธิ์จัดการรอบงานนี้".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn actor(permissions: &[&str]) -> ActorContext {
        ActorContext {
            user_id: Uuid::new_v4(),
            permissions: permissions
                .iter()
                .map(|permission| permission.to_string())
                .collect(),
        }
    }

    #[test]
    fn wildcard_actor_can_manage_all_workflow_windows() {
        let actor = actor(&[codes::WILDCARD]);

        assert_eq!(
            resolve_workflow_window_manage_access(&actor),
            WorkflowWindowManageAccess::All
        );
    }

    #[test]
    fn manage_permission_requires_matching_permission() {
        let actor = actor(&[codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL]);

        assert!(require_workflow_window_manage_permission(
            &actor,
            codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL
        )
        .is_ok());
        assert!(
            require_workflow_window_manage_permission(&actor, codes::ROLES_UPDATE_ALL).is_err()
        );
    }
}
