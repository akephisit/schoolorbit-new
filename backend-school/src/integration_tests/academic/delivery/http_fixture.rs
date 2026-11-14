use sqlx::PgPool;
use uuid::Uuid;

pub(crate) fn academic_router<S: Clone + Send + Sync + 'static>(
    pool: &PgPool,
    actor: Uuid,
    permissions: &[&str],
    routes: axum::Router<S>,
    select_state: impl FnOnce(
        school_auth::runtime::AuthRuntime,
        std::sync::Arc<school_academic_timetable_http::websockets::WebSocketManager>,
    ) -> S,
) -> axum::Router {
    use school_auth::{
        config::SessionConfig, runtime::AuthRuntime, session_crypto::SessionHmacKey,
        session_service::AuthenticatedSession,
    };
    use school_authorization::PermissionCache;
    use school_tenancy::{AdminClient, AdminClientConfig, PoolManager, TenantContext};
    use std::sync::Arc;
    let permission_cache = Arc::new(PermissionCache::new());
    let revision = permission_cache.snapshot_revision("delivery-removal-http", actor);
    assert!(permission_cache.fill_if_current(
        "delivery-removal-http",
        actor,
        revision,
        permissions.iter().map(|p| p.to_string()).collect()
    ));
    let runtime = AuthRuntime {
        admin_client: Arc::new(AdminClient::new(
            "http://127.0.0.1:9".into(),
            "test-secret".into(),
            AdminClientConfig::from_env().unwrap(),
        )),
        pool_manager: Arc::new(PoolManager::new(school_test_db::verify_tenant_schema)),
        permission_cache,
        identity_cache: Arc::new(school_auth::session_cache::SessionCache::new()),
        config: Arc::new(SessionConfig::for_tests(SessionHmacKey::for_tests(
            [81; 32],
        ))),
        session_events: tokio::sync::broadcast::channel(8).0,
        permission_events: tokio::sync::broadcast::channel(8).0,
    };
    let session = AuthenticatedSession::for_tests(
        runtime.identity_cache.clone(),
        TenantContext {
            tenant_id: Uuid::new_v4(),
            subdomain: "delivery-removal-http".into(),
            pool: pool.clone().into(),
        },
        Uuid::new_v4(),
        actor,
        "staff",
    );
    let websocket_manager =
        Arc::new(school_academic_timetable_http::websockets::WebSocketManager::new());
    let state = select_state(runtime, websocket_manager);
    axum::Router::new()
        .nest("/api/academic", routes)
        .layer(axum::Extension(session))
        .with_state(state)
}
