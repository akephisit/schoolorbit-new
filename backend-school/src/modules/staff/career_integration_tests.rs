use super::handlers::{career, staff};
use crate::AppState;
use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::get,
    Extension, Router,
};
use bytes::Bytes;
use school_auth::{
    config::SessionConfig, runtime::AuthRuntime, session_crypto::SessionHmacKey,
    session_service::AuthenticatedSession,
};
use school_authorization::PermissionCache;
use school_file_platform::{
    malware_scanner::{MalwareScanner, ScanOutcome},
    platform_service::FilePlatform,
    platform_types::DownloadGrant,
    storage_provider::{ObjectMetadata, StorageError, StorageProvider, StoredObject},
};
use school_permissions::registry::codes;
use school_tenancy::{AdminClient, AdminClientConfig, PoolManager, TenantContext};
use school_test_db::{create_named_test_pool_with_max_connections, run_test_migrations};
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;
use url::Url;
use uuid::Uuid;

struct UnexpectedStorageProvider;

#[async_trait]
impl StorageProvider for UnexpectedStorageProvider {
    async fn check_readiness(&self) -> Result<(), StorageError> {
        panic!("career HTTP operation must not check provider readiness")
    }

    async fn put(&self, _object: &StoredObject, _body: Bytes) -> Result<(), StorageError> {
        panic!("career HTTP operation must not store another object")
    }

    async fn get(&self, _object: &StoredObject, _max_bytes: u64) -> Result<Bytes, StorageError> {
        panic!("career HTTP operation must not read object bytes")
    }

    async fn head(&self, _object: &StoredObject) -> Result<Option<ObjectMetadata>, StorageError> {
        panic!("career HTTP operation must not inspect provider metadata")
    }

    async fn delete(&self, _object: &StoredObject) -> Result<(), StorageError> {
        panic!("career HTTP operation must complete deletion without provider work")
    }

    async fn private_download_grant(
        &self,
        _object: &StoredObject,
        _filename: &str,
        _ttl: Duration,
    ) -> Result<DownloadGrant, StorageError> {
        panic!("career HTTP operation must not issue download grants")
    }

    fn public_location(&self, _object: &StoredObject) -> Result<Url, StorageError> {
        panic!("career HTTP operation must not expose public locations")
    }
}

struct UnexpectedScanner;

#[async_trait]
impl MalwareScanner for UnexpectedScanner {
    async fn scan(&self, _content: &[u8]) -> ScanOutcome {
        panic!("career HTTP operation runs after scanning")
    }
}

fn file_platform() -> FilePlatform {
    FilePlatform::new(
        Arc::new(UnexpectedStorageProvider),
        Arc::new(UnexpectedScanner),
    )
}

struct Fixture {
    pool: PgPool,
    state: AppState,
    tenant: TenantContext,
    actor: Uuid,
    same_unit: Uuid,
    child_unit: Uuid,
    outside: Uuid,
    student: Uuid,
}
impl Fixture {
    async fn new(name: &str) -> Self {
        let pool = create_named_test_pool_with_max_connections(name, 5).await;
        run_test_migrations(&pool).await;
        let mut users = Vec::new();
        for kind in ["staff", "staff", "staff", "staff", "student"] {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'synthetic-hash','Fixture','Person',$3,'active')").bind(id).bind(id.to_string()).bind(kind).execute(&pool).await.unwrap();
            users.push(id);
        }
        let root:Uuid=sqlx::query_scalar("INSERT INTO organization_units(code,name) VALUES('career_root','Fixture root') RETURNING id").fetch_one(&pool).await.unwrap();
        let child:Uuid=sqlx::query_scalar("INSERT INTO organization_units(code,name,parent_unit_id) VALUES('career_child','Fixture child',$1) RETURNING id").bind(root).fetch_one(&pool).await.unwrap();
        for (user, unit) in [(users[0], root), (users[1], root), (users[2], child)] {
            sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code,is_primary) VALUES($1,$2,'member',true)").bind(user).bind(unit).execute(&pool).await.unwrap();
        }
        let admin_client = Arc::new(AdminClient::new(
            "http://127.0.0.1:9".into(),
            "test-secret".into(),
            AdminClientConfig::from_env().unwrap(),
        ));
        let pool_manager = Arc::new(PoolManager::new());
        let permission_cache = Arc::new(PermissionCache::new());
        let (session_events, _) = tokio::sync::broadcast::channel(8);
        let (permission_events, _) = tokio::sync::broadcast::channel(8);
        let auth_runtime = AuthRuntime {
            admin_client: admin_client.clone(),
            pool_manager: pool_manager.clone(),
            permission_cache: permission_cache.clone(),
            identity_cache: Arc::new(school_auth::session_cache::SessionCache::new()),
            config: Arc::new(SessionConfig::for_tests(SessionHmacKey::for_tests(
                [71; 32],
            ))),
            session_events,
            permission_events: permission_events.clone(),
        };
        let state = AppState {
            admin_client,
            pool_manager,
            permission_cache,
            auth_runtime,
            websocket_manager: Arc::new(
                crate::modules::academic::websockets::WebSocketManager::new(),
            ),
            notification_channel: tokio::sync::broadcast::channel(8).0,
            permission_event_channel: permission_events,
            work_event_channel: tokio::sync::broadcast::channel(8).0,
            file_platform: Arc::new(file_platform()),
            certificate_verification_limiter: Arc::new(
                school_certificates::verification_limiter::CertificateVerificationLimiter::new(),
            ),
        };
        let tenant = TenantContext {
            tenant_id: Uuid::new_v4(),
            subdomain: name.into(),
            pool: pool.clone(),
        };
        Self {
            pool,
            state,
            tenant,
            actor: users[0],
            same_unit: users[1],
            child_unit: users[2],
            outside: users[3],
            student: users[4],
        }
    }
    fn router(&self, permissions: &[&str]) -> Router {
        self.state
            .permission_cache
            .invalidate_user(&self.tenant.subdomain, self.actor);
        let revision = self
            .state
            .permission_cache
            .snapshot_revision(&self.tenant.subdomain, self.actor);
        assert!(self.state.permission_cache.fill_if_current(
            &self.tenant.subdomain,
            self.actor,
            revision,
            permissions.iter().map(|code| code.to_string()).collect()
        ));
        let session = AuthenticatedSession::for_tests(
            self.state.auth_runtime.identity_cache.clone(),
            self.tenant.clone(),
            Uuid::new_v4(),
            self.actor,
            "staff",
        );
        Router::new()
            .route(
                "/api/staff/personnel-rank-milestones",
                get(super::handlers::personnel::get_rank_milestone_overview),
            )
            .route(
                "/api/staff/{id}/career-history",
                get(career::list_career_history).post(career::append_career_history),
            )
            .route(
                "/api/staff/{id}/career-history/{entryId}",
                axum::routing::patch(career::correct_career_history),
            )
            .route(
                "/api/staff/{id}/public",
                get(staff::get_public_staff_profile),
            )
            .layer(Extension(session))
            .with_state(self.state.clone())
    }
    async fn request(
        &self,
        permissions: &[&str],
        method: &str,
        path: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        let response = self
            .router(permissions)
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 100000).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }
}
#[tokio::test]
async fn career_reads_preserve_own_unit_tree_school_and_union_scope() {
    let f = Fixture::new("career_http_read_scope").await;
    for (permissions, target, expected) in [
        (vec![codes::STAFF_PROFILE_READ_OWN], f.actor, StatusCode::OK),
        (
            vec![codes::STAFF_PROFILE_READ_OWN],
            f.same_unit,
            StatusCode::FORBIDDEN,
        ),
        (
            vec![codes::STAFF_PROFILE_READ_ORGANIZATION_UNIT],
            f.same_unit,
            StatusCode::OK,
        ),
        (
            vec![codes::STAFF_PROFILE_READ_ORGANIZATION_UNIT],
            f.child_unit,
            StatusCode::FORBIDDEN,
        ),
        (
            vec![codes::STAFF_PROFILE_READ_ORGANIZATION_TREE],
            f.child_unit,
            StatusCode::OK,
        ),
        (
            vec![codes::STAFF_PROFILE_READ_ORGANIZATION_TREE],
            f.outside,
            StatusCode::FORBIDDEN,
        ),
        (
            vec![
                codes::STAFF_PROFILE_READ_OWN,
                codes::STAFF_PROFILE_READ_ORGANIZATION_TREE,
            ],
            f.actor,
            StatusCode::OK,
        ),
        (
            vec![
                codes::STAFF_PROFILE_READ_OWN,
                codes::STAFF_PROFILE_READ_ORGANIZATION_TREE,
            ],
            f.child_unit,
            StatusCode::OK,
        ),
        (
            vec![codes::STAFF_PROFILE_READ_SCHOOL],
            f.outside,
            StatusCode::OK,
        ),
        (vec![], f.actor, StatusCode::FORBIDDEN),
        (
            vec![codes::STAFF_UPDATE_ALL],
            f.actor,
            StatusCode::FORBIDDEN,
        ),
        (
            vec![codes::STAFF_PROFILE_READ_SCHOOL],
            Uuid::new_v4(),
            StatusCode::NOT_FOUND,
        ),
        (
            vec![codes::STAFF_PROFILE_READ_SCHOOL],
            f.student,
            StatusCode::NOT_FOUND,
        ),
    ] {
        let (status, body) = f
            .request(
                &permissions,
                "GET",
                &format!("/api/staff/{target}/career-history"),
                serde_json::json!({}),
            )
            .await;
        assert_eq!(status, expected, "{permissions:?} / {target}");
        assert_eq!(body["success"], expected == StatusCode::OK);
    }
    let (status, _) = f
        .request(
            &[codes::STAFF_PROFILE_READ_SCHOOL],
            "GET",
            &format!(
                "/api/staff/{}/career-history?cursor={}",
                f.actor,
                Uuid::new_v4()
            ),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
#[tokio::test]
async fn career_writer_acknowledges_without_granting_profile_reads() {
    let f = Fixture::new("career_http_write_scope").await;
    let id = Uuid::new_v4();
    let payload = serde_json::json!({"id":id,"entry":{"fact":{"kind":"academic_rank","value":"none"},"effectiveDate":"2020-01-01"}});
    let path = format!("/api/staff/{}/career-history", f.outside);
    let (status, _) = f
        .request(
            &[codes::STAFF_PROFILE_READ_SCHOOL],
            "POST",
            &path,
            payload.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = f
        .request(&[codes::STAFF_UPDATE_ALL], "POST", &path, payload.clone())
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"], serde_json::json!({"id":id,"revision":1}));
    let (status, retry) = f
        .request(&[codes::STAFF_UPDATE_ALL], "POST", &path, payload.clone())
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, retry);
    let (status, _) = f
        .request(
            &[codes::STAFF_UPDATE_ALL],
            "GET",
            &path,
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let foreign_path = format!("/api/staff/{}/career-history", f.same_unit);
    let (status, error) = f
        .request(
            &[codes::STAFF_UPDATE_ALL],
            "POST",
            &foreign_path,
            payload.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(error.get("data").is_none());
    let correction = serde_json::json!({"expectedRevision":1,"expectedIsCurrent":false,"entry":payload["entry"],"reason":"ตรวจคำสั่งต้นฉบับ"});
    let (status, _) = f
        .request(
            &[codes::STAFF_PROFILE_READ_SCHOOL],
            "PATCH",
            &format!("{path}/{id}"),
            correction.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = f
        .request(
            &[codes::STAFF_UPDATE_ALL],
            "PATCH",
            &format!("{path}/{id}"),
            correction,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = f
        .request(
            &[codes::STAFF_UPDATE_ALL],
            "POST",
            &format!("/api/staff/{}/career-history", f.student),
            serde_json::json!({"id":Uuid::new_v4(),"entry":payload["entry"]}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, public) = f
        .request(
            &[],
            "GET",
            &format!("/api/staff/{}/public", f.outside),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let serialized = public.to_string();
    for field in [
        "current_career",
        "effectiveDate",
        "orderDate",
        "orderNumber",
        "careerHistory",
    ] {
        assert!(!serialized.contains(field));
    }
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM staff_career_history WHERE user_id=$1")
            .bind(f.outside)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn career_routes_require_authentication_through_production_router() {
    let mut fixture = Fixture::new("career-http-auth").await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let tenant_id = fixture.tenant.tenant_id;
    let directory = Router::new().route(
        "/internal/schools/{subdomain}",
        get(move || async move {
            axum::Json(
                serde_json::json!({"id":tenant_id,"db_connection_string":"career-auth-test-pool"}),
            )
        }),
    );
    let server = tokio::spawn(async move {
        axum::serve(listener, directory).await.unwrap();
    });
    fixture.state.auth_runtime.admin_client = Arc::new(AdminClient::new(
        format!("http://{address}"),
        "test-secret".into(),
        AdminClientConfig::for_tests(Duration::from_secs(1), 1, Duration::from_millis(1)),
    ));
    fixture
        .state
        .auth_runtime
        .pool_manager
        .insert_test_pool("career-auth-test-pool", fixture.pool.clone())
        .await;
    for (method, path) in [
        ("GET", "/api/staff/personnel-rank-milestones".into()),
        (
            "GET",
            format!("/api/staff/{}/career-history", fixture.actor),
        ),
        (
            "POST",
            format!("/api/staff/{}/career-history", fixture.actor),
        ),
        (
            "PATCH",
            format!(
                "/api/staff/{}/career-history/{}",
                fixture.actor,
                Uuid::new_v4()
            ),
        ),
    ] {
        let response = crate::app::build_app(fixture.state.clone())
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header(
                        "origin",
                        format!("https://{}.schoolorbit.test", fixture.tenant.subdomain),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let bytes = to_bytes(response.into_body(), 10000).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["success"], false);
    }
    server.abort();
}

#[tokio::test]
async fn rank_milestone_overview_preserves_profile_scopes_status_and_missing_info() {
    let f = Fixture::new("rank_milestone_scopes").await;
    for (permission, count) in [
        (codes::STAFF_PROFILE_READ_OWN, 1),
        (codes::STAFF_PROFILE_READ_ORGANIZATION_UNIT, 2),
        (codes::STAFF_PROFILE_READ_ORGANIZATION_TREE, 3),
        (codes::STAFF_PROFILE_READ_SCHOOL, 4),
    ] {
        let (status, body) = f
            .request(
                &[permission],
                "GET",
                "/api/staff/personnel-rank-milestones?bucket=incomplete",
                serde_json::json!({}),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["data"]["filteredTotal"], count);
        assert_eq!(body["data"]["counts"]["incomplete"], count);
        assert_eq!(
            body["data"]["items"].as_array().unwrap().len(),
            count as usize
        );
        assert!(body["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["milestone"]["ordinaryDate"].is_null()));
    }
    let (status, _) = f
        .request(
            &[],
            "GET",
            "/api/staff/personnel-rank-milestones",
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(f.outside)
        .execute(&f.pool)
        .await
        .unwrap();
    let (_, body) = f
        .request(
            &[codes::STAFF_PROFILE_READ_SCHOOL],
            "GET",
            "/api/staff/personnel-rank-milestones?bucket=incomplete",
            serde_json::json!({}),
        )
        .await;
    assert_eq!(body["data"]["total"], 3);
    let (_, body) = f
        .request(
            &[codes::STAFF_PROFILE_READ_SCHOOL],
            "GET",
            "/api/staff/personnel-rank-milestones?status=all&bucket=incomplete",
            serde_json::json!({}),
        )
        .await;
    assert_eq!(body["data"]["total"], 4);
    for query in ["page=0", "page=10001", "bucket=eligible", "status=invalid"] {
        let (status, _) = f
            .request(
                &[codes::STAFF_PROFILE_READ_SCHOOL],
                "GET",
                &format!("/api/staff/personnel-rank-milestones?{query}"),
                serde_json::json!({}),
            )
            .await;
        assert!(status.is_client_error());
    }
}
