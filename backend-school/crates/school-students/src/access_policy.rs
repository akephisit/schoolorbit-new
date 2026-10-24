use sqlx::PgPool;
use uuid::Uuid;

use school_authorization::resource_access_policy::{
    self, ResourceAccessPermissions, ResourceAccessTarget,
};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;

use crate::services::StudentListAccess;

const STUDENT_PROFILE_ACCESS: ResourceAccessPermissions = ResourceAccessPermissions {
    own: Some(codes::STUDENT_READ_OWN),
    assigned: Some(codes::STUDENT_READ_ASSIGNED),
    organization_unit: None,
    organization_tree: None,
    school: Some(codes::STUDENT_READ_SCHOOL),
};

const STUDENT_PII_ACCESS: ResourceAccessPermissions = ResourceAccessPermissions {
    own: Some(codes::STUDENT_PII_READ_OWN),
    assigned: Some(codes::STUDENT_PII_READ_ASSIGNED),
    organization_unit: None,
    organization_tree: None,
    school: Some(codes::STUDENT_PII_READ_SCHOOL),
};

pub fn resolve_student_list_access(actor: &ActorContext) -> Result<StudentListAccess, AppError> {
    if actor.has_permission(codes::STUDENT_READ_SCHOOL) {
        return Ok(StudentListAccess::School);
    }
    let own = actor.has_permission(codes::STUDENT_READ_OWN);
    let assigned = actor.has_permission(codes::STUDENT_READ_ASSIGNED);
    match (own, assigned) {
        (true, true) => Ok(StudentListAccess::OwnAndAssigned(actor.user_id)),
        (true, false) => Ok(StudentListAccess::Own(actor.user_id)),
        (false, true) => Ok(StudentListAccess::Assigned(actor.user_id)),
        (false, false) => Err(AppError::Forbidden("ไม่มีสิทธิ์ดูรายชื่อนักเรียน".into())),
    }
}

pub async fn can_read_student_profile(
    pool: &PgPool,
    actor: &ActorContext,
    target_user_id: Uuid,
) -> Result<(), AppError> {
    authorize_student_profile_read(pool, actor, target_user_id)
        .await
        .map(|_| ())
}

/// Authorize the profile and decide its separate PII scope from the same assignment snapshot.
pub async fn authorize_student_profile_read(
    pool: &PgPool,
    actor: &ActorContext,
    target_user_id: Uuid,
) -> Result<bool, AppError> {
    let target = student_resource_target(pool, target_user_id).await?;
    resource_access_policy::require_resource_access(
        pool,
        actor,
        STUDENT_PROFILE_ACCESS,
        &target,
        "ไม่มีสิทธิ์ดูข้อมูลนักเรียนนี้",
    )
    .await?;
    Ok(
        resource_access_policy::can_access_direct_resource(actor, STUDENT_PII_ACCESS, &target)
            .is_some(),
    )
}

async fn student_resource_target(
    pool: &PgPool,
    target_user_id: Uuid,
) -> Result<ResourceAccessTarget, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "students.student_resource_target",
    )
    .await?;
    student_resource_target_on_connection(&mut connection, target_user_id).await
}

async fn student_resource_target_on_connection(
    connection: &mut sqlx::PgConnection,
    target_user_id: Uuid,
) -> Result<ResourceAccessTarget, AppError> {
    let assigned_user_ids = school_database::DbSession::new(&mut *connection)
        .scalar(
            "students.src.access_policy.student_resource_target_on_connection",
            r#"
        SELECT DISTINCT advisor.user_id
        FROM student_academic_years student_year
        JOIN homeroom_placements placement
          ON placement.student_academic_year_id = student_year.id
         AND placement.status = 'current'
        JOIN homeroom_advisors advisor ON advisor.homeroom_id = placement.homeroom_id
        WHERE student_year.student_id = $1
          AND student_year.status = 'active'
        "#,
        )
        .bind(target_user_id)
        .fetch_all()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to load assigned student advisors: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถตรวจสอบผู้รับผิดชอบนักเรียนได้".to_string())
        })?;

    Ok(ResourceAccessTarget {
        owner_user_id: Some(target_user_id),
        assigned_user_ids,
        organization_unit_ids: Vec::new(),
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
    fn student_list_access_prefers_school_scope() {
        let actor = actor(
            Uuid::new_v4(),
            &[codes::STUDENT_READ_ASSIGNED, codes::STUDENT_READ_SCHOOL],
        );

        let access = resolve_student_list_access(&actor).expect("school access should resolve");

        assert_eq!(access, StudentListAccess::School);
    }

    #[test]
    fn student_list_access_supports_assigned_scope() {
        let user_id = Uuid::new_v4();
        let actor = actor(user_id, &[codes::STUDENT_READ_ASSIGNED]);

        let access = resolve_student_list_access(&actor).expect("assigned access should resolve");

        assert_eq!(access, StudentListAccess::Assigned(user_id));
    }
    #[test]
    fn student_list_access_retains_own_and_assigned_union() {
        let user_id = Uuid::new_v4();
        let actor = actor(
            user_id,
            &[codes::STUDENT_READ_OWN, codes::STUDENT_READ_ASSIGNED],
        );
        assert_eq!(
            resolve_student_list_access(&actor).unwrap(),
            StudentListAccess::OwnAndAssigned(user_id)
        );
    }
}
