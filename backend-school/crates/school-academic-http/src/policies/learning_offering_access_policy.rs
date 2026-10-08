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

pub async fn learning_offering_access(
    pool: &PgPool,
    actor: &ActorContext,
    offering_id: Uuid,
    action: OfferingAction,
) -> Result<AcademicResourceAccess, AppError> {
    let target: Option<(Option<Uuid>, bool)> = sqlx::query_as(
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
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!(
            reason = "learning_offering_access_target_query_failed",
            database_error = %error
        );
        AppError::InternalServerError("ไม่สามารถตรวจสอบสิทธิ์การเปิดสอนได้".to_string())
    })?;

    let Some((owning_organization_unit_id, actor_is_assigned)) = target else {
        return Err(AppError::NotFound("ไม่พบการเปิดสอน".to_string()));
    };

    let filter = learning_offering_list_access(pool, actor, action).await?;
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
    let filter = learning_offering_list_access(pool, actor, action).await?;
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
    if learning_offering_access(pool, actor, offering_id, action).await?
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
    let mut unique_ids = offering_ids.to_vec();
    unique_ids.sort_unstable();
    unique_ids.dedup();
    if unique_ids.is_empty() {
        require_learning_offering_list_access(pool, actor, action).await?;
        return Ok(());
    }
    let filter = require_learning_offering_list_access(pool, actor, action).await?;
    let targets: Vec<(Uuid, Option<Uuid>, bool)> = sqlx::query_as(
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
    .fetch_all(pool)
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
    let offering_id: Uuid =
        sqlx::query_scalar("SELECT learning_offering_id FROM learning_groups WHERE id = $1")
            .bind(learning_group_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มเรียน".to_string()))?;
    require_learning_offering_access(pool, actor, offering_id, action).await?;
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
