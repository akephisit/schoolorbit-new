use crate::audit::AuditLogBuilder;
use crate::models::{CreateRoleRequest, Role, UpdateRoleRequest};
use school_database::DbSession;
use school_errors::AppError;
use serde_json::json;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::{lock_audit_actor, StatusTransitionOutcome};

const ROLE_SELECT_WITH_PERMISSIONS: &str = r#"
SELECT r.*,
       COALESCE(
           array_agg(p.code) FILTER (WHERE p.code IS NOT NULL),
           '{}'
       ) as permissions
FROM roles r
LEFT JOIN role_permissions rp ON r.id = rp.role_id
LEFT JOIN permissions p ON rp.permission_id = p.id AND p.is_active = true
"#;

async fn insert_role_permissions(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    role_id: Uuid,
    permission_ids: &[Uuid],
) -> Result<(), AppError> {
    if permission_ids.is_empty() {
        return Ok(());
    }

    DbSession::new(&mut **tx)
        .query(
            "staff.services.role_service.insert_role_permissions",
            "INSERT INTO role_permissions (role_id, permission_id)
         SELECT $1, permission_id
         FROM UNNEST($2::uuid[]) AS permissions(permission_id)
         ON CONFLICT DO NOTHING",
        )
        .bind(role_id)
        .bind(permission_ids)
        .execute()
        .await
        .map_err(|e| {
            tracing::error!(
                "❌ Failed to assign permissions: {}",
                school_database::error_class(&e)
            );
            AppError::InternalServerError("ไม่สามารถกำหนดสิทธิ์ได้".to_string())
        })?;

    Ok(())
}

pub async fn list_roles(pool: &PgPool, include_inactive: bool) -> Result<Vec<Role>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "staff.services.role_service.list_roles",
    )
    .await?;
    list_roles_on_connection(&mut connection, include_inactive).await
}

pub async fn list_roles_on_connection(
    connection: &mut sqlx::PgConnection,
    include_inactive: bool,
) -> Result<Vec<Role>, AppError> {
    let active_filter = if include_inactive {
        ""
    } else {
        "WHERE r.is_active = true"
    };
    let sql = format!(
        "{} {} GROUP BY r.id ORDER BY r.level DESC, r.name",
        ROLE_SELECT_WITH_PERMISSIONS, active_filter
    );
    DbSession::new(connection)
        .query_as::<Role>(
            "staff.services.role_service.list_roles_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .fetch_all()
        .await
        .map_err(|e| {
            tracing::error!(
                "❌ Database error (list_roles): {}",
                school_database::error_class(&e)
            );
            AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
        })
}

pub async fn get_role(pool: &PgPool, role_id: Uuid) -> Result<Role, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "staff.services.role_service.get_role",
    )
    .await?;
    get_role_on_connection(&mut connection, role_id).await
}

pub async fn get_role_on_connection(
    connection: &mut sqlx::PgConnection,
    role_id: Uuid,
) -> Result<Role, AppError> {
    let sql = format!(
        "{} WHERE r.id = $1 GROUP BY r.id",
        ROLE_SELECT_WITH_PERMISSIONS
    );
    DbSession::new(connection)
        .query_as::<Role>(
            "staff.services.role_service.get_role_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(role_id)
        .fetch_optional()
        .await
        .map_err(|e| {
            tracing::error!("❌ Database error: {}", school_database::error_class(&e));
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?
        .ok_or_else(|| AppError::NotFound("ไม่พบบทบาท".to_string()))
}

/// สร้าง role ใหม่ + assign permissions ใน transaction
pub async fn create_role(pool: &PgPool, payload: CreateRoleRequest) -> Result<Uuid, AppError> {
    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(
            "❌ Failed to start transaction: {}",
            school_database::error_class(&e)
        );
        AppError::InternalServerError("ไม่สามารถเริ่มต้น Transaction ได้".to_string())
    })?;

    let role_id: Uuid = DbSession::new(&mut *tx)
        .scalar(
            "staff.services.role_service.create_role",
            "INSERT INTO roles (code, name, name_en, description, user_type, level)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
        )
        .bind(&payload.code)
        .bind(&payload.name)
        .bind(&payload.name_en)
        .bind(&payload.description)
        .bind(&payload.user_type)
        .bind(role_level_or_default(payload.level))
        .fetch_one()
        .await
        .map_err(|e| {
            tracing::error!(
                "❌ Failed to create role: {}",
                school_database::error_class(&e)
            );
            let err_msg = e.to_string();
            if err_msg.contains("duplicate key value") && err_msg.contains("code") {
                AppError::BadRequest("รหัสบทบาทนี้มีอยู่ในระบบแล้ว".to_string())
            } else {
                AppError::InternalServerError("ไม่สามารถสร้างบทบาทได้".to_string())
            }
        })?;

    // Insert permissions if provided
    if let Some(permissions) = &payload.permissions {
        if !permissions.is_empty() {
            let perm_ids: Vec<Uuid> = DbSession::new(&mut *tx)
                .scalar(
                    "staff.services.role_service.create_role",
                    "SELECT id FROM permissions WHERE code = ANY($1) AND is_active = true",
                )
                .bind(permissions)
                .fetch_all()
                .await
                .map_err(|e| {
                    tracing::error!(
                        "❌ Failed to find permissions: {}",
                        school_database::error_class(&e)
                    );
                    AppError::InternalServerError("ไม่พบสิทธิ์การใช้งานที่ระบุ".to_string())
                })?;

            insert_role_permissions(&mut tx, role_id, &perm_ids).await?;
        }
    }

    tx.commit().await.map_err(|e| {
        tracing::error!(
            "❌ Failed to commit transaction: {}",
            school_database::error_class(&e)
        );
        AppError::InternalServerError("ไม่สามารถบันทึกข้อมูลได้".to_string())
    })?;

    Ok(role_id)
}

/// Update role + sync permissions ใน transaction
/// Caller ต้อง invalidate permission cache เอง (handler รับผิดชอบ side effect นี้)
pub async fn update_role(
    pool: &PgPool,
    role_id: Uuid,
    payload: UpdateRoleRequest,
    actor_user_id: Uuid,
) -> Result<StatusTransitionOutcome, AppError> {
    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!(
            "❌ Failed to start transaction: {}",
            school_database::error_class(&e)
        );
        AppError::InternalServerError("ไม่สามารถเริ่มต้น Transaction ได้".to_string())
    })?;
    lock_audit_actor(&mut tx, actor_user_id).await?;

    let status_outcome = if let Some(is_active) = payload.is_active {
        transition_role_status(&mut tx, role_id, is_active, actor_user_id).await?
    } else {
        StatusTransitionOutcome::Unchanged
    };

    let query_result = DbSession::new(&mut *tx)
        .query(
            "staff.services.role_service.update_role",
            "UPDATE roles
         SET
            name = COALESCE($2, name),
            name_en = COALESCE($3, name_en),
            description = COALESCE($4, description),
            user_type = COALESCE($5, user_type),
            level = COALESCE($6, level),
            updated_at = NOW()
         WHERE id = $1",
        )
        .bind(role_id)
        .bind(&payload.name)
        .bind(&payload.name_en)
        .bind(&payload.description)
        .bind(&payload.user_type)
        .bind(payload.level)
        .execute()
        .await
        .map_err(|e| {
            tracing::error!(
                "❌ Failed to update role: {}",
                school_database::error_class(&e)
            );
            let err_msg = e.to_string();
            if err_msg.contains("duplicate key value") && err_msg.contains("code") {
                AppError::BadRequest("รหัสบทบาทนี้มีอยู่ในระบบแล้ว".to_string())
            } else {
                AppError::InternalServerError("เกิดข้อผิดพลาดในการอัปเดตบทบาท".to_string())
            }
        })?;

    if query_result.rows_affected() == 0 {
        if let Err(rb_err) = tx.rollback().await {
            tracing::error!("⚠️ Transaction rollback failed: {}", rb_err);
        }
        return Err(AppError::NotFound("ไม่พบบทบาท".to_string()));
    }

    // Re-sync permissions if provided
    if let Some(permissions) = &payload.permissions {
        DbSession::new(&mut *tx)
            .query(
                "staff.services.role_service.update_role",
                "DELETE FROM role_permissions WHERE role_id = $1",
            )
            .bind(role_id)
            .execute()
            .await
            .map_err(|e| {
                tracing::error!(
                    "❌ Failed to clear old permissions: {}",
                    school_database::error_class(&e)
                );
                AppError::InternalServerError("เกิดข้อผิดพลาดในการลบสิทธิ์เดิม".to_string())
            })?;

        if !permissions.is_empty() {
            let perm_ids: Vec<Uuid> = DbSession::new(&mut *tx)
                .scalar(
                    "staff.services.role_service.update_role",
                    "SELECT id FROM permissions WHERE code = ANY($1) AND is_active = true",
                )
                .bind(permissions)
                .fetch_all()
                .await
                .map_err(|e| {
                    tracing::error!(
                        "❌ Failed to find permissions: {}",
                        school_database::error_class(&e)
                    );
                    AppError::InternalServerError("ไม่พบสิทธิ์การใช้งานที่ระบุ".to_string())
                })?;

            insert_role_permissions(&mut tx, role_id, &perm_ids).await?;
        }
    }

    tx.commit().await.map_err(|e| {
        tracing::error!(
            "❌ Failed to commit transaction: {}",
            school_database::error_class(&e)
        );
        AppError::InternalServerError("ไม่สามารถบันทึกข้อมูลได้".to_string())
    })?;

    Ok(status_outcome)
}

#[derive(Debug, FromRow)]
struct RoleStatusRow {
    name: String,
    is_active: bool,
    is_system: bool,
}

async fn transition_role_status(
    tx: &mut Transaction<'_, Postgres>,
    role_id: Uuid,
    is_active: bool,
    actor_user_id: Uuid,
) -> Result<StatusTransitionOutcome, AppError> {
    let role = DbSession::new(&mut **tx).query_as::<RoleStatusRow>("staff.services.role_service.transition_role_status",
        "SELECT name, is_active, is_system
         FROM roles
         WHERE id = $1
         FOR UPDATE",
    )
    .bind(role_id)
    .fetch_optional()
    .await
    .map_err(|error| {
        tracing::error!(%role_id, error_class = school_database::error_class(&error), "failed to lock role for status transition");
        AppError::InternalServerError("ไม่สามารถตรวจสอบสถานะบทบาทได้".to_string())
    })?
    .ok_or_else(|| AppError::NotFound("ไม่พบบทบาท".to_string()))?;

    if !is_active && role.is_system {
        return Err(AppError::Conflict("ไม่สามารถปิดใช้งานบทบาทระบบได้".to_string()));
    }

    if role.is_active == is_active {
        return Ok(StatusTransitionOutcome::Unchanged);
    }

    DbSession::new(&mut **tx).query("staff.services.role_service.transition_role_status",
        "UPDATE roles
         SET is_active = $2, updated_at = NOW()
         WHERE id = $1",
    )
    .bind(role_id)
    .bind(is_active)
    .execute()
    .await
    .map_err(|error| {
        tracing::error!(%role_id, error_class = school_database::error_class(&error), "failed to update role status");
        AppError::InternalServerError("ไม่สามารถเปลี่ยนสถานะบทบาทได้".to_string())
    })?;

    let action = if is_active {
        "reactivate"
    } else {
        "deactivate"
    };
    AuditLogBuilder::new(action, "role")
        .user(actor_user_id, None, None)
        .entity(role_id, Some(role.name))
        .old_values(json!({ "is_active": role.is_active }))
        .new_values(json!({ "is_active": is_active }))
        .changes(json!({
            "is_active": {
                "from": role.is_active,
                "to": is_active
            }
        }))
        .description(if is_active {
            "เปิดใช้งานบทบาท"
        } else {
            "ปิดใช้งานบทบาท"
        })
        .save_in_transaction(tx)
        .await
        .map_err(|_error| {
            tracing::error!(%role_id, reason = "staff_role_status_audit_failed", "failed to save role status audit");
            AppError::InternalServerError("ไม่สามารถบันทึกประวัติการเปลี่ยนสถานะได้".to_string())
        })?;

    Ok(StatusTransitionOutcome::Changed { is_active })
}

pub async fn set_role_active(
    pool: &PgPool,
    role_id: Uuid,
    is_active: bool,
    actor_user_id: Uuid,
) -> Result<StatusTransitionOutcome, AppError> {
    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(%role_id, error_class = school_database::error_class(&error), "failed to start role status transaction");
        AppError::InternalServerError("ไม่สามารถเริ่มต้น Transaction ได้".to_string())
    })?;
    lock_audit_actor(&mut tx, actor_user_id).await?;

    let outcome = transition_role_status(&mut tx, role_id, is_active, actor_user_id).await?;

    tx.commit().await.map_err(|error| {
        tracing::error!(%role_id, error_class = school_database::error_class(&error), "failed to commit role status transaction");
        AppError::InternalServerError("ไม่สามารถบันทึกสถานะบทบาทได้".to_string())
    })?;

    Ok(outcome)
}

fn role_level_or_default(level: Option<i32>) -> i32 {
    level.unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_level_or_default_uses_zero_when_missing() {
        assert_eq!(role_level_or_default(None), 0);
        assert_eq!(role_level_or_default(Some(7)), 7);
    }
}
