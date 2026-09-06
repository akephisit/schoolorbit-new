use super::resource_access_policy::{
    self, AcademicResourceAccess, AcademicResourceListFilter, AcademicResourcePermissions,
};
use crate::{error::AppError, middleware::permission::ActorContext, permissions::registry::codes};
use sqlx::PgPool;
use uuid::Uuid;

pub fn can_manage_school(actor: &ActorContext) -> bool {
    actor.has_permission(codes::ACADEMIC_GRADEBOOK_MANAGE_SCHOOL)
}
pub fn can_manage_group(actor: &ActorContext, assigned: bool) -> bool {
    can_manage_school(actor)
        || (assigned && actor.has_permission(codes::ACADEMIC_GRADEBOOK_MANAGE_ASSIGNED))
}
pub fn can_confirm_group_phase(actor: &ActorContext, primary: bool) -> bool {
    primary && can_manage_group(actor, true)
}
pub fn can_read_group(
    filter: &AcademicResourceListFilter,
    owner: Option<Uuid>,
    assigned: bool,
) -> bool {
    resource_access_policy::academic_resource_access_for(filter, owner, assigned)
        != AcademicResourceAccess::None
}
pub async fn list_access(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<AcademicResourceListFilter, AppError> {
    let filter = resource_access_policy::resolve_academic_resource_list_filter(
        pool,
        actor,
        AcademicResourcePermissions {
            assigned: &[
                codes::ACADEMIC_GRADEBOOK_READ_ASSIGNED,
                codes::ACADEMIC_GRADEBOOK_MANAGE_ASSIGNED,
            ],
            organization_unit: &[codes::ACADEMIC_GRADEBOOK_READ_ORGANIZATION_UNIT],
            organization_tree: &[],
            school: &[
                codes::ACADEMIC_GRADEBOOK_READ_SCHOOL,
                codes::ACADEMIC_GRADEBOOK_MANAGE_SCHOOL,
            ],
        },
    )
    .await?;
    if !filter.includes_school_owned
        && filter.assigned_actor_id.is_none()
        && filter.organization_unit_ids.is_empty()
    {
        return Err(AppError::Forbidden("Gradebook access denied".into()));
    }
    Ok(filter)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_assigned_and_exact_unit_scopes_are_unioned() {
        let unit = Uuid::new_v4();
        let filter = AcademicResourceListFilter {
            assigned_actor_id: Some(Uuid::new_v4()),
            organization_unit_ids: vec![unit],
            ..Default::default()
        };
        assert!(can_read_group(&filter, Some(unit), false));
        assert!(can_read_group(&filter, None, true));
        assert!(!can_read_group(&filter, Some(Uuid::new_v4()), false));
    }
    #[test]
    fn school_management_does_not_replace_primary_confirmation_role() {
        let actor = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![codes::ACADEMIC_GRADEBOOK_MANAGE_SCHOOL.into()],
        };
        assert!(can_manage_group(&actor, false));
        assert!(!can_confirm_group_phase(&actor, false));
        assert!(can_confirm_group_phase(&actor, true));
    }
}
