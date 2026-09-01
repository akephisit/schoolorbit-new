use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::permissions::registry::codes;
use crate::policies::resource_access_policy::{
    self, AcademicResourceAccess, AcademicResourceListFilter, AcademicResourcePermissions,
};

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

pub async fn require_assessment_plan_list_access(
    pool: &PgPool,
    actor: &ActorContext,
    action: AssessmentAction,
) -> Result<AcademicResourceListFilter, AppError> {
    let filter = assessment_plan_list_access(pool, actor, action).await?;
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
          AND offering.kind = 'course'
        "#,
    )
    .bind(offering_id)
    .bind(actor.user_id)
    .fetch_optional(pool)
    .await?;
    let Some((owning_organization_unit_id, actor_is_assigned)) = target else {
        return Err(AppError::NotFound("ไม่พบรายวิชาที่เปิดสอน".to_string()));
    };

    let filter = assessment_plan_list_access(pool, actor, action).await?;
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
