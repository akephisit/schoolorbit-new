use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::academic::services::subject_service::{self, SubjectGroupAccess};
use crate::permissions::registry::codes;
use crate::policies::resource_access_policy::{
    self, ResourceAccessPermissions, UserResourceListAccess,
};

pub async fn resolve_subject_read_access(
    actor: &ActorContext,
    pool: &PgPool,
) -> Result<Option<SubjectGroupAccess>, AppError> {
    let Some(access) = resource_access_policy::resolve_user_resource_list_access(
        actor,
        curriculum_read_permissions(actor),
    ) else {
        return Ok(None);
    };

    subject_group_access_from_list_access(pool, access)
        .await
        .map(Some)
}

pub async fn resolve_subject_manage_access(
    actor: &ActorContext,
    pool: &PgPool,
    all_permission: &'static str,
) -> Result<Option<SubjectGroupAccess>, AppError> {
    let Some(access) = resource_access_policy::resolve_user_resource_list_access(
        actor,
        curriculum_manage_permissions(all_permission),
    ) else {
        return Ok(None);
    };

    subject_group_access_from_list_access(pool, access)
        .await
        .map(Some)
}

pub fn ensure_curriculum_read(actor: &ActorContext) -> Result<(), AppError> {
    ensure_curriculum_access(
        actor,
        curriculum_read_permissions(actor),
        codes::ACADEMIC_CURRICULUM_READ_ALL,
    )
}

pub fn ensure_curriculum_create(actor: &ActorContext) -> Result<(), AppError> {
    ensure_curriculum_access(
        actor,
        curriculum_manage_permissions(codes::ACADEMIC_CURRICULUM_CREATE_ALL),
        codes::ACADEMIC_CURRICULUM_CREATE_ALL,
    )
}

pub fn ensure_curriculum_update(actor: &ActorContext) -> Result<(), AppError> {
    ensure_curriculum_access(
        actor,
        curriculum_manage_permissions(codes::ACADEMIC_CURRICULUM_UPDATE_ALL),
        codes::ACADEMIC_CURRICULUM_UPDATE_ALL,
    )
}

pub fn ensure_curriculum_delete(actor: &ActorContext) -> Result<(), AppError> {
    ensure_curriculum_access(
        actor,
        curriculum_manage_permissions(codes::ACADEMIC_CURRICULUM_DELETE_ALL),
        codes::ACADEMIC_CURRICULUM_DELETE_ALL,
    )
}

fn ensure_curriculum_access(
    actor: &ActorContext,
    permissions: ResourceAccessPermissions,
    school_permission: &'static str,
) -> Result<(), AppError> {
    if resource_access_policy::resolve_user_resource_list_access(actor, permissions).is_some() {
        return Ok(());
    }

    Err(AppError::Forbidden(format!("ไม่มีสิทธิ์ {school_permission}")))
}

pub async fn ensure_subject_manage(
    actor: &ActorContext,
    pool: &PgPool,
    subject_id: Uuid,
    manage_code: &'static str,
    read_only: bool,
) -> Result<(), AppError> {
    let access = if read_only && !actor.has_permission(manage_code) {
        resolve_subject_read_access(actor, pool).await?
    } else {
        resolve_subject_manage_access(actor, pool, manage_code).await?
    };
    let Some(access) = access else {
        return Err(AppError::Forbidden(format!("ไม่มีสิทธิ์ {}", manage_code)));
    };

    let subject_group = subject_service::get_subject_group_id(pool, subject_id).await?;
    if !subject_service::subject_group_access_allows(&access, subject_group) {
        return Err(AppError::Forbidden(
            "ไม่สามารถจัดการวิชาในกลุ่มสาระอื่นได้".to_string(),
        ));
    }

    Ok(())
}

fn curriculum_read_permissions(actor: &ActorContext) -> ResourceAccessPermissions {
    let tree_permission =
        if actor.has_permission(codes::ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE) {
            codes::ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE
        } else {
            codes::ACADEMIC_CURRICULUM_READ_ORGANIZATION_TREE
        };

    ResourceAccessPermissions {
        own: None,
        assigned: None,
        organization_unit: Some(codes::ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT),
        organization_tree: Some(tree_permission),
        school: Some(codes::ACADEMIC_CURRICULUM_READ_ALL),
    }
}

fn curriculum_manage_permissions(all_permission: &'static str) -> ResourceAccessPermissions {
    ResourceAccessPermissions {
        own: None,
        assigned: None,
        organization_unit: Some(codes::ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT),
        organization_tree: Some(codes::ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE),
        school: Some(all_permission),
    }
}

async fn subject_group_access_from_list_access(
    pool: &PgPool,
    access: UserResourceListAccess,
) -> Result<SubjectGroupAccess, AppError> {
    let Some(organization_unit_ids) =
        resource_access_policy::accessible_organization_unit_ids(pool, access).await?
    else {
        return Ok(SubjectGroupAccess::All);
    };

    Ok(SubjectGroupAccess::Groups(
        get_subject_group_ids_for_organization_units(pool, &organization_unit_ids).await?,
    ))
}

async fn get_subject_group_ids_for_organization_units(
    pool: &PgPool,
    organization_unit_ids: &[Uuid],
) -> Result<Vec<Uuid>, AppError> {
    if organization_unit_ids.is_empty() {
        return Ok(Vec::new());
    }

    sqlx::query_scalar(
        r#"SELECT DISTINCT subject_group_id
        FROM organization_units
        WHERE id = ANY($1)
          AND subject_group_id IS NOT NULL
          AND is_active = true
        "#,
    )
    .bind(organization_unit_ids)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!(
            "Failed to fetch subject groups for organization units: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถตรวจสอบกลุ่มสาระได้".to_string())
    })
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
    fn curriculum_read_uses_manage_tree_when_actor_has_manage_tree() {
        let actor = actor(
            Uuid::new_v4(),
            &[codes::ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE],
        );

        let permissions = curriculum_read_permissions(&actor);

        assert_eq!(
            permissions.organization_tree,
            Some(codes::ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE)
        );
    }

    #[test]
    fn curriculum_read_uses_read_tree_without_manage_tree() {
        let actor = actor(
            Uuid::new_v4(),
            &[codes::ACADEMIC_CURRICULUM_READ_ORGANIZATION_TREE],
        );

        let permissions = curriculum_read_permissions(&actor);

        assert_eq!(
            permissions.organization_tree,
            Some(codes::ACADEMIC_CURRICULUM_READ_ORGANIZATION_TREE)
        );
    }

    #[test]
    fn curriculum_manage_uses_requested_school_permission() {
        let permissions = curriculum_manage_permissions(codes::ACADEMIC_CURRICULUM_UPDATE_ALL);

        assert_eq!(
            permissions.school,
            Some(codes::ACADEMIC_CURRICULUM_UPDATE_ALL)
        );
    }

    #[test]
    fn curriculum_plan_helpers_accept_the_same_scoped_permissions_as_the_frontend() {
        let scoped_reader = actor(
            Uuid::new_v4(),
            &[codes::ACADEMIC_CURRICULUM_READ_ORGANIZATION_TREE],
        );
        let scoped_manager = actor(
            Uuid::new_v4(),
            &[codes::ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT],
        );

        assert!(ensure_curriculum_read(&scoped_reader).is_ok());
        assert!(ensure_curriculum_create(&scoped_manager).is_ok());
        assert!(ensure_curriculum_update(&scoped_manager).is_ok());
        assert!(ensure_curriculum_delete(&scoped_manager).is_ok());
    }

    #[test]
    fn curriculum_plan_helpers_reject_unrelated_permissions() {
        let unrelated = actor(Uuid::new_v4(), &[codes::ACADEMIC_COURSE_PLAN_READ_ALL]);

        assert!(matches!(
            ensure_curriculum_read(&unrelated),
            Err(AppError::Forbidden(_))
        ));
        assert!(matches!(
            ensure_curriculum_update(&unrelated),
            Err(AppError::Forbidden(_))
        ));
    }
}
