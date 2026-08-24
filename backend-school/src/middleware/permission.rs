use crate::db::permission_cache::PermissionCache;
use crate::error::AppError;
use crate::permissions::registry::codes;
use sqlx::PgPool;
use uuid::Uuid;

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

    #[allow(dead_code)]
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
            Err(AppError::Forbidden(format!(
                "ไม่มีสิทธิ์ {}",
                required_permission
            )))
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

    #[allow(dead_code)]
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

/// Fetch user's effective permissions from DB (position-aware + delegations).
/// This is the single source of truth used by actor context and permission checks.
async fn fetch_user_permissions(
    user_id: Uuid,
    pool: &sqlx::PgPool,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT DISTINCT code FROM (
            -- 1. Role-based permissions
            SELECT p.code
            FROM user_roles ur
            JOIN roles r ON ur.role_id = r.id AND r.is_active = true
            JOIN role_permissions rp ON r.id = rp.role_id
            JOIN permissions p ON rp.permission_id = p.id AND p.is_active = true
            WHERE ur.user_id = $1 AND ur.ended_at IS NULL

            UNION

            -- 2. Organization permission grants (position-aware)
            --    opg.position_code IS NULL  → applies to all positions
            --    opg.position_code = om.position_code → applies to that specific position only
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

            -- 3. Delegated permissions (from organization leader → this user)
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
    use crate::{
        modules::academic::cutover_test_support::{
            apply_migrations_through, seed_academic_cutover_fixture, CutoverFixture,
        },
        test_helpers::create_named_test_pool,
    };

    fn actor(permissions: &[&str]) -> ActorContext {
        ActorContext {
            user_id: Uuid::new_v4(),
            permissions: permissions
                .iter()
                .map(|permission| permission.to_string())
                .collect(),
        }
    }

    #[test]
    fn permission_matches_allows_exact_permission_and_wildcard() {
        assert!(permission_matches(
            &[codes::STAFF_READ_ALL.to_string()],
            codes::STAFF_READ_ALL
        ));
        assert!(permission_matches(
            &[codes::WILDCARD.to_string()],
            codes::STAFF_READ_ALL
        ));
    }

    #[test]
    fn permission_matches_rejects_unrelated_permission() {
        assert!(!permission_matches(
            &[codes::STAFF_READ_ALL.to_string()],
            codes::ROLES_ASSIGN_ALL
        ));
    }

    #[test]
    fn module_permission_matches_handles_empty_exact_prefix_and_unrelated_modules() {
        assert!(module_permission_matches(&[], ""));
        assert!(module_permission_matches(
            &["academic".to_string()],
            "academic"
        ));
        assert!(module_permission_matches(
            &[codes::LEARNING_OFFERING_READ_SCHOOL.to_string()],
            "learning_offering"
        ));
        assert!(!module_permission_matches(
            &[codes::LEARNING_OFFERING_READ_SCHOOL.to_string()],
            "academic_course_plan"
        ));
        assert!(!module_permission_matches(
            &[codes::LEARNING_OFFERING_READ_SCHOOL.to_string()],
            "academic"
        ));
    }

    #[test]
    fn module_permission_matches_allows_wildcard_and_global_action_permissions() {
        assert!(module_permission_matches(
            &[codes::WILDCARD.to_string()],
            "academic_course_plan"
        ));
        assert!(module_permission_matches(
            &["*.read.school".to_string()],
            "academic_course_plan"
        ));
    }

    #[test]
    fn actor_context_require_helpers_return_forbidden_when_missing_permissions() {
        let actor = actor(&[codes::STAFF_READ_ALL]);

        assert!(actor.require_permission(codes::STAFF_READ_ALL).is_ok());
        assert!(matches!(
            actor.require_permission(codes::ROLES_ASSIGN_ALL),
            Err(AppError::Forbidden(message)) if message.contains(codes::ROLES_ASSIGN_ALL)
        ));
        assert!(matches!(
            actor.require_any_permission(&[codes::ROLES_ASSIGN_ALL, codes::ROLES_UPDATE_ALL]),
            Err(AppError::Forbidden(message))
                if message.contains(codes::ROLES_ASSIGN_ALL)
                    && message.contains(codes::ROLES_UPDATE_ALL)
        ));
    }

    #[tokio::test]
    async fn effective_permissions_exclude_inactive_cutover_evidence() {
        let pool = create_named_test_pool("permission_cutover_effective").await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        crate::db::migration::run_tenant_migrations(&pool)
            .await
            .unwrap();

        sqlx::query(
            r#"INSERT INTO user_roles (user_id, role_id, is_primary, started_at)
               VALUES (
                   '50000000-0000-0000-0000-000000000002',
                   'a1b2c957-bf35-47f8-bbf4-8a67ce6b777f',
                   true,
                   '2025-05-01'
               )"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let permissions = fetch_user_permissions(
            Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap(),
            &pool,
        )
        .await
        .unwrap();

        assert!(permissions
            .iter()
            .any(|code| code == "academic_context.read.school"));
        assert!(permissions
            .iter()
            .any(|code| code == "academic_year.read.school"));
        assert!(!permissions
            .iter()
            .any(|code| code == "academic_structure.read.all"));
    }
}
