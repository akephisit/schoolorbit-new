use crate::models::WorkflowWindow;
use school_authorization::ActorContext;
use school_authorization::{
    accessible_exact_units_for_permission, accessible_tree_units_for_permission,
};
use school_errors::AppError;
use school_permissions::registry::codes;
use school_permissions::registry::{PermissionDef, ALL_PERMISSIONS};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum WorkflowWindowManageAccess {
    All,
    Permissions(Vec<String>),
}

pub fn resolve_workflow_window_manage_access(actor: &ActorContext) -> WorkflowWindowManageAccess {
    if actor.has_permission(codes::WILDCARD) {
        return WorkflowWindowManageAccess::All;
    }

    WorkflowWindowManageAccess::Permissions(
        actor
            .permissions
            .iter()
            .filter(|permission| eligible_management_permission(permission).is_some())
            .cloned()
            .collect(),
    )
}

pub fn require_workflow_window_manage_permission(
    actor: &ActorContext,
    managed_by_permission: &str,
) -> Result<(), AppError> {
    if eligible_management_permission(managed_by_permission).is_some()
        && actor.has_permission(managed_by_permission)
    {
        return Ok(());
    }

    Err(AppError::Forbidden("ไม่มีสิทธิ์จัดการรอบงานนี้".to_string()))
}

fn eligible_management_permission(code: &str) -> Option<&'static PermissionDef> {
    ALL_PERMISSIONS.iter().find(|permission| {
        permission.code == code
            && permission.action == "manage"
            && matches!(
                permission.scope,
                "school" | "all" | "organization_unit" | "organization_tree"
            )
    })
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
    if matches!(permission.scope, "school" | "all") {
        return Ok(());
    }
    let unit = organization_unit_id
        .ok_or_else(|| AppError::Forbidden("รอบงานแบบหน่วยงานต้องระบุหน่วยงาน".into()))?;
    if actor.has_permission(codes::WILDCARD) {
        return Ok(());
    }
    let units = match permission.scope {
        "organization_unit" => {
            accessible_exact_units_for_permission(pool, actor.user_id, permission.code).await?
        }
        "organization_tree" => {
            accessible_tree_units_for_permission(pool, actor.user_id, permission.code).await?
        }
        _ => return Err(AppError::Forbidden("ขอบเขตสิทธิ์ไม่รองรับรอบงานนี้".into())),
    };
    if units.contains(&unit) {
        Ok(())
    } else {
        Err(AppError::Forbidden("ไม่มีสิทธิ์จัดการรอบงานของหน่วยงานนี้".into()))
    }
}

// module_code is a workflow namespace (e.g. organization_work/correspondence), not a
// permission prefix. Authorization follows the canonical management capability and unit target.
pub async fn filter_manageable_workflow_windows(
    pool: &PgPool,
    actor: &ActorContext,
    windows: Vec<WorkflowWindow>,
) -> Result<Vec<WorkflowWindow>, AppError> {
    let wildcard = actor.has_permission(codes::WILDCARD);
    let mut allowed_by_permission = std::collections::HashMap::new();
    for window in &windows {
        let Some(permission) = eligible_management_permission(&window.managed_by_permission) else {
            continue;
        };
        if wildcard
            || !actor.has_permission(permission.code)
            || allowed_by_permission.contains_key(permission.code)
        {
            continue;
        }
        let units = match permission.scope {
            "organization_unit" => {
                accessible_exact_units_for_permission(pool, actor.user_id, permission.code).await?
            }
            "organization_tree" => {
                accessible_tree_units_for_permission(pool, actor.user_id, permission.code).await?
            }
            _ => Vec::new(),
        };
        allowed_by_permission.insert(permission.code, units);
    }
    Ok(windows
        .into_iter()
        .filter(|window| {
            let Some(permission) = eligible_management_permission(&window.managed_by_permission)
            else {
                return false;
            };
            actor.has_permission(permission.code)
                && match permission.scope {
                    "school" | "all" => true,
                    "organization_unit" | "organization_tree" => {
                        window.organization_unit_id.is_some_and(|id| {
                            wildcard
                                || allowed_by_permission
                                    .get(permission.code)
                                    .is_some_and(|units| units.contains(&id))
                        })
                    }
                    _ => false,
                }
        })
        .collect())
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
    fn permission_coverage_only_canonical_resolvable_management_grants_are_eligible() {
        for code in [
            codes::LEARNING_OFFERING_READ_SCHOOL,
            codes::WILDCARD,
            "invented.manage.school",
            codes::LEARNING_OFFERING_MANAGE_ASSIGNED,
        ] {
            assert!(
                require_workflow_window_manage_permission(&actor(&[codes::WILDCARD]), code)
                    .is_err(),
                "{code}"
            );
        }
        let actor = actor(&[
            codes::LEARNING_OFFERING_READ_SCHOOL,
            codes::LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
        ]);
        assert_eq!(
            resolve_workflow_window_manage_access(&actor),
            WorkflowWindowManageAccess::Permissions(vec![
                codes::LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT.into()
            ])
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
