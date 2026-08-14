use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::{
        files::{platform_types::FilePurpose, repository::PlatformFile},
        question_bank::services as question_bank_service,
    },
    permissions::registry::codes,
    policies::{
        achievement_access_policy, question_bank_access_policy, staff_access_policy,
        student_access_policy,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilePolicyAction {
    Create,
    Read,
    Delete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileTargetKind {
    Staff,
    Student,
    Other,
}

pub fn simple_file_access(
    actor: &ActorContext,
    purpose: FilePurpose,
    action: FilePolicyAction,
    owner_user_id: Option<Uuid>,
    profile_target_kind: Option<ProfileTargetKind>,
) -> Option<bool> {
    match purpose {
        FilePurpose::SchoolLogo | FilePurpose::SchoolBanner => Some(match action {
            FilePolicyAction::Read => {
                actor.has_any_permission(&[codes::SETTINGS_READ_ALL, codes::SETTINGS_UPDATE_ALL])
            }
            FilePolicyAction::Create | FilePolicyAction::Delete => {
                actor.has_permission(codes::SETTINGS_UPDATE_ALL)
            }
        }),
        FilePurpose::ProfileImage => {
            let Some(owner_user_id) = owner_user_id else {
                return Some(false);
            };
            if owner_user_id == actor.user_id {
                return Some(true);
            }
            match action {
                FilePolicyAction::Read => None,
                FilePolicyAction::Create | FilePolicyAction::Delete => {
                    Some(match profile_target_kind {
                        Some(ProfileTargetKind::Staff) => {
                            actor.has_permission(codes::STAFF_UPDATE_ALL)
                        }
                        Some(ProfileTargetKind::Student) => {
                            actor.has_permission(codes::STUDENT_UPDATE_ALL)
                        }
                        Some(ProfileTargetKind::Other) | None => false,
                    })
                }
            }
        }
        FilePurpose::AchievementImage => None,
        FilePurpose::AdmissionApplicationDocument => Some(match action {
            FilePolicyAction::Read => actor.has_any_permission(&[
                codes::ADMISSION_READ_ALL,
                codes::ADMISSION_MANAGE_ALL,
                codes::ADMISSION_VERIFY_ALL,
            ]),
            FilePolicyAction::Create | FilePolicyAction::Delete => actor
                .has_any_permission(&[codes::ADMISSION_MANAGE_ALL, codes::ADMISSION_VERIFY_ALL]),
        }),
        FilePurpose::QuestionBankImage
        | FilePurpose::Transcript
        | FilePurpose::Certificate
        | FilePurpose::IdentityCard
        | FilePurpose::CourseMaterial
        | FilePurpose::AssignmentAttachment
        | FilePurpose::GenericPrivateDocument
        | FilePurpose::CertificateTemplateBackground
        | FilePurpose::CertificateTemplateImage
        | FilePurpose::CertificateTemplateFont => None,
    }
}

pub const fn related_resource_access(
    requested_resource_matches: bool,
    domain_policy_allows: bool,
) -> bool {
    requested_resource_matches && domain_policy_allows
}

pub fn portal_application_access(
    authenticated_application_id: Uuid,
    requested_application_id: Uuid,
) -> bool {
    authenticated_application_id == requested_application_id
}

pub async fn authorize_create(
    pool: &PgPool,
    actor: &ActorContext,
    purpose: FilePurpose,
    resource_id: Option<Uuid>,
) -> Result<Uuid, AppError> {
    match purpose {
        FilePurpose::SchoolLogo | FilePurpose::SchoolBanner => {
            require_no_resource(resource_id)?;
            require_simple_access(actor, purpose, FilePolicyAction::Create, None, None)?;
            Ok(actor.user_id)
        }
        FilePurpose::ProfileImage => {
            let owner_user_id = resource_id.unwrap_or(actor.user_id);
            let target_kind = profile_target_kind(pool, owner_user_id).await?;
            require_simple_access(
                actor,
                purpose,
                FilePolicyAction::Create,
                Some(owner_user_id),
                Some(target_kind),
            )?;
            Ok(owner_user_id)
        }
        FilePurpose::AdmissionApplicationDocument => {
            let application_id = required_resource(resource_id)?;
            require_simple_access(actor, purpose, FilePolicyAction::Create, None, None)?;
            require_application_exists(pool, application_id).await?;
            Ok(actor.user_id)
        }
        FilePurpose::QuestionBankImage => {
            let subject_id = required_resource(resource_id)?;
            question_bank_access_policy::require_subject_create_access(pool, actor, subject_id)
                .await?;
            Ok(actor.user_id)
        }
        FilePurpose::AchievementImage => {
            let owner_user_id = resource_id.unwrap_or(actor.user_id);
            require_user_exists(pool, owner_user_id).await?;
            achievement_access_policy::can_create_achievement_for(actor, owner_user_id)?;
            Ok(owner_user_id)
        }
        FilePurpose::Transcript
        | FilePurpose::Certificate
        | FilePurpose::IdentityCard
        | FilePurpose::CourseMaterial
        | FilePurpose::AssignmentAttachment
        | FilePurpose::GenericPrivateDocument
        | FilePurpose::CertificateTemplateBackground
        | FilePurpose::CertificateTemplateImage
        | FilePurpose::CertificateTemplateFont => Err(explicit_domain_policy_required()),
    }
}

pub async fn authorize_existing(
    pool: &PgPool,
    actor: &ActorContext,
    file: &PlatformFile,
    action: FilePolicyAction,
    resource_id: Option<Uuid>,
) -> Result<(), AppError> {
    match file.purpose {
        FilePurpose::SchoolLogo | FilePurpose::SchoolBanner => {
            require_no_resource(resource_id)?;
            require_simple_access(actor, file.purpose, action, None, None)
        }
        FilePurpose::ProfileImage => {
            let owner_user_id = file
                .owner_user_id
                .ok_or_else(|| AppError::Forbidden("ไม่อนุญาตให้เข้าถึงไฟล์นี้".to_string()))?;
            if resource_id.is_some_and(|resource_id| resource_id != owner_user_id) {
                return Err(unrelated_resource());
            }
            let target_kind = profile_target_kind(pool, owner_user_id).await?;
            if simple_file_access(
                actor,
                file.purpose,
                action,
                Some(owner_user_id),
                Some(target_kind),
            )
            .is_some_and(|allowed| allowed)
            {
                return Ok(());
            }
            if action != FilePolicyAction::Read {
                return Err(forbidden());
            }
            match target_kind {
                ProfileTargetKind::Staff => {
                    staff_access_policy::can_read_staff_profile(pool, actor, owner_user_id).await
                }
                ProfileTargetKind::Student => {
                    student_access_policy::can_read_student_profile(pool, actor, owner_user_id)
                        .await
                }
                ProfileTargetKind::Other => Err(forbidden()),
            }
        }
        FilePurpose::AdmissionApplicationDocument => {
            require_simple_access(actor, file.purpose, action, None, None)?;
            if let Some(application_id) = resource_id {
                let related = application_references_file(pool, application_id, file.id).await?;
                if !related_resource_access(related, true) {
                    return Err(unrelated_resource());
                }
                return Ok(());
            }
            if file.owner_user_id == Some(actor.user_id) {
                Ok(())
            } else {
                Err(unrelated_resource())
            }
        }
        FilePurpose::QuestionBankImage => {
            if let Some(question_id) = resource_id {
                let scope = question_bank_service::fetch_question_scope(pool, question_id).await?;
                let referenced = question_bank_service::fetch_question_file_ids(pool, question_id)
                    .await?
                    .contains(&file.id);
                if !referenced {
                    return Err(unrelated_resource());
                }
                return match action {
                    FilePolicyAction::Read => {
                        question_bank_access_policy::require_question_read_access(
                            pool, actor, &scope,
                        )
                        .await
                    }
                    FilePolicyAction::Create | FilePolicyAction::Delete => {
                        question_bank_access_policy::require_question_manage_access(
                            pool, actor, &scope,
                        )
                        .await
                    }
                };
            }
            if file.owner_user_id == Some(actor.user_id)
                && actor.has_any_permission(&[
                    codes::ACADEMIC_QUESTION_BANK_MANAGE_ASSIGNED,
                    codes::ACADEMIC_QUESTION_BANK_MANAGE_ORGANIZATION_UNIT,
                    codes::ACADEMIC_QUESTION_BANK_MANAGE_SCHOOL,
                ])
            {
                Ok(())
            } else {
                Err(unrelated_resource())
            }
        }
        FilePurpose::AchievementImage => {
            if let Some(achievement_id) = resource_id {
                let relationship = sqlx::query_as::<_, (Uuid, Option<Uuid>)>(
                    "SELECT user_id, image_file_id FROM staff_achievements WHERE id = $1",
                )
                .bind(achievement_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(unrelated_resource)?;
                if relationship.1 != Some(file.id) {
                    return Err(unrelated_resource());
                }
                return match action {
                    FilePolicyAction::Read => {
                        achievement_access_policy::can_read_achievement(actor, relationship.0)
                    }
                    FilePolicyAction::Create => {
                        achievement_access_policy::can_create_achievement_for(actor, relationship.0)
                    }
                    FilePolicyAction::Delete => {
                        achievement_access_policy::can_update_achievement(actor, relationship.0)
                    }
                };
            }
            let owner_user_id = file.owner_user_id.ok_or_else(unrelated_resource)?;
            if owner_user_id != actor.user_id {
                return Err(unrelated_resource());
            }
            achievement_access_policy::can_create_achievement_for(actor, owner_user_id)
        }
        FilePurpose::Transcript
        | FilePurpose::Certificate
        | FilePurpose::IdentityCard
        | FilePurpose::CourseMaterial
        | FilePurpose::AssignmentAttachment
        | FilePurpose::GenericPrivateDocument
        | FilePurpose::CertificateTemplateBackground
        | FilePurpose::CertificateTemplateImage
        | FilePurpose::CertificateTemplateFont => Err(explicit_domain_policy_required()),
    }
}

pub async fn authorize_portal_application(
    pool: &PgPool,
    authenticated_application_id: Uuid,
    requested_application_id: Uuid,
    file_id: Option<Uuid>,
) -> Result<(), AppError> {
    if !portal_application_access(authenticated_application_id, requested_application_id) {
        return Err(forbidden());
    }
    require_application_exists(pool, requested_application_id).await?;
    if let Some(file_id) = file_id {
        if !application_references_file(pool, requested_application_id, file_id).await? {
            return Err(unrelated_resource());
        }
    }
    Ok(())
}

fn require_simple_access(
    actor: &ActorContext,
    purpose: FilePurpose,
    action: FilePolicyAction,
    owner_user_id: Option<Uuid>,
    profile_target_kind: Option<ProfileTargetKind>,
) -> Result<(), AppError> {
    match simple_file_access(actor, purpose, action, owner_user_id, profile_target_kind) {
        Some(true) => Ok(()),
        Some(false) | None => Err(forbidden()),
    }
}

async fn profile_target_kind(
    pool: &PgPool,
    target_user_id: Uuid,
) -> Result<ProfileTargetKind, AppError> {
    let user_type = sqlx::query_scalar::<_, String>("SELECT user_type FROM users WHERE id = $1")
        .bind(target_user_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบผู้ใช้".to_string()))?;
    Ok(match user_type.as_str() {
        "staff" => ProfileTargetKind::Staff,
        "student" => ProfileTargetKind::Student,
        _ => ProfileTargetKind::Other,
    })
}

async fn require_application_exists(pool: &PgPool, application_id: Uuid) -> Result<(), AppError> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM admission_applications WHERE id = $1)",
    )
    .bind(application_id)
    .fetch_one(pool)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound("ไม่พบใบสมัคร".to_string()))
    }
}

async fn require_user_exists(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    let exists = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound("ไม่พบผู้ใช้".to_string()))
    }
}

async fn application_references_file(
    pool: &PgPool,
    application_id: Uuid,
    file_id: Uuid,
) -> Result<bool, AppError> {
    sqlx::query_scalar(
        r#"
SELECT EXISTS(
    SELECT 1
    FROM admission_application_documents
    WHERE application_id = $1
      AND file_id = $2
      AND deleted_at IS NULL
)
"#,
    )
    .bind(application_id)
    .bind(file_id)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

fn required_resource(resource_id: Option<Uuid>) -> Result<Uuid, AppError> {
    resource_id.ok_or_else(|| AppError::BadRequest("ต้องระบุ resourceId".to_string()))
}

fn require_no_resource(resource_id: Option<Uuid>) -> Result<(), AppError> {
    if resource_id.is_none() {
        Ok(())
    } else {
        Err(AppError::BadRequest("purpose นี้ไม่รับ resourceId".to_string()))
    }
}

fn forbidden() -> AppError {
    AppError::Forbidden("ไม่อนุญาตให้เข้าถึงไฟล์นี้".to_string())
}

fn unrelated_resource() -> AppError {
    AppError::NotFound("ไม่พบไฟล์ใน resource ที่ระบุ".to_string())
}

fn explicit_domain_policy_required() -> AppError {
    AppError::Forbidden("purpose นี้ต้องใช้งานผ่านระบบเจ้าของเอกสาร".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::registry::codes;

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
    fn own_profile_create_read_and_delete_are_allowed() {
        let user_id = Uuid::new_v4();
        let actor = actor(user_id, &[]);

        for action in [
            FilePolicyAction::Create,
            FilePolicyAction::Read,
            FilePolicyAction::Delete,
        ] {
            assert_eq!(
                simple_file_access(
                    &actor,
                    FilePurpose::ProfileImage,
                    action,
                    Some(user_id),
                    Some(ProfileTargetKind::Staff),
                ),
                Some(true),
            );
        }
    }

    #[test]
    fn cross_user_profile_requires_the_exact_people_permission() {
        let target_user_id = Uuid::new_v4();
        let unrelated = actor(Uuid::new_v4(), &[]);
        let staff_manager = actor(Uuid::new_v4(), &[codes::STAFF_UPDATE_ALL]);
        let student_manager = actor(Uuid::new_v4(), &[codes::STUDENT_UPDATE_ALL]);

        assert_eq!(
            simple_file_access(
                &unrelated,
                FilePurpose::ProfileImage,
                FilePolicyAction::Delete,
                Some(target_user_id),
                Some(ProfileTargetKind::Staff),
            ),
            Some(false),
        );
        assert_eq!(
            simple_file_access(
                &staff_manager,
                FilePurpose::ProfileImage,
                FilePolicyAction::Delete,
                Some(target_user_id),
                Some(ProfileTargetKind::Staff),
            ),
            Some(true),
        );
        assert_eq!(
            simple_file_access(
                &student_manager,
                FilePurpose::ProfileImage,
                FilePolicyAction::Delete,
                Some(target_user_id),
                Some(ProfileTargetKind::Staff),
            ),
            Some(false),
        );
    }

    #[test]
    fn school_branding_uses_settings_permissions_by_action() {
        let reader = actor(Uuid::new_v4(), &[codes::SETTINGS_READ_ALL]);
        let manager = actor(Uuid::new_v4(), &[codes::SETTINGS_UPDATE_ALL]);

        assert_eq!(
            simple_file_access(
                &reader,
                FilePurpose::SchoolLogo,
                FilePolicyAction::Read,
                None,
                None,
            ),
            Some(true),
        );
        assert_eq!(
            simple_file_access(
                &reader,
                FilePurpose::SchoolLogo,
                FilePolicyAction::Create,
                None,
                None,
            ),
            Some(false),
        );
        assert_eq!(
            simple_file_access(
                &manager,
                FilePurpose::SchoolBanner,
                FilePolicyAction::Delete,
                None,
                None,
            ),
            Some(true),
        );
    }

    #[test]
    fn admission_staff_permissions_are_action_specific() {
        let reader = actor(Uuid::new_v4(), &[codes::ADMISSION_READ_ALL]);
        let manager = actor(Uuid::new_v4(), &[codes::ADMISSION_MANAGE_ALL]);

        assert_eq!(
            simple_file_access(
                &reader,
                FilePurpose::AdmissionApplicationDocument,
                FilePolicyAction::Read,
                None,
                None,
            ),
            Some(true),
        );
        assert_eq!(
            simple_file_access(
                &reader,
                FilePurpose::AdmissionApplicationDocument,
                FilePolicyAction::Delete,
                None,
                None,
            ),
            Some(false),
        );
        assert_eq!(
            simple_file_access(
                &manager,
                FilePurpose::AdmissionApplicationDocument,
                FilePolicyAction::Create,
                None,
                None,
            ),
            Some(true),
        );
    }

    #[test]
    fn admission_and_question_resources_require_relationship_and_domain_access() {
        assert!(related_resource_access(true, true));
        assert!(!related_resource_access(false, true));
        assert!(!related_resource_access(true, false));
        assert!(!related_resource_access(false, false));
    }

    #[test]
    fn portal_session_cannot_cross_application_scope() {
        let application_id = Uuid::new_v4();
        assert!(portal_application_access(application_id, application_id));
        assert!(!portal_application_access(application_id, Uuid::new_v4()));
    }

    #[test]
    fn purposes_with_domain_owned_scope_require_authoritative_lookup() {
        let actor = actor(
            Uuid::new_v4(),
            &[codes::ACADEMIC_QUESTION_BANK_MANAGE_SCHOOL],
        );
        assert_eq!(
            simple_file_access(
                &actor,
                FilePurpose::QuestionBankImage,
                FilePolicyAction::Read,
                Some(actor.user_id),
                None,
            ),
            None,
        );
        assert_eq!(
            simple_file_access(
                &actor,
                FilePurpose::GenericPrivateDocument,
                FilePolicyAction::Create,
                Some(actor.user_id),
                None,
            ),
            None,
        );
        for purpose in [
            FilePurpose::CertificateTemplateBackground,
            FilePurpose::CertificateTemplateImage,
            FilePurpose::CertificateTemplateFont,
        ] {
            assert_eq!(
                simple_file_access(
                    &actor,
                    purpose,
                    FilePolicyAction::Create,
                    Some(actor.user_id),
                    None,
                ),
                None,
            );
        }
    }
}
