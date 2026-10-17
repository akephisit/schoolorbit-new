use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Extension, Router,
};
use school_auth::{
    config::SessionConfig, runtime::AuthRuntime, session_crypto::SessionHmacKey,
    session_service::AuthenticatedSession,
};
use school_authorization::PermissionCache;
use school_permissions::registry::codes;
use school_students::http::{routes, state::StudentHttpState};
use school_tenancy::{AdminClient, AdminClientConfig, PoolManager, TenantContext};
use school_test_db::{create_named_test_pool, run_test_migrations};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

struct Fixture {
    pool: PgPool,
    state: StudentHttpState,
    tenant: TenantContext,
    actor: Uuid,
    student: Uuid,
}

impl Fixture {
    async fn new(name: &str) -> Self {
        let pool = create_named_test_pool(name).await;
        run_test_migrations(&pool).await;
        let actor = Uuid::new_v4();
        let student = Uuid::new_v4();
        for (id, kind) in [(actor, "staff"), (student, "student")] {
            sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'not-a-login','Synthetic','Register',$3,'active')")
                .bind(id).bind(id.to_string()).bind(kind).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO student_info(user_id,student_id) VALUES($1,'HTTP-REGISTRY')")
            .bind(student)
            .execute(&pool)
            .await
            .unwrap();
        let runtime = AuthRuntime {
            admin_client: Arc::new(AdminClient::new(
                "http://127.0.0.1:9".into(),
                "test-secret".into(),
                AdminClientConfig::from_env().unwrap(),
            )),
            pool_manager: Arc::new(PoolManager::new()),
            permission_cache: Arc::new(PermissionCache::new()),
            identity_cache: Arc::new(school_auth::session_cache::SessionCache::new()),
            config: Arc::new(SessionConfig::for_tests(SessionHmacKey::for_tests(
                [73; 32],
            ))),
            session_events: tokio::sync::broadcast::channel(8).0,
            permission_events: tokio::sync::broadcast::channel(8).0,
        };
        Self {
            tenant: TenantContext {
                tenant_id: Uuid::new_v4(),
                subdomain: name.into(),
                pool: pool.clone().into(),
            },
            pool,
            state: StudentHttpState {
                auth_runtime: runtime,
                placement_rosters: &crate::http_state::PLACEMENT_ROSTERS,
            },
            actor,
            student,
        }
    }

    fn router(&self, permissions: &[&str]) -> Router {
        let cache = &self.state.auth_runtime.permission_cache;
        cache.invalidate_user(&self.tenant.subdomain, self.actor);
        let revision = cache.snapshot_revision(&self.tenant.subdomain, self.actor);
        assert!(cache.fill_if_current(
            &self.tenant.subdomain,
            self.actor,
            revision,
            permissions
                .iter()
                .map(|permission| permission.to_string())
                .collect()
        ));
        let session = AuthenticatedSession::for_tests(
            self.state.auth_runtime.identity_cache.clone(),
            self.tenant.clone(),
            Uuid::new_v4(),
            self.actor,
            "staff",
        );
        Router::new()
            .nest("/api", routes::protected_routes())
            .layer(Extension(session))
            .with_state(self.state.clone())
    }

    async fn request(
        &self,
        permissions: &[&str],
        method: &str,
        path: &str,
        body: &str,
    ) -> (StatusCode, Value) {
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
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/json"
        );
        let body = to_bytes(response.into_body(), 100_000).await.unwrap();
        (
            status,
            serde_json::from_slice(&body).expect("student response must be JSON"),
        )
    }

    async fn snapshot(&self) -> String {
        sqlx::query_scalar("SELECT jsonb_build_object('student',(SELECT to_jsonb(u) FROM users u WHERE id=$1),'info',(SELECT to_jsonb(s) FROM student_info s WHERE user_id=$1),'users',(SELECT count(*) FROM users),'registrations',(SELECT count(*) FROM student_info),'parents',(SELECT count(*) FROM student_parents),'years',(SELECT count(*) FROM student_academic_years),'placements',(SELECT count(*) FROM homeroom_placements),'memberships',(SELECT count(*) FROM learning_group_students))::text")
            .bind(self.student).fetch_one(&self.pool).await.unwrap()
    }
}

#[tokio::test]
async fn student_register_transport_rejections_are_json_400_and_never_mutate() {
    let f = Fixture::new("student_register_http_rejections").await;
    let permissions = &[
        codes::STUDENT_READ_SCHOOL,
        codes::STUDENT_CREATE_ALL,
        codes::STUDENT_UPDATE_ALL,
    ];
    let profile = format!("/api/students/{}", f.student);
    let parents = format!("{profile}/parents");
    let before = f.snapshot().await;
    let valid_create = json!({"student_id":"NEW-CODE","password":"Valid-password","first_name":"New","last_name":"Student"});
    let mut invalid_enum = valid_create.clone();
    invalid_enum["registration_status"] = json!("not-a-status");
    let mut wrong_type = valid_create.clone();
    wrong_type["first_name"] = json!(42);
    let mut unknown_guardian = valid_create.clone();
    unknown_guardian["parents"] = json!([{
        "first_name":"Guardian", "last_name":"Fixture", "phone":"0812345678",
        "relationship":"guardian", "unexpected":true
    }]);
    for (method, path, body) in [
        ("GET", "/api/students?unexpected=1".to_string(), "".to_string()),
        ("GET", "/api/students?academicYearId=old".into(), "".into()),
        ("GET", "/api/students?registrationStatus=invalid".into(), "".into()),
        ("GET", "/api/students?page=invalid".into(), "".into()),
        ("GET", format!("{profile}?academicYearId=invalid"), "".into()),
        ("GET", format!("{profile}?unexpected=1"), "".into()),
        ("POST", "/api/students".into(), "{".into()),
        ("POST", "/api/students".into(), invalid_enum.to_string()),
        ("POST", "/api/students".into(), wrong_type.to_string()),
        ("POST", "/api/students".into(), unknown_guardian.to_string()),
        ("PUT", profile.clone(), "{".into()),
        ("PUT", profile.clone(), json!({"phone":[]}).to_string()),
        ("PUT", profile.clone(), json!({"first_name":"must-not-write","student_id":"changed"}).to_string()),
        ("PUT", profile.clone(), json!({"student_number":4}).to_string()),
        ("PUT", profile.clone(), json!({"registration_status":"invented"}).to_string()),
        ("PUT", profile.clone(), json!({"registration_status_updated_by":f.actor}).to_string()),
        ("POST", parents.clone(), "{".into()),
        ("POST", parents.clone(), json!({"phone":[]}).to_string()),
        ("POST", parents, json!({"first_name":"Guardian","last_name":"Fixture","phone":"0812345678","relationship":"guardian","unexpected":true}).to_string()),
    ] {
        let (status, data) = f.request(permissions, method, &path, &body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{method} {path}");
        assert_eq!(data["success"], false);
        assert!(data["error"].as_str().is_some_and(|value| !value.is_empty()));
        assert!(!data.to_string().contains("must-not-write"));
    }
    assert_eq!(before, f.snapshot().await);
    let (status, listed) = f.request(permissions, "GET", "/api/students", "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["success"], true);
}

#[tokio::test]
async fn student_register_transport_checks_exact_permissions_and_resource_scope_before_decoding() {
    let f = Fixture::new("student_register_http_permission_first").await;
    let profile = format!("/api/students/{}", f.student);
    let before = f.snapshot().await;
    for (method, path, body) in [
        ("GET", "/api/students?registrationStatus=invalid".into(), ""),
        ("GET", format!("{profile}?academicYearId=invalid"), ""),
        ("POST", "/api/students".into(), "{"),
        ("PUT", profile.clone(), "{"),
        ("POST", format!("{profile}/parents"), "{"),
    ] {
        let (status, data) = f.request(&[], method, &path, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}");
        assert_eq!(data["success"], false);
    }
    let (status, data) = f
        .request(
            &[codes::STUDENT_READ_OWN],
            "GET",
            &format!("{profile}?academicYearId=invalid"),
            "",
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(data["success"], false);
    assert_eq!(before, f.snapshot().await);
}
