use crate::db::permission_cache::PermissionCache;
use crate::error::AppError;
use crate::middleware::permission::get_cached_user_permissions;
use crate::modules::staff::models::{CreateOrganizationUnitRequest, UpdateOrganizationUnitRequest};
use crate::test_helpers::{create_test_pool, create_test_user, run_test_migrations};
use sqlx::PgPool;
use uuid::Uuid;

use super::{organization_unit_service, role_service, StatusTransitionOutcome};

async fn permission_id(pool: &PgPool, code: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM permissions WHERE code = $1")
        .bind(code)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|error| panic!("missing test permission {code}: {error}"))
}

fn assert_has_permission(permissions: &[String], code: &str) {
    assert!(
        permissions.iter().any(|permission| permission == code),
        "expected {code} in {permissions:?}"
    );
}

fn assert_lacks_permission(permissions: &[String], code: &str) {
    assert!(
        permissions.iter().all(|permission| permission != code),
        "did not expect {code} in {permissions:?}"
    );
}

#[tokio::test]
async fn inactive_authorization_sources_stop_granting_permissions() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;

    let fixture = Uuid::new_v4().simple().to_string();
    let target_user_id = create_test_user(
        &pool,
        &format!("status-target-{}@example.test", &fixture[..8]),
        "Test1234!",
    )
    .await
    .expect("target test user should be created");
    let delegator_user_id = create_test_user(
        &pool,
        &format!("status-from-{}@example.test", &fixture[..8]),
        "Test1234!",
    )
    .await
    .expect("delegator test user should be created");

    let role_id: Uuid = sqlx::query_scalar(
        "INSERT INTO roles (code, name, user_type, level)
         VALUES ($1, $2, 'staff', 1)
         RETURNING id",
    )
    .bind(format!("TSTAT{}", &fixture[..8]))
    .bind(format!("test status role {}", &fixture[..8]))
    .fetch_one(&pool)
    .await
    .expect("test role should be created");

    let role_permission_id = permission_id(&pool, "roles.read.all").await;
    let organization_permission_id = permission_id(&pool, "roles.create.all").await;
    let scoped_delegation_permission_id = permission_id(&pool, "roles.update.all").await;
    let unscoped_delegation_permission_id = permission_id(&pool, "roles.delete.all").await;

    sqlx::query("INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2)")
        .bind(role_id)
        .bind(role_permission_id)
        .execute(&pool)
        .await
        .expect("role permission should be assigned");
    sqlx::query(
        "INSERT INTO user_roles (user_id, role_id, is_primary)
         VALUES ($1, $2, true)",
    )
    .bind(target_user_id)
    .bind(role_id)
    .execute(&pool)
    .await
    .expect("test role should be assigned");

    let organization_unit_id: Uuid = sqlx::query_scalar(
        "INSERT INTO organization_units (code, name, category, unit_type)
         VALUES ($1, $2, 'other', 'unit')
         RETURNING id",
    )
    .bind(format!("TUNIT{}", &fixture[..8]))
    .bind(format!("test status unit {}", &fixture[..8]))
    .fetch_one(&pool)
    .await
    .expect("test organization unit should be created");

    sqlx::query(
        "INSERT INTO organization_members
            (user_id, organization_unit_id, position_code)
         VALUES ($1, $2, 'member')",
    )
    .bind(target_user_id)
    .bind(organization_unit_id)
    .execute(&pool)
    .await
    .expect("test organization membership should be created");
    sqlx::query(
        "INSERT INTO organization_permission_grants
            (organization_unit_id, permission_id)
         VALUES ($1, $2)",
    )
    .bind(organization_unit_id)
    .bind(organization_permission_id)
    .execute(&pool)
    .await
    .expect("organization permission should be granted");

    sqlx::query(
        "INSERT INTO organization_permission_delegations
            (from_user_id, to_user_id, permission_id, organization_unit_id)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(delegator_user_id)
    .bind(target_user_id)
    .bind(scoped_delegation_permission_id)
    .bind(organization_unit_id)
    .execute(&pool)
    .await
    .expect("scoped delegation should be created");
    sqlx::query(
        "INSERT INTO organization_permission_delegations
            (from_user_id, to_user_id, permission_id, organization_unit_id)
         VALUES ($1, $2, $3, NULL)",
    )
    .bind(delegator_user_id)
    .bind(target_user_id)
    .bind(unscoped_delegation_permission_id)
    .execute(&pool)
    .await
    .expect("unscoped delegation should be created");

    let tenant = format!("status-test-{}", &fixture[..8]);
    let cache = PermissionCache::new();
    let active_permissions = get_cached_user_permissions(&tenant, target_user_id, &pool, &cache)
        .await
        .expect("active permissions should load");
    for code in [
        "roles.read.all",
        "roles.create.all",
        "roles.update.all",
        "roles.delete.all",
    ] {
        assert_has_permission(&active_permissions, code);
    }

    sqlx::query("UPDATE roles SET is_active = false WHERE id = $1")
        .bind(role_id)
        .execute(&pool)
        .await
        .expect("test role should deactivate");
    sqlx::query("UPDATE organization_units SET is_active = false WHERE id = $1")
        .bind(organization_unit_id)
        .execute(&pool)
        .await
        .expect("test organization unit should deactivate");
    cache.invalidate_tenant(&tenant);

    let inactive_permissions = get_cached_user_permissions(&tenant, target_user_id, &pool, &cache)
        .await
        .expect("inactive permissions should load");
    for code in ["roles.read.all", "roles.create.all", "roles.update.all"] {
        assert_lacks_permission(&inactive_permissions, code);
    }
    assert_has_permission(&inactive_permissions, "roles.delete.all");

    sqlx::query("UPDATE roles SET is_active = true WHERE id = $1")
        .bind(role_id)
        .execute(&pool)
        .await
        .expect("test role should reactivate");
    sqlx::query("UPDATE organization_units SET is_active = true WHERE id = $1")
        .bind(organization_unit_id)
        .execute(&pool)
        .await
        .expect("test organization unit should reactivate");
    cache.invalidate_tenant(&tenant);

    let reactivated_permissions =
        get_cached_user_permissions(&tenant, target_user_id, &pool, &cache)
            .await
            .expect("reactivated permissions should load");
    for code in [
        "roles.read.all",
        "roles.create.all",
        "roles.update.all",
        "roles.delete.all",
    ] {
        assert_has_permission(&reactivated_permissions, code);
    }
}

#[tokio::test]
async fn role_status_transitions_are_protected_idempotent_and_audited() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;

    let fixture = Uuid::new_v4().simple().to_string();
    let actor_user_id = create_test_user(
        &pool,
        &format!("status-actor-{}@example.test", &fixture[..8]),
        "Test1234!",
    )
    .await
    .expect("actor test user should be created");
    let role_id: Uuid = sqlx::query_scalar(
        "INSERT INTO roles (code, name, user_type, level)
         VALUES ($1, $2, 'staff', 1)
         RETURNING id",
    )
    .bind(format!("TROLE{}", &fixture[..8]))
    .bind(format!("test transition role {}", &fixture[..8]))
    .fetch_one(&pool)
    .await
    .expect("test role should be created");

    let first_deactivation = role_service::set_role_active(&pool, role_id, false, actor_user_id)
        .await
        .expect("custom role should deactivate");
    assert_eq!(
        first_deactivation,
        StatusTransitionOutcome::Changed { is_active: false }
    );
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT is_active FROM roles WHERE id = $1")
            .bind(role_id)
            .fetch_one(&pool)
            .await
            .expect("role status should load")
    );

    let first_audit: (
        String,
        Option<Uuid>,
        Option<serde_json::Value>,
        Option<serde_json::Value>,
    ) = sqlx::query_as(
        "SELECT action, user_id, old_values, new_values
             FROM audit_logs
             WHERE entity_type = 'role' AND entity_id = $1",
    )
    .bind(role_id)
    .fetch_one(&pool)
    .await
    .expect("deactivation audit should exist");
    assert_eq!(first_audit.0, "deactivate");
    assert_eq!(first_audit.1, Some(actor_user_id));
    assert_eq!(
        first_audit.2,
        Some(serde_json::json!({ "is_active": true }))
    );
    assert_eq!(
        first_audit.3,
        Some(serde_json::json!({ "is_active": false }))
    );

    let repeated_deactivation = role_service::set_role_active(&pool, role_id, false, actor_user_id)
        .await
        .expect("repeated deactivation should be idempotent");
    assert_eq!(repeated_deactivation, StatusTransitionOutcome::Unchanged);
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_logs WHERE entity_type = 'role' AND entity_id = $1",
    )
    .bind(role_id)
    .fetch_one(&pool)
    .await
    .expect("audit count should load");
    assert_eq!(audit_count, 1);

    let admin_id: Uuid = sqlx::query_scalar("SELECT id FROM roles WHERE code = 'ADMIN'")
        .fetch_one(&pool)
        .await
        .expect("ADMIN role should exist");
    let protected_result =
        role_service::set_role_active(&pool, admin_id, false, actor_user_id).await;
    assert!(matches!(protected_result, Err(AppError::Conflict(_))));
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT is_active FROM roles WHERE id = $1")
            .bind(admin_id)
            .fetch_one(&pool)
            .await
            .expect("ADMIN role status should load")
    );

    let reactivation = role_service::set_role_active(&pool, role_id, true, actor_user_id)
        .await
        .expect("custom role should reactivate");
    assert_eq!(
        reactivation,
        StatusTransitionOutcome::Changed { is_active: true }
    );
    let audits: Vec<(String, Option<serde_json::Value>, Option<serde_json::Value>)> =
        sqlx::query_as(
            "SELECT action, old_values, new_values
             FROM audit_logs
             WHERE entity_type = 'role' AND entity_id = $1
             ORDER BY created_at, id",
        )
        .bind(role_id)
        .fetch_all(&pool)
        .await
        .expect("role audits should load");
    assert_eq!(audits.len(), 2);
    assert_eq!(audits[1].0, "reactivate");
    assert_eq!(audits[1].1, Some(serde_json::json!({ "is_active": false })));
    assert_eq!(audits[1].2, Some(serde_json::json!({ "is_active": true })));
}

#[tokio::test]
async fn organization_unit_soft_deactivation_enforces_hierarchy_and_audit_rules() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;

    let fixture = Uuid::new_v4().simple().to_string();
    let actor_user_id = create_test_user(
        &pool,
        &format!("unit-status-actor-{}@example.test", &fixture[..8]),
        "Test1234!",
    )
    .await
    .expect("actor test user should be created");
    let parent_id: Uuid = sqlx::query_scalar(
        "INSERT INTO organization_units (code, name, category, unit_type)
         VALUES ($1, $2, 'other', 'unit')
         RETURNING id",
    )
    .bind(format!("TPARENT{}", &fixture[..8]))
    .bind(format!("test parent {}", &fixture[..8]))
    .fetch_one(&pool)
    .await
    .expect("test parent should be created");
    let child_id: Uuid = sqlx::query_scalar(
        "INSERT INTO organization_units (code, name, parent_unit_id, category, unit_type)
         VALUES ($1, $2, $3, 'other', 'unit')
         RETURNING id",
    )
    .bind(format!("TCHILD{}", &fixture[..8]))
    .bind(format!("test child {}", &fixture[..8]))
    .bind(parent_id)
    .fetch_one(&pool)
    .await
    .expect("test child should be created");

    let active_child_result = organization_unit_service::set_organization_unit_active(
        &pool,
        parent_id,
        false,
        actor_user_id,
    )
    .await;
    assert!(matches!(active_child_result, Err(AppError::Conflict(_))));

    let child_deactivation = organization_unit_service::set_organization_unit_active(
        &pool,
        child_id,
        false,
        actor_user_id,
    )
    .await
    .expect("leaf unit should deactivate");
    assert_eq!(
        child_deactivation,
        StatusTransitionOutcome::Changed { is_active: false }
    );
    let repeated_child_deactivation = organization_unit_service::set_organization_unit_active(
        &pool,
        child_id,
        false,
        actor_user_id,
    )
    .await
    .expect("repeated unit deactivation should be idempotent");
    assert_eq!(
        repeated_child_deactivation,
        StatusTransitionOutcome::Unchanged
    );

    let parent_deactivation = organization_unit_service::set_organization_unit_active(
        &pool,
        parent_id,
        false,
        actor_user_id,
    )
    .await
    .expect("parent with only inactive children should deactivate");
    assert_eq!(
        parent_deactivation,
        StatusTransitionOutcome::Changed { is_active: false }
    );

    let child_reactivation = organization_unit_service::set_organization_unit_active(
        &pool,
        child_id,
        true,
        actor_user_id,
    )
    .await;
    assert!(matches!(child_reactivation, Err(AppError::Conflict(_))));

    let create_under_inactive_parent = organization_unit_service::create_organization_unit(
        &pool,
        CreateOrganizationUnitRequest {
            code: format!("TNEW{}", &fixture[..8]),
            name: format!("test new child {}", &fixture[..8]),
            name_en: None,
            description: None,
            parent_unit_id: Some(parent_id),
            phone: None,
            email: None,
            location: None,
            category: None,
            unit_type: None,
            subject_group_id: None,
        },
    )
    .await;
    assert!(matches!(
        create_under_inactive_parent,
        Err(AppError::Conflict(_))
    ));

    let moving_unit_id: Uuid = sqlx::query_scalar(
        "INSERT INTO organization_units (code, name, category, unit_type)
         VALUES ($1, $2, 'other', 'unit')
         RETURNING id",
    )
    .bind(format!("TMOVE{}", &fixture[..8]))
    .bind(format!("test moving unit {}", &fixture[..8]))
    .fetch_one(&pool)
    .await
    .expect("moving unit should be created");
    let move_under_inactive_parent = organization_unit_service::update_organization_unit(
        &pool,
        moving_unit_id,
        UpdateOrganizationUnitRequest {
            name: None,
            name_en: None,
            description: None,
            parent_unit_id: Some(parent_id),
            phone: None,
            email: None,
            location: None,
            is_active: None,
            category: None,
            unit_type: None,
            subject_group_id: None,
        },
        actor_user_id,
    )
    .await;
    assert!(matches!(
        move_under_inactive_parent,
        Err(AppError::Conflict(_))
    ));

    let active_units = organization_unit_service::list_organization_units(&pool, false)
        .await
        .expect("active units should list");
    assert!(active_units.iter().all(|unit| unit.id != parent_id));
    assert!(active_units.iter().all(|unit| unit.id != child_id));
    let all_units = organization_unit_service::list_organization_units(&pool, true)
        .await
        .expect("all units should list");
    assert!(all_units.iter().any(|unit| unit.id == parent_id));
    assert!(all_units.iter().any(|unit| unit.id == child_id));

    let school_id: Uuid =
        sqlx::query_scalar("SELECT id FROM organization_units WHERE code = 'SCHOOL'")
            .fetch_one(&pool)
            .await
            .expect("SCHOOL unit should exist");
    let school_result = organization_unit_service::set_organization_unit_active(
        &pool,
        school_id,
        false,
        actor_user_id,
    )
    .await;
    assert!(matches!(school_result, Err(AppError::Conflict(_))));

    organization_unit_service::set_organization_unit_active(&pool, parent_id, true, actor_user_id)
        .await
        .expect("parent should reactivate");
    organization_unit_service::set_organization_unit_active(&pool, child_id, true, actor_user_id)
        .await
        .expect("child should reactivate after parent");

    let child_audits: Vec<(String, Option<serde_json::Value>, Option<serde_json::Value>)> =
        sqlx::query_as(
            "SELECT action, old_values, new_values
             FROM audit_logs
             WHERE entity_type = 'organization_unit' AND entity_id = $1
             ORDER BY created_at, id",
        )
        .bind(child_id)
        .fetch_all(&pool)
        .await
        .expect("unit audits should load");
    assert_eq!(child_audits.len(), 2);
    assert_eq!(child_audits[0].0, "deactivate");
    assert_eq!(
        child_audits[0].1,
        Some(serde_json::json!({ "is_active": true }))
    );
    assert_eq!(
        child_audits[0].2,
        Some(serde_json::json!({ "is_active": false }))
    );
    assert_eq!(child_audits[1].0, "reactivate");
}
