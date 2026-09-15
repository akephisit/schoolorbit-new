use std::{net::IpAddr, sync::Arc};

use chrono::Utc;
use school_auth::{
    config::SessionConfig,
    events::SessionRevocationEvent,
    session_cache::SessionCache,
    session_crypto::{RawSessionToken, SessionHmacKey},
    session_repository::SessionMaintenanceMode,
    session_service::{authenticate, login, revalidate, LoginCommand, SessionServiceContext},
};
use school_authorization::PermissionCache;
use school_tenancy::TenantContext;
use school_test_db::{create_named_test_pool, create_test_user, run_test_migrations};
use tokio::sync::broadcast;
use uuid::Uuid;

use school_staff::services::staff_service;

#[tokio::test]
async fn soft_deleted_staff_cannot_reuse_a_warm_session_after_root_invalidation() {
    let pool = create_named_test_pool("auth_staff_soft_delete").await;
    run_test_migrations(&pool).await;

    let username = format!("staff-session-{}@example.test", Uuid::new_v4());
    let user_id = create_test_user(&pool, &username, "Test1234!")
        .await
        .expect("active staff fixture should be created");
    let tenant = TenantContext {
        tenant_id: Uuid::new_v4(),
        subdomain: "auth-staff-soft-delete".to_string(),
        pool: pool.clone(),
    };
    let permission_cache = Arc::new(PermissionCache::new());
    let identity_cache = Arc::new(SessionCache::new());
    let (session_events, _receiver) = broadcast::channel::<SessionRevocationEvent>(8);
    let context = SessionServiceContext::new(
        tenant.clone(),
        Arc::clone(&permission_cache),
        Arc::clone(&identity_cache),
        Arc::new(SessionConfig::for_tests(SessionHmacKey::for_tests(
            [41; 32],
        ))),
        session_events,
    );
    let now = Utc::now();
    let token_bytes = [42; 32];
    let token_hash = RawSessionToken::from_bytes(token_bytes).token_hash();
    let login = login(
        &context,
        LoginCommand {
            username: &username,
            password: "Test1234!",
            remember_me: false,
            source: "203.0.113.42".parse::<IpAddr>().unwrap(),
            user_agent: Some("SchoolOrbit integration test"),
            now,
        },
        || Ok(RawSessionToken::from_bytes(token_bytes)),
    )
    .await
    .expect("active staff should log in");

    assert!(authenticate(
        &context,
        token_hash,
        now,
        SessionMaintenanceMode::RotateAndTouch,
        || panic!("fresh session must not rotate"),
    )
    .await
    .expect("warm authentication should succeed")
    .is_some());
    assert!(revalidate(&login.authenticated, now)
        .await
        .expect("warm realtime validation should succeed"));

    staff_service::soft_delete_staff(&pool, user_id)
        .await
        .expect("staff soft-delete should succeed");
    identity_cache.invalidate_identity_user(&tenant.subdomain, user_id);
    permission_cache.invalidate_user(&tenant.subdomain, user_id);

    assert!(authenticate(
        &context,
        token_hash,
        now,
        SessionMaintenanceMode::RotateAndTouch,
        || panic!("inactive staff session must not rotate"),
    )
    .await
    .expect("inactive account should be rejected without a store failure")
    .is_none());
    assert!(!revalidate(&login.authenticated, now)
        .await
        .expect("inactive realtime session should be rejected"));
}
