use school_authorization::ActorContext;
use school_authorization::resource_access_policy::{accessible_organization_unit_ids, require_resource_access, ResourceAccessPermissions, ResourceAccessTarget, UserResourceListAccess};
use school_permissions::registry::{PermissionDef, ALL_PERMISSIONS};
use sqlx::PgPool;
use uuid::Uuid;
use crate::models::WorkflowWindow;
use school_errors::AppError;
use school_permissions::registry::codes;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum WorkflowWindowManageAccess {
    All,
    Permissions(Vec<String>),
}

pub fn resolve_workflow_window_manage_access(actor: &ActorContext) -> WorkflowWindowManageAccess {
    if actor.has_permission(codes::WILDCARD) {
        return WorkflowWindowManageAccess::All;
    }

    WorkflowWindowManageAccess::Permissions(actor.permissions.iter()
        .filter(|permission| eligible_management_permission(permission).is_some())
        .cloned().collect())
}

pub fn require_workflow_window_manage_permission(
    actor: &ActorContext,
    managed_by_permission: &str,
) -> Result<(), AppError> {
    if eligible_management_permission(managed_by_permission).is_some()
        && actor.has_permission(managed_by_permission) {
        return Ok(());
    }

    Err(AppError::Forbidden("ไม่มีสิทธิ์จัดการรอบงานนี้".to_string()))
}

fn eligible_management_permission(code: &str) -> Option<&'static PermissionDef> {
    ALL_PERMISSIONS.iter().find(|permission| permission.code == code
        && permission.action == "manage"
        && matches!(permission.scope, "school" | "all" | "organization_unit" | "organization_tree"))
}

pub async fn require_workflow_window_management_access(
    pool: &PgPool,
    actor: &ActorContext,
    managed_by_permission: &str,
    organization_unit_id: Option<Uuid>,
) -> Result<(), AppError> {
    require_workflow_window_manage_permission(actor, managed_by_permission)?;
    let permission = eligible_management_permission(managed_by_permission)
        .ok_or_else(|| AppError::Forbidden("ไม่มีสิทธิ์จัดการรอบงานนี้".into()))?;
    let permissions = match permission.scope {
        "school" | "all" => return Ok(()),
        "organization_unit" => ResourceAccessPermissions { organization_unit: Some(permission.code), ..Default::default() },
        "organization_tree" => ResourceAccessPermissions { organization_tree: Some(permission.code), ..Default::default() },
        _ => return Err(AppError::Forbidden("ขอบเขตสิทธิ์ไม่รองรับรอบงานนี้".into())),
    };
    let target = ResourceAccessTarget {
        organization_unit_ids: organization_unit_id.into_iter().collect(),
        ..Default::default()
    };
    require_resource_access(pool, actor, permissions, &target, "ไม่มีสิทธิ์จัดการรอบงานของหน่วยงานนี้").await.map(|_| ())
}

// module_code is a workflow namespace (e.g. organization_work/correspondence), not a
// permission prefix. Authorization follows the canonical management capability and unit target.
pub async fn filter_manageable_workflow_windows(
    pool: &PgPool,
    actor: &ActorContext,
    windows: Vec<WorkflowWindow>,
) -> Result<Vec<WorkflowWindow>, AppError> {
    let wildcard = actor.has_permission(codes::WILDCARD);
    let need_units = !wildcard && windows.iter().any(|window| eligible_management_permission(&window.managed_by_permission).is_some_and(|p| p.scope == "organization_unit"));
    let need_tree = !wildcard && windows.iter().any(|window| eligible_management_permission(&window.managed_by_permission).is_some_and(|p| p.scope == "organization_tree"));
    let units = if need_units { accessible_organization_unit_ids(pool, UserResourceListAccess::OrganizationUnit(actor.user_id)).await?.unwrap_or_default() } else { Vec::new() };
    let tree = if need_tree { accessible_organization_unit_ids(pool, UserResourceListAccess::OrganizationTree(actor.user_id)).await?.unwrap_or_default() } else { Vec::new() };
    Ok(windows.into_iter().filter(|window| {
        let Some(permission) = eligible_management_permission(&window.managed_by_permission) else { return false; };
        actor.has_permission(permission.code) && match permission.scope {
            "school" | "all" => true,
            "organization_unit" => window.organization_unit_id.is_some_and(|id| wildcard || units.contains(&id)),
            "organization_tree" => window.organization_unit_id.is_some_and(|id| wildcard || tree.contains(&id)),
            _ => false,
        }
    }).collect())
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
        let actor = actor(&[codes::LEARNING_OFFERING_MANAGE_SCHOOL]);

        assert!(require_workflow_window_manage_permission(
            &actor,
            codes::LEARNING_OFFERING_MANAGE_SCHOOL
        )
        .is_ok());
        assert!(
            require_workflow_window_manage_permission(&actor, codes::ROLES_UPDATE_ALL).is_err()
        );
    }
}
