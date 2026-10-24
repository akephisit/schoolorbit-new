use school_authorization::{
    accessible_exact_units_for_permission_on_connection, has_exact_unit_permission_on_connection,
    ActorContext,
};
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use school_permissions::registry::codes;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CertificateAction {
    Read,
    Create,
    Update,
    Delete,
    Submit,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CertificateOwnerListScope {
    ExactUnits(Vec<Uuid>),
    School,
}

pub async fn require_template_action(
    pool: &PgPool,
    actor: &ActorContext,
    template_id: Uuid,
    action: CertificateAction,
) -> Result<CertificateAccessGrant, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "certificates.require_template_action",
    )
    .await?;
    require_template_action_on_connection(&mut connection, actor, template_id, action).await
}

pub async fn require_template_action_on_connection(
    connection: &mut PgConnection,
    actor: &ActorContext,
    template_id: Uuid,
    action: CertificateAction,
) -> Result<CertificateAccessGrant, AppError> {
    let owner_organization_unit_id = DbSession::new(&mut *connection)
        .scalar::<Option<Uuid>>(
            "certificates.src.access_policy.require_template_action_on_connection",
            "SELECT campaign.owner_organization_unit_id
         FROM certificate_templates template
         JOIN certificate_campaigns campaign ON campaign.id = template.campaign_id
         WHERE template.id = $1
           AND campaign.status <> 'purging'",
        )
        .bind(template_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบแม่แบบเกียรติบัตร".to_string()))?;
    require_owner_action_on_connection(&mut *connection, actor, owner_organization_unit_id, action)
        .await
}

pub async fn require_owner_action(
    pool: &PgPool,
    actor: &ActorContext,
    owner_organization_unit_id: Option<Uuid>,
    action: CertificateAction,
) -> Result<CertificateAccessGrant, AppError> {
    if matches!(
        owner_action_preflight(actor, owner_organization_unit_id, action)?,
        OwnerActionPreflight::School
    ) {
        return Ok(CertificateAccessGrant {
            scope: CertificateAccessScope::School,
        });
    }
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "certificates.require_owner_action",
    )
    .await?;
    require_owner_action_on_connection(&mut connection, actor, owner_organization_unit_id, action)
        .await
}

pub async fn require_owner_action_on_connection(
    connection: &mut PgConnection,
    actor: &ActorContext,
    owner_organization_unit_id: Option<Uuid>,
    action: CertificateAction,
) -> Result<CertificateAccessGrant, AppError> {
    let OwnerActionPreflight::Lookup {
        owner_id,
        exact_unit_permission,
    } = owner_action_preflight(actor, owner_organization_unit_id, action)?
    else {
        return Ok(CertificateAccessGrant {
            scope: CertificateAccessScope::School,
        });
    };
    if has_exact_unit_permission_on_connection(
        &mut *connection,
        actor.user_id,
        owner_id,
        exact_unit_permission,
    )
    .await?
    {
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
    if matches!(
        owner_list_preflight(actor, action)?,
        OwnerListPreflight::School
    ) {
        return Ok(CertificateOwnerListScope::School);
    }
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "certificates.owner_list_scope")
            .await?;
    owner_list_scope_on_connection(&mut connection, actor, action).await
}

pub async fn owner_list_scope_on_connection(
    connection: &mut PgConnection,
    actor: &ActorContext,
    action: CertificateAction,
) -> Result<CertificateOwnerListScope, AppError> {
    let OwnerListPreflight::Lookup(exact_unit_permission) = owner_list_preflight(actor, action)?
    else {
        return Ok(CertificateOwnerListScope::School);
    };
    let units = accessible_exact_units_for_permission_on_connection(
        &mut *connection,
        actor.user_id,
        exact_unit_permission,
    )
    .await?;
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
        CertificateAction::Download => ActionPermissions {
            organization_unit: Some(codes::CERTIFICATE_DOWNLOAD_ORGANIZATION_UNIT),
            school: codes::CERTIFICATE_DOWNLOAD_SCHOOL,
        },
    }
}

fn certificate_forbidden() -> AppError {
    AppError::Forbidden("ไม่มีสิทธิ์จัดการกิจกรรมเกียรติบัตรนี้".to_string())
}

enum OwnerActionPreflight {
    School,
    Lookup {
        owner_id: Uuid,
        exact_unit_permission: &'static str,
    },
}

fn owner_action_preflight(
    actor: &ActorContext,
    owner_organization_unit_id: Option<Uuid>,
    action: CertificateAction,
) -> Result<OwnerActionPreflight, AppError> {
    let permissions = action_permissions(action);
    if actor.has_permission(permissions.school) {
        return Ok(OwnerActionPreflight::School);
    }
    let Some(owner_id) = owner_organization_unit_id else {
        return Err(certificate_forbidden());
    };
    let Some(exact_unit_permission) = permissions.organization_unit else {
        return Err(certificate_forbidden());
    };
    Ok(OwnerActionPreflight::Lookup {
        owner_id,
        exact_unit_permission,
    })
}

enum OwnerListPreflight {
    School,
    Lookup(&'static str),
}

fn owner_list_preflight(
    actor: &ActorContext,
    action: CertificateAction,
) -> Result<OwnerListPreflight, AppError> {
    let permissions = action_permissions(action);
    if actor.has_permission(permissions.school) {
        return Ok(OwnerListPreflight::School);
    }
    let Some(exact_unit_permission) = permissions.organization_unit else {
        return Err(certificate_forbidden());
    };
    Ok(OwnerListPreflight::Lookup(exact_unit_permission))
}

#[cfg(test)]
mod pool_facade_preflight_tests {
    use super::*;
    async fn closed_pool() -> sqlx::PgPool {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .unwrap();
        pool.close().await;
        pool
    }

    #[tokio::test]
    async fn pool_facade_preflight_missing_owner_denials_precede_checkout() {
        let pool = closed_pool().await;
        let actor = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![],
        };
        for action in [
            CertificateAction::Read,
            CertificateAction::Create,
            CertificateAction::Update,
            CertificateAction::Delete,
            CertificateAction::Submit,
            CertificateAction::Download,
        ] {
            assert!(matches!(
                require_owner_action(&pool, &actor, None, action).await,
                Err(AppError::Forbidden(_))
            ));
        }
    }
    #[tokio::test]
    async fn pool_facade_preflight_direct_certificate_grants_stay_database_free() {
        let pool = closed_pool().await;
        for action in [
            CertificateAction::Read,
            CertificateAction::Create,
            CertificateAction::Update,
            CertificateAction::Delete,
            CertificateAction::Submit,
            CertificateAction::Download,
        ] {
            let actor = ActorContext {
                user_id: Uuid::new_v4(),
                permissions: vec![action_permissions(action).school.into()],
            };
            assert_eq!(
                require_owner_action(&pool, &actor, None, action)
                    .await
                    .unwrap()
                    .scope,
                CertificateAccessScope::School
            );
            assert!(matches!(
                owner_list_scope(&pool, &actor, action).await.unwrap(),
                CertificateOwnerListScope::School
            ));
        }
    }
}
