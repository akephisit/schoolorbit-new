use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::question_bank::models::QuestionScopeRow;
use crate::permissions::registry::codes;
use crate::policies::resource_access_policy::{self, UserResourceListAccess};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionBankAccess {
    pub read_school: bool,
    pub read_assigned_user_id: Option<Uuid>,
    pub read_subject_group_ids: Vec<Uuid>,
    pub manage_school: bool,
    pub manage_assigned_user_id: Option<Uuid>,
    pub manage_subject_group_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PermissionFlags {
    read_school: bool,
    read_assigned: bool,
    read_subject_group: bool,
    manage_school: bool,
    manage_assigned: bool,
    manage_subject_group: bool,
}

pub async fn resolve_access(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<QuestionBankAccess, AppError> {
    let flags = permission_flags(actor);
    if !flags.read_school && !flags.read_assigned && !flags.read_subject_group {
        actor.require_any_permission(&[
            codes::ACADEMIC_QUESTION_BANK_READ_ASSIGNED,
            codes::ACADEMIC_QUESTION_BANK_READ_ORGANIZATION_UNIT,
            codes::ACADEMIC_QUESTION_BANK_READ_SCHOOL,
            codes::ACADEMIC_QUESTION_BANK_MANAGE_ASSIGNED,
            codes::ACADEMIC_QUESTION_BANK_MANAGE_ORGANIZATION_UNIT,
            codes::ACADEMIC_QUESTION_BANK_MANAGE_SCHOOL,
        ])?;
    }

    let subject_group_ids = if flags.read_subject_group || flags.manage_subject_group {
        actor_subject_group_ids(pool, actor.user_id).await?
    } else {
        Vec::new()
    };

    Ok(QuestionBankAccess {
        read_school: flags.read_school,
        read_assigned_user_id: flags.read_assigned.then_some(actor.user_id),
        read_subject_group_ids: if flags.read_subject_group {
            subject_group_ids.clone()
        } else {
            Vec::new()
        },
        manage_school: flags.manage_school,
        manage_assigned_user_id: flags.manage_assigned.then_some(actor.user_id),
        manage_subject_group_ids: if flags.manage_subject_group {
            subject_group_ids
        } else {
            Vec::new()
        },
    })
}

pub async fn require_question_read_access(
    pool: &PgPool,
    actor: &ActorContext,
    scope: &QuestionScopeRow,
) -> Result<(), AppError> {
    let flags = permission_flags(actor);
    if flags.read_school {
        return Ok(());
    }
    if flags.read_assigned
        && (scope.owner_user_id == actor.user_id
            || subject_is_assigned_to_actor(pool, scope.subject_id, actor.user_id).await?)
    {
        return Ok(());
    }
    if flags.read_subject_group
        && subject_group_is_accessible(pool, scope.subject_group_id, actor.user_id).await?
    {
        return Ok(());
    }
    Err(AppError::Forbidden("ไม่มีสิทธิ์ดูข้อสอบนี้".to_string()))
}

pub async fn require_question_manage_access(
    pool: &PgPool,
    actor: &ActorContext,
    scope: &QuestionScopeRow,
) -> Result<(), AppError> {
    let flags = permission_flags(actor);
    if flags.manage_school {
        return Ok(());
    }
    if flags.manage_assigned && scope.owner_user_id == actor.user_id {
        return Ok(());
    }
    if flags.manage_subject_group
        && subject_group_is_accessible(pool, scope.subject_group_id, actor.user_id).await?
    {
        return Ok(());
    }
    Err(AppError::Forbidden("ไม่มีสิทธิ์จัดการข้อสอบนี้".to_string()))
}

pub async fn require_subject_create_access(
    pool: &PgPool,
    actor: &ActorContext,
    subject_id: Uuid,
) -> Result<(), AppError> {
    let flags = permission_flags(actor);
    let subject_group_id = subject_group_id_for_subject(pool, subject_id)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายวิชาในคลังวิชา".to_string()))?;

    if flags.manage_school {
        return Ok(());
    }
    if flags.manage_assigned
        && subject_is_assigned_to_actor(pool, Some(subject_id), actor.user_id).await?
    {
        return Ok(());
    }
    if flags.manage_subject_group
        && subject_group_is_accessible(pool, subject_group_id, actor.user_id).await?
    {
        return Ok(());
    }

    Err(AppError::Forbidden(
        "ไม่มีสิทธิ์สร้างข้อสอบสำหรับรายวิชานี้".to_string(),
    ))
}

fn permission_flags(actor: &ActorContext) -> PermissionFlags {
    let manage_school = actor.has_permission(codes::ACADEMIC_QUESTION_BANK_MANAGE_SCHOOL);
    let manage_assigned = actor.has_permission(codes::ACADEMIC_QUESTION_BANK_MANAGE_ASSIGNED);
    let manage_subject_group =
        actor.has_permission(codes::ACADEMIC_QUESTION_BANK_MANAGE_ORGANIZATION_UNIT);

    PermissionFlags {
        read_school: manage_school
            || actor.has_permission(codes::ACADEMIC_QUESTION_BANK_READ_SCHOOL),
        read_assigned: manage_assigned
            || actor.has_permission(codes::ACADEMIC_QUESTION_BANK_READ_ASSIGNED),
        read_subject_group: manage_subject_group
            || actor.has_permission(codes::ACADEMIC_QUESTION_BANK_READ_ORGANIZATION_UNIT),
        manage_school,
        manage_assigned,
        manage_subject_group,
    }
}

async fn actor_subject_group_ids(pool: &PgPool, actor_id: Uuid) -> Result<Vec<Uuid>, AppError> {
    let Some(organization_unit_ids) = resource_access_policy::accessible_organization_unit_ids(
        pool,
        UserResourceListAccess::OrganizationUnit(actor_id),
    )
    .await?
    else {
        return Ok(Vec::new());
    };

    if organization_unit_ids.is_empty() {
        return Ok(Vec::new());
    }

    sqlx::query_scalar(
        r#"
SELECT DISTINCT subject_group_id
FROM organization_units
WHERE id = ANY($1)
  AND subject_group_id IS NOT NULL
  AND is_active = true
"#,
    )
    .bind(&organization_unit_ids)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!(
            "Failed to fetch question bank subject group access: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถตรวจสอบกลุ่มสาระได้".to_string())
    })
}

async fn subject_group_is_accessible(
    pool: &PgPool,
    subject_group_id: Option<Uuid>,
    actor_id: Uuid,
) -> Result<bool, AppError> {
    let Some(subject_group_id) = subject_group_id else {
        return Ok(false);
    };
    let subject_group_ids = actor_subject_group_ids(pool, actor_id).await?;
    Ok(subject_group_ids.contains(&subject_group_id))
}

async fn subject_group_id_for_subject(
    pool: &PgPool,
    subject_id: Uuid,
) -> Result<Option<Option<Uuid>>, AppError> {
    sqlx::query_scalar("SELECT group_id FROM subjects WHERE id = $1")
        .bind(subject_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| {
            tracing::error!("Failed to fetch question bank subject: {}", error);
            AppError::InternalServerError("ไม่สามารถตรวจสอบรายวิชาได้".to_string())
        })
}

async fn subject_is_assigned_to_actor(
    pool: &PgPool,
    subject_id: Option<Uuid>,
    actor_id: Uuid,
) -> Result<bool, AppError> {
    let Some(subject_id) = subject_id else {
        return Ok(false);
    };
    sqlx::query_scalar(
        r#"
SELECT EXISTS(
    SELECT 1
    FROM classroom_courses cc
    JOIN classroom_course_instructors cci ON cci.classroom_course_id = cc.id
    WHERE cc.subject_id = $1
      AND cci.instructor_id = $2
)
"#,
    )
    .bind(subject_id)
    .bind(actor_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to check assigned question subject: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบรายวิชาที่รับผิดชอบได้".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(permissions: &[&str]) -> ActorContext {
        ActorContext {
            user_id: Uuid::new_v4(),
            permissions: permissions
                .iter()
                .map(|permission| permission.to_string())
                .collect(),
        }
    }

    #[test]
    fn manage_assigned_also_grants_assigned_read() {
        let flags = permission_flags(&actor(&[codes::ACADEMIC_QUESTION_BANK_MANAGE_ASSIGNED]));
        assert!(flags.manage_assigned);
        assert!(flags.read_assigned);
        assert!(!flags.read_school);
    }

    #[test]
    fn school_manage_implies_school_read_without_widening_other_scopes() {
        let flags = permission_flags(&actor(&[codes::ACADEMIC_QUESTION_BANK_MANAGE_SCHOOL]));
        assert!(flags.manage_school);
        assert!(flags.read_school);
        assert!(!flags.read_assigned);
        assert!(!flags.read_subject_group);
    }
}
