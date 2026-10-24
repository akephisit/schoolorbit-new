use school_database::DbSession;
use sqlx::PgPool;
use uuid::Uuid;

use school_authorization::ActorContext;
use school_authorization::{
    self as resource_access_policy, AcademicResourceAccess, AcademicResourceListFilter,
    AcademicResourcePermissions,
};
use school_errors::AppError;
use school_permissions::registry::codes;

const OFFERING_READ_ASSIGNED_PERMISSIONS: &[&str] = &[
    codes::LEARNING_OFFERING_READ_ASSIGNED,
    codes::LEARNING_OFFERING_MANAGE_ASSIGNED,
];
const OFFERING_READ_UNIT_PERMISSIONS: &[&str] = &[
    codes::LEARNING_OFFERING_READ_ORGANIZATION_UNIT,
    codes::LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
];
const OFFERING_READ_TREE_PERMISSIONS: &[&str] = &[
    codes::LEARNING_OFFERING_READ_ORGANIZATION_TREE,
    codes::LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE,
];
const OFFERING_READ_SCHOOL_PERMISSIONS: &[&str] = &[
    codes::LEARNING_OFFERING_READ_SCHOOL,
    codes::LEARNING_OFFERING_MANAGE_SCHOOL,
];
const OFFERING_MANAGE_ASSIGNED_PERMISSIONS: &[&str] = &[codes::LEARNING_OFFERING_MANAGE_ASSIGNED];
const OFFERING_MANAGE_UNIT_PERMISSIONS: &[&str] =
    &[codes::LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT];
const OFFERING_MANAGE_TREE_PERMISSIONS: &[&str] =
    &[codes::LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE];
const OFFERING_MANAGE_SCHOOL_PERMISSIONS: &[&str] = &[codes::LEARNING_OFFERING_MANAGE_SCHOOL];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OfferingAction {
    Read,
    Manage,
}

pub async fn learning_offering_list_access(
    pool: &PgPool,
    actor: &ActorContext,
    action: OfferingAction,
) -> Result<AcademicResourceListFilter, AppError> {
    resource_access_policy::resolve_academic_resource_list_filter(
        pool,
        actor,
        offering_permissions(action),
    )
    .await
}

pub async fn learning_offering_list_access_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    action: OfferingAction,
) -> Result<AcademicResourceListFilter, AppError> {
    resource_access_policy::resolve_academic_resource_list_filter_on_connection(
        connection,
        actor,
        offering_permissions(action),
    )
    .await
}

pub async fn learning_offering_access(
    pool: &PgPool,
    actor: &ActorContext,
    offering_id: Uuid,
    action: OfferingAction,
) -> Result<AcademicResourceAccess, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_http.policies.learning_offering_access_policy.learning_offering_access",
    )
    .await?;
    learning_offering_access_on_connection(&mut connection, actor, offering_id, action).await
}

pub async fn learning_offering_access_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    offering_id: Uuid,
    action: OfferingAction,
) -> Result<AcademicResourceAccess, AppError> {
    let target: Option<(Option<Uuid>, bool)> = DbSession::new(connection).query_as("academic_http.policies.learning_offering_access_policy.learning_offering_access_on_connection",
        r#"
        SELECT offering.owning_organization_unit_id,
               EXISTS (
                   SELECT 1
                   FROM learning_groups learning_group
                   JOIN learning_group_teachers teacher
                     ON teacher.learning_group_id = learning_group.id
                   WHERE learning_group.learning_offering_id = offering.id
                     AND teacher.teacher_id = $2
               ) AS actor_is_assigned
        FROM learning_offerings offering
        WHERE offering.id = $1
        "#,
    )
    .bind(offering_id)
    .bind(actor.user_id)
    .fetch_optional()
    .await
    .map_err(|error| {
        tracing::error!(
            reason = "learning_offering_access_target_query_failed",
            error_class = school_database::error_class(&error)
        );
        AppError::InternalServerError("ไม่สามารถตรวจสอบสิทธิ์การเปิดสอนได้".to_string())
    })?;

    let Some((owning_organization_unit_id, actor_is_assigned)) = target else {
        return Err(AppError::NotFound("ไม่พบการเปิดสอน".to_string()));
    };

    let filter = learning_offering_list_access_on_connection(connection, actor, action).await?;
    Ok(resource_access_policy::academic_resource_access_for(
        &filter,
        owning_organization_unit_id,
        actor_is_assigned,
    ))
}

pub async fn require_learning_offering_list_access(
    pool: &PgPool,
    actor: &ActorContext,
    action: OfferingAction,
) -> Result<AcademicResourceListFilter, AppError> {
    require_list_filter(learning_offering_list_access(pool, actor, action).await?)
}

pub async fn require_learning_offering_list_access_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    action: OfferingAction,
) -> Result<AcademicResourceListFilter, AppError> {
    let filter = learning_offering_list_access_on_connection(connection, actor, action).await?;
    require_list_filter(filter)
}

fn require_list_filter(
    filter: AcademicResourceListFilter,
) -> Result<AcademicResourceListFilter, AppError> {
    if !filter.includes_school_owned
        && filter.organization_unit_ids.is_empty()
        && filter.organization_tree_unit_ids.is_empty()
        && filter.assigned_actor_id.is_none()
    {
        Err(AppError::Forbidden("ไม่มีสิทธิ์เข้าถึงรายการเปิดสอน".to_string()))
    } else {
        Ok(filter)
    }
}

pub async fn require_learning_offering_access(
    pool: &PgPool,
    actor: &ActorContext,
    offering_id: Uuid,
    action: OfferingAction,
) -> Result<(), AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_http.policies.learning_offering_access_policy.require_learning_offering_access",
    )
    .await?;
    require_learning_offering_access_on_connection(&mut connection, actor, offering_id, action)
        .await
}

pub async fn require_learning_offering_access_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    offering_id: Uuid,
    action: OfferingAction,
) -> Result<(), AppError> {
    if learning_offering_access_on_connection(connection, actor, offering_id, action).await?
        == AcademicResourceAccess::None
    {
        Err(AppError::Forbidden("ไม่มีสิทธิ์เข้าถึงรายการเปิดสอนนี้".to_string()))
    } else {
        Ok(())
    }
}

pub async fn require_learning_offering_batch_access(
    pool: &PgPool,
    actor: &ActorContext,
    offering_ids: &[Uuid],
    action: OfferingAction,
) -> Result<(), AppError> {
    let preflight = resource_access_policy::academic_resource_list_filter_without_database(
        actor,
        offering_permissions(action),
    )
    .map(require_list_filter)
    .transpose()?;
    if offering_ids.is_empty() && preflight.is_some() {
        return Ok(());
    }
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(pool, "academic_http.policies.learning_offering_access_policy.require_learning_offering_batch_access").await?;
    let filter = match preflight {
        Some(filter) => filter,
        None => {
            require_learning_offering_list_access_on_connection(&mut connection, actor, action)
                .await?
        }
    };
    require_batch_with_filter(&mut connection, actor, offering_ids, filter).await
}

pub async fn require_learning_offering_batch_access_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    offering_ids: &[Uuid],
    action: OfferingAction,
) -> Result<(), AppError> {
    let filter =
        require_learning_offering_list_access_on_connection(connection, actor, action).await?;
    require_batch_with_filter(connection, actor, offering_ids, filter).await
}

async fn require_batch_with_filter(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    offering_ids: &[Uuid],
    filter: AcademicResourceListFilter,
) -> Result<(), AppError> {
    let mut unique_ids = offering_ids.to_vec();
    unique_ids.sort_unstable();
    unique_ids.dedup();
    if unique_ids.is_empty() {
        return Ok(());
    }
    let targets: Vec<(Uuid, Option<Uuid>, bool)> = DbSession::new(connection).query_as("academic_http.policies.learning_offering_access_policy.require_learning_offering_batch_access_on_connection",
        r#"SELECT offering.id, offering.owning_organization_unit_id,
                  EXISTS (
                      SELECT 1
                      FROM learning_groups learning_group
                      JOIN learning_group_teachers teacher
                        ON teacher.learning_group_id = learning_group.id
                      WHERE learning_group.learning_offering_id = offering.id
                        AND teacher.teacher_id = $2
                  ) AS actor_is_assigned
           FROM learning_offerings offering
           WHERE offering.id = ANY($1)
           ORDER BY offering.id"#,
    )
    .bind(&unique_ids)
    .bind(actor.user_id)
    .fetch_all()
    .await?;
    if targets.len() != unique_ids.len() {
        return Err(AppError::NotFound(
            "ไม่พบรายการเปิดสอนบางรายการในชุดการเปลี่ยนแปลง".to_string(),
        ));
    }
    if targets.iter().any(|(_, owner_id, actor_is_assigned)| {
        resource_access_policy::academic_resource_access_for(&filter, *owner_id, *actor_is_assigned)
            == AcademicResourceAccess::None
    }) {
        return Err(AppError::Forbidden(
            "ไม่มีสิทธิ์เข้าถึงรายการเปิดสอนบางรายการในชุดการเปลี่ยนแปลง".to_string(),
        ));
    }
    Ok(())
}

pub async fn require_learning_group_access(
    pool: &PgPool,
    actor: &ActorContext,
    learning_group_id: Uuid,
    action: OfferingAction,
) -> Result<Uuid, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_http.policies.learning_offering_access_policy.require_learning_group_access",
    )
    .await?;
    require_learning_group_access_on_connection(&mut connection, actor, learning_group_id, action)
        .await
}

pub async fn require_learning_group_access_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    learning_group_id: Uuid,
    action: OfferingAction,
) -> Result<Uuid, AppError> {
    let offering_id: Uuid =
        DbSession::new(connection).scalar("academic_http.policies.learning_offering_access_policy.require_learning_group_access_on_connection", "SELECT learning_offering_id FROM learning_groups WHERE id = $1")
            .bind(learning_group_id)
            .fetch_optional()
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มเรียน".to_string()))?;
    require_learning_offering_access_on_connection(connection, actor, offering_id, action).await?;
    Ok(offering_id)
}

pub fn learning_offering_owner_allowed(
    filter: &AcademicResourceListFilter,
    owning_organization_unit_id: Uuid,
) -> bool {
    resource_access_policy::academic_resource_access_for(
        filter,
        Some(owning_organization_unit_id),
        false,
    ) != AcademicResourceAccess::None
}

fn offering_permissions(action: OfferingAction) -> AcademicResourcePermissions {
    match action {
        OfferingAction::Read => AcademicResourcePermissions {
            assigned: OFFERING_READ_ASSIGNED_PERMISSIONS,
            organization_unit: OFFERING_READ_UNIT_PERMISSIONS,
            organization_tree: OFFERING_READ_TREE_PERMISSIONS,
            school: OFFERING_READ_SCHOOL_PERMISSIONS,
        },
        OfferingAction::Manage => AcademicResourcePermissions {
            assigned: OFFERING_MANAGE_ASSIGNED_PERMISSIONS,
            organization_unit: OFFERING_MANAGE_UNIT_PERMISSIONS,
            organization_tree: OFFERING_MANAGE_TREE_PERMISSIONS,
            school: OFFERING_MANAGE_SCHOOL_PERMISSIONS,
        },
    }
}

#[cfg(test)]
mod database_free_policy_tests {
    use super::*;
    #[tokio::test]
    async fn list_grants_and_denials_preserve_database_free_paths() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .unwrap();
        pool.close().await;
        let granted = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![codes::LEARNING_OFFERING_READ_SCHOOL.into()],
        };
        assert!(
            require_learning_offering_list_access(&pool, &granted, OfferingAction::Read)
                .await
                .unwrap()
                .includes_school_owned
        );
        let denied = ActorContext {
            user_id: granted.user_id,
            permissions: vec![],
        };
        assert!(matches!(
            require_learning_offering_list_access(&pool, &denied, OfferingAction::Read).await,
            Err(AppError::Forbidden(_))
        ));
        assert!(
            require_learning_offering_batch_access(&pool, &granted, &[], OfferingAction::Read)
                .await
                .is_ok()
        );
        assert!(matches!(
            require_learning_offering_batch_access(&pool, &denied, &[], OfferingAction::Read).await,
            Err(AppError::Forbidden(_))
        ));
    }
}
