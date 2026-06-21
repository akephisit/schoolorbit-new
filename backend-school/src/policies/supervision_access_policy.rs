use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::supervision::services::SupervisionObservationListAccess;
use crate::permissions::registry::codes;
use crate::policies::resource_access_policy::{
    accessible_organization_unit_ids, require_user_resource_access, ResourceAccessPermissions,
    UserResourceListAccess,
};

pub fn can_manage_school(actor: &ActorContext) -> bool {
    actor.has_permission(codes::SUPERVISION_MANAGE_SCHOOL)
}

pub fn can_manage_organization_unit(actor: &ActorContext) -> bool {
    actor.has_permission(codes::SUPERVISION_MANAGE_ORGANIZATION_UNIT)
}

pub fn can_manage_organization_tree(actor: &ActorContext) -> bool {
    actor.has_permission(codes::SUPERVISION_MANAGE_ORGANIZATION_TREE)
}

pub fn can_request_own(actor: &ActorContext) -> bool {
    actor.has_permission(codes::SUPERVISION_REQUEST_OWN)
}

pub fn can_evaluate_assigned(actor: &ActorContext) -> bool {
    actor.has_permission(codes::SUPERVISION_EVALUATE_ASSIGNED)
}

pub fn can_approve_school(actor: &ActorContext) -> bool {
    actor.has_permission(codes::SUPERVISION_APPROVE_SCHOOL)
}

pub fn require_supervision_access(actor: &ActorContext) -> Result<(), AppError> {
    if actor.has_module_permission("supervision") {
        Ok(())
    } else {
        Err(AppError::Forbidden("ไม่มีสิทธิ์ใช้งานระบบนิเทศการสอน".to_string()))
    }
}

pub fn require_school_report_access(actor: &ActorContext) -> Result<(), AppError> {
    if can_manage_school(actor)
        || can_approve_school(actor)
        || actor.has_permission(codes::SUPERVISION_READ_SCHOOL)
    {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "ไม่มีสิทธิ์ดูรายงานนิเทศทั้งโรงเรียน".to_string(),
        ))
    }
}

pub fn require_manage_school(actor: &ActorContext) -> Result<(), AppError> {
    if can_manage_school(actor) {
        Ok(())
    } else {
        Err(AppError::Forbidden("ไม่มีสิทธิ์จัดการระบบนิเทศการสอน".to_string()))
    }
}

pub fn require_request_own(actor: &ActorContext) -> Result<(), AppError> {
    if can_request_own(actor) {
        Ok(())
    } else {
        Err(AppError::Forbidden("ไม่มีสิทธิ์จองคาบนิเทศของตนเอง".to_string()))
    }
}

pub fn require_evaluate_assigned(actor: &ActorContext) -> Result<(), AppError> {
    if can_evaluate_assigned(actor) {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "ไม่มีสิทธิ์ประเมินรายการนิเทศที่ได้รับมอบหมาย".to_string(),
        ))
    }
}

pub fn require_approve_school(actor: &ActorContext) -> Result<(), AppError> {
    if can_approve_school(actor) {
        Ok(())
    } else {
        Err(AppError::Forbidden("ไม่มีสิทธิ์อนุมัติผลนิเทศการสอน".to_string()))
    }
}

pub async fn resolve_observation_list_access(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<SupervisionObservationListAccess, AppError> {
    let mut access = observation_base_list_access(actor)
        .ok_or_else(|| AppError::Forbidden("ไม่มีสิทธิ์ดูรายการนิเทศ".to_string()))?;

    if access.school {
        return Ok(access);
    }

    if can_manage_organization_tree(actor)
        || actor.has_permission(codes::SUPERVISION_READ_ORGANIZATION_TREE)
    {
        access.organization_unit_ids = accessible_organization_unit_ids(
            pool,
            UserResourceListAccess::OrganizationTree(actor.user_id),
        )
        .await?
        .unwrap_or_default();
    } else if can_manage_organization_unit(actor)
        || actor.has_permission(codes::SUPERVISION_READ_ORGANIZATION_UNIT)
    {
        access.organization_unit_ids = accessible_organization_unit_ids(
            pool,
            UserResourceListAccess::OrganizationUnit(actor.user_id),
        )
        .await?
        .unwrap_or_default();
    }

    Ok(access)
}

pub async fn require_observation_management_access(
    pool: &PgPool,
    actor: &ActorContext,
    observed_user_id: Uuid,
) -> Result<(), AppError> {
    require_user_resource_access(
        pool,
        actor,
        supervision_manage_permissions(),
        observed_user_id,
        "ไม่มีสิทธิ์จัดการคำขอนิเทศนี้",
    )
    .await
    .map(|_| ())
}

pub async fn require_observation_read_access(
    pool: &PgPool,
    actor: &ActorContext,
    observed_user_id: Uuid,
    evaluator_user_ids: &[Uuid],
) -> Result<(), AppError> {
    if can_manage_school(actor) || can_approve_school(actor) {
        return Ok(());
    }

    if evaluator_user_ids.contains(&actor.user_id)
        && actor.has_permission(codes::SUPERVISION_READ_ASSIGNED)
    {
        return Ok(());
    }

    if require_observation_management_access(pool, actor, observed_user_id)
        .await
        .is_ok()
    {
        return Ok(());
    }

    require_user_resource_access(
        pool,
        actor,
        supervision_read_permissions(),
        observed_user_id,
        "ไม่มีสิทธิ์ดูรายการนิเทศนี้",
    )
    .await
    .map(|_| ())
}

fn observation_base_list_access(actor: &ActorContext) -> Option<SupervisionObservationListAccess> {
    if can_manage_school(actor)
        || can_approve_school(actor)
        || actor.has_permission(codes::SUPERVISION_READ_SCHOOL)
    {
        return Some(SupervisionObservationListAccess::school());
    }

    let mut access = SupervisionObservationListAccess::default();

    if actor.has_permission(codes::SUPERVISION_READ_OWN) {
        access.own_user_id = Some(actor.user_id);
    }

    if actor.has_permission(codes::SUPERVISION_READ_ASSIGNED) {
        access.assigned_user_id = Some(actor.user_id);
    }

    if can_manage_organization_tree(actor)
        || can_manage_organization_unit(actor)
        || actor.has_permission(codes::SUPERVISION_READ_ORGANIZATION_TREE)
        || actor.has_permission(codes::SUPERVISION_READ_ORGANIZATION_UNIT)
    {
        return Some(access);
    }

    if access.is_empty() {
        None
    } else {
        Some(access)
    }
}

fn supervision_read_permissions() -> ResourceAccessPermissions {
    ResourceAccessPermissions {
        own: Some(codes::SUPERVISION_READ_OWN),
        assigned: Some(codes::SUPERVISION_READ_ASSIGNED),
        organization_unit: Some(codes::SUPERVISION_READ_ORGANIZATION_UNIT),
        organization_tree: Some(codes::SUPERVISION_READ_ORGANIZATION_TREE),
        school: Some(codes::SUPERVISION_READ_SCHOOL),
    }
}

fn supervision_manage_permissions() -> ResourceAccessPermissions {
    ResourceAccessPermissions {
        own: None,
        assigned: None,
        organization_unit: Some(codes::SUPERVISION_MANAGE_ORGANIZATION_UNIT),
        organization_tree: Some(codes::SUPERVISION_MANAGE_ORGANIZATION_TREE),
        school: Some(codes::SUPERVISION_MANAGE_SCHOOL),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(permissions: &[&str]) -> ActorContext {
        actor_with_id(Uuid::new_v4(), permissions)
    }

    fn actor_with_id(user_id: Uuid, permissions: &[&str]) -> ActorContext {
        ActorContext {
            user_id,
            permissions: permissions
                .iter()
                .map(|permission| permission.to_string())
                .collect(),
        }
    }

    #[test]
    fn manage_school_grants_school_level_control() {
        let actor = actor(&[codes::SUPERVISION_MANAGE_SCHOOL]);

        assert!(can_manage_school(&actor));
        assert!(!can_manage_organization_unit(&actor));
        assert!(!can_manage_organization_tree(&actor));
        assert!(require_supervision_access(&actor).is_ok());
        assert!(require_school_report_access(&actor).is_ok());
        assert!(require_manage_school(&actor).is_ok());
    }

    #[test]
    fn organization_unit_management_is_scoped_management_not_school_management() {
        let actor = actor(&[codes::SUPERVISION_MANAGE_ORGANIZATION_UNIT]);

        assert!(can_manage_organization_unit(&actor));
        assert!(!can_manage_school(&actor));
        assert!(require_supervision_access(&actor).is_ok());
        assert!(require_school_report_access(&actor).is_err());
        assert!(require_manage_school(&actor).is_err());
    }

    #[test]
    fn organization_tree_management_is_scoped_management_not_school_management() {
        let actor = actor(&[codes::SUPERVISION_MANAGE_ORGANIZATION_TREE]);

        assert!(can_manage_organization_tree(&actor));
        assert!(!can_manage_school(&actor));
        assert!(require_supervision_access(&actor).is_ok());
        assert!(require_school_report_access(&actor).is_err());
        assert!(require_manage_school(&actor).is_err());
    }

    #[test]
    fn request_own_is_separate_from_manage_school() {
        let actor = actor(&[codes::SUPERVISION_REQUEST_OWN]);

        assert!(can_request_own(&actor));
        assert!(require_manage_school(&actor).is_err());
    }

    #[test]
    fn assigned_evaluation_requires_assigned_permission() {
        let actor = actor(&[codes::SUPERVISION_EVALUATE_ASSIGNED]);

        assert!(can_evaluate_assigned(&actor));
        assert!(require_evaluate_assigned(&actor).is_ok());
    }

    #[test]
    fn observation_list_access_includes_own_and_assigned_scopes() {
        let user_id = Uuid::new_v4();
        let actor = actor_with_id(
            user_id,
            &[
                codes::SUPERVISION_READ_OWN,
                codes::SUPERVISION_READ_ASSIGNED,
            ],
        );

        let access =
            observation_base_list_access(&actor).expect("read scopes should produce access");

        assert!(!access.school);
        assert_eq!(access.own_user_id, Some(user_id));
        assert_eq!(access.assigned_user_id, Some(user_id));
    }
}
