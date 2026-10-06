use school_authorization::{AcademicResourceAccess, AcademicResourceListFilter, ActorContext};
use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurriculumAction {
    Read,
    Manage,
}
fn allowed(actor: &ActorContext, action: CurriculumAction) -> bool {
    match action {
        CurriculumAction::Read => actor.has_any_permission(&[
            codes::ACADEMIC_CURRICULUM_READ_SCHOOL,
            codes::ACADEMIC_CURRICULUM_MANAGE_SCHOOL,
        ]),
        CurriculumAction::Manage => {
            actor.has_any_permission(&[codes::ACADEMIC_CURRICULUM_MANAGE_SCHOOL])
        }
    }
}
pub async fn academic_curriculum_list_access(
    _pool: &PgPool,
    actor: &ActorContext,
    action: CurriculumAction,
) -> Result<AcademicResourceListFilter, AppError> {
    Ok(AcademicResourceListFilter {
        includes_school_owned: allowed(actor, action),
        ..Default::default()
    })
}
pub async fn require_academic_curriculum_list_access(
    pool: &PgPool,
    actor: &ActorContext,
    action: CurriculumAction,
) -> Result<AcademicResourceListFilter, AppError> {
    let filter = academic_curriculum_list_access(pool, actor, action).await?;
    if !filter.includes_school_owned {
        return Err(AppError::Forbidden("ไม่มีสิทธิ์เข้าถึงหลักสูตรของโรงเรียน".into()));
    }
    Ok(filter)
}
pub async fn academic_curriculum_access(
    _pool: &PgPool,
    actor: &ActorContext,
    _id: Uuid,
    action: CurriculumAction,
) -> Result<AcademicResourceAccess, AppError> {
    if !allowed(actor, action) {
        return Ok(AcademicResourceAccess::None);
    }
    Ok(AcademicResourceAccess::School)
}
pub async fn require_academic_curriculum_access(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    action: CurriculumAction,
) -> Result<(), AppError> {
    if academic_curriculum_access(pool, actor, id, action).await? == AcademicResourceAccess::None {
        return Err(AppError::Forbidden("ไม่มีสิทธิ์เข้าถึงหลักสูตรของโรงเรียน".into()));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn actor(permissions: &[&str]) -> ActorContext {
        ActorContext {
            user_id: Uuid::new_v4(),
            permissions: permissions.iter().map(|p| p.to_string()).collect(),
        }
    }
    #[test]
    fn school_read_and_manage_are_separate_capabilities() {
        let reader = actor(&[codes::ACADEMIC_CURRICULUM_READ_SCHOOL]);
        assert!(allowed(&reader, CurriculumAction::Read));
        assert!(!allowed(&reader, CurriculumAction::Manage));
        let manager = actor(&[codes::ACADEMIC_CURRICULUM_MANAGE_SCHOOL]);
        assert!(allowed(&manager, CurriculumAction::Read));
        assert!(allowed(&manager, CurriculumAction::Manage));
    }
    #[test]
    fn retired_owner_grants_do_not_authorize_curriculum_actions() {
        for code in [
            "academic_curriculum.read.organization_unit",
            "academic_curriculum.manage.organization_tree",
            "academic_catalog.manage.school",
        ] {
            let denied = actor(&[code]);
            assert!(!allowed(&denied, CurriculumAction::Read));
            assert!(!allowed(&denied, CurriculumAction::Manage));
        }
        assert!(!allowed(&actor(&[]), CurriculumAction::Read));
    }
}
