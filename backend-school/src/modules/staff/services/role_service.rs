use crate::error::AppError;
use crate::modules::staff::models::{CreateRoleRequest, Role, UpdateRoleRequest};
use sqlx::PgPool;
use uuid::Uuid;

const ROLE_SELECT_WITH_PERMISSIONS: &str = r#"
SELECT r.*,
       COALESCE(
           array_agg(p.code) FILTER (WHERE p.code IS NOT NULL),
           '{}'
       ) as permissions
FROM roles r
LEFT JOIN role_permissions rp ON r.id = rp.role_id
LEFT JOIN permissions p ON rp.permission_id = p.id
"#;

pub async fn list_roles(pool: &PgPool) -> Result<Vec<Role>, AppError> {
    let sql = format!(
        "{} WHERE r.is_active = true GROUP BY r.id ORDER BY r.level DESC, r.name",
        ROLE_SELECT_WITH_PERMISSIONS
    );
    sqlx::query_as::<_, Role>(&sql)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Database error (list_roles): {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
        })
}

pub async fn get_role(pool: &PgPool, role_id: Uuid) -> Result<Role, AppError> {
    let sql = format!(
        "{} WHERE r.id = $1 GROUP BY r.id",
        ROLE_SELECT_WITH_PERMISSIONS
    );
    sqlx::query_as::<_, Role>(&sql)
        .bind(role_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?
        .ok_or_else(|| AppError::NotFound("ไม่พบบทบาท".to_string()))
}

/// สร้าง role ใหม่ + assign permissions ใน transaction
pub async fn create_role(pool: &PgPool, payload: CreateRoleRequest) -> Result<Uuid, AppError> {
    let mut tx = pool.begin().await.map_err(|e| {
        eprintln!("❌ Failed to start transaction: {}", e);
        AppError::InternalServerError("ไม่สามารถเริ่มต้น Transaction ได้".to_string())
    })?;

    let role_id: Uuid = sqlx::query_scalar(
        "INSERT INTO roles (code, name, name_en, description, user_type, level)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(&payload.code)
    .bind(&payload.name)
    .bind(&payload.name_en)
    .bind(&payload.description)
    .bind(&payload.user_type)
    .bind(payload.level.unwrap_or(0))
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to create role: {}", e);
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
            let perm_ids: Vec<Uuid> =
                sqlx::query_scalar("SELECT id FROM permissions WHERE code = ANY($1)")
                    .bind(permissions)
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(|e| {
                        eprintln!("❌ Failed to find permissions: {}", e);
                        AppError::InternalServerError("ไม่พบสิทธิ์การใช้งานที่ระบุ".to_string())
                    })?;

            for perm_id in perm_ids {
                sqlx::query(
                    "INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2)",
                )
                .bind(role_id)
                .bind(perm_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    eprintln!("❌ Failed to assign permission: {}", e);
                    AppError::InternalServerError("ไม่สามารถกำหนดสิทธิ์ได้".to_string())
                })?;
            }
        }
    }

    tx.commit().await.map_err(|e| {
        eprintln!("❌ Failed to commit transaction: {}", e);
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
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(|e| {
        eprintln!("❌ Failed to start transaction: {}", e);
        AppError::InternalServerError("ไม่สามารถเริ่มต้น Transaction ได้".to_string())
    })?;

    let query_result = sqlx::query(
        "UPDATE roles
         SET
            name = COALESCE($2, name),
            name_en = COALESCE($3, name_en),
            description = COALESCE($4, description),
            user_type = COALESCE($5, user_type),
            level = COALESCE($6, level),
            is_active = COALESCE($7, is_active),
            updated_at = NOW()
         WHERE id = $1",
    )
    .bind(role_id)
    .bind(&payload.name)
    .bind(&payload.name_en)
    .bind(&payload.description)
    .bind(&payload.user_type)
    .bind(payload.level)
    .bind(payload.is_active)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to update role: {}", e);
        let err_msg = e.to_string();
        if err_msg.contains("duplicate key value") && err_msg.contains("code") {
            AppError::BadRequest("รหัสบทบาทนี้มีอยู่ในระบบแล้ว".to_string())
        } else {
            AppError::InternalServerError("เกิดข้อผิดพลาดในการอัปเดตบทบาท".to_string())
        }
    })?;

    if query_result.rows_affected() == 0 {
        if let Err(rb_err) = tx.rollback().await {
            eprintln!("⚠️ Transaction rollback failed: {}", rb_err);
        }
        return Err(AppError::NotFound("ไม่พบบทบาท".to_string()));
    }

    // Re-sync permissions if provided
    if let Some(permissions) = &payload.permissions {
        sqlx::query("DELETE FROM role_permissions WHERE role_id = $1")
            .bind(role_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                eprintln!("❌ Failed to clear old permissions: {}", e);
                AppError::InternalServerError("เกิดข้อผิดพลาดในการลบสิทธิ์เดิม".to_string())
            })?;

        if !permissions.is_empty() {
            let perm_ids: Vec<Uuid> =
                sqlx::query_scalar("SELECT id FROM permissions WHERE code = ANY($1)")
                    .bind(permissions)
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(|e| {
                        eprintln!("❌ Failed to find permissions: {}", e);
                        AppError::InternalServerError("ไม่พบสิทธิ์การใช้งานที่ระบุ".to_string())
                    })?;

            for perm_id in perm_ids {
                sqlx::query(
                    "INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2)",
                )
                .bind(role_id)
                .bind(perm_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    eprintln!("❌ Failed to assign new permission: {}", e);
                    AppError::InternalServerError("ไม่สามารถกำหนดสิทธิ์ใหม่ได้".to_string())
                })?;
            }
        }
    }

    tx.commit().await.map_err(|e| {
        eprintln!("❌ Failed to commit transaction: {}", e);
        AppError::InternalServerError("ไม่สามารถบันทึกข้อมูลได้".to_string())
    })?;

    Ok(())
}
