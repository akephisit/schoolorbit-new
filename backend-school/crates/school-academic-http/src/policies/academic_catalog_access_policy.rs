use sqlx::PgPool;
use uuid::Uuid;

use school_authorization::ActorContext;
use school_authorization::{
    self as resource_access_policy, AcademicResourceAccess, AcademicResourceListFilter,
    AcademicResourcePermissions,
};
use school_errors::AppError;
use school_permissions::registry::codes;

const NO_PERMISSIONS: &[&str] = &[];
const CATALOG_READ_UNIT_PERMISSIONS: &[&str] = &[codes::ACADEMIC_CATALOG_MANAGE_ORGANIZATION_UNIT];
const CATALOG_READ_TREE_PERMISSIONS: &[&str] = &[codes::ACADEMIC_CATALOG_MANAGE_ORGANIZATION_TREE];
const CATALOG_READ_SCHOOL_PERMISSIONS: &[&str] = &[
    codes::ACADEMIC_CATALOG_READ_SCHOOL,
    codes::ACADEMIC_CATALOG_MANAGE_SCHOOL,
];
const CATALOG_MANAGE_UNIT_PERMISSIONS: &[&str] =
    &[codes::ACADEMIC_CATALOG_MANAGE_ORGANIZATION_UNIT];
const CATALOG_MANAGE_TREE_PERMISSIONS: &[&str] =
    &[codes::ACADEMIC_CATALOG_MANAGE_ORGANIZATION_TREE];
const CATALOG_MANAGE_SCHOOL_PERMISSIONS: &[&str] = &[codes::ACADEMIC_CATALOG_MANAGE_SCHOOL];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogResourceRef {
    Subject(Uuid),
    Activity(Uuid),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogAction {
    Read,
    Manage,
}

pub async fn academic_catalog_list_access(
    pool: &PgPool,
    actor: &ActorContext,
    action: CatalogAction,
) -> Result<AcademicResourceListFilter, AppError> {
    resource_access_policy::resolve_academic_resource_list_filter(
        pool,
        actor,
        catalog_permissions(action),
    )
    .await
}

pub async fn academic_catalog_access(
    pool: &PgPool,
    actor: &ActorContext,
    resource: CatalogResourceRef,
    action: CatalogAction,
) -> Result<AcademicResourceAccess, AppError> {
    let owning_organization_unit_id = catalog_resource_owner(pool, resource).await?;
    let filter = academic_catalog_list_access(pool, actor, action).await?;
    Ok(resource_access_policy::academic_resource_access_for(
        &filter,
        owning_organization_unit_id,
        false,
    ))
}

pub async fn require_academic_catalog_list_access(
    pool: &PgPool,
    actor: &ActorContext,
    action: CatalogAction,
) -> Result<AcademicResourceListFilter, AppError> {
    let filter = academic_catalog_list_access(pool, actor, action).await?;
    if !filter.includes_school_owned
        && filter.organization_unit_ids.is_empty()
        && filter.organization_tree_unit_ids.is_empty()
        && filter.assigned_actor_id.is_none()
    {
        Err(AppError::Forbidden(
            "ไม่มีสิทธิ์เข้าถึงคลังวิชาและกิจกรรม".to_string(),
        ))
    } else {
        Ok(filter)
    }
}

pub async fn require_academic_catalog_access(
    pool: &PgPool,
    actor: &ActorContext,
    resource: CatalogResourceRef,
    action: CatalogAction,
) -> Result<(), AppError> {
    if academic_catalog_access(pool, actor, resource, action).await? == AcademicResourceAccess::None
    {
        Err(AppError::Forbidden("ไม่มีสิทธิ์เข้าถึงทรัพยากรนี้".to_string()))
    } else {
        Ok(())
    }
}

fn catalog_permissions(action: CatalogAction) -> AcademicResourcePermissions {
    match action {
        CatalogAction::Read => AcademicResourcePermissions {
            assigned: NO_PERMISSIONS,
            organization_unit: CATALOG_READ_UNIT_PERMISSIONS,
            organization_tree: CATALOG_READ_TREE_PERMISSIONS,
            school: CATALOG_READ_SCHOOL_PERMISSIONS,
        },
        CatalogAction::Manage => AcademicResourcePermissions {
            assigned: NO_PERMISSIONS,
            organization_unit: CATALOG_MANAGE_UNIT_PERMISSIONS,
            organization_tree: CATALOG_MANAGE_TREE_PERMISSIONS,
            school: CATALOG_MANAGE_SCHOOL_PERMISSIONS,
        },
    }
}

async fn catalog_resource_owner(
    pool: &PgPool,
    resource: CatalogResourceRef,
) -> Result<Option<Uuid>, AppError> {
    let owner = match resource {
        CatalogResourceRef::Subject(subject_id) => {
            sqlx::query_scalar("SELECT owning_organization_unit_id FROM subjects WHERE id = $1")
                .bind(subject_id)
                .fetch_optional(pool)
                .await
        }
        CatalogResourceRef::Activity(activity_id) => {
            sqlx::query_scalar("SELECT owning_organization_unit_id FROM activities WHERE id = $1")
                .bind(activity_id)
                .fetch_optional(pool)
                .await
        }
    }
    .map_err(|error| {
        tracing::error!(
            reason = "academic_catalog_owner_query_failed",
            database_error = %error
        );
        AppError::InternalServerError("ไม่สามารถตรวจสอบเจ้าของข้อมูลคลังวิชาได้".to_string())
    })?;

    owner.ok_or_else(|| AppError::NotFound("ไม่พบข้อมูลคลังวิชาหรือกิจกรรม".to_string()))
}

#[derive(Clone, Copy)]
pub enum SubjectGroupRegistryAction {
    Read,
    Create,
    Update,
    Deactivate,
}

/// Registry management is school-level; owning a unit never grants roster or permission administration.
pub fn require_subject_group_registry_access(
    actor: &ActorContext,
    action: SubjectGroupRegistryAction,
) -> Result<(), AppError> {
    if actor.has_permission(codes::ACADEMIC_CATALOG_MANAGE_SCHOOL) {
        return Ok(());
    }
    match action {
        SubjectGroupRegistryAction::Read => actor
            .require_any_permission(&[codes::ROLES_READ_ALL, codes::ACADEMIC_CATALOG_READ_SCHOOL]),
        SubjectGroupRegistryAction::Create => actor.require_permission(codes::ROLES_CREATE_ALL),
        SubjectGroupRegistryAction::Update => actor.require_permission(codes::ROLES_UPDATE_ALL),
        SubjectGroupRegistryAction::Deactivate => {
            actor.require_all_permissions(&[codes::ROLES_UPDATE_ALL, codes::ROLES_DELETE_ALL])
        }
    }
}

#[cfg(test)]
mod learning_area_registry_tests {
    use super::*;
    fn actor(permissions: &[&str]) -> ActorContext {
        ActorContext {
            user_id: Uuid::nil(),
            permissions: permissions.iter().map(|value| value.to_string()).collect(),
        }
    }
    #[test]
    fn learning_area_registry_separates_read_write_and_unit_scopes() {
        use SubjectGroupRegistryAction::*;
        for read in [codes::ACADEMIC_CATALOG_READ_SCHOOL, codes::ROLES_READ_ALL] {
            assert!(require_subject_group_registry_access(&actor(&[read]), Read).is_ok());
            assert!(require_subject_group_registry_access(&actor(&[read]), Create).is_err());
            assert!(require_subject_group_registry_access(&actor(&[read]), Update).is_err());
        }
        for scoped in [
            codes::ACADEMIC_CATALOG_MANAGE_ORGANIZATION_UNIT,
            codes::ACADEMIC_CATALOG_MANAGE_ORGANIZATION_TREE,
        ] {
            for action in [Read, Create, Update, Deactivate] {
                assert!(require_subject_group_registry_access(&actor(&[scoped]), action).is_err());
            }
        }
        for full in [codes::ACADEMIC_CATALOG_MANAGE_SCHOOL, codes::WILDCARD] {
            for action in [Read, Create, Update, Deactivate] {
                assert!(require_subject_group_registry_access(&actor(&[full]), action).is_ok());
            }
        }
        assert!(
            require_subject_group_registry_access(&actor(&[codes::ROLES_UPDATE_ALL]), Update)
                .is_ok()
        );
        assert!(require_subject_group_registry_access(
            &actor(&[codes::ROLES_UPDATE_ALL]),
            Deactivate
        )
        .is_err());
        assert!(require_subject_group_registry_access(
            &actor(&[codes::ROLES_DELETE_ALL]),
            Deactivate
        )
        .is_err());
        assert!(require_subject_group_registry_access(
            &actor(&[codes::ROLES_UPDATE_ALL, codes::ROLES_DELETE_ALL]),
            Deactivate
        )
        .is_ok());
    }
}
