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

const READ_ASSIGNED_PERMISSIONS: &[&str] = &[
    codes::ACADEMIC_ASSESSMENT_READ_ASSIGNED,
    codes::ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED,
];
const READ_ORGANIZATION_UNIT_PERMISSIONS: &[&str] =
    &[codes::ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT];
const READ_SCHOOL_PERMISSIONS: &[&str] = &[
    codes::ACADEMIC_ASSESSMENT_READ_SCHOOL,
    codes::ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
];
const MANAGE_ASSIGNED_PERMISSIONS: &[&str] = &[codes::ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED];
const MANAGE_SCHOOL_PERMISSIONS: &[&str] = &[codes::ACADEMIC_ASSESSMENT_MANAGE_SCHOOL];
const NO_PERMISSIONS: &[&str] = &[];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssessmentAction {
    Read,
    Manage,
}

pub async fn assessment_plan_list_access(
    pool: &PgPool,
    actor: &ActorContext,
    action: AssessmentAction,
) -> Result<AcademicResourceListFilter, AppError> {
    resource_access_policy::resolve_academic_resource_list_filter(
        pool,
        actor,
        assessment_permissions(action),
    )
    .await
}

pub async fn assessment_plan_list_access_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    action: AssessmentAction,
) -> Result<AcademicResourceListFilter, AppError> {
    resource_access_policy::resolve_academic_resource_list_filter_on_connection(
        connection,
        actor,
        assessment_permissions(action),
    )
    .await
}

pub async fn require_assessment_plan_list_access(
    pool: &PgPool,
    actor: &ActorContext,
    action: AssessmentAction,
) -> Result<AcademicResourceListFilter, AppError> {
    require_list_filter(assessment_plan_list_access(pool, actor, action).await?)
}

pub async fn require_assessment_plan_list_access_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    action: AssessmentAction,
) -> Result<AcademicResourceListFilter, AppError> {
    require_list_filter(assessment_plan_list_access_on_connection(connection, actor, action).await?)
}
fn require_list_filter(
    filter: AcademicResourceListFilter,
) -> Result<AcademicResourceListFilter, AppError> {
    if filter.includes_school_owned
        || !filter.organization_unit_ids.is_empty()
        || filter.assigned_actor_id.is_some()
    {
        Ok(filter)
    } else {
        Err(AppError::Forbidden(
            "ไม่มีสิทธิ์เข้าถึงโครงสร้างคะแนนรายวิชา".to_string(),
        ))
    }
}

pub async fn require_assessment_plan_access(
    pool: &PgPool,
    actor: &ActorContext,
    offering_id: Uuid,
    action: AssessmentAction,
) -> Result<(), AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_assessment.policy.assessment.require_assessment_plan_access",
    )
    .await?;
    require_assessment_plan_access_on_connection(&mut connection, actor, offering_id, action).await
}

pub async fn require_assessment_plan_access_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    offering_id: Uuid,
    action: AssessmentAction,
) -> Result<(), AppError> {
    let target: Option<(Option<Uuid>, Uuid, Uuid, Vec<Uuid>)> = DbSession::new(connection)
        .query_as(
            "academic_assessment.policy.assessment.require_assessment_plan_access_on_connection",
            r#"
        SELECT offering.owning_organization_unit_id,offering.academic_year_id,offering.academic_term_id,
               ARRAY (
                   SELECT learning_group.id
                   FROM learning_groups learning_group
                   WHERE learning_group.learning_offering_id = offering.id
                   ORDER BY learning_group.id
               ) AS learning_group_ids
        FROM learning_offerings offering
        WHERE offering.id = $1
          AND offering.kind = 'course'
        "#,
        )
        .bind(offering_id)
        .fetch_optional()
        .await?;
    let Some((owning_organization_unit_id, year, term, group_ids)) = target else {
        return Err(AppError::NotFound("ไม่พบรายวิชาที่เปิดสอน".to_string()));
    };

    let filter = assessment_plan_list_access_on_connection(connection, actor, action).await?;
    let actor_is_assigned = if filter.assigned_actor_id.is_some() {
        school_academic_delivery::services::versions::group_teacher_assignments_on_connection(
            connection, year, term, &group_ids,
        )
        .await?
        .groups
        .iter()
        .any(|group| group.teacher_ids.contains(&actor.user_id))
    } else {
        false
    };
    if resource_access_policy::academic_resource_access_for(
        &filter,
        owning_organization_unit_id,
        actor_is_assigned,
    ) == AcademicResourceAccess::None
    {
        Err(AppError::Forbidden(
            "ไม่มีสิทธิ์เข้าถึงโครงสร้างคะแนนรายวิชานี้".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn assessment_permissions(action: AssessmentAction) -> AcademicResourcePermissions {
    match action {
        AssessmentAction::Read => AcademicResourcePermissions {
            assigned: READ_ASSIGNED_PERMISSIONS,
            organization_unit: READ_ORGANIZATION_UNIT_PERMISSIONS,
            organization_tree: NO_PERMISSIONS,
            school: READ_SCHOOL_PERMISSIONS,
        },
        AssessmentAction::Manage => AcademicResourcePermissions {
            assigned: MANAGE_ASSIGNED_PERMISSIONS,
            organization_unit: NO_PERMISSIONS,
            organization_tree: NO_PERMISSIONS,
            school: MANAGE_SCHOOL_PERMISSIONS,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn assessment_list_policy_preserves_database_free_school_grants_and_denials() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(50))
            .connect_lazy("postgres://invalid:invalid@127.0.0.1:1/invalid")
            .unwrap();
        let granted = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![codes::ACADEMIC_ASSESSMENT_READ_SCHOOL.into()],
        };
        assert!(
            require_assessment_plan_list_access(&pool, &granted, AssessmentAction::Read)
                .await
                .unwrap()
                .includes_school_owned
        );
        let denied = ActorContext {
            user_id: granted.user_id,
            permissions: vec![],
        };
        assert!(matches!(
            require_assessment_plan_list_access(&pool, &denied, AssessmentAction::Read).await,
            Err(AppError::Forbidden(_))
        ));
    }
}
