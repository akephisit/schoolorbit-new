//! Exact and tree organization-unit resolution for generated permissions.

use school_errors::AppError;
use sqlx::{Executor, PgConnection, PgPool, Postgres};
use uuid::Uuid;

/// Resolves exact active organization units for one generated permission code.
///
/// This checks the permission's originating role, position grant, or delegation against each
/// target unit and never traverses the unit tree.
pub async fn accessible_exact_units_for_permission(
    pool: &PgPool,
    actor_user_id: Uuid,
    permission_code: &str,
) -> Result<Vec<Uuid>, AppError> {
    query_exact_units_for_permission(pool, actor_user_id, permission_code, None).await
}

pub async fn has_exact_unit_permission(
    pool: &PgPool,
    actor_user_id: Uuid,
    organization_unit_id: Uuid,
    permission_code: &str,
) -> Result<bool, AppError> {
    Ok(query_exact_units_for_permission(
        pool,
        actor_user_id,
        permission_code,
        Some(organization_unit_id),
    )
    .await?
    .contains(&organization_unit_id))
}

/// Checks the same exact-unit policy using the caller's existing connection or transaction.
pub async fn has_exact_unit_permission_on_connection(
    connection: &mut PgConnection,
    actor_user_id: Uuid,
    organization_unit_id: Uuid,
    permission_code: &str,
) -> Result<bool, AppError> {
    Ok(query_exact_units_for_permission(
        connection,
        actor_user_id,
        permission_code,
        Some(organization_unit_id),
    )
    .await?
    .contains(&organization_unit_id))
}

/// Resolves the active descendants of every exact unit grant for one permission code.
pub async fn accessible_tree_units_for_permission(
    pool: &PgPool,
    actor_user_id: Uuid,
    permission_code: &str,
) -> Result<Vec<Uuid>, AppError> {
    let root_ids =
        accessible_exact_units_for_permission(pool, actor_user_id, permission_code).await?;
    if root_ids.is_empty() {
        return Ok(Vec::new());
    }

    sqlx::query_scalar(
        r#"
        WITH RECURSIVE organization_tree AS (
            SELECT unit.id
            FROM organization_units unit
            WHERE unit.id = ANY($1)
              AND unit.is_active = true
            UNION
            SELECT child.id
            FROM organization_units child
            JOIN organization_tree parent_tree ON parent_tree.id = child.parent_unit_id
            WHERE child.is_active = true
        )
        SELECT id
        FROM organization_tree
        ORDER BY id
        "#,
    )
    .bind(&root_ids)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!(
            reason = "organization_tree_permission_query_failed",
            database_error = %error
        );
        AppError::InternalServerError("ไม่สามารถตรวจสอบสิทธิ์สายงานได้".to_string())
    })
}

async fn query_exact_units_for_permission<'e>(
    executor: impl Executor<'e, Database = Postgres>,
    actor_user_id: Uuid,
    permission_code: &str,
    target_organization_unit_id: Option<Uuid>,
) -> Result<Vec<Uuid>, AppError> {
    sqlx::query_scalar(EXACT_UNITS_SQL)
        .bind(actor_user_id)
        .bind(permission_code)
        .bind(target_organization_unit_id)
        .fetch_all(executor)
        .await
        .map_err(|error| {
            tracing::error!(
                reason = "exact_organization_unit_permission_query_failed",
                database_error = %error
            );
            AppError::InternalServerError("ไม่สามารถตรวจสอบสิทธิ์หน่วยงานได้".to_string())
        })
}

const EXACT_UNITS_SQL: &str = r#"
        SELECT target_unit.id
        FROM organization_units target_unit
        WHERE target_unit.is_active = true
          AND ($3::uuid IS NULL OR target_unit.id = $3)
          AND (
              EXISTS (
                  SELECT 1
                  FROM organization_members om
                  JOIN user_roles ur ON ur.user_id = om.user_id
                  JOIN roles r ON r.id = ur.role_id AND r.is_active = true
                  JOIN role_permissions rp ON rp.role_id = r.id
                  JOIN permissions p ON p.id = rp.permission_id AND p.is_active = true
                  WHERE om.user_id = $1
                    AND om.organization_unit_id = target_unit.id
                    AND om.started_at <= CURRENT_DATE
                    AND (om.ended_at IS NULL OR om.ended_at > CURRENT_DATE)
                    AND ur.started_at <= CURRENT_DATE
                    AND (ur.ended_at IS NULL OR ur.ended_at > CURRENT_DATE)
                    AND (
                        ur.organization_unit_id IS NULL
                        OR ur.organization_unit_id = target_unit.id
                    )
                    AND p.code = $2
              )
              OR EXISTS (
                  SELECT 1
                  FROM organization_members om
                  JOIN organization_permission_grants opg
                    ON opg.organization_unit_id = om.organization_unit_id
                  JOIN permissions p ON p.id = opg.permission_id AND p.is_active = true
                  WHERE om.user_id = $1
                    AND om.organization_unit_id = target_unit.id
                    AND om.started_at <= CURRENT_DATE
                    AND (om.ended_at IS NULL OR om.ended_at > CURRENT_DATE)
                    AND p.code = $2
                    AND (
                        opg.position_code IS NULL
                        OR opg.position_code = om.position_code
                    )
              )
              OR EXISTS (
                  SELECT 1
                  FROM organization_permission_delegations opd
                  JOIN permissions p ON p.id = opd.permission_id AND p.is_active = true
                  WHERE opd.to_user_id = $1
                    AND opd.organization_unit_id = target_unit.id
                    AND p.code = $2
                    AND opd.started_at <= NOW()
                    AND opd.revoked_at IS NULL
                    AND (opd.expires_at IS NULL OR opd.expires_at > NOW())
              )
          )
        ORDER BY target_unit.id
        "#;

/// Batch the same exact permission-origin policy for bounded recipient acceptance.
pub(crate) async fn exact_units_for_users_on_connection(
    connection: &mut PgConnection,
    users: &[Uuid],
    code: &str,
    tree: bool,
) -> Result<Vec<(Uuid, Uuid)>, AppError> {
    if users.is_empty() {
        return Ok(vec![]);
    }
    let exact = EXACT_UNITS_SQL.replace("$1", "actor.user_id");
    let roots=format!("SELECT actor.user_id,permitted.id AS organization_unit_id FROM unnest($1::uuid[]) actor(user_id) CROSS JOIN LATERAL ({exact}) permitted");
    let query = if tree {
        format!("WITH RECURSIVE roots AS ({roots}), tree(user_id,organization_unit_id) AS (SELECT user_id,organization_unit_id FROM roots UNION SELECT tree.user_id,child.id FROM tree JOIN organization_units child ON child.parent_unit_id=tree.organization_unit_id AND child.is_active) SELECT user_id,organization_unit_id FROM tree ORDER BY user_id,organization_unit_id")
    } else {
        format!("{roots} ORDER BY actor.user_id,permitted.id")
    };
    sqlx::query_as(sqlx::AssertSqlSafe(query))
        .bind(users)
        .bind(code)
        .bind(Option::<Uuid>::None)
        .fetch_all(connection)
        .await
        .map_err(AppError::from)
}
