use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceAccessScope {
    Own,
    Assigned,
    OrganizationUnit,
    OrganizationTree,
    School,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceAccessGrant {
    pub scope: ResourceAccessScope,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ResourceAccessPermissions {
    pub own: Option<&'static str>,
    pub assigned: Option<&'static str>,
    pub organization_unit: Option<&'static str>,
    pub organization_tree: Option<&'static str>,
    pub school: Option<&'static str>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResourceAccessTarget {
    pub owner_user_id: Option<Uuid>,
    pub assigned_user_ids: Vec<Uuid>,
    pub organization_unit_ids: Vec<Uuid>,
}

impl ResourceAccessTarget {
    pub fn owned_by(owner_user_id: Uuid) -> Self {
        Self {
            owner_user_id: Some(owner_user_id),
            assigned_user_ids: Vec::new(),
            organization_unit_ids: Vec::new(),
        }
    }

    pub fn with_organization_units(mut self, organization_unit_ids: Vec<Uuid>) -> Self {
        self.organization_unit_ids = organization_unit_ids;
        self
    }
}

pub fn can_access_direct_resource(
    actor: &ActorContext,
    permissions: ResourceAccessPermissions,
    target: &ResourceAccessTarget,
) -> Option<ResourceAccessGrant> {
    if has_optional_permission(actor, permissions.school) {
        return Some(ResourceAccessGrant {
            scope: ResourceAccessScope::School,
        });
    }

    if target.owner_user_id == Some(actor.user_id)
        && has_optional_permission(actor, permissions.own)
    {
        return Some(ResourceAccessGrant {
            scope: ResourceAccessScope::Own,
        });
    }

    if target.assigned_user_ids.contains(&actor.user_id)
        && has_optional_permission(actor, permissions.assigned)
    {
        return Some(ResourceAccessGrant {
            scope: ResourceAccessScope::Assigned,
        });
    }

    None
}

pub async fn require_user_resource_access(
    pool: &PgPool,
    actor: &ActorContext,
    permissions: ResourceAccessPermissions,
    target_user_id: Uuid,
    forbidden_message: &'static str,
) -> Result<ResourceAccessGrant, AppError> {
    let target = user_resource_target(pool, target_user_id).await?;
    require_resource_access(pool, actor, permissions, &target, forbidden_message).await
}

pub async fn require_resource_access(
    pool: &PgPool,
    actor: &ActorContext,
    permissions: ResourceAccessPermissions,
    target: &ResourceAccessTarget,
    forbidden_message: &'static str,
) -> Result<ResourceAccessGrant, AppError> {
    if let Some(grant) = can_access_direct_resource(actor, permissions, target) {
        return Ok(grant);
    }

    if !target.organization_unit_ids.is_empty()
        && has_optional_permission(actor, permissions.organization_unit)
        && actor_is_member_of_any_organization_unit(
            pool,
            actor.user_id,
            &target.organization_unit_ids,
        )
        .await?
    {
        return Ok(ResourceAccessGrant {
            scope: ResourceAccessScope::OrganizationUnit,
        });
    }

    if !target.organization_unit_ids.is_empty()
        && has_optional_permission(actor, permissions.organization_tree)
        && actor_organization_tree_contains_any(pool, actor.user_id, &target.organization_unit_ids)
            .await?
    {
        return Ok(ResourceAccessGrant {
            scope: ResourceAccessScope::OrganizationTree,
        });
    }

    Err(AppError::Forbidden(forbidden_message.to_string()))
}

async fn user_resource_target(
    pool: &PgPool,
    target_user_id: Uuid,
) -> Result<ResourceAccessTarget, AppError> {
    let organization_unit_ids = sqlx::query_scalar(
        r#"
        SELECT organization_unit_id
        FROM organization_members
        WHERE user_id = $1
          AND (ended_at IS NULL OR ended_at > CURRENT_DATE)
        "#,
    )
    .bind(target_user_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!(
            "Failed to load resource target organization units: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถตรวจสอบหน่วยงานของข้อมูลได้".to_string())
    })?;

    Ok(ResourceAccessTarget::owned_by(target_user_id)
        .with_organization_units(organization_unit_ids))
}

async fn actor_is_member_of_any_organization_unit(
    pool: &PgPool,
    actor_user_id: Uuid,
    target_organization_unit_ids: &[Uuid],
) -> Result<bool, AppError> {
    sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM organization_members
            WHERE user_id = $1
              AND organization_unit_id = ANY($2)
              AND (ended_at IS NULL OR ended_at > CURRENT_DATE)
        )
        "#,
    )
    .bind(actor_user_id)
    .bind(target_organization_unit_ids)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!(
            "Failed to check organization unit resource access: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถตรวจสอบสิทธิ์หน่วยงานได้".to_string())
    })
}

async fn actor_organization_tree_contains_any(
    pool: &PgPool,
    actor_user_id: Uuid,
    target_organization_unit_ids: &[Uuid],
) -> Result<bool, AppError> {
    sqlx::query_scalar(
        r#"
        WITH RECURSIVE actor_roots AS (
            SELECT organization_unit_id
            FROM organization_members
            WHERE user_id = $1
              AND (ended_at IS NULL OR ended_at > CURRENT_DATE)
        ),
        organization_tree AS (
            SELECT organization_unit_id
            FROM actor_roots
            UNION
            SELECT child.id
            FROM organization_units child
            JOIN organization_tree parent_tree
              ON child.parent_unit_id = parent_tree.organization_unit_id
            WHERE child.is_active = true
        )
        SELECT EXISTS (
            SELECT 1
            FROM organization_tree
            WHERE organization_unit_id = ANY($2)
        )
        "#,
    )
    .bind(actor_user_id)
    .bind(target_organization_unit_ids)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!(
            "Failed to check organization tree resource access: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถตรวจสอบสิทธิ์สายงานได้".to_string())
    })
}

fn has_optional_permission(actor: &ActorContext, permission: Option<&str>) -> bool {
    permission.is_some_and(|permission| actor.has_permission(permission))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::registry::codes;

    fn actor(user_id: Uuid, permissions: &[&str]) -> ActorContext {
        ActorContext {
            user_id,
            permissions: permissions
                .iter()
                .map(|permission| permission.to_string())
                .collect(),
        }
    }

    fn profile_permissions() -> ResourceAccessPermissions {
        ResourceAccessPermissions {
            own: Some(codes::STAFF_PROFILE_READ_OWN),
            assigned: Some(codes::ORGANIZATION_WORK_READ_OWN),
            organization_unit: Some(codes::STAFF_PROFILE_READ_ORGANIZATION_UNIT),
            organization_tree: Some(codes::STAFF_PROFILE_READ_ORGANIZATION_TREE),
            school: Some(codes::STAFF_PROFILE_READ_SCHOOL),
        }
    }

    #[test]
    fn school_scope_allows_any_resource_before_narrower_scopes() {
        let actor = actor(
            Uuid::new_v4(),
            &[
                codes::STAFF_PROFILE_READ_OWN,
                codes::STAFF_PROFILE_READ_SCHOOL,
            ],
        );
        let target = ResourceAccessTarget::owned_by(actor.user_id);

        let grant = can_access_direct_resource(&actor, profile_permissions(), &target)
            .expect("school permission should allow access");

        assert_eq!(grant.scope, ResourceAccessScope::School);
    }

    #[test]
    fn own_scope_requires_the_actor_to_own_the_resource() {
        let actor = actor(Uuid::new_v4(), &[codes::STAFF_PROFILE_READ_OWN]);
        let own_target = ResourceAccessTarget::owned_by(actor.user_id);
        let other_target = ResourceAccessTarget::owned_by(Uuid::new_v4());

        let own_grant = can_access_direct_resource(&actor, profile_permissions(), &own_target)
            .expect("own target should be allowed");
        let other_grant = can_access_direct_resource(&actor, profile_permissions(), &other_target);

        assert_eq!(own_grant.scope, ResourceAccessScope::Own);
        assert!(other_grant.is_none());
    }

    #[test]
    fn assigned_scope_requires_actor_assignment() {
        let actor = actor(Uuid::new_v4(), &[codes::ORGANIZATION_WORK_READ_OWN]);
        let assigned_target = ResourceAccessTarget {
            owner_user_id: Some(Uuid::new_v4()),
            assigned_user_ids: vec![actor.user_id],
            organization_unit_ids: Vec::new(),
        };
        let unassigned_target = ResourceAccessTarget {
            owner_user_id: Some(Uuid::new_v4()),
            assigned_user_ids: vec![Uuid::new_v4()],
            organization_unit_ids: Vec::new(),
        };

        let assigned_grant =
            can_access_direct_resource(&actor, profile_permissions(), &assigned_target)
                .expect("assigned actor should be allowed");
        let unassigned_grant =
            can_access_direct_resource(&actor, profile_permissions(), &unassigned_target);

        assert_eq!(assigned_grant.scope, ResourceAccessScope::Assigned);
        assert!(unassigned_grant.is_none());
    }
}
