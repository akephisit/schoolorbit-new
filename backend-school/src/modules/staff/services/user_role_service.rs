use crate::error::AppError;
use crate::modules::staff::models::*;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
struct UserRoleAssignmentRow {
    id: Uuid,
    user_id: Uuid,
    role_id: Uuid,
    department_id: Option<Uuid>,
    is_primary: bool,
    started_at: NaiveDate,
    ended_at: Option<NaiveDate>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    role_code: String,
    role_name: String,
    role_name_en: Option<String>,
    role_description: Option<String>,
    role_user_type: String,
    role_level: i32,
    role_permissions: Vec<String>,
    role_is_active: bool,
    role_created_at: DateTime<Utc>,
    role_updated_at: DateTime<Utc>,
}

pub async fn get_user_roles(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<UserRoleAssignmentResponse>, AppError> {
    let rows = sqlx::query_as::<_, UserRoleAssignmentRow>(
        r#"SELECT
            ur.id,
            ur.user_id,
            ur.role_id,
            ur.department_id,
            ur.is_primary,
            ur.started_at,
            ur.ended_at,
            ur.notes,
            ur.created_at,
            ur.updated_at,
            r.code AS role_code,
            r.name AS role_name,
            r.name_en AS role_name_en,
            r.description AS role_description,
            r.user_type AS role_user_type,
            r.level AS role_level,
            COALESCE(
                array_agg(p.code) FILTER (WHERE p.code IS NOT NULL),
                '{}'
            ) AS role_permissions,
            r.is_active AS role_is_active,
            r.created_at AS role_created_at,
            r.updated_at AS role_updated_at
         FROM user_roles ur
         JOIN roles r ON ur.role_id = r.id
         LEFT JOIN role_permissions rp ON r.id = rp.role_id
         LEFT JOIN permissions p ON rp.permission_id = p.id
         WHERE ur.user_id = $1 AND ur.ended_at IS NULL AND r.is_active = true
         GROUP BY ur.id, r.id
         ORDER BY ur.is_primary DESC, r.level DESC, r.name"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        eprintln!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
    })?;

    Ok(rows
        .into_iter()
        .map(|row| UserRoleAssignmentResponse {
            id: row.id,
            user_id: row.user_id,
            role_id: row.role_id,
            department_id: row.department_id,
            role: Role {
                id: row.role_id,
                code: row.role_code,
                name: row.role_name,
                name_en: row.role_name_en,
                description: row.role_description,
                user_type: row.role_user_type,
                level: row.role_level,
                permissions: row.role_permissions,
                is_active: row.role_is_active,
                created_at: row.role_created_at,
                updated_at: row.role_updated_at,
            },
            is_primary: row.is_primary,
            started_at: row.started_at,
            ended_at: row.ended_at,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

pub enum AssignRoleOutcome {
    Created(Uuid),
    UserNotFound,
    RoleNotFound,
    UserTypeMismatch(String),
}

pub async fn assign_user_role(
    pool: &PgPool,
    user_id: Uuid,
    payload: AssignRoleRequest,
) -> Result<AssignRoleOutcome, AppError> {
    let user_type: Option<String> = sqlx::query_scalar("SELECT user_type FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to fetch user: {}", e);
            AppError::InternalServerError("ไม่สามารถตรวจสอบข้อมูลผู้ใช้ได้".to_string())
        })?;

    let role_user_type: Option<String> =
        sqlx::query_scalar("SELECT user_type FROM roles WHERE id = $1 AND is_active = true")
            .bind(payload.role_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| {
                eprintln!("Failed to fetch role: {}", e);
                AppError::InternalServerError("ไม่สามารถตรวจสอบข้อมูลบทบาทได้".to_string())
            })?;

    if let Some(outcome) = assign_role_outcome_for_types(user_type, role_user_type) {
        return Ok(outcome);
    }

    let user_role_id: Uuid = sqlx::query_scalar(
        "INSERT INTO user_roles (user_id, role_id, is_primary, started_at, notes)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(user_id)
    .bind(payload.role_id)
    .bind(payload.is_primary.unwrap_or(false))
    .bind(
        payload
            .started_at
            .unwrap_or_else(|| chrono::Utc::now().naive_utc().date()),
    )
    .bind(payload.notes)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to assign role: {}", e);
        AppError::InternalServerError("ไม่สามารถมอบหมายบทบาทได้".to_string())
    })?;

    Ok(AssignRoleOutcome::Created(user_role_id))
}

pub async fn remove_user_role(
    pool: &PgPool,
    user_id: Uuid,
    role_id: Uuid,
) -> Result<bool, AppError> {
    let result = sqlx::query(
        "UPDATE user_roles SET ended_at = CURRENT_DATE, updated_at = NOW()
         WHERE user_id = $1 AND role_id = $2 AND ended_at IS NULL",
    )
    .bind(user_id)
    .bind(role_id)
    .execute(pool)
    .await
    .map_err(|e| {
        eprintln!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(result.rows_affected() > 0)
}

fn assign_role_outcome_for_types(
    user_type: Option<String>,
    role_user_type: Option<String>,
) -> Option<AssignRoleOutcome> {
    let user_type = match user_type {
        Some(user_type) => user_type,
        None => return Some(AssignRoleOutcome::UserNotFound),
    };
    let role_user_type = match role_user_type {
        Some(role_user_type) => role_user_type,
        None => return Some(AssignRoleOutcome::RoleNotFound),
    };

    if user_type != role_user_type {
        Some(AssignRoleOutcome::UserTypeMismatch(role_user_type))
    } else {
        None
    }
}

pub async fn get_user_permissions(pool: &PgPool, user_id: Uuid) -> Result<Vec<String>, AppError> {
    sqlx::query_scalar(
        "SELECT DISTINCT p.code
         FROM user_roles ur
         JOIN role_permissions rp ON ur.role_id = rp.role_id
         JOIN permissions p ON rp.permission_id = p.id
         WHERE ur.user_id = $1 AND ur.ended_at IS NULL
         ORDER BY p.code",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        eprintln!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assign_role_outcome_for_types_reports_missing_user_or_role() {
        assert!(matches!(
            assign_role_outcome_for_types(None, Some("staff".to_string())),
            Some(AssignRoleOutcome::UserNotFound)
        ));
        assert!(matches!(
            assign_role_outcome_for_types(Some("staff".to_string()), None),
            Some(AssignRoleOutcome::RoleNotFound)
        ));
    }

    #[test]
    fn assign_role_outcome_for_types_reports_type_mismatch() {
        assert!(matches!(
            assign_role_outcome_for_types(Some("student".to_string()), Some("staff".to_string())),
            Some(AssignRoleOutcome::UserTypeMismatch(role_user_type)) if role_user_type == "staff"
        ));
    }

    #[test]
    fn assign_role_outcome_for_types_allows_matching_user_and_role_types() {
        assert!(assign_role_outcome_for_types(
            Some("staff".to_string()),
            Some("staff".to_string())
        )
        .is_none());
    }
}
