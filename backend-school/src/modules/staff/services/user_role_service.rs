use crate::error::AppError;
use crate::modules::staff::models::*;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn get_user_roles(pool: &PgPool, user_id: Uuid) -> Result<Vec<Role>, AppError> {
    sqlx::query_as::<_, Role>(
        "SELECT r.* FROM roles r
         JOIN user_roles ur ON ur.role_id = r.id
         WHERE ur.user_id = $1 AND ur.ended_at IS NULL AND r.is_active = true
         ORDER BY ur.is_primary DESC, r.level DESC"
    )
    .bind(user_id).fetch_all(pool).await
    .map_err(|e| {
        eprintln!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
    })
}

pub enum AssignRoleOutcome {
    Created(Uuid),
    UserNotFound,
    RoleNotFound,
    UserTypeMismatch(String),
}

pub async fn assign_user_role(pool: &PgPool, user_id: Uuid, payload: AssignRoleRequest) -> Result<AssignRoleOutcome, AppError> {
    let user_type: Option<String> = sqlx::query_scalar("SELECT user_type FROM users WHERE id = $1")
        .bind(user_id).fetch_optional(pool).await
        .map_err(|e| {
            eprintln!("Failed to fetch user: {}", e);
            AppError::InternalServerError("ไม่สามารถตรวจสอบข้อมูลผู้ใช้ได้".to_string())
        })?;

    let user_type = match user_type {
        Some(ut) => ut,
        None => return Ok(AssignRoleOutcome::UserNotFound),
    };

    let role_user_type: Option<String> = sqlx::query_scalar(
        "SELECT user_type FROM roles WHERE id = $1 AND is_active = true"
    )
    .bind(payload.role_id).fetch_optional(pool).await
    .map_err(|e| {
        eprintln!("Failed to fetch role: {}", e);
        AppError::InternalServerError("ไม่สามารถตรวจสอบข้อมูลบทบาทได้".to_string())
    })?;

    let role_user_type = match role_user_type {
        Some(rut) => rut,
        None => return Ok(AssignRoleOutcome::RoleNotFound),
    };

    if user_type != role_user_type {
        return Ok(AssignRoleOutcome::UserTypeMismatch(role_user_type));
    }

    let user_role_id: Uuid = sqlx::query_scalar(
        "INSERT INTO user_roles (user_id, role_id, is_primary, started_at, notes)
         VALUES ($1, $2, $3, $4, $5) RETURNING id"
    )
    .bind(user_id).bind(payload.role_id)
    .bind(payload.is_primary.unwrap_or(false))
    .bind(payload.started_at.unwrap_or_else(|| chrono::Utc::now().naive_utc().date()))
    .bind(payload.notes)
    .fetch_one(pool).await
    .map_err(|e| {
        eprintln!("Failed to assign role: {}", e);
        AppError::InternalServerError("ไม่สามารถมอบหมายบทบาทได้".to_string())
    })?;

    Ok(AssignRoleOutcome::Created(user_role_id))
}

pub async fn remove_user_role(pool: &PgPool, user_id: Uuid, role_id: Uuid) -> Result<bool, AppError> {
    let result = sqlx::query(
        "UPDATE user_roles SET ended_at = CURRENT_DATE, updated_at = NOW()
         WHERE user_id = $1 AND role_id = $2 AND ended_at IS NULL"
    )
    .bind(user_id).bind(role_id).execute(pool).await
    .map_err(|e| {
        eprintln!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(result.rows_affected() > 0)
}

pub async fn get_user_permissions(pool: &PgPool, user_id: Uuid) -> Result<Vec<String>, AppError> {
    sqlx::query_scalar(
        "SELECT DISTINCT p.code
         FROM user_roles ur
         JOIN role_permissions rp ON ur.role_id = rp.role_id
         JOIN permissions p ON rp.permission_id = p.id
         WHERE ur.user_id = $1 AND ur.ended_at IS NULL
         ORDER BY p.code"
    )
    .bind(user_id).fetch_all(pool).await
    .map_err(|e| {
        eprintln!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })
}
