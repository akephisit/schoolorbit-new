use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    permissions::registry::codes,
    policies::resource_access_policy::{
        accessible_exact_units_for_permission, has_exact_unit_permission,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CertificateAction {
    Read,
    Create,
    Update,
    Delete,
    Submit,
    Issue,
    Revoke,
    Download,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CertificateAccessScope {
    OrganizationUnit(Uuid),
    School,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CertificateAccessGrant {
    pub scope: CertificateAccessScope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CertificateCampaignAccessTarget {
    pub campaign_id: Uuid,
    pub owner_organization_unit_id: Option<Uuid>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CertificateOwnerListScope {
    ExactUnits(Vec<Uuid>),
    School,
}

pub async fn require_campaign_action(
    pool: &PgPool,
    actor: &ActorContext,
    campaign: &CertificateCampaignAccessTarget,
    action: CertificateAction,
) -> Result<CertificateAccessGrant, AppError> {
    require_owner_action(pool, actor, campaign.owner_organization_unit_id, action).await
}

pub async fn require_template_action(
    pool: &PgPool,
    actor: &ActorContext,
    template_id: Uuid,
    action: CertificateAction,
) -> Result<CertificateAccessGrant, AppError> {
    let owner_organization_unit_id = sqlx::query_scalar::<_, Option<Uuid>>(
        "SELECT campaign.owner_organization_unit_id
         FROM certificate_templates template
         JOIN certificate_campaigns campaign ON campaign.id = template.campaign_id
         WHERE template.id = $1",
    )
    .bind(template_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบแม่แบบเกียรติบัตร".to_string()))?;
    require_owner_action(pool, actor, owner_organization_unit_id, action).await
}

pub async fn require_owner_action(
    pool: &PgPool,
    actor: &ActorContext,
    owner_organization_unit_id: Option<Uuid>,
    action: CertificateAction,
) -> Result<CertificateAccessGrant, AppError> {
    let permissions = action_permissions(action);
    if actor.has_permission(permissions.school) {
        return Ok(CertificateAccessGrant {
            scope: CertificateAccessScope::School,
        });
    }

    let Some(owner_id) = owner_organization_unit_id else {
        return Err(certificate_forbidden());
    };
    let Some(exact_unit_permission) = permissions.organization_unit else {
        return Err(certificate_forbidden());
    };
    if has_exact_unit_permission(pool, actor.user_id, owner_id, exact_unit_permission).await? {
        return Ok(CertificateAccessGrant {
            scope: CertificateAccessScope::OrganizationUnit(owner_id),
        });
    }

    Err(certificate_forbidden())
}

pub async fn owner_list_scope(
    pool: &PgPool,
    actor: &ActorContext,
    action: CertificateAction,
) -> Result<CertificateOwnerListScope, AppError> {
    let permissions = action_permissions(action);
    if actor.has_permission(permissions.school) {
        return Ok(CertificateOwnerListScope::School);
    }
    let Some(exact_unit_permission) = permissions.organization_unit else {
        return Err(certificate_forbidden());
    };
    let units =
        accessible_exact_units_for_permission(pool, actor.user_id, exact_unit_permission).await?;
    if units.is_empty() {
        Err(certificate_forbidden())
    } else {
        Ok(CertificateOwnerListScope::ExactUnits(units))
    }
}

#[derive(Clone, Copy)]
struct ActionPermissions {
    organization_unit: Option<&'static str>,
    school: &'static str,
}

fn action_permissions(action: CertificateAction) -> ActionPermissions {
    match action {
        CertificateAction::Read => ActionPermissions {
            organization_unit: Some(codes::CERTIFICATE_READ_ORGANIZATION_UNIT),
            school: codes::CERTIFICATE_READ_SCHOOL,
        },
        CertificateAction::Create => ActionPermissions {
            organization_unit: Some(codes::CERTIFICATE_CREATE_ORGANIZATION_UNIT),
            school: codes::CERTIFICATE_CREATE_SCHOOL,
        },
        CertificateAction::Update => ActionPermissions {
            organization_unit: Some(codes::CERTIFICATE_UPDATE_ORGANIZATION_UNIT),
            school: codes::CERTIFICATE_UPDATE_SCHOOL,
        },
        CertificateAction::Delete => ActionPermissions {
            organization_unit: Some(codes::CERTIFICATE_DELETE_ORGANIZATION_UNIT),
            school: codes::CERTIFICATE_DELETE_SCHOOL,
        },
        CertificateAction::Submit => ActionPermissions {
            organization_unit: Some(codes::CERTIFICATE_SUBMIT_ORGANIZATION_UNIT),
            school: codes::CERTIFICATE_SUBMIT_SCHOOL,
        },
        CertificateAction::Issue => ActionPermissions {
            organization_unit: None,
            school: codes::CERTIFICATE_ISSUE_SCHOOL,
        },
        CertificateAction::Revoke => ActionPermissions {
            organization_unit: None,
            school: codes::CERTIFICATE_REVOKE_SCHOOL,
        },
        CertificateAction::Download => ActionPermissions {
            organization_unit: Some(codes::CERTIFICATE_DOWNLOAD_ORGANIZATION_UNIT),
            school: codes::CERTIFICATE_DOWNLOAD_SCHOOL,
        },
    }
}

fn certificate_forbidden() -> AppError {
    AppError::Forbidden("ไม่มีสิทธิ์จัดการกิจกรรมเกียรติบัตรนี้".to_string())
}
