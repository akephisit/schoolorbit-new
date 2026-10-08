use super::services::public::{get_organization, get_statistics};
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, seed_release_two_predecessor,
};
use crate::{app::build_app, AppState};
use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    extract::{Path, State},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use bytes::Bytes;
use school_auth::{config::SessionConfig, runtime::AuthRuntime, session_crypto::SessionHmacKey};
use school_authorization::PermissionCache;
use school_file_platform::{
    malware_scanner::{MalwareScanner, ScanOutcome},
    platform_service::FilePlatform,
    platform_types::DownloadGrant,
    storage_provider::{ObjectMetadata, StorageError, StorageProvider, StoredObject},
};
use school_tenancy::{AdminClient, AdminClientConfig, PoolManager};
use school_test_db::{create_named_test_pool, run_test_migrations};
use sqlx::PgPool;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tower::ServiceExt;
use url::Url;
use uuid::Uuid;

async fn user(pool: &PgPool, label: &str, kind: &str, gender: Option<&str>, status: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO users(username,password_hash,first_name,last_name,user_type,gender,status) VALUES($1,'synthetic-hash',$1,'ทดสอบ',$2,$3,$4) RETURNING id")
        .bind(label).bind(kind).bind(gender).bind(status).fetch_one(pool).await.unwrap()
}

async fn statistics_fixture() -> PgPool {
    let pool = create_named_test_pool("public_school_statistics").await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 84).await.unwrap();
    sqlx::query("UPDATE student_academic_years SET status='withdrawn'")
        .execute(&pool)
        .await
        .unwrap();
    let (year, grade, program, room): (Uuid,Uuid,Uuid,Uuid) = sqlx::query_as("SELECT academic_year_id,grade_level_id,study_program_id,id FROM homerooms WHERE academic_year_id=(SELECT id FROM academic_years WHERE status='active') ORDER BY name LIMIT 1").fetch_one(&pool).await.unwrap();
    let current_room: Uuid = sqlx::query_scalar(
        "SELECT id FROM homerooms WHERE academic_year_id=$1 AND id<>$2 ORDER BY name LIMIT 1",
    )
    .bind(year)
    .bind(room)
    .fetch_one(&pool)
    .await
    .unwrap();
    for (index, gender) in [Some("male"), Some("female"), Some("other"), None]
        .into_iter()
        .enumerate()
    {
        let id = user(
            &pool,
            &format!("public-student-{index}"),
            "student",
            gender,
            if index == 0 { "suspended" } else { "active" },
        )
        .await;
        let student_year = Uuid::new_v4();
        sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) VALUES($1,$2,$3,$4,$5,'active')").bind(student_year).bind(id).bind(year).bind(grade).bind(program).execute(&pool).await.unwrap();
        if index < 2 {
            // Ended placement remains as history and must never count twice.
            sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,end_date,status,enrollment_type) VALUES($1,$2,$3,$4,CURRENT_DATE-10,CURRENT_DATE-1,'ended','normal'),($5,$2,$3,$6,CURRENT_DATE,NULL,'current','normal')")
                .bind(Uuid::new_v4()).bind(student_year).bind(year).bind(room).bind(Uuid::new_v4()).bind(current_room).execute(&pool).await.unwrap();
        }
    }
    sqlx::query("UPDATE users SET status='inactive' WHERE user_type='staff'")
        .execute(&pool)
        .await
        .unwrap();
    for (index, code) in [
        "teacher",
        "assistant_teacher",
        "contract_teacher",
        "government_employee_teacher",
        "school_director",
        "support_staff",
        "teacher",
    ]
    .into_iter()
    .enumerate()
    {
        let id = user(
            &pool,
            &format!("public-staff-{index}"),
            "staff",
            None,
            if index == 6 { "resigned" } else { "active" },
        )
        .await;
        sqlx::query("INSERT INTO staff_info(user_id,job_position_id,academic_rank) SELECT $1,id,CASE WHEN code IN ('school_director','support_staff') THEN 'not_applicable' ELSE 'none' END FROM staff_job_positions WHERE code=$2").bind(id).bind(code).execute(&pool).await.unwrap();
    }
    run_test_migrations(&pool).await;
    pool
}

#[tokio::test]
async fn public_statistics_use_active_year_and_current_placements_with_complete_gender_totals() {
    let pool = statistics_fixture().await;
    let stats = get_statistics(&pool).await.unwrap();
    assert_eq!(stats.academic_year.as_ref().unwrap().year, 2025);
    assert_eq!(stats.students.total, 4);
    assert_eq!(
        (
            stats.students.male,
            stats.students.female,
            stats.students.other_or_unspecified
        ),
        (1, 1, 2)
    );
    assert_eq!(stats.unassigned_students.total, 2);
    assert_eq!(stats.total_homerooms, 2);
    assert_eq!((stats.total_teachers, stats.total_staff), (4, 6));
    assert_eq!(
        stats.grades.iter().map(|g| g.students.total).sum::<i64>(),
        4
    );
    assert_eq!(
        stats
            .grades
            .iter()
            .flat_map(|g| g.homerooms.iter())
            .map(|h| h.students.total)
            .sum::<i64>(),
        2
    );
    assert!(stats
        .grades
        .iter()
        .flat_map(|g| g.homerooms.iter())
        .any(|h| h.students.total == 0));
    let json = serde_json::to_string(&stats).unwrap();
    for forbidden in [
        "studentId",
        "userId",
        "nationalId",
        "password",
        "public-student",
    ] {
        assert!(!json.contains(forbidden));
    }
    sqlx::query("UPDATE academic_years SET status='closed' WHERE status='active'")
        .execute(&pool)
        .await
        .unwrap();
    let empty = get_statistics(&pool).await.unwrap();
    assert!(empty.academic_year.is_none());
    assert!(empty.grades.is_empty());
    assert_eq!((empty.students.total, empty.total_homerooms), (0, 0));
    assert_eq!(empty.total_teachers, 4);
}

#[tokio::test]
async fn public_organization_only_exposes_active_units_and_current_members() {
    let pool = create_named_test_pool("public_school_organization").await;
    run_test_migrations(&pool).await;
    let root = Uuid::new_v4();
    let child = Uuid::new_v4();
    let inactive = Uuid::new_v4();
    sqlx::query("INSERT INTO organization_units(id,code,name,unit_type,parent_unit_id,is_active) VALUES($1,'PUBLICROOT','โรงเรียนทดสอบ','school',NULL,true),($2,'PUBLICCHILD','ฝ่ายทดสอบ','division',$1,true),($3,'PUBLICINACTIVE','หน่วยงานปิด','division',$1,false)").bind(root).bind(child).bind(inactive).execute(&pool).await.unwrap();
    for (index, (position, start, end, status)) in [
        ("director", -1, None, "active"),
        ("deputy_director", -1, None, "active"),
        ("head", -1, None, "active"),
        ("deputy_head", -1, None, "active"),
        ("member", -1, None, "active"),
        ("coordinator", -1, None, "active"),
        ("head", 1, None, "active"),
        ("head", -2, Some(0), "active"),
        ("head", -1, None, "resigned"),
    ]
    .into_iter()
    .enumerate()
    {
        let id = user(&pool, &format!("leader-{index}"), "staff", None, status).await;
        sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code,started_at,ended_at) VALUES($1,$2,$3,CURRENT_DATE+$4,CASE WHEN $5::int IS NULL THEN NULL ELSE CURRENT_DATE+$5 END)").bind(id).bind(child).bind(position).bind(start).bind(end).execute(&pool).await.unwrap();
    }
    let org = get_organization(&pool).await.unwrap();
    assert!(!org.units.iter().any(|u| u.id == inactive));
    assert!(org
        .units
        .iter()
        .find(|u| u.id == root)
        .unwrap()
        .members
        .is_empty());
    let unit = org.units.iter().find(|u| u.id == child).unwrap();
    assert_eq!(unit.parent_id, Some(root));
    assert_eq!(unit.members.len(), 6);
    assert_eq!(
        unit.members
            .iter()
            .map(|m| m.position_code.as_str())
            .collect::<Vec<_>>(),
        vec![
            "director",
            "deputy_director",
            "head",
            "deputy_head",
            "coordinator",
            "member"
        ]
    );
    let data = serde_json::to_string(&org).unwrap();
    for forbidden in [
        "userId",
        "email",
        "nationalId",
        "leader-6",
        "leader-7",
        "leader-8",
    ] {
        assert!(!data.contains(forbidden));
    }
}
struct UnexpectedStorageProvider;

#[async_trait]
impl StorageProvider for UnexpectedStorageProvider {
    async fn check_readiness(&self) -> Result<(), StorageError> {
        panic!("public school HTTP operation must not check provider readiness")
    }

    async fn put(&self, _object: &StoredObject, _body: Bytes) -> Result<(), StorageError> {
        panic!("public school HTTP operation must not store another object")
    }

    async fn get(&self, _object: &StoredObject, _max_bytes: u64) -> Result<Bytes, StorageError> {
        panic!("public school HTTP operation must not read object bytes")
    }

    async fn head(&self, _object: &StoredObject) -> Result<Option<ObjectMetadata>, StorageError> {
        panic!("public school HTTP operation must not inspect provider metadata")
    }

    async fn delete(&self, _object: &StoredObject) -> Result<(), StorageError> {
        panic!("public school HTTP operation must complete deletion without provider work")
    }

    async fn private_download_grant(
        &self,
        _object: &StoredObject,
        _filename: &str,
        _ttl: Duration,
    ) -> Result<DownloadGrant, StorageError> {
        panic!("public school HTTP operation must not issue download grants")
    }

    fn public_location(&self, _object: &StoredObject) -> Result<Url, StorageError> {
        panic!("public school HTTP operation must not expose public locations")
    }
}

struct UnexpectedScanner;

#[async_trait]
impl MalwareScanner for UnexpectedScanner {
    async fn scan(&self, _content: &[u8]) -> ScanOutcome {
        panic!("public school HTTP operation runs after scanning")
    }
}

fn file_platform() -> FilePlatform {
    FilePlatform::new(
        Arc::new(UnexpectedStorageProvider),
        Arc::new(UnexpectedScanner),
    )
}

#[derive(Clone)]
struct Directory(Arc<HashMap<String, (Uuid, String)>>);
async fn directory(State(state): State<Directory>, Path(subdomain): Path<String>) -> Response {
    match state.0.get(&subdomain) {
        Some((id, url)) => {
            Json(serde_json::json!({"id":id,"name":subdomain,"db_connection_string":url}))
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[tokio::test]
async fn public_http_reads_need_no_session_and_keep_tenants_and_private_routes_separate() {
    let a = create_named_test_pool("public_http_school_a").await;
    let b = create_named_test_pool("public_http_school_b").await;
    run_test_migrations(&a).await;
    run_test_migrations(&b).await;
    let actor_id = user(&a, "public-http-teacher", "staff", None, "active").await;
    let pool_manager = Arc::new(PoolManager::new());
    pool_manager
        .insert_test_pool("test-pool://public-a", a.clone())
        .await;
    pool_manager
        .insert_test_pool("test-pool://public-b", b.clone())
        .await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let dir = Directory(Arc::new(HashMap::from([
        (
            "school-a".into(),
            (Uuid::new_v4(), "test-pool://public-a".into()),
        ),
        (
            "school-b".into(),
            (Uuid::new_v4(), "test-pool://public-b".into()),
        ),
    ])));
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route("/internal/schools/{subdomain}", get(directory))
                .with_state(dir),
        )
        .await
        .unwrap();
    });
    let admin_client = Arc::new(AdminClient::new(
        format!("http://{address}"),
        "test-secret".into(),
        AdminClientConfig::for_tests(Duration::from_secs(1), 1, Duration::from_millis(1)),
    ));
    let permission_cache = Arc::new(PermissionCache::new());
    let permission_events = tokio::sync::broadcast::channel(8).0;
    let runtime = AuthRuntime {
        admin_client: admin_client.clone(),
        pool_manager: pool_manager.clone(),
        permission_cache: permission_cache.clone(),
        identity_cache: Arc::new(school_auth::session_cache::SessionCache::new()),
        config: Arc::new({
            let mut config = SessionConfig::for_tests(SessionHmacKey::for_tests([71; 32]));
            config.base_domain = "schoolorbit.app".into();
            config
        }),
        session_events: tokio::sync::broadcast::channel(8).0,
        permission_events: permission_events.clone(),
    };
    let state = AppState {
        admin_client,
        pool_manager,
        permission_cache,
        auth_runtime: runtime,
        websocket_manager: Arc::new(crate::modules::academic::websockets::WebSocketManager::new()),
        notification_channel: tokio::sync::broadcast::channel(8).0,
        permission_event_channel: permission_events,
        work_event_channel: tokio::sync::broadcast::channel(8).0,
        file_platform: Arc::new(file_platform()),
        certificate_verification_limiter: Arc::new(
            school_certificates::verification_limiter::CertificateVerificationLimiter::new(),
        ),
    };
    use axum::extract::FromRef;
    let academic = school_academic_http::state::AcademicHttpState::from_ref(&state);
    let certificates = school_certificates_http::state::CertificateHttpState::from_ref(&state);
    assert!(Arc::ptr_eq(
        &academic.auth_runtime.identity_cache,
        &state.auth_runtime.identity_cache
    ));
    assert!(Arc::ptr_eq(
        &academic.auth_runtime.permission_cache,
        &state.permission_cache
    ));
    assert!(Arc::ptr_eq(
        &certificates.auth_runtime.pool_manager,
        &state.pool_manager
    ));
    assert!(Arc::ptr_eq(
        &certificates.file_platform,
        &state.file_platform
    ));
    assert!(Arc::ptr_eq(
        &certificates.certificate_verification_limiter,
        &state.certificate_verification_limiter
    ));
    let session = school_auth::session_service::AuthenticatedSession::for_tests(
        state.auth_runtime.identity_cache.clone(),
        school_tenancy::TenantContext {
            tenant_id: Uuid::new_v4(),
            subdomain: "school-a".into(),
            pool: a.clone(),
        },
        Uuid::new_v4(),
        actor_id,
        "staff",
    );
    let narrow_routes = Router::new()
        .route(
            "/academic",
            get(school_academic_http::core::handlers::list_years),
        )
        .route(
            "/certificates",
            get(school_certificates_http::handlers::list_certificate_campaigns),
        )
        .layer(axum::Extension(session))
        .with_state(state.clone());
    for path in ["/academic", "/certificates"] {
        let response = narrow_routes
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "narrow handler must enforce permission: {path}"
        );
    }
    let app = build_app(state);
    for (tenant, count) in [("school-a", 1), ("school-b", 0)] {
        for path in [
            "/api/school/public/statistics",
            "/api/school/public/organization",
            "/api/public/academic-context/options",
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .header("Origin", format!("https://{tenant}.schoolorbit.app"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["cache-control"], "no-store");
            let body: serde_json::Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 1_048_576).await.unwrap())
                    .unwrap();
            assert_eq!(body["success"], true);
            if path.ends_with("statistics") {
                assert_eq!(body["data"]["totalStaff"], count);
            }
        }
    }
    for (path, origin, hint, expected) in [
        (
            "/api/school/public/statistics",
            None,
            None,
            StatusCode::BAD_REQUEST,
        ),
        (
            "/api/school/public/organization",
            Some("https://school-a.schoolorbit.app"),
            Some("school-b"),
            StatusCode::BAD_REQUEST,
        ),
        (
            "/api/school/public/statistics",
            Some("https://missing.schoolorbit.app"),
            None,
            StatusCode::NOT_FOUND,
        ),
        (
            "/api/academic/years",
            Some("https://school-a.schoolorbit.app"),
            None,
            StatusCode::UNAUTHORIZED,
        ),
        (
            "/api/certificates/campaigns",
            Some("https://school-a.schoolorbit.app"),
            None,
            StatusCode::UNAUTHORIZED,
        ),
        (
            "/api/school/settings",
            Some("https://school-a.schoolorbit.app"),
            None,
            StatusCode::UNAUTHORIZED,
        ),
    ] {
        let mut request = Request::builder().uri(path);
        if let Some(origin) = origin {
            request = request.header("Origin", origin);
        }
        if let Some(hint) = hint {
            request = request.header("X-School-Subdomain", hint);
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), expected, "{path}");
    }
    server.abort();
}
