use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::permissions::registry::codes;
use crate::policies::resource_access_policy::{
    self, ResourceAccessPermissions, ResourceAccessTarget, UserResourceListAccess,
};

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

pub fn resolve_student_list_access(
    actor: &ActorContext,
) -> Result<UserResourceListAccess, AppError> {
    resource_access_policy::resolve_user_resource_list_access(actor, STUDENT_PROFILE_ACCESS)
        .ok_or_else(|| AppError::Forbidden("ไม่มีสิทธิ์ดูรายชื่อนักเรียน".to_string()))
}

pub async fn can_read_student_profile(
    pool: &PgPool,
    actor: &ActorContext,
    target_user_id: Uuid,
) -> Result<(), AppError> {
    let target = student_resource_target(pool, target_user_id).await?;
    resource_access_policy::require_resource_access(
        pool,
        actor,
        STUDENT_PROFILE_ACCESS,
        &target,
        "ไม่มีสิทธิ์ดูข้อมูลนักเรียนนี้",
    )
    .await
    .map(|_| ())
}

pub async fn can_read_student_pii(
    pool: &PgPool,
    actor: &ActorContext,
    target_user_id: Uuid,
) -> Result<bool, AppError> {
    let target = student_resource_target(pool, target_user_id).await?;
    Ok(
        resource_access_policy::can_access_direct_resource(actor, STUDENT_PII_ACCESS, &target)
            .is_some(),
    )
}

async fn student_resource_target(
    pool: &PgPool,
    target_user_id: Uuid,
) -> Result<ResourceAccessTarget, AppError> {
    let assigned_user_ids = sqlx::query_scalar(
        r#"
        SELECT DISTINCT ca.user_id
        FROM student_class_enrollments sce
        JOIN classroom_advisors ca ON ca.classroom_id = sce.class_room_id
        WHERE sce.student_id = $1
          AND sce.status = 'active'
        "#,
    )
    .bind(target_user_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load assigned student advisors: {}", error);
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

        assert_eq!(access, UserResourceListAccess::School);
    }

    #[test]
    fn student_list_access_supports_assigned_scope() {
        let user_id = Uuid::new_v4();
        let actor = actor(user_id, &[codes::STUDENT_READ_ASSIGNED]);

        let access = resolve_student_list_access(&actor).expect("assigned access should resolve");

        assert_eq!(access, UserResourceListAccess::Assigned(user_id));
    }
}
