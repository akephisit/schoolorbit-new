//! Shared academic-resource authorization contracts.

use std::collections::BTreeSet;

use school_errors::AppError;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    accessible_exact_units_for_permission, accessible_tree_units_for_permission, ActorContext,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcademicResourceAccess {
    None,
    Assigned,
    OrganizationUnit,
    OrganizationTree,
    School,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AcademicResourceListFilter {
    pub assigned_actor_id: Option<Uuid>,
    pub organization_unit_ids: Vec<Uuid>,
    pub organization_tree_unit_ids: Vec<Uuid>,
    pub includes_school_owned: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct AcademicResourcePermissions {
    pub assigned: &'static [&'static str],
    pub organization_unit: &'static [&'static str],
    pub organization_tree: &'static [&'static str],
    pub school: &'static [&'static str],
}

impl AcademicResourceListFilter {
    pub fn allowed_organization_unit_ids(&self) -> Vec<Uuid> {
        let mut ids = self.organization_unit_ids.clone();
        ids.extend(self.organization_tree_unit_ids.iter().copied());
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

pub fn academic_resource_access_for(
    filter: &AcademicResourceListFilter,
    owning_organization_unit_id: Option<Uuid>,
    actor_is_assigned: bool,
) -> AcademicResourceAccess {
    if filter.includes_school_owned {
        return AcademicResourceAccess::School;
    }

    if let Some(owner_id) = owning_organization_unit_id {
        if filter.organization_unit_ids.contains(&owner_id) {
            return AcademicResourceAccess::OrganizationUnit;
        }
        if filter.organization_tree_unit_ids.contains(&owner_id) {
            return AcademicResourceAccess::OrganizationTree;
        }
    }

    if actor_is_assigned && filter.assigned_actor_id.is_some() {
        return AcademicResourceAccess::Assigned;
    }

    AcademicResourceAccess::None
}

pub async fn resolve_academic_resource_list_filter(
    pool: &PgPool,
    actor: &ActorContext,
    permissions: AcademicResourcePermissions,
) -> Result<AcademicResourceListFilter, AppError> {
    if actor.has_any_permission(permissions.school) {
        return Ok(AcademicResourceListFilter {
            includes_school_owned: true,
            ..AcademicResourceListFilter::default()
        });
    }

    let mut exact_units = BTreeSet::new();
    for permission in permissions.organization_unit {
        if actor.has_permission(permission) {
            exact_units.extend(
                accessible_exact_units_for_permission(pool, actor.user_id, permission).await?,
            );
        }
    }

    let mut tree_units = BTreeSet::new();
    for permission in permissions.organization_tree {
        if actor.has_permission(permission) {
            tree_units.extend(
                accessible_tree_units_for_permission(pool, actor.user_id, permission).await?,
            );
        }
    }

    Ok(AcademicResourceListFilter {
        assigned_actor_id: actor
            .has_any_permission(permissions.assigned)
            .then_some(actor.user_id),
        organization_unit_ids: exact_units.into_iter().collect(),
        organization_tree_unit_ids: tree_units.into_iter().collect(),
        includes_school_owned: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn school_scope_covers_school_and_organization_owned_resources() {
        let filter = AcademicResourceListFilter {
            includes_school_owned: true,
            ..AcademicResourceListFilter::default()
        };

        assert_eq!(
            academic_resource_access_for(&filter, None, false),
            AcademicResourceAccess::School
        );
        assert_eq!(
            academic_resource_access_for(&filter, Some(Uuid::new_v4()), false),
            AcademicResourceAccess::School
        );
    }

    #[test]
    fn independent_scopes_are_preserved_and_deduplicated() {
        let exact_unit = Uuid::new_v4();
        let shared_unit = Uuid::new_v4();
        let tree_unit = Uuid::new_v4();
        let filter = AcademicResourceListFilter {
            assigned_actor_id: Some(Uuid::new_v4()),
            organization_unit_ids: vec![exact_unit, shared_unit],
            organization_tree_unit_ids: vec![shared_unit, tree_unit],
            includes_school_owned: false,
        };
        let mut expected = vec![exact_unit, shared_unit, tree_unit];
        expected.sort_unstable();

        assert_eq!(filter.allowed_organization_unit_ids(), expected);
        assert_eq!(
            academic_resource_access_for(&filter, Some(exact_unit), true),
            AcademicResourceAccess::OrganizationUnit
        );
        assert_eq!(
            academic_resource_access_for(&filter, Some(tree_unit), false),
            AcademicResourceAccess::OrganizationTree
        );
        assert_eq!(
            academic_resource_access_for(&filter, None, true),
            AcademicResourceAccess::Assigned
        );
    }
}
