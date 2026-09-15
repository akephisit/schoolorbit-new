use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::PgPool;
use uuid::Uuid;

use crate::PermissionCache;

#[derive(Clone, Debug)]
pub struct ActorContext {
    pub user_id: Uuid,
    pub permissions: Vec<String>,
}

impl ActorContext {
    pub fn has_permission(&self, required_permission: &str) -> bool {
        permission_matches(&self.permissions, required_permission)
    }

    pub fn has_any_permission(&self, required_permissions: &[&str]) -> bool {
        required_permissions
            .iter()
            .any(|permission| self.has_permission(permission))
    }

    pub fn has_all_permissions(&self, required_permissions: &[&str]) -> bool {
        required_permissions
            .iter()
            .all(|permission| self.has_permission(permission))
    }

    pub fn has_module_permission(&self, module: &str) -> bool {
        module_permission_matches(&self.permissions, module)
    }

    pub fn require_permission(&self, required_permission: &str) -> Result<(), AppError> {
        if self.has_permission(required_permission) {
            Ok(())
        } else {
            Err(AppError::Forbidden(format!("ไม่มีสิทธิ์ {required_permission}")))
        }
    }

    pub fn require_any_permission(&self, required_permissions: &[&str]) -> Result<(), AppError> {
        if self.has_any_permission(required_permissions) {
            Ok(())
        } else {
            Err(AppError::Forbidden(format!(
                "ไม่มีสิทธิ์ {}",
                required_permissions.join(" หรือ ")
            )))
        }
    }

    pub fn require_all_permissions(&self, required_permissions: &[&str]) -> Result<(), AppError> {
        if self.has_all_permissions(required_permissions) {
            Ok(())
        } else {
            Err(AppError::Forbidden(format!(
                "ไม่มีสิทธิ์ครบถ้วน: {}",
                required_permissions.join(", ")
            )))
        }
    }
}

pub async fn get_cached_user_permissions(
    tenant: &str,
    user_id: Uuid,
    pool: &PgPool,
    cache: &PermissionCache,
) -> Result<Vec<String>, sqlx::Error> {
    if let Some(permissions) = cache.get(tenant, user_id) {
        return Ok(permissions);
    }
    let revision = cache.snapshot_revision(tenant, user_id);
    let permissions = fetch_user_permissions(user_id, pool).await?;
    if !cache.fill_if_current(tenant, user_id, revision, permissions.clone()) {
        tracing::debug!(
            tenant,
            user_id = %user_id,
            "Skipped stale permission cache fill after invalidation"
        );
    }
    Ok(permissions)
}

pub fn permission_matches(permissions: &[String], required_permission: &str) -> bool {
    permissions
        .iter()
        .any(|permission| permission == codes::WILDCARD || permission == required_permission)
}

pub fn module_permission_matches(permissions: &[String], module: &str) -> bool {
    if module.is_empty() {
        return true;
    }
    let module_prefix = format!("{module}.");
    permissions.iter().any(|permission| {
        permission == codes::WILDCARD
            || permission == module
            || permission.starts_with(&module_prefix)
            || permission.starts_with("*.")
    })
}

async fn fetch_user_permissions(user_id: Uuid, pool: &PgPool) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT DISTINCT code FROM (
            SELECT p.code
            FROM user_roles ur
            JOIN roles r ON ur.role_id = r.id AND r.is_active = true
            JOIN role_permissions rp ON r.id = rp.role_id
            JOIN permissions p ON rp.permission_id = p.id AND p.is_active = true
            WHERE ur.user_id = $1 AND ur.ended_at IS NULL

            UNION

            SELECT p.code
            FROM organization_members om
            JOIN organization_units ou
              ON om.organization_unit_id = ou.id AND ou.is_active = true
            JOIN organization_permission_grants opg
              ON ou.id = opg.organization_unit_id
            JOIN permissions p ON opg.permission_id = p.id AND p.is_active = true
            WHERE om.user_id = $1
              AND (om.ended_at IS NULL OR om.ended_at > CURRENT_DATE)
              AND (opg.position_code IS NULL OR opg.position_code = om.position_code)

            UNION

            SELECT p.code
            FROM organization_permission_delegations opd
            LEFT JOIN organization_units delegated_ou
              ON delegated_ou.id = opd.organization_unit_id
            JOIN permissions p ON opd.permission_id = p.id AND p.is_active = true
            WHERE opd.to_user_id = $1
              AND opd.revoked_at IS NULL
              AND (opd.expires_at IS NULL OR opd.expires_at > NOW())
              AND (opd.organization_unit_id IS NULL OR delegated_ou.is_active = true)
        ) AS perms
        WHERE EXISTS (
            SELECT 1
            FROM users active_user
            WHERE active_user.id = $1
              AND active_user.status = 'active'
        )
        ORDER BY code
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn load_actor_context(
    user_id: Uuid,
    tenant: &str,
    pool: &PgPool,
    cache: &PermissionCache,
) -> Result<ActorContext, AppError> {
    let permissions = get_cached_user_permissions(tenant, user_id, pool, cache)
        .await
        .map_err(|_| AppError::InternalServerError("ไม่สามารถตรวจสอบสิทธิ์ได้".to_string()))?;
    Ok(ActorContext {
        user_id,
        permissions,
    })
}

pub async fn load_actor_context_for_session(
    user_id: Uuid,
    tenant: &str,
    pool: &PgPool,
    cache: &PermissionCache,
) -> Result<ActorContext, AppError> {
    load_actor_context(user_id, tenant, pool, cache).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(permissions: &[&str]) -> ActorContext {
        ActorContext {
            user_id: Uuid::new_v4(),
            permissions: permissions.iter().map(ToString::to_string).collect(),
        }
    }

    #[test]
    fn exact_wildcard_and_module_matching_are_preserved() {
        assert!(permission_matches(
            &[codes::STAFF_READ_ALL.to_string()],
            codes::STAFF_READ_ALL
        ));
        assert!(permission_matches(
            &[codes::WILDCARD.to_string()],
            codes::ROLES_ASSIGN_ALL
        ));
        assert!(module_permission_matches(
            &[codes::LEARNING_OFFERING_READ_SCHOOL.to_string()],
            "learning_offering"
        ));
        assert!(module_permission_matches(
            &["*.read.school".to_string()],
            "academic_course_plan"
        ));
        assert!(!module_permission_matches(
            &[codes::LEARNING_OFFERING_READ_SCHOOL.to_string()],
            "academic"
        ));
    }

    #[test]
    fn actor_requirements_fail_closed_with_the_required_codes() {
        let actor = actor(&[codes::STAFF_READ_ALL]);
        assert!(actor.require_permission(codes::STAFF_READ_ALL).is_ok());
        assert!(matches!(
            actor.require_any_permission(&[codes::ROLES_ASSIGN_ALL, codes::ROLES_UPDATE_ALL]),
            Err(AppError::Forbidden(message))
                if message.contains(codes::ROLES_ASSIGN_ALL)
                    && message.contains(codes::ROLES_UPDATE_ALL)
        ));
    }
}
