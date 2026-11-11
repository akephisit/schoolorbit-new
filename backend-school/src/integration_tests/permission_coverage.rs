//! Real session/CSRF transport regressions for independent permission capabilities.
use crate::integration_tests::auth::session_http_tests::protected_router::ProtectedFixture;
use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request, StatusCode},
    middleware::from_fn_with_state,
    Router,
};
use school_auth::session_crypto::RawSessionToken;
use school_permissions::registry::codes;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

pub(crate) struct CoverageActor {
    pub id: Uuid,
    session: Uuid,
    token: RawSessionToken,
}

pub(crate) async fn coverage_actor(
    f: &ProtectedFixture,
    permissions: &[&str],
    token_byte: u8,
) -> CoverageActor {
    let id = f
        .insert_user(
            &f.tenant_a_pool,
            &format!("coverage-{token_byte}"),
            "active",
        )
        .await;
    coverage_actor_for_user(f, id, permissions, token_byte).await
}

async fn coverage_actor_for_user(
    f: &ProtectedFixture,
    id: Uuid,
    permissions: &[&str],
    token_byte: u8,
) -> CoverageActor {
    let role: Uuid = sqlx::query_scalar(
        "INSERT INTO roles(code,name,user_type) VALUES($1,'Coverage fixture','staff') RETURNING id",
    )
    .bind(format!("COVERAGE_{}", Uuid::new_v4().simple()))
    .fetch_one(&f.tenant_a_pool)
    .await
    .unwrap();
    for permission in permissions {
        assert_eq!(sqlx::query("INSERT INTO role_permissions(role_id,permission_id) SELECT $1,id FROM permissions WHERE code=$2")
            .bind(role).bind(permission).execute(&f.tenant_a_pool).await.unwrap().rows_affected(),1);
    }
    sqlx::query("INSERT INTO user_roles(user_id,role_id) VALUES($1,$2)")
        .bind(id)
        .bind(role)
        .execute(&f.tenant_a_pool)
        .await
        .unwrap();
    let (session, token) = f
        .insert_session(&f.tenant_a_pool, id, token_byte, false)
        .await;
    CoverageActor { id, session, token }
}

pub(crate) fn coverage_router(f: &ProtectedFixture) -> Router {
    let platform =
        Arc::new(crate::integration_tests::staff::career_integration_tests::file_platform());
    let (events, _) = tokio::sync::broadcast::channel(16);
    let routes = school_staff::http::routes::protected_routes()
        .with_state(school_staff::http::state::StaffHttpState {
            auth_runtime: f.runtime.clone(),
            file_platform: platform.clone(),
        })
        .merge(
            school_navigation::http::routes::protected_system_routes().with_state(
                school_navigation::http::state::NavigationHttpState {
                    auth_runtime: f.runtime.clone(),
                },
            ),
        )
        .merge(
            school_workflow::http::routes::workflow_routes()
                .merge(school_workflow::http::routes::work_routes())
                .with_state(school_workflow::http::state::WorkflowHttpState {
                    auth_runtime: f.runtime.clone(),
                    file_platform: platform,
                    work_event_channel: events,
                }),
        )
        .nest(
            "/supervision",
            school_supervision::http::handlers::routes().with_state(
                school_supervision::http::state::SupervisionHttpState {
                    auth_runtime: f.runtime.clone(),
                },
            ),
        );
    let manager = Arc::new(school_academic_timetable_http::websockets::WebSocketManager::new());
    let core = school_academic_core_http::state::AcademicCoreHttpState {
        auth_runtime: f.runtime.clone(),
        websocket_manager: manager.clone(),
        placement_rosters: &crate::http_state::PLACEMENT_ROSTERS,
    };
    let delivery = school_academic_delivery_http::state::AcademicDeliveryHttpState {
        auth_runtime: f.runtime.clone(),
        websocket_manager: manager.clone(),
        assessment_publication: &crate::adapters::assessment_publication::ASSESSMENT_PUBLICATION,
        publication_history: &crate::adapters::publication_history::PUBLICATION_HISTORY,
    };
    let timetable = school_academic_timetable_http::state::AcademicTimetableHttpState {
        auth_runtime: f.runtime.clone(),
        websocket_manager: manager.clone(),
        collaboration: manager.collaboration(),
        staff_directory: &crate::adapters::staff_directory::STAFF_DIRECTORY,
        publication_history: &crate::adapters::publication_history::PUBLICATION_HISTORY,
    };
    let academic = school_academic_core_http::routes::academic_routes()
        .with_state(core)
        .merge(school_academic_delivery_http::routes::academic_routes().with_state(delivery))
        .merge(school_academic_timetable_http::routes::academic_routes().with_state(timetable));
    let routes = routes.nest("/academic", academic).nest(
        "/admission",
        school_admission::http::routes::admission_staff_routes().with_state(
            school_admission::http::state::AdmissionHttpState {
                auth_runtime: f.runtime.clone(),
                placement_rosters: &crate::http_state::PLACEMENT_ROSTERS,
                file_authorization: &school_files_http::FILE_AUTHORIZATION,
                file_platform: Arc::new(
                    crate::integration_tests::staff::career_integration_tests::file_platform(),
                ),
            },
        ),
    );
    Router::new()
        .nest("/api", routes)
        .route_layer(from_fn_with_state(
            f.runtime.clone(),
            crate::middleware::session::session_middleware,
        ))
}

pub(crate) async fn coverage_request(
    app: &Router,
    f: &ProtectedFixture,
    actor: &CoverageActor,
    method: Method,
    path: &str,
    payload: Value,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::ORIGIN, "https://tenant-a.schoolorbit.test")
        .header(header::CONTENT_TYPE, "application/json")
        .header(
            header::COOKIE,
            format!(
                "{}={}",
                school_auth::config::SESSION_COOKIE_NAME,
                actor.token.encode().expose_for_cookie()
            ),
        )
        .header("x-csrf-token", f.csrf(actor.session))
        .body(Body::from(payload.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or_else(|_| {
        panic!(
            "unexpected non-JSON response {status}: {}",
            String::from_utf8_lossy(&bytes)
        )
    });
    (status, value)
}

async fn fixture(name: &str) -> ProtectedFixture {
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::ERROR)
        .try_init();
    let pool = school_test_db::create_named_test_pool(name).await;
    school_test_db::run_test_migrations(&pool).await;
    ProtectedFixture::with_prepared_tenant_pool(name, pool).await
}

#[tokio::test]
async fn permission_coverage_staff_assignment_changes_are_atomic_and_differential() {
    let f = fixture("permission_coverage_staff").await;
    let updater = coverage_actor(&f, &[codes::STAFF_UPDATE_ALL], 31).await;
    let assigner =
        coverage_actor(&f, &[codes::STAFF_UPDATE_ALL, codes::ROLES_ASSIGN_ALL], 32).await;
    let remover = coverage_actor(&f, &[codes::STAFF_UPDATE_ALL, codes::ROLES_REMOVE_ALL], 33).await;
    let creator = coverage_actor(&f, &[codes::STAFF_CREATE_ALL], 34).await;
    let target = f
        .insert_user(&f.tenant_a_pool, "coverage-target", "active")
        .await;
    let roles: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM roles WHERE is_active AND user_type='staff' ORDER BY id LIMIT 2",
    )
    .fetch_all(&f.tenant_a_pool)
    .await
    .unwrap();
    let assignment: Uuid =
        sqlx::query_scalar("INSERT INTO user_roles(user_id,role_id) VALUES($1,$2) RETURNING id")
            .bind(target)
            .bind(roles[0])
            .fetch_one(&f.tenant_a_pool)
            .await
            .unwrap();
    let unit: Uuid =
        sqlx::query_scalar("SELECT id FROM organization_units WHERE is_active ORDER BY id LIMIT 1")
            .fetch_one(&f.tenant_a_pool)
            .await
            .unwrap();
    let app = coverage_router(&f);
    let path = format!("/api/staff/{target}");
    for payload in [
        json!({"nickname":"must-not-save","role_ids":[roles[1]]}),
        json!({"nickname":"must-not-save","organization_assignments":[{"organization_unit_id":unit,"position_code":"head"}]}),
    ] {
        let (status, body) =
            coverage_request(&app, &f, &updater, Method::PUT, &path, payload).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        let unchanged:(Option<String>,Uuid)=sqlx::query_as("SELECT nickname,(SELECT role_id FROM user_roles WHERE user_id=users.id AND ended_at IS NULL) FROM users WHERE id=$1").bind(target).fetch_one(&f.tenant_a_pool).await.unwrap();
        assert_eq!(unchanged, (None, roles[0]));
    }
    let (status, body) = coverage_request(
        &app,
        &f,
        &updater,
        Method::PUT,
        &path,
        json!({"nickname":"allowed","role_ids":[roles[0]],"organization_assignments":[]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let preserved: Uuid = sqlx::query_scalar("SELECT id FROM user_roles WHERE user_id=$1")
        .bind(target)
        .fetch_one(&f.tenant_a_pool)
        .await
        .unwrap();
    assert_eq!(
        assignment, preserved,
        "unchanged grants preserve assignment identity"
    );
    let (status, body) = coverage_request(
        &app,
        &f,
        &assigner,
        Method::PUT,
        &path,
        json!({"role_ids":[roles[0],roles[1]]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _) = coverage_request(
        &app,
        &f,
        &assigner,
        Method::PUT,
        &path,
        json!({"role_ids":[roles[1]]}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "assignment capability cannot remove a role"
    );
    let (status, body) = coverage_request(
        &app,
        &f,
        &remover,
        Method::PUT,
        &path,
        json!({"role_ids":[roles[1]]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let mut create = json!({"username":"coverage-created","password":"Synthetic-123!","title":"นาย","first_name":"สร้าง","last_name":"ทดสอบ","role_ids":[roles[0]]});
    let (status, _) = coverage_request(
        &app,
        &f,
        &creator,
        Method::POST,
        "/api/staff",
        create.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM users WHERE username='coverage-created'")
            .fetch_one(&f.tenant_a_pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    create["role_ids"] = json!([]);
    let (status, body) =
        coverage_request(&app, &f, &creator, Method::POST, "/api/staff", create).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code) VALUES($1,$2,'member')").bind(target).bind(unit).execute(&f.tenant_a_pool).await.unwrap();
    let unchanged = json!({"organization_assignments":[{"organization_unit_id":unit,"position_code":"member"}]});
    assert_eq!(
        coverage_request(&app, &f, &updater, Method::PUT, &path, unchanged)
            .await
            .0,
        StatusCode::OK
    );
    for patch in [
        json!({"organization_assignments":[]}),
        json!({"organization_assignments":[{"organization_unit_id":unit,"position_code":"head"}]}),
        json!({"role_ids":[roles[1]],"primary_role_id":roles[1]}),
    ] {
        assert_eq!(
            coverage_request(&app, &f, &remover, Method::PUT, &path, patch)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    let org_create = json!({"username":"coverage-create-org-denied","password":"Synthetic-123!","title":"นาย","first_name":"สร้าง","last_name":"ทดสอบ","role_ids":[],"organization_assignments":[{"organization_unit_id":unit,"position_code":"head"}]});
    assert_eq!(
        coverage_request(&app, &f, &creator, Method::POST, "/api/staff", org_create)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let denied_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM users WHERE username='coverage-create-org-denied'",
    )
    .fetch_one(&f.tenant_a_pool)
    .await
    .unwrap();
    assert_eq!(denied_count, 0);
}

#[tokio::test]
async fn permission_coverage_features_require_exact_management_capabilities() {
    let f = fixture("permission_coverage_features").await;
    let module_reader = coverage_actor(&f, &[codes::ATTENDANCE_READ_SCHOOL], 41).await;
    let reader = coverage_actor(&f, &[codes::FEATURES_READ_ALL], 42).await;
    let updater = coverage_actor(&f, &[codes::FEATURES_UPDATE_ALL], 43).await;
    let feature: Uuid = sqlx::query_scalar(
        "SELECT id FROM feature_toggles WHERE module='attendance' ORDER BY id LIMIT 1",
    )
    .fetch_one(&f.tenant_a_pool)
    .await
    .unwrap();
    let app = coverage_router(&f);
    for path in [
        "/api/admin/features".into(),
        format!("/api/admin/features/{feature}"),
    ] {
        assert_eq!(
            coverage_request(&app, &f, &module_reader, Method::GET, &path, Value::Null)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
        let (status, body) =
            coverage_request(&app, &f, &reader, Method::GET, &path, Value::Null).await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    for (method, path) in [
        (Method::PUT, format!("/api/admin/features/{feature}")),
        (
            Method::POST,
            format!("/api/admin/features/{feature}/toggle"),
        ),
    ] {
        for actor in [&module_reader, &reader] {
            assert_eq!(
                coverage_request(
                    &app,
                    &f,
                    actor,
                    method.clone(),
                    &path,
                    json!({"is_enabled":false})
                )
                .await
                .0,
                StatusCode::FORBIDDEN
            );
        }
        let (status, body) = coverage_request(
            &app,
            &f,
            &updater,
            method,
            &path,
            json!({"is_enabled":false}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
}

#[tokio::test]
async fn permission_coverage_workflow_rejects_supplied_read_permissions_and_outside_unit_scope() {
    let f = fixture("permission_coverage_workflow").await;
    let reader = coverage_actor(&f, &[codes::LEARNING_OFFERING_READ_SCHOOL], 51).await;
    let wildcard = coverage_actor(&f, &[codes::WILDCARD], 52).await;
    let manager =
        coverage_actor(&f, &[codes::LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT], 53).await;
    let units: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM organization_units WHERE is_active ORDER BY id LIMIT 2")
            .fetch_all(&f.tenant_a_pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code) VALUES($1,$2,'member')").bind(manager.id).bind(units[0]).execute(&f.tenant_a_pool).await.unwrap();
    sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code) VALUES($1,$2,'member')").bind(manager.id).bind(units[1]).execute(&f.tenant_a_pool).await.unwrap();
    sqlx::query("UPDATE user_roles SET organization_unit_id=$2 WHERE user_id=$1")
        .bind(manager.id)
        .bind(units[0])
        .execute(&f.tenant_a_pool)
        .await
        .unwrap();
    let app = coverage_router(&f);
    let payload = |permission: &str, unit: Option<Uuid>| json!({"moduleCode":"organization_work","workflowCode":"coverage","title":"Coverage work","managedByPermission":permission,"organizationUnitId":unit});
    for permission in [
        codes::LEARNING_OFFERING_READ_SCHOOL,
        "invented.manage.school",
        codes::WILDCARD,
        codes::LEARNING_OFFERING_MANAGE_ASSIGNED,
    ] {
        for actor in [&reader, &wildcard] {
            let (status, body) = coverage_request(
                &app,
                &f,
                actor,
                Method::POST,
                "/api/workflow-windows",
                payload(permission, None),
            )
            .await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{permission}: {body}");
        }
    }
    for unit in [None, Some(units[1])] {
        assert_eq!(
            coverage_request(
                &app,
                &f,
                &manager,
                Method::POST,
                "/api/workflow-windows",
                payload(codes::LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT, unit)
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    let (status, created) = coverage_request(
        &app,
        &f,
        &manager,
        Method::POST,
        "/api/workflow-windows",
        payload(
            codes::LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
            Some(units[0]),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let window = created["data"]["id"].as_str().unwrap();
    // An old malicious record cannot make a reader a manager through list, update or assignment.
    let forged:Uuid=sqlx::query_scalar("INSERT INTO workflow_windows(module_code,workflow_code,title,managed_by_permission) VALUES('organization_work','coverage','Forged',$1) RETURNING id").bind(codes::LEARNING_OFFERING_READ_SCHOOL).fetch_one(&f.tenant_a_pool).await.unwrap();
    let (_, listed) = coverage_request(
        &app,
        &f,
        &reader,
        Method::GET,
        "/api/me/workflow-windows/manageable",
        Value::Null,
    )
    .await;
    assert!(listed["data"]["items"].as_array().unwrap().is_empty());
    let (status, _) = coverage_request(
        &app,
        &f,
        &reader,
        Method::PATCH,
        &format!("/api/workflow-windows/{forged}"),
        json!({"status":"open"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status,_)=coverage_request(&app,&f,&reader,Method::POST,"/api/work-items",json!({"workflowWindowId":forged,"moduleCode":"organization_work","sourceResourceType":"coverage","sourceResourceId":Uuid::new_v4(),"title":"Forbidden task","actionPath":"/staff/work","assignees":[]})).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, listed) = coverage_request(
        &app,
        &f,
        &manager,
        Method::GET,
        "/api/me/workflow-windows/manageable",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert!(listed["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["id"] == window));
}

#[tokio::test]
async fn permission_coverage_staff_retained_role_rows_keep_scope_dates_notes_and_identity() {
    use school_authorization::ActorContext;
    use school_staff::services::staff_service;
    let f = fixture("permission_coverage_staff_retention").await;
    let editor = coverage_actor(&f, &[codes::STAFF_UPDATE_ALL, codes::ROLES_REMOVE_ALL], 71).await;
    let target = f
        .insert_user(&f.tenant_a_pool, "coverage-history-target", "active")
        .await;
    let units: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM organization_units WHERE is_active ORDER BY id LIMIT 2")
            .fetch_all(&f.tenant_a_pool)
            .await
            .unwrap();
    let mut roles = Vec::new();
    for permission in [
        codes::ROLES_ASSIGN_ALL,
        codes::WILDCARD,
        codes::STAFF_PROFILE_READ_ORGANIZATION_UNIT,
        codes::STAFF_READ_ALL,
    ] {
        let role:Uuid=sqlx::query_scalar("INSERT INTO roles(code,name,user_type) VALUES($1,'Historical authority fixture','staff') RETURNING id").bind(format!("HISTORY_{}",Uuid::new_v4().simple())).fetch_one(&f.tenant_a_pool).await.unwrap();
        sqlx::query("INSERT INTO role_permissions(role_id,permission_id) SELECT $1,id FROM permissions WHERE code=$2").bind(role).bind(permission).execute(&f.tenant_a_pool).await.unwrap();
        roles.push(role);
    }
    // Each row contains authority-defining provenance the ordinary editor must preserve.
    for (index, scope, start, end) in [
        (0, None, -30, Some(-1)),
        (1, None, 10, None),
        (2, Some(units[0]), -20, None),
        (3, None, -20, None),
    ] {
        sqlx::query("INSERT INTO user_roles(user_id,role_id,organization_unit_id,started_at,ended_at,notes) VALUES($1,$2,$3,CURRENT_DATE+$4,CASE WHEN $5::int IS NULL THEN NULL ELSE CURRENT_DATE+$5 END,'immutable provenance')")
            .bind(target).bind(roles[index]).bind(scope).bind(start).bind(end).execute(&f.tenant_a_pool).await.unwrap();
    }
    let original: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(role) FROM user_roles role WHERE user_id=$1 ORDER BY role_id,id",
    )
    .bind(target)
    .fetch_all(&f.tenant_a_pool)
    .await
    .unwrap();
    let app = coverage_router(&f);
    let _managed_lease = f
        .runtime
        .pool_manager
        .get_pool("test-pool://session-router-tenant-a", "tenant-a")
        .await
        .unwrap();

    // Regranting an ended role requires ASSIGN, even when paired with a valid removal.
    for retained in [roles[0]] {
        let (status, body) = coverage_request(
            &app,
            &f,
            &editor,
            Method::PUT,
            &format!("/api/staff/{target}"),
            json!({"nickname":"must-not-save","role_ids":[retained,roles[2]]}),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        let actual: Vec<Value> = sqlx::query_scalar(
            "SELECT to_jsonb(role) FROM user_roles role WHERE user_id=$1 ORDER BY role_id,id",
        )
        .bind(target)
        .fetch_all(&f.tenant_a_pool)
        .await
        .unwrap();
        assert_eq!(
            original, actual,
            "denied mixed-role replacement is wholly atomic"
        );
    }
    let (status, body) = coverage_request(
        &app,
        &f,
        &editor,
        Method::PUT,
        &format!("/api/staff/{target}"),
        json!({"role_ids":[roles[1],roles[2]]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let retained:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(role) FROM user_roles role WHERE user_id=$1 AND role_id=ANY($2) ORDER BY role_id,id").bind(target).bind(&roles[..3]).fetch_all(&f.tenant_a_pool).await.unwrap();
    let expected: Vec<_> = original
        .iter()
        .filter(|row| row["role_id"] != roles[3].to_string())
        .cloned()
        .collect();
    assert_eq!(
        retained, expected,
        "scope, timing and immutable identities survive actual differential removal"
    );
    let active_count:i64=sqlx::query_scalar("SELECT count(*) FROM user_roles WHERE user_id=$1 AND started_at<=CURRENT_DATE AND (ended_at IS NULL OR ended_at>CURRENT_DATE)").bind(target).fetch_one(&f.tenant_a_pool).await.unwrap();
    assert_eq!(active_count, 1);
    let effective = school_authorization::load_actor_context(
        target,
        "tenant-a",
        &f.tenant_a_pool,
        &school_authorization::PermissionCache::new(),
    )
    .await
    .unwrap();
    assert!(
        !effective.has_permission(codes::WILDCARD),
        "retained future wildcard role is not effective before its actual start"
    );
    let actor = ActorContext {
        user_id: editor.id,
        permissions: vec![
            codes::ROLES_ASSIGN_ALL.into(),
            codes::ROLES_REMOVE_ALL.into(),
        ],
    };
    let patch = serde_json::from_value(json!({"role_ids":[roles[0],roles[2]]})).unwrap();
    staff_service::update_staff(&f.tenant_a_pool, target, patch, &actor)
        .await
        .unwrap();
    let historical: Value =
        sqlx::query_scalar("SELECT to_jsonb(role) FROM user_roles role WHERE id=$1")
            .bind(
                Uuid::parse_str(
                    original
                        .iter()
                        .find(|row| row["role_id"] == roles[0].to_string())
                        .unwrap()["id"]
                        .as_str()
                        .unwrap(),
                )
                .unwrap(),
            )
            .fetch_one(&f.tenant_a_pool)
            .await
            .unwrap();
    assert_eq!(
        historical,
        *original
            .iter()
            .find(|row| row["role_id"] == roles[0].to_string())
            .unwrap()
    );
    let canceled: Value = sqlx::query_scalar(
        "SELECT to_jsonb(role) FROM user_roles role WHERE user_id=$1 AND role_id=$2",
    )
    .bind(target)
    .bind(roles[1])
    .fetch_one(&f.tenant_a_pool)
    .await
    .unwrap();
    assert_eq!(
        canceled["ended_at"], canceled["started_at"],
        "explicit future removal cancels only the future interval"
    );
    let (status, body) = coverage_request(
        &app,
        &f,
        &editor,
        Method::PUT,
        &format!("/api/staff/{target}"),
        json!({"role_ids":[roles[1],roles[2]]}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a canceled scheduled role requires a new ASSIGN grant: {body}"
    );
}

async fn academic_fixture(name: &str) -> ProtectedFixture {
    academic_fixture_variant(name, false).await
}
async fn academic_fixture_variant(name: &str, include_science: bool) -> ProtectedFixture {
    use crate::integration_tests::academic::cutover_test_support::*;
    let pool = school_test_db::create_named_test_pool(name).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    if include_science {
        seed_learning_offering_policy_science_course(&pool)
            .await
            .unwrap();
    }
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    school_test_db::run_test_migrations(&pool).await;
    ProtectedFixture::with_prepared_tenant_pool(name, pool).await
}

#[tokio::test]
async fn permission_coverage_academic_regions_and_context_use_independent_exact_actions() {
    let f = academic_fixture("permission_coverage_academic_context").await;
    let app = coverage_router(&f);
    for (index, permission) in [
        codes::ACADEMIC_YEAR_READ_SCHOOL,
        codes::ACADEMIC_YEAR_MANAGE_SCHOOL,
        codes::ACADEMIC_TERM_READ_SCHOOL,
        codes::ACADEMIC_TERM_MANAGE_SCHOOL,
    ]
    .into_iter()
    .enumerate()
    {
        let actor = coverage_actor(&f, &[permission], 81 + index as u8).await;
        let (status, body) = coverage_request(
            &app,
            &f,
            &actor,
            Method::GET,
            "/api/academic/setup/workspace",
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{permission}: {body}");
        let year = index < 2;
        assert_eq!(body["data"]["canReadYears"], year);
        assert_eq!(body["data"]["canReadTerms"], !year);
        assert!(!body["data"]["years"].as_array().unwrap().is_empty());
        if year {
            for field in [
                "terms",
                "bellSchedules",
                "bellScheduleSetupYearIds",
                "bellScheduleSetupTermIds",
                "schoolDaysEditableTermIds",
            ] {
                assert!(
                    body["data"][field].as_array().unwrap().is_empty(),
                    "year-only actor received {field}"
                );
            }
        }
    }
    let unrelated = coverage_actor(&f, &[codes::FEATURES_READ_ALL], 85).await;
    assert_eq!(
        coverage_request(
            &app,
            &f,
            &unrelated,
            Method::GET,
            "/api/academic/setup/workspace",
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let year: Uuid =
        sqlx::query_scalar("SELECT academic_year_id FROM homerooms ORDER BY id LIMIT 1")
            .fetch_one(&f.tenant_a_pool)
            .await
            .unwrap();
    let path = format!("/api/academic/student-years/homeroom-options?academicYearId={year}");
    assert_eq!(
        coverage_request(&app, &f, &unrelated, Method::GET, &path, Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let student_reader = coverage_actor(&f, &[codes::STUDENT_ACADEMIC_YEAR_READ_SCHOOL], 86).await;
    let (status, body) =
        coverage_request(&app, &f, &student_reader, Method::GET, &path, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let expected: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM homerooms WHERE academic_year_id=$1 ORDER BY code,id")
            .bind(year)
            .fetch_all(&f.tenant_a_pool)
            .await
            .unwrap();
    assert_eq!(
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|room| Uuid::parse_str(room["id"].as_str().unwrap()).unwrap())
            .collect::<Vec<_>>(),
        expected
    );
    for room in body["data"].as_array().unwrap() {
        assert_eq!(room.as_object().unwrap().len(), 6);
    }
    let (status, body) = coverage_request(
        &app,
        &f,
        &student_reader,
        Method::GET,
        &format!(
            "/api/academic/student-years/homeroom-options?academicYearId={}",
            Uuid::new_v4()
        ),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"], json!([]));
    assert_eq!(
        coverage_request(
            &app,
            &f,
            &student_reader,
            Method::GET,
            &format!("/api/academic/homerooms?academicYearId={year}"),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    sqlx::query("INSERT INTO homerooms(code,name,academic_year_id,grade_level_id,study_program_id,room_number,capacity) SELECT 'COVERAGE-'||n,'Overflow '||n,room.academic_year_id,room.grade_level_id,room.study_program_id,(n+10000)::text,30 FROM generate_series(1,2001) n CROSS JOIN LATERAL (SELECT academic_year_id,grade_level_id,study_program_id FROM homerooms WHERE academic_year_id=$1 ORDER BY id LIMIT 1) room").bind(year).execute(&f.tenant_a_pool).await.unwrap();
    let (status, body) =
        coverage_request(&app, &f, &student_reader, Method::GET, &path, Value::Null).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_ne!(
        body["success"], true,
        "overflow must never masquerade as a complete dropdown"
    );
}

#[tokio::test]
async fn permission_coverage_admission_context_does_not_grant_applicant_reads() {
    let f = academic_fixture("permission_coverage_admission_context").await;
    let app = coverage_router(&f);
    let (year, grade): (Uuid, Uuid) =
        sqlx::query_as("SELECT academic_year_id,grade_level_id FROM homerooms ORDER BY id LIMIT 1")
            .fetch_one(&f.tenant_a_pool)
            .await
            .unwrap();
    let round:Uuid=sqlx::query_scalar("INSERT INTO admission_rounds(academic_year_id,grade_level_id,name,apply_start_date,apply_end_date) VALUES($1,$2,'Synthetic action context',CURRENT_DATE,CURRENT_DATE+1) RETURNING id").bind(year).bind(grade).fetch_one(&f.tenant_a_pool).await.unwrap();
    for (index, permission) in [
        codes::ADMISSION_MANAGE_ALL,
        codes::ADMISSION_SCORES_ALL,
        codes::ADMISSION_ENROLL_ALL,
    ]
    .into_iter()
    .enumerate()
    {
        let actor = coverage_actor(&f, &[permission], 91 + index as u8).await;
        let (list_status, list) = coverage_request(
            &app,
            &f,
            &actor,
            Method::GET,
            &format!("/api/admission/rounds?academicYearId={year}"),
            Value::Null,
        )
        .await;
        assert_eq!(list_status, StatusCode::OK, "{list}");
        assert!(list["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == round.to_string()));
        for suffix in ["", "/subjects", "/tracks"] {
            let (status, body) = coverage_request(
                &app,
                &f,
                &actor,
                Method::GET,
                &format!("/api/admission/rounds/{round}{suffix}"),
                Value::Null,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{permission} {suffix}: {body}");
        }
        assert_eq!(
            coverage_request(
                &app,
                &f,
                &actor,
                Method::GET,
                &format!("/api/admission/rounds/{round}/applications"),
                Value::Null
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    let unrelated = coverage_actor(&f, &[codes::FEATURES_READ_ALL], 94).await;
    for suffix in ["", "/subjects", "/tracks"] {
        assert_eq!(
            coverage_request(
                &app,
                &f,
                &unrelated,
                Method::GET,
                &format!("/api/admission/rounds/{round}{suffix}"),
                Value::Null
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    sqlx::query("INSERT INTO admission_rounds(academic_year_id,grade_level_id,name,apply_start_date,apply_end_date) SELECT $1,$2,'Bounded context '||n,CURRENT_DATE,CURRENT_DATE+1 FROM generate_series(1,2001) n").bind(year).bind(grade).execute(&f.tenant_a_pool).await.unwrap();
    let manager = coverage_actor(&f, &[codes::ADMISSION_MANAGE_ALL], 95).await;
    let (status, body) = coverage_request(
        &app,
        &f,
        &manager,
        Method::GET,
        &format!("/api/admission/rounds?academicYearId={year}"),
        Value::Null,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "round options reject overflow instead of silently truncating: {body}"
    );
}

#[tokio::test]
async fn permission_coverage_daily_teaching_accepts_today_without_planner_access() {
    let f = academic_fixture("permission_coverage_daily_today").await;
    let app = coverage_router(&f);
    let today = coverage_actor(&f, &[codes::ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL], 101).await;
    let planner = coverage_actor(&f, &[codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL], 102).await;
    let (term,date):(Uuid,chrono::NaiveDate)=sqlx::query_as("SELECT academic_term_id,effective_from FROM academic_timetable_versions WHERE status='published' ORDER BY effective_from,id LIMIT 1").fetch_one(&f.tenant_a_pool).await.unwrap();
    let path = format!("/api/academic/timetable/daily-teaching?academicTermId={term}&date={date}");
    assert_eq!(
        coverage_request(&app, &f, &planner, Method::GET, &path, Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, body) = coverage_request(&app, &f, &today, Method::GET, &path, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["academicTermId"], term.to_string());
}

#[tokio::test]
async fn permission_coverage_offering_capability_matches_current_resource_manage_scope() {
    let f = academic_fixture_variant("permission_coverage_offering_caps", true).await;
    let (first,first_unit,teacher,second,second_unit):(Uuid,Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT first_offering.id,first_offering.owning_organization_unit_id,teacher.teacher_id,second_offering.id,second_offering.owning_organization_unit_id FROM learning_offerings first_offering JOIN learning_groups first_group ON first_group.learning_offering_id=first_offering.id JOIN learning_group_teachers teacher ON teacher.learning_group_id=first_group.id JOIN learning_offerings second_offering ON second_offering.owning_organization_unit_id<>first_offering.owning_organization_unit_id WHERE NOT EXISTS(SELECT 1 FROM learning_groups second_group JOIN learning_group_teachers second_teacher ON second_teacher.learning_group_id=second_group.id WHERE second_group.learning_offering_id=second_offering.id AND second_teacher.teacher_id=teacher.teacher_id) ORDER BY first_offering.id,teacher.teacher_id,second_offering.id LIMIT 1").fetch_one(&f.tenant_a_pool).await.unwrap();
    let offerings = [first, second];
    let units = [first_unit, second_unit];
    let unit_manager = coverage_actor(
        &f,
        &[
            codes::LEARNING_OFFERING_READ_SCHOOL,
            codes::LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
        ],
        111,
    )
    .await;
    sqlx::query("UPDATE user_roles SET organization_unit_id=$2 WHERE user_id=$1")
        .bind(unit_manager.id)
        .bind(units[0])
        .execute(&f.tenant_a_pool)
        .await
        .unwrap();
    for unit in &units {
        sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code) VALUES($1,$2,'member')").bind(unit_manager.id).bind(unit).execute(&f.tenant_a_pool).await.unwrap();
    }
    sqlx::query("DELETE FROM user_roles WHERE user_id=$1")
        .bind(teacher)
        .execute(&f.tenant_a_pool)
        .await
        .unwrap();
    let assigned = coverage_actor_for_user(
        &f,
        teacher,
        &[
            codes::LEARNING_OFFERING_READ_SCHOOL,
            codes::LEARNING_OFFERING_MANAGE_ASSIGNED,
        ],
        112,
    )
    .await;
    let app = coverage_router(&f);
    for actor in [&unit_manager, &assigned] {
        for (index, offering) in offerings.iter().enumerate() {
            let (status, body) = coverage_request(
                &app,
                &f,
                actor,
                Method::GET,
                &format!("/api/academic/offerings/{offering}"),
                Value::Null,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            assert_eq!(
                body["data"]["canManage"],
                index == 0,
                "current resource decides capabilities"
            );
            if index == 1 {
                let (status, body) = coverage_request(
                    &app,
                    &f,
                    actor,
                    Method::PATCH,
                    &format!("/api/academic/offerings/{offering}"),
                    json!({"rowVersion":1,"targets":[]}),
                )
                .await;
                assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
            }
        }
    }
}

#[tokio::test]
async fn permission_coverage_delegation_capabilities_require_current_unit_leadership_and_grant_origin(
) {
    let f = fixture("permission_coverage_delegation_caps").await;
    let units: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM organization_units WHERE is_active ORDER BY id LIMIT 2")
            .fetch_all(&f.tenant_a_pool)
            .await
            .unwrap();
    // Isolate grant origin: baseline head-position grants would be independent valid authority.
    sqlx::query("DELETE FROM organization_permission_grants WHERE organization_unit_id=ANY($1)")
        .bind(&units)
        .execute(&f.tenant_a_pool)
        .await
        .unwrap();
    let leader = coverage_actor(
        &f,
        &[
            codes::ROLES_READ_ALL,
            codes::ORGANIZATION_WORK_APPROVE_ORGANIZATION_UNIT,
        ],
        121,
    )
    .await;
    let school = coverage_actor(&f, &[codes::ROLES_READ_ALL, codes::ROLES_UPDATE_ALL], 122).await;
    let creator = coverage_actor(&f, &[], 123).await;
    let recipient = coverage_actor(&f, &[], 124).await;
    sqlx::query("UPDATE user_roles SET organization_unit_id=$2 WHERE user_id=$1")
        .bind(leader.id)
        .bind(units[0])
        .execute(&f.tenant_a_pool)
        .await
        .unwrap();
    for unit in &units {
        sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code,started_at) VALUES($1,$2,'head',CURRENT_DATE-1)").bind(leader.id).bind(unit).execute(&f.tenant_a_pool).await.unwrap();
    }
    let app = coverage_router(&f);
    for (index, unit) in units.iter().enumerate() {
        let (status, body) = coverage_request(
            &app,
            &f,
            &leader,
            Method::GET,
            &format!("/api/organization/units/{unit}"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["data"]["can_manage_delegations"], index == 0);
        let status = coverage_request(
            &app,
            &f,
            &leader,
            Method::GET,
            &format!("/api/organization/units/{unit}/delegations"),
            Value::Null,
        )
        .await
        .0;
        assert_eq!(
            status,
            if index == 0 {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            }
        );
    }
    for from in [leader.id, creator.id] {
        sqlx::query("INSERT INTO organization_permission_delegations(from_user_id,to_user_id,organization_unit_id,permission_id) SELECT $1,$2,$3,id FROM permissions WHERE code=$4")
            .bind(from).bind(recipient.id).bind(units[0]).bind(codes::STAFF_PROFILE_READ_ORGANIZATION_UNIT).execute(&f.tenant_a_pool).await.unwrap();
    }
    let path = format!("/api/organization/units/{}/delegations", units[0]);
    let (status, body) = coverage_request(&app, &f, &leader, Method::GET, &path, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for row in body["data"].as_array().unwrap() {
        assert_eq!(
            row["can_revoke"],
            row["from_user_id"] == leader.id.to_string()
        );
    }
    let (status, body) = coverage_request(&app, &f, &school, Method::GET, &path, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["data"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["can_revoke"] == true));
    for (start, end) in [(1, None), (-2, Some(0))] {
        sqlx::query("UPDATE organization_members SET started_at=CURRENT_DATE+$3,ended_at=CASE WHEN $4::int IS NULL THEN NULL ELSE CURRENT_DATE+$4 END WHERE user_id=$1 AND organization_unit_id=$2").bind(leader.id).bind(units[0]).bind(start).bind(end).execute(&f.tenant_a_pool).await.unwrap();
        let (status, body) = coverage_request(
            &app,
            &f,
            &leader,
            Method::GET,
            &format!("/api/organization/units/{}", units[0]),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["data"]["can_manage_delegations"], false);
        assert_eq!(
            coverage_request(&app, &f, &leader, Method::GET, &path, Value::Null)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
}

#[tokio::test]
async fn permission_coverage_score_track_roster_is_minimal_filtered_and_complete() {
    let f = academic_fixture("permission_coverage_score_track").await;
    let app = coverage_router(&f);
    let scorer = coverage_actor(&f, &[codes::ADMISSION_SCORES_ALL], 131).await;
    let unrelated = coverage_actor(&f, &[codes::ADMISSION_READ_ALL], 132).await;
    let (year,grade,program):(Uuid,Uuid,Uuid)=sqlx::query_as("SELECT academic_year_id,grade_level_id,study_program_id FROM homerooms ORDER BY id LIMIT 1").fetch_one(&f.tenant_a_pool).await.unwrap();
    let round:Uuid=sqlx::query_scalar("INSERT INTO admission_rounds(academic_year_id,grade_level_id,name,apply_start_date,apply_end_date) VALUES($1,$2,'Score context',CURRENT_DATE,CURRENT_DATE+1) RETURNING id").bind(year).bind(grade).fetch_one(&f.tenant_a_pool).await.unwrap();
    let mut tracks = Vec::new();
    for name in ["Track A", "Track B"] {
        let id:Uuid=sqlx::query_scalar("INSERT INTO admission_tracks(admission_round_id,academic_year_id,study_program_id,name) VALUES($1,$2,$3,$4) RETURNING id").bind(round).bind(year).bind(program).bind(name).fetch_one(&f.tenant_a_pool).await.unwrap();
        tracks.push(id);
    }
    for (index, status) in [
        "submitted",
        "verified",
        "scored",
        "accepted",
        "absent",
        "rejected",
    ]
    .into_iter()
    .enumerate()
    {
        sqlx::query("INSERT INTO admission_applications(admission_round_id,admission_track_id,national_id,application_number,first_name,last_name,status,phone,address_line) VALUES($1,$2,'encrypted-score-fixture',$3,'Synthetic','Scoring',$4,'private-phone','private-address-sentinel')")
            .bind(round).bind(tracks[0]).bind(format!("COVERAGE-SCORE-{index}")).bind(status).execute(&f.tenant_a_pool).await.unwrap();
    }
    sqlx::query("INSERT INTO admission_applications(admission_round_id,admission_track_id,national_id,first_name,last_name,status) VALUES($1,$2,'encrypted-score-fixture','Other','Track','verified')").bind(round).bind(tracks[1]).execute(&f.tenant_a_pool).await.unwrap();
    let path = format!(
        "/api/admission/rounds/{round}/score-track-roster?trackId={}",
        tracks[0]
    );
    assert_eq!(
        coverage_request(&app, &f, &unrelated, Method::GET, &path, Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, body) = coverage_request(&app, &f, &scorer, Method::GET, &path, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let rows = body["data"].as_array().unwrap();
    assert_eq!(
        rows.len(),
        4,
        "only score-eligible statuses enter the roster"
    );
    for row in rows {
        assert_eq!(
            row.as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from(["id", "applicationNumber", "fullName"])
        );
        assert_eq!(row["fullName"], "Synthetic Scoring");
    }
    assert_eq!(
        coverage_request(
            &app,
            &f,
            &scorer,
            Method::GET,
            &format!("/api/admission/rounds/{round}/applications"),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        coverage_request(
            &app,
            &f,
            &scorer,
            Method::GET,
            &format!(
                "/api/admission/rounds/{}/score-track-roster?trackId={}",
                Uuid::new_v4(),
                tracks[0]
            ),
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    sqlx::query("INSERT INTO admission_applications(admission_round_id,admission_track_id,national_id,first_name,last_name,status) SELECT $1,$2,'encrypted-score-fixture','Bounded','Roster','verified' FROM generate_series(1,10001)").bind(round).bind(tracks[0]).execute(&f.tenant_a_pool).await.unwrap();
    let (status, body) = coverage_request(&app, &f, &scorer, Method::GET, &path, Value::Null).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_ne!(body["success"], true);
}
