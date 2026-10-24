use crate::models::OrganizationPermissionGrantInput;
use school_database::DbSession;
use school_errors::AppError;
use serde::Serialize;
use std::collections::HashSet;

use sqlx::{PgPool, Row};
#[cfg(feature = "api-docs")]
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct OrganizationPermissionGrant {
    pub permission_id: Uuid,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub position_code: Option<String>,
}

fn unique_permission_grants(
    grants: Vec<OrganizationPermissionGrantInput>,
) -> Vec<OrganizationPermissionGrantInput> {
    let mut seen = HashSet::new();
    grants
        .into_iter()
        .filter(|grant| seen.insert((grant.permission_id, grant.position_code.clone())))
        .collect()
}

async fn bulk_insert_organization_permission_grants(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    organization_unit_id: Uuid,
    grants: &[OrganizationPermissionGrantInput],
) -> Result<(), AppError> {
    if grants.is_empty() {
        return Ok(());
    }

    let permission_ids: Vec<Uuid> = grants.iter().map(|grant| grant.permission_id).collect();
    let position_codes: Vec<Option<String>> = grants
        .iter()
        .map(|grant| grant.position_code.clone())
        .collect();

    DbSession::new(&mut **tx).query("staff.services.organization_permission_service.bulk_insert_organization_permission_grants",
        "INSERT INTO organization_permission_grants
            (organization_unit_id, permission_id, position_code)
         SELECT $1, permission_id, position_code
         FROM UNNEST($2::uuid[], $3::text[]) AS grants(permission_id, position_code)",
    )
    .bind(organization_unit_id)
    .bind(&permission_ids)
    .bind(&position_codes)
    .execute()
    .await
    .map_err(|e| {
        tracing::error!("Failed to insert organization permission grants: {}", school_database::error_class(&e));
        AppError::InternalServerError("ไม่สามารถกำหนดสิทธิ์ของหน่วยงานได้".to_string())
    })?;

    Ok(())
}

pub async fn list_organization_permission_grants(
    pool: &PgPool,
    organization_unit_id: Uuid,
) -> Result<Vec<OrganizationPermissionGrant>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "staff.services.organization_permission_service.list_organization_permission_grants",
    )
    .await?;
    list_organization_permission_grants_on_connection(&mut connection, organization_unit_id).await
}

pub async fn list_organization_permission_grants_on_connection(
    connection: &mut sqlx::PgConnection,
    organization_unit_id: Uuid,
) -> Result<Vec<OrganizationPermissionGrant>, AppError> {
    let rows = DbSession::new(connection).query("staff.services.organization_permission_service.list_organization_permission_grants_on_connection",
        r#"
        SELECT permission_id, position_code
        FROM organization_permission_grants
        WHERE organization_unit_id = $1
        ORDER BY position_code NULLS FIRST, permission_id
        "#,
    )
    .bind(organization_unit_id)
    .fetch_all()
    .await
    .map_err(|e| {
        tracing::error!("Failed to list organization permission grants: {}", school_database::error_class(&e));
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงสิทธิ์ของหน่วยงาน".to_string())
    })?;

    Ok(rows
        .into_iter()
        .map(|row| OrganizationPermissionGrant {
            permission_id: row.get("permission_id"),
            position_code: row.get("position_code"),
        })
        .collect())
}

pub async fn replace_organization_permission_grants(
    pool: &PgPool,
    organization_unit_id: Uuid,
    grants: Vec<OrganizationPermissionGrantInput>,
) -> Result<(), AppError> {
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "staff.services.organization_permission_service.replace_organization_permission_grants",
    )
    .await
    .map_err(|e| {
        tracing::error!(
            "Failed to start organization permission transaction: {}",
            school_database::error_class(&e)
        );
        AppError::InternalServerError("ไม่สามารถเริ่มต้น Transaction ได้".to_string())
    })?;

    DbSession::new(&mut *tx)
        .query(
            "staff.services.organization_permission_service.replace_organization_permission_grants",
            "DELETE FROM organization_permission_grants WHERE organization_unit_id = $1",
        )
        .bind(organization_unit_id)
        .execute()
        .await
        .map_err(|e| {
            tracing::error!(
                "Failed to clear organization permission grants: {}",
                school_database::error_class(&e)
            );
            AppError::InternalServerError("ไม่สามารถลบสิทธิ์เดิมของหน่วยงานได้".to_string())
        })?;

    let grants = unique_permission_grants(grants);
    bulk_insert_organization_permission_grants(&mut tx, organization_unit_id, &grants).await?;

    tx.commit().await.map_err(|e| {
        tracing::error!(
            "Failed to commit organization permission transaction: {}",
            school_database::error_class(&e)
        );
        AppError::InternalServerError("ไม่สามารถบันทึกข้อมูลได้".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_permission_grants_preserves_order_and_removes_duplicates() {
        let permission_a = Uuid::new_v4();
        let permission_b = Uuid::new_v4();

        assert_eq!(
            unique_permission_grants(vec![
                OrganizationPermissionGrantInput {
                    permission_id: permission_a,
                    position_code: None,
                },
                OrganizationPermissionGrantInput {
                    permission_id: permission_b,
                    position_code: Some("head".to_string()),
                },
                OrganizationPermissionGrantInput {
                    permission_id: permission_a,
                    position_code: None,
                },
            ])
            .into_iter()
            .map(|grant| (grant.permission_id, grant.position_code))
            .collect::<Vec<_>>(),
            vec![
                (permission_a, None),
                (permission_b, Some("head".to_string()))
            ]
        );
    }

    #[test]
    fn unique_permission_grants_keeps_same_permission_for_different_positions() {
        let permission_id = Uuid::new_v4();

        assert_eq!(
            unique_permission_grants(vec![
                OrganizationPermissionGrantInput {
                    permission_id,
                    position_code: Some("head".to_string()),
                },
                OrganizationPermissionGrantInput {
                    permission_id,
                    position_code: Some("member".to_string()),
                },
                OrganizationPermissionGrantInput {
                    permission_id,
                    position_code: None,
                },
            ])
            .into_iter()
            .map(|grant| grant.position_code)
            .collect::<Vec<_>>(),
            vec![Some("head".to_string()), Some("member".to_string()), None]
        );
    }
}
