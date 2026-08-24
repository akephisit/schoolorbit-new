use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::{
        files::{
            platform_types::{FilePurpose, FileVisibility},
            repository::PlatformFile,
        },
        question_bank::services as question_bank_service,
    },
    permissions::registry::codes,
    policies::{
        achievement_access_policy,
        certificate_access_policy::{self, CertificateAction},
        question_bank_access_policy, staff_access_policy, student_access_policy,
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
        FilePurpose::SchoolFont => Some(actor.has_permission(codes::FONT_MANAGE_SCHOOL)),
        FilePurpose::QuestionBankImage
        | FilePurpose::Transcript
        | FilePurpose::Certificate
        | FilePurpose::IdentityCard
        | FilePurpose::CourseMaterial
        | FilePurpose::AssignmentAttachment
        | FilePurpose::GenericPrivateDocument
        | FilePurpose::CertificateTemplateBackground
        | FilePurpose::CertificateTemplateImage => None,
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
        FilePurpose::CertificateTemplateBackground | FilePurpose::CertificateTemplateImage => {
            let template_id = required_resource(resource_id)?;
            certificate_access_policy::require_template_action(
                pool,
                actor,
                template_id,
                CertificateAction::Update,
            )
            .await?;
            Ok(actor.user_id)
        }
        FilePurpose::SchoolFont => {
            if let Some(template_id) = resource_id {
                certificate_access_policy::require_template_action(
                    pool,
                    actor,
                    template_id,
                    CertificateAction::Update,
                )
                .await?;
            } else {
                require_simple_access(actor, purpose, FilePolicyAction::Create, None, None)?;
            }
            Ok(actor.user_id)
        }
        FilePurpose::Transcript
        | FilePurpose::Certificate
        | FilePurpose::IdentityCard
        | FilePurpose::CourseMaterial
        | FilePurpose::AssignmentAttachment
        | FilePurpose::GenericPrivateDocument => Err(explicit_domain_policy_required()),
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
        FilePurpose::CertificateTemplateBackground | FilePurpose::CertificateTemplateImage => {
            authorize_certificate_template_file(pool, actor, file, action, resource_id).await
        }
        FilePurpose::SchoolFont => {
            authorize_school_font_file(pool, actor, file, action, resource_id).await
        }
        FilePurpose::Transcript
        | FilePurpose::Certificate
        | FilePurpose::IdentityCard
        | FilePurpose::CourseMaterial
        | FilePurpose::AssignmentAttachment
        | FilePurpose::GenericPrivateDocument => Err(explicit_domain_policy_required()),
    }
}

/// Holds the owning template and campaign locks from the final unreferenced check
/// through the File Platform lifecycle transition. Template attachment takes the
/// same locks before accepting a ready upload, so a delete cannot race an attach.
pub async fn authorize_certificate_template_delete_guard<'a>(
    pool: &'a PgPool,
    actor: &ActorContext,
    file: &PlatformFile,
    resource_id: Option<Uuid>,
) -> Result<Transaction<'a, Postgres>, AppError> {
    if !matches!(
        file.purpose,
        FilePurpose::CertificateTemplateBackground | FilePurpose::CertificateTemplateImage
    ) {
        return Err(explicit_domain_policy_required());
    }
    if file.visibility != FileVisibility::Private {
        return Err(forbidden());
    }
    let requested_template_id = required_resource(resource_id)?;
    let authorization = sqlx::query_as::<_, (Uuid, Uuid, Uuid, bool, Option<Uuid>)>(
        "SELECT upload.template_id,
                template.campaign_id,
                upload.uploaded_by,
                (
                    COALESCE(template.background_file_id = upload.file_id, false)
                    OR EXISTS (
                        SELECT 1 FROM certificate_template_assets asset
                        WHERE asset.template_id = upload.template_id
                          AND asset.file_id = upload.file_id
                    )
                ) AS referenced,
                campaign.owner_organization_unit_id
         FROM certificate_template_file_uploads upload
         JOIN certificate_templates template ON template.id = upload.template_id
         JOIN certificate_campaigns campaign ON campaign.id = template.campaign_id
         WHERE upload.file_id = $1
           AND upload.purpose_code = $2
           AND campaign.status <> 'purging'",
    )
    .bind(file.id)
    .bind(file.purpose.code())
    .fetch_optional(pool)
    .await?
    .ok_or_else(unrelated_resource)?;
    if authorization.0 != requested_template_id {
        return Err(unrelated_resource());
    }
    if authorization.3 {
        return Err(AppError::Conflict(
            "ไฟล์นี้ถูกใช้อยู่ในแม่แบบ กรุณาถอดผ่านหน้าจัดการแม่แบบ".to_string(),
        ));
    }
    if authorization.2 != actor.user_id {
        return Err(forbidden());
    }
    certificate_access_policy::require_owner_action(
        pool,
        actor,
        authorization.4,
        CertificateAction::Update,
    )
    .await?;

    let mut tx = pool.begin().await?;
    let (locked_owner_id, locked_status) = sqlx::query_as::<_, (Option<Uuid>, String)>(
        "SELECT owner_organization_unit_id, status
         FROM certificate_campaigns
         WHERE id = $1
         FOR UPDATE",
    )
    .bind(authorization.1)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(unrelated_resource)?;
    if locked_status == "purging" {
        return Err(AppError::Conflict(
            "certificate_campaign_purging".to_string(),
        ));
    }
    if locked_owner_id != authorization.4 {
        return Err(AppError::Conflict(
            "หน่วยงานเจ้าของกิจกรรมเปลี่ยนแล้ว กรุณาโหลดข้อมูลล่าสุด".to_string(),
        ));
    }

    let locked = sqlx::query_as::<_, (Uuid, Uuid, Uuid, bool)>(
        "SELECT upload.template_id,
                template.campaign_id,
                upload.uploaded_by,
                (
                    COALESCE(template.background_file_id = upload.file_id, false)
                    OR EXISTS (
                        SELECT 1 FROM certificate_template_assets asset
                        WHERE asset.template_id = upload.template_id
                          AND asset.file_id = upload.file_id
                    )
                ) AS referenced
         FROM certificate_template_file_uploads upload
         JOIN certificate_templates template ON template.id = upload.template_id
         WHERE upload.file_id = $1
           AND upload.purpose_code = $2
         FOR UPDATE OF template, upload",
    )
    .bind(file.id)
    .bind(file.purpose.code())
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(unrelated_resource)?;
    if locked.0 != requested_template_id || locked.1 != authorization.1 || locked.2 != actor.user_id
    {
        return Err(unrelated_resource());
    }
    if locked.3 {
        return Err(AppError::Conflict(
            "ไฟล์นี้ถูกใช้อยู่ในแม่แบบ กรุณาถอดผ่านหน้าจัดการแม่แบบ".to_string(),
        ));
    }
    Ok(tx)
}

/// Keeps a temporary school-font staging relation locked until the caller has
/// committed the File Platform lifecycle transition. Attach flows lock the same
/// staging row before promotion, so cleanup cannot race into `school_fonts`.
pub async fn authorize_school_font_delete_guard<'a>(
    pool: &'a PgPool,
    actor: &ActorContext,
    file: &PlatformFile,
    resource_id: Option<Uuid>,
) -> Result<Transaction<'a, Postgres>, AppError> {
    if file.purpose != FilePurpose::SchoolFont {
        return Err(explicit_domain_policy_required());
    }
    if file.visibility != FileVisibility::Private {
        return Err(forbidden());
    }

    let Some(requested_template_id) = resource_id else {
        require_simple_access(actor, file.purpose, FilePolicyAction::Delete, None, None)?;
        let mut tx = pool.begin().await?;
        let staged = sqlx::query_scalar::<_, Uuid>(
            "SELECT uploaded_by
             FROM school_font_file_uploads
             WHERE file_id = $1
               AND purpose_code = 'school_font'
             FOR UPDATE",
        )
        .bind(file.id)
        .fetch_optional(&mut *tx)
        .await?;
        if staged.is_none() {
            return Err(unrelated_resource());
        }
        require_school_font_is_temporary(&mut tx, file.id).await?;
        return Ok(tx);
    };

    let authorization = sqlx::query_as::<_, (Uuid, Uuid, Option<Uuid>)>(
        "SELECT upload.template_id,
                template.campaign_id,
                campaign.owner_organization_unit_id
         FROM certificate_school_font_file_uploads AS upload
         JOIN certificate_templates AS template ON template.id = upload.template_id
         JOIN certificate_campaigns AS campaign ON campaign.id = template.campaign_id
         WHERE upload.file_id = $1
           AND upload.purpose_code = 'school_font'
           AND campaign.status <> 'purging'",
    )
    .bind(file.id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(unrelated_resource)?;
    if authorization.0 != requested_template_id {
        return Err(unrelated_resource());
    }
    certificate_access_policy::require_owner_action(
        pool,
        actor,
        authorization.2,
        CertificateAction::Update,
    )
    .await?;

    let mut tx = pool.begin().await?;
    let (locked_owner_id, locked_status) = sqlx::query_as::<_, (Option<Uuid>, String)>(
        "SELECT owner_organization_unit_id, status
         FROM certificate_campaigns
         WHERE id = $1
         FOR UPDATE",
    )
    .bind(authorization.1)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(unrelated_resource)?;
    if locked_status == "purging" {
        return Err(AppError::Conflict(
            "certificate_campaign_purging".to_string(),
        ));
    }
    if locked_owner_id != authorization.2 {
        return Err(AppError::Conflict(
            "หน่วยงานเจ้าของกิจกรรมเปลี่ยนแล้ว กรุณาโหลดข้อมูลล่าสุด".to_string(),
        ));
    }

    let locked = sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT upload.template_id, template.campaign_id
         FROM certificate_school_font_file_uploads AS upload
         JOIN certificate_templates AS template ON template.id = upload.template_id
         WHERE upload.file_id = $1
           AND upload.purpose_code = 'school_font'
         FOR UPDATE OF template, upload",
    )
    .bind(file.id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(unrelated_resource)?;
    if locked.0 != requested_template_id || locked.1 != authorization.1 {
        return Err(unrelated_resource());
    }
    require_school_font_is_temporary(&mut tx, file.id).await?;
    Ok(tx)
}

async fn require_school_font_is_temporary(
    transaction: &mut Transaction<'_, Postgres>,
    file_id: Uuid,
) -> Result<(), AppError> {
    let durable = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM school_fonts WHERE file_id = $1)",
    )
    .bind(file_id)
    .fetch_one(&mut **transaction)
    .await?;
    if durable {
        Err(unrelated_resource())
    } else {
        Ok(())
    }
}

async fn authorize_school_font_file(
    pool: &PgPool,
    actor: &ActorContext,
    file: &PlatformFile,
    action: FilePolicyAction,
    resource_id: Option<Uuid>,
) -> Result<(), AppError> {
    if file.visibility != FileVisibility::Private {
        return Err(forbidden());
    }
    if matches!(action, FilePolicyAction::Create | FilePolicyAction::Delete) {
        return Err(explicit_domain_policy_required());
    }

    if let Some(template_id) = resource_id {
        let related = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(
                SELECT 1
                FROM certificate_school_font_file_uploads AS upload
                JOIN certificate_templates AS template ON template.id = upload.template_id
                JOIN certificate_campaigns AS campaign ON campaign.id = template.campaign_id
                WHERE upload.file_id = $1
                  AND upload.purpose_code = 'school_font'
                  AND upload.template_id = $2
                  AND campaign.status <> 'purging'
            )",
        )
        .bind(file.id)
        .bind(template_id)
        .fetch_one(pool)
        .await?;
        if !related {
            return Err(unrelated_resource());
        }
        certificate_access_policy::require_template_action(
            pool,
            actor,
            template_id,
            CertificateAction::Update,
        )
        .await?;
        return Ok(());
    }

    require_simple_access(actor, file.purpose, action, None, None)?;
    let staged = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(
            SELECT 1
            FROM school_font_file_uploads AS upload
            WHERE upload.file_id = $1
              AND upload.purpose_code = 'school_font'
        )",
    )
    .bind(file.id)
    .fetch_one(pool)
    .await?;
    if staged {
        Ok(())
    } else {
        Err(unrelated_resource())
    }
}

async fn authorize_certificate_template_file(
    pool: &PgPool,
    actor: &ActorContext,
    file: &PlatformFile,
    action: FilePolicyAction,
    resource_id: Option<Uuid>,
) -> Result<(), AppError> {
    if file.visibility != FileVisibility::Private {
        return Err(forbidden());
    }
    let requested_template_id = required_resource(resource_id)?;
    let relationship = sqlx::query_as::<_, (Uuid, Uuid, bool)>(
        "SELECT upload.template_id,
                upload.uploaded_by,
                (
                    COALESCE(template.background_file_id = upload.file_id, false)
                    OR EXISTS (
                        SELECT 1 FROM certificate_template_assets asset
                        WHERE asset.template_id = upload.template_id
                          AND asset.file_id = upload.file_id
                    )
                ) AS referenced
         FROM certificate_template_file_uploads upload
         JOIN certificate_templates template ON template.id = upload.template_id
         JOIN certificate_campaigns campaign ON campaign.id = template.campaign_id
         WHERE upload.file_id = $1
           AND upload.purpose_code = $2
           AND campaign.status <> 'purging'",
    )
    .bind(file.id)
    .bind(file.purpose.code())
    .fetch_optional(pool)
    .await?
    .ok_or_else(unrelated_resource)?;
    if relationship.0 != requested_template_id {
        return Err(unrelated_resource());
    }

    match action {
        FilePolicyAction::Read => {
            if relationship.2 {
                certificate_access_policy::require_template_action(
                    pool,
                    actor,
                    relationship.0,
                    CertificateAction::Read,
                )
                .await?;
                return Ok(());
            }
            if relationship.1 != actor.user_id {
                return Err(forbidden());
            }
            certificate_access_policy::require_template_action(
                pool,
                actor,
                relationship.0,
                CertificateAction::Update,
            )
            .await?;
            Ok(())
        }
        FilePolicyAction::Delete => {
            if relationship.2 {
                return Err(AppError::Conflict(
                    "ไฟล์นี้ถูกใช้อยู่ในแม่แบบ กรุณาถอดผ่านหน้าจัดการแม่แบบ".to_string(),
                ));
            }
            if relationship.1 != actor.user_id {
                return Err(forbidden());
            }
            certificate_access_policy::require_template_action(
                pool,
                actor,
                relationship.0,
                CertificateAction::Update,
            )
            .await?;
            Ok(())
        }
        FilePolicyAction::Create => Err(explicit_domain_policy_required()),
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
    use crate::test_helpers::{
        create_named_test_pool, create_named_test_pool_with_max_connections, create_test_user,
        run_test_migrations,
    };

    fn actor(user_id: Uuid, permissions: &[&str]) -> ActorContext {
        ActorContext {
            user_id,
            permissions: permissions
                .iter()
                .map(|permission| permission.to_string())
                .collect(),
        }
    }

    fn assert_lock_not_available(error: sqlx::Error) {
        let lock_code = match error {
            sqlx::Error::Database(error) => error.code().map(|code| code.into_owned()),
            other => panic!("expected a database lock error, got {other}"),
        };
        assert_eq!(lock_code.as_deref(), Some("55P03"));
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
    fn central_school_font_access_requires_only_the_font_manager_permission() {
        let ordinary = actor(Uuid::new_v4(), &[]);
        let font_manager = actor(Uuid::new_v4(), &[codes::FONT_MANAGE_SCHOOL]);
        let certificate_manager = actor(Uuid::new_v4(), &[codes::CERTIFICATE_UPDATE_SCHOOL]);

        for action in [
            FilePolicyAction::Create,
            FilePolicyAction::Read,
            FilePolicyAction::Delete,
        ] {
            assert_eq!(
                simple_file_access(&ordinary, FilePurpose::SchoolFont, action, None, None),
                Some(false),
            );
            assert_eq!(
                simple_file_access(
                    &certificate_manager,
                    FilePurpose::SchoolFont,
                    action,
                    None,
                    None
                ),
                Some(false),
            );
            assert_eq!(
                simple_file_access(&font_manager, FilePurpose::SchoolFont, action, None, None),
                Some(true),
            );
        }
    }

    #[tokio::test]
    async fn school_font_upload_uses_central_permission_or_exact_template_update_policy() {
        let pool = create_named_test_pool("school_font_upload_policy").await;
        run_test_migrations(&pool).await;
        let academic_year_id: Uuid = sqlx::query_scalar(
            "INSERT INTO academic_years (year, name, start_date, end_date, status)
             VALUES (2997, 'School font policy test', '2997-01-01', '2997-12-31', 'planning')
             RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .expect("academic-year fixture should insert");
        let campaign_id: Uuid = sqlx::query_scalar(
            "INSERT INTO certificate_campaigns (
                academic_year_id, name, event_date, status
             ) VALUES ($1, 'School font policy test', '2997-06-01', 'active')
             RETURNING id",
        )
        .bind(academic_year_id)
        .fetch_one(&pool)
        .await
        .expect("campaign fixture should insert");
        let template_id: Uuid = sqlx::query_scalar(
            "INSERT INTO certificate_templates (campaign_id, name, normalized_name)
             VALUES ($1, 'School font policy test', 'school-font-policy-test')
             RETURNING id",
        )
        .bind(campaign_id)
        .fetch_one(&pool)
        .await
        .expect("template fixture should insert");

        let font_manager = actor(Uuid::new_v4(), &[codes::FONT_MANAGE_SCHOOL]);
        let certificate_manager = actor(Uuid::new_v4(), &[codes::CERTIFICATE_UPDATE_SCHOOL]);
        let ordinary = actor(Uuid::new_v4(), &[]);

        assert_eq!(
            authorize_create(&pool, &font_manager, FilePurpose::SchoolFont, None)
                .await
                .expect("central manager should authorize school-font upload"),
            font_manager.user_id
        );
        assert!(
            authorize_create(&pool, &ordinary, FilePurpose::SchoolFont, None)
                .await
                .is_err(),
            "ordinary users must not upload into the central library"
        );
        assert_eq!(
            authorize_create(
                &pool,
                &certificate_manager,
                FilePurpose::SchoolFont,
                Some(template_id),
            )
            .await
            .expect("school-wide certificate updater should authorize the exact template"),
            certificate_manager.user_id
        );
        assert!(
            authorize_create(
                &pool,
                &font_manager,
                FilePurpose::SchoolFont,
                Some(template_id),
            )
            .await
            .is_err(),
            "central font management must not imply certificate template access"
        );
        assert!(
            authorize_create(
                &pool,
                &certificate_manager,
                FilePurpose::SchoolFont,
                Some(Uuid::new_v4()),
            )
            .await
            .is_err(),
            "template-context upload must fail closed for an unknown template"
        );
    }

    #[tokio::test]
    async fn school_font_delete_guard_locks_staging_and_rejects_durable_fonts() {
        let pool = create_named_test_pool_with_max_connections("school_font_delete_guard", 2).await;
        run_test_migrations(&pool).await;
        let actor_id = create_test_user(
            &pool,
            "school-font-delete-guard@example.test",
            "test-password",
        )
        .await
        .expect("actor fixture should insert");
        let file_id: Uuid = sqlx::query_scalar(
            "INSERT INTO files (
                display_filename, purpose_code, visibility, lifecycle_status,
                retention_class, inspection_metadata, created_by
             ) VALUES (
                'school-font-delete-guard.ttf', 'school_font', 'private', 'ready',
                'temporary', '{\"kind\":\"font\"}'::jsonb, $1
             )
             RETURNING id",
        )
        .bind(actor_id)
        .fetch_one(&pool)
        .await
        .expect("school-font file fixture should insert");
        sqlx::query(
            "INSERT INTO school_font_file_uploads (file_id, uploaded_by)
             VALUES ($1, $2)",
        )
        .bind(file_id)
        .bind(actor_id)
        .execute(&pool)
        .await
        .expect("central staging fixture should insert");
        let file = PlatformFile {
            id: file_id,
            owner_user_id: Some(actor_id),
            purpose: FilePurpose::SchoolFont,
            visibility: FileVisibility::Private,
            lifecycle_status: crate::modules::files::platform_types::FileLifecycleStatus::Ready,
            current_version: None,
            display_filename: "school-font-delete-guard.ttf".to_string(),
            detected_mime_type: "font/ttf".to_string(),
            byte_size: 1024,
        };
        let manager = actor(actor_id, &[codes::FONT_MANAGE_SCHOOL]);

        assert!(
            authorize_existing(&pool, &manager, &file, FilePolicyAction::Delete, None,)
                .await
                .is_err(),
            "school-font deletion must be authorized only through the locking guard"
        );
        let guard = authorize_school_font_delete_guard(&pool, &manager, &file, None)
            .await
            .expect("temporary central school-font cleanup should authorize");
        let lock_error = sqlx::query(
            "SELECT file_id
             FROM school_font_file_uploads
             WHERE file_id = $1
             FOR UPDATE NOWAIT",
        )
        .bind(file_id)
        .fetch_one(&pool)
        .await
        .expect_err("cleanup authorization must retain the staging-row lock");
        assert_lock_not_available(lock_error);
        guard
            .rollback()
            .await
            .expect("test cleanup guard should roll back");

        sqlx::query(
            "INSERT INTO school_fonts (
                file_id, display_name, font_family, normalized_family,
                font_weight, font_style, rights_confirmed_by,
                rights_confirmed_at, created_by
             ) VALUES (
                $1, 'School font', 'School Font', 'school font',
                400, 'normal', $2, NOW(), $2
             )",
        )
        .bind(file_id)
        .bind(actor_id)
        .execute(&pool)
        .await
        .expect("durable school-font fixture should insert");
        assert!(
            authorize_school_font_delete_guard(&pool, &manager, &file, None)
                .await
                .is_err(),
            "generic cleanup must fail closed once a durable school-font row exists"
        );
    }

    #[tokio::test]
    async fn certificate_school_font_delete_guard_requires_and_locks_the_exact_template() {
        let pool =
            create_named_test_pool_with_max_connections("certificate_school_font_delete_guard", 2)
                .await;
        run_test_migrations(&pool).await;
        let actor_id = create_test_user(
            &pool,
            "certificate-school-font-delete-guard@example.test",
            "test-password",
        )
        .await
        .expect("actor fixture should insert");
        let academic_year_id: Uuid = sqlx::query_scalar(
            "INSERT INTO academic_years (year, name, start_date, end_date, status)
             VALUES (2996, 'Certificate school font guard', '2996-01-01', '2996-12-31', 'planning')
             RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .expect("academic-year fixture should insert");
        let campaign_id: Uuid = sqlx::query_scalar(
            "INSERT INTO certificate_campaigns (
                academic_year_id, name, event_date, status, created_by
             ) VALUES ($1, 'Certificate school font guard', '2996-06-01', 'active', $2)
             RETURNING id",
        )
        .bind(academic_year_id)
        .bind(actor_id)
        .fetch_one(&pool)
        .await
        .expect("campaign fixture should insert");
        let template_id: Uuid = sqlx::query_scalar(
            "INSERT INTO certificate_templates (campaign_id, name, normalized_name)
             VALUES ($1, 'Guard template', 'guard-template')
             RETURNING id",
        )
        .bind(campaign_id)
        .fetch_one(&pool)
        .await
        .expect("template fixture should insert");
        let other_template_id: Uuid = sqlx::query_scalar(
            "INSERT INTO certificate_templates (campaign_id, name, normalized_name)
             VALUES ($1, 'Other guard template', 'other-guard-template')
             RETURNING id",
        )
        .bind(campaign_id)
        .fetch_one(&pool)
        .await
        .expect("other template fixture should insert");
        let file_id: Uuid = sqlx::query_scalar(
            "INSERT INTO files (
                display_filename, purpose_code, visibility, lifecycle_status,
                retention_class, inspection_metadata, created_by
             ) VALUES (
                'certificate-school-font-delete-guard.ttf', 'school_font',
                'private', 'ready', 'temporary', '{\"kind\":\"font\"}'::jsonb, $1
             )
             RETURNING id",
        )
        .bind(actor_id)
        .fetch_one(&pool)
        .await
        .expect("school-font file fixture should insert");
        sqlx::query(
            "INSERT INTO certificate_school_font_file_uploads
                (file_id, template_id, uploaded_by)
             VALUES ($1, $2, $3)",
        )
        .bind(file_id)
        .bind(template_id)
        .bind(actor_id)
        .execute(&pool)
        .await
        .expect("certificate staging fixture should insert");
        let file = PlatformFile {
            id: file_id,
            owner_user_id: Some(actor_id),
            purpose: FilePurpose::SchoolFont,
            visibility: FileVisibility::Private,
            lifecycle_status: crate::modules::files::platform_types::FileLifecycleStatus::Ready,
            current_version: None,
            display_filename: "certificate-school-font-delete-guard.ttf".to_string(),
            detected_mime_type: "font/ttf".to_string(),
            byte_size: 1024,
        };
        let updater = actor(actor_id, &[codes::CERTIFICATE_UPDATE_SCHOOL]);

        assert!(
            authorize_school_font_delete_guard(&pool, &updater, &file, Some(other_template_id),)
                .await
                .is_err(),
            "another template must not authorize cleanup"
        );
        assert!(
            authorize_school_font_delete_guard(&pool, &updater, &file, None)
                .await
                .is_err(),
            "certificate authority must not fall back to central staging"
        );

        let guard = authorize_school_font_delete_guard(&pool, &updater, &file, Some(template_id))
            .await
            .expect("exact-template updater should authorize cleanup");
        let lock_error = sqlx::query(
            "SELECT file_id
             FROM certificate_school_font_file_uploads
             WHERE file_id = $1
             FOR UPDATE NOWAIT",
        )
        .bind(file_id)
        .fetch_one(&pool)
        .await
        .expect_err("cleanup authorization must retain the certificate staging-row lock");
        assert_lock_not_available(lock_error);
        let template_lock_error = sqlx::query(
            "SELECT id
             FROM certificate_templates
             WHERE id = $1
             FOR UPDATE NOWAIT",
        )
        .bind(template_id)
        .fetch_one(&pool)
        .await
        .expect_err("cleanup authorization must retain the exact template lock");
        assert_lock_not_available(template_lock_error);
        guard
            .rollback()
            .await
            .expect("test cleanup guard should roll back");
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
