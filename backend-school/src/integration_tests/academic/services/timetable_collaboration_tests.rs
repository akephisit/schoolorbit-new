//! Real session middleware, native socket transport and HTTP share one domain authority.
use std::{sync::Arc, time::Duration};

use axum::{middleware::from_fn_with_state, routing::get, Router};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_tungstenite::{connect_async, tungstenite::{client::IntoClientRequest, Message}, MaybeTlsStream, WebSocketStream};
use uuid::Uuid;

use crate::integration_tests::auth::session_http_tests::protected_router::ProtectedFixture;
use school_academic_http::{state::{AcademicHttpState, AcademicWebSocketState}, websockets::{timetable_websocket_handler, WebSocketManager}};
use school_academic_timetable::{collaboration::TimetableMutationContext, models::timetable_block::*, services::timetable_block_service};

use super::timetable_block_service_tests::{draft_version, migrated_pool};

const ACTOR: &str = "50000000-0000-0000-0000-000000000002";
type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

struct Fixture {
    auth: ProtectedFixture,
    manager: Arc<WebSocketManager>,
    server: JoinHandle<()>,
    address: std::net::SocketAddr,
    cookie: String,
    csrf: String,
    version: Uuid,
    term: Uuid,
    block: TimetableBlock,
}

impl Drop for Fixture { fn drop(&mut self) { self.server.abort(); } }

impl Fixture {
    async fn new(name: &str) -> Self {
        let pool = migrated_pool(name).await;
        let actor = Uuid::parse_str(ACTOR).unwrap();
        let (version, term, _, bell) = draft_version(&pool).await;
        let period: Uuid = sqlx::query_scalar("SELECT id FROM bell_schedule_periods WHERE bell_schedule_id=$1 AND is_active ORDER BY order_index LIMIT 1").bind(bell).fetch_one(&pool).await.unwrap();
        let blocks = timetable_block_service::create_structural_blocks(&pool, &TimetableMutationContext::for_tests(&pool, actor), CreateStructuralTimetableBlocksRequest {
            timetable_version_id: version, academic_term_id: term,
            structural_kind: TimetableStructuralKind::FlagCeremony, title: "Shared authority".into(), note: None,
            slots: ["MON", "TUE"].into_iter().map(|day| TimetableStructuralSlotInput { day_of_week: day.into(), bell_schedule_period_id: period }).collect(),
            homeroom_ids: vec![], teacher_ids: vec![], all_homerooms: true, all_teachers: false, room_id: None,
        }).await.unwrap();
        let auth = ProtectedFixture::with_prepared_tenant_pool(name, pool).await;
        grant(&auth.tenant_a_pool, actor, &["academic_timetable.read.school", "academic_timetable.manage.school", "academic_timetable.publish.school"]).await;
        let (session, token) = auth.insert_session(&auth.tenant_a_pool, actor, 191, false).await;
        let cookie = format!("__Host-schoolorbit_session={}", token.encode().expose_for_cookie());
        let csrf = auth.csrf(session);
        let manager = Arc::new(WebSocketManager::new());
        let http = AcademicHttpState {
            collaboration: manager.collaboration(), staff_directory: &crate::adapters::staff_directory::STAFF_DIRECTORY,
            placement_rosters: &crate::http_state::PLACEMENT_ROSTERS, auth_runtime: auth.runtime.clone(), websocket_manager: manager.clone(),
            result_locks: &crate::adapters::academic_result_locks::RESULT_LOCKS,
            lifecycle_providers: &crate::adapters::academic_lifecycle::LIFECYCLE_PROVIDERS,
        };
        let ws = AcademicWebSocketState { collaboration: manager.collaboration(), staff_directory: http.staff_directory, auth_runtime: auth.runtime.clone(), websocket_manager: manager.clone() };
        let app = Router::new().nest("/api/academic", school_academic_http::routes::academic_routes().with_state(http))
            .route_layer(from_fn_with_state(auth.runtime.clone(), crate::middleware::session::session_middleware))
            .merge(Router::new().route("/ws/timetable", get(timetable_websocket_handler)).with_state(ws));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });
        Self { auth, manager, server, address, cookie, csrf, version, term, block: blocks[0].clone() }
    }

    async fn socket(&self) -> (Socket, Value) { self.socket_with_cookie(&self.cookie).await }

    async fn socket_with_cookie(&self, cookie: &str) -> (Socket, Value) {
        let mut request = format!("ws://{}/ws/timetable?academicTermId={}&timetableVersionId={}&school_subdomain=tenant-a", self.address, self.term, self.version).into_client_request().unwrap();
        request.headers_mut().insert("origin", "http://localhost:5173".parse().unwrap());
        request.headers_mut().insert("cookie", cookie.parse().unwrap());
        let (mut socket, _) = connect_async(request).await.expect("real authenticated socket upgrade");
        let sync = event(&mut socket, "DraftStateSync").await;
        (socket, sync)
    }

    fn update(&self, proof: Option<(Uuid, Uuid)>, note: &str) -> reqwest::RequestBuilder {
        let client = reqwest::Client::new();
        let mut request = client.put(format!("http://{}/api/academic/timetable-blocks/{}", self.address, self.block.id))
            .header("origin", "http://localhost:5173").header("x-school-subdomain", "tenant-a")
            .header("cookie", &self.cookie).header("x-csrf-token", &self.csrf)
            .json(&json!({"timetableVersionId": self.version, "rowVersion": self.block.row_version, "note": note}));
        if let Some((connection, token)) = proof {
            request = request.header("x-timetable-connection-id", connection.to_string())
                .header("x-timetable-lease-token", token.to_string()).header("x-timetable-mutation-id", Uuid::new_v4().to_string());
        }
        request
    }

    fn source(&self) -> Value { json!({"kind": "existing_block", "blockId": self.block.id, "rowVersion": self.block.row_version}) }
}

async fn grant(pool: &sqlx::PgPool, actor: Uuid, codes: &[&str]) {
    let role: Uuid = sqlx::query_scalar("INSERT INTO roles(code,name,user_type) VALUES($1,'Collaboration test','staff') RETURNING id")
        .bind(format!("COLLAB_{}", Uuid::new_v4().simple())).fetch_one(pool).await.unwrap();
    for code in codes {
        assert_eq!(sqlx::query("INSERT INTO role_permissions(role_id,permission_id) SELECT $1,id FROM permissions WHERE code=$2").bind(role).bind(code).execute(pool).await.unwrap().rows_affected(), 1);
    }
    sqlx::query("INSERT INTO user_roles(user_id,role_id) VALUES($1,$2)").bind(actor).bind(role).execute(pool).await.unwrap();
}

async fn event(socket: &mut Socket, kind: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let message = socket.next().await.expect("socket stays open").expect("socket frame");
            match message {
                Message::Text(wire) => {
                    let value: Value = serde_json::from_str(&wire).unwrap();
                    assert_ne!(value["type"], "DraftReconcileRequired", "unexpected reconciliation: {value}");
                    if value["type"] == kind { return value; }
                }
                Message::Ping(payload) => socket.send(Message::Pong(payload)).await.unwrap(),
                Message::Close(close) => panic!("socket closed before {kind}: {close:?}"),
                _ => {}
            }
        }
    }).await.unwrap_or_else(|_| panic!("timed out waiting for {kind}"))
}

async fn command(socket: &mut Socket, kind: &str, payload: Value) -> Value {
    socket.send(Message::Text(json!({"type":kind,"payload":payload}).to_string().into())).await.unwrap();
    event(socket, "DraftCommandResult").await
}

fn uuid(value: &Value) -> Uuid { Uuid::parse_str(value.as_str().unwrap()).unwrap() }

#[tokio::test]
async fn actual_http_cannot_bypass_tab_lease_and_committed_delta_carries_db_revision() {
    let fixture = Fixture::new("collaboration_http_socket_authority").await;
    assert!(Arc::strong_count(&fixture.manager.collaboration()) >= 3, "HTTP/socket use the same live authority");
    let (mut first, initial) = fixture.socket().await;
    let (mut second, second_initial) = fixture.socket().await;
    let connection = uuid(&initial["payload"]["connectionId"]);
    let other = uuid(&second_initial["payload"]["connectionId"]);
    assert_ne!(connection, other, "same user's two tabs own different leases");
    assert_eq!(initial["payload"]["workspace"]["blocks"].as_array().unwrap().len(), 2);
    let claim = command(&mut first, "ClientClaim", json!({"requestId":Uuid::new_v4(),"source":fixture.source(),"purpose":"edit"})).await;
    assert_eq!(claim["payload"]["accepted"], true, "{claim}");
    let token = uuid(&claim["payload"]["leaseToken"]);
    for proof in [None, Some((connection, Uuid::new_v4())), Some((other, token))] {
        let response = fixture.update(proof, "must be refused").send().await.unwrap();
        assert!([403,409].contains(&response.status().as_u16()), "unguarded HTTP request: {} {}", response.status(), response.text().await.unwrap());
    }
    let response = fixture.update(Some((connection, token)), "accepted own-tab save").send().await.unwrap();
    assert_eq!(response.status(), 200, "{}", response.text().await.unwrap());
    let own = event(&mut first, "DraftPatched").await;
    let peer = event(&mut second, "DraftPatched").await;
    assert_eq!(own["seq"], peer["seq"]);
    let patch = &peer["payload"];
    assert_eq!(patch["upsertedBlocks"].as_array().unwrap().len(), 1, "unchanged block is not echoed");
    assert_eq!(patch["upsertedBlocks"][0]["id"], fixture.block.id.to_string());
    assert_eq!(patch["upsertedBlocks"][0]["note"], "accepted own-tab save");
    let revision: i64 = sqlx::query_scalar("SELECT row_version FROM academic_timetable_versions WHERE id=$1").bind(fixture.version).fetch_one(&fixture.auth.tenant_a_pool).await.unwrap();
    assert_eq!(patch["version"]["rowVersion"], revision);
    assert!(revision > initial["payload"]["workspace"]["version"]["rowVersion"].as_i64().unwrap());
    let effective_from: chrono::NaiveDate = sqlx::query_scalar("SELECT effective_from FROM academic_delivery_versions WHERE id=(SELECT delivery_version_id FROM academic_timetable_versions WHERE id=$1)")
        .bind(fixture.version).fetch_one(&fixture.auth.tenant_a_pool).await.unwrap();
    let preview = reqwest::Client::new().post(format!("http://{}/api/academic/timetable-versions/{}/publication-preview", fixture.address, fixture.version))
        .header("origin", "http://localhost:5173").header("x-school-subdomain", "tenant-a")
        .header("cookie", &fixture.cookie).header("x-csrf-token", &fixture.csrf)
        .json(&json!({"rowVersion": revision, "effectiveFrom": effective_from})).send().await.unwrap();
    assert_eq!(preview.status(), 200, "publication uses patch revision without workspace GET: {}", preview.text().await.unwrap());
    let (mut joined, latest) = fixture.socket().await;
    assert_eq!(latest["seq"], peer["seq"]);
    assert_eq!(latest["payload"]["workspace"]["version"]["rowVersion"], revision);
    joined.close(None).await.unwrap();
    first.close(None).await.unwrap();
    second.close(None).await.unwrap();
}

#[tokio::test]
async fn warm_native_socket_presence_and_lease_frames_work_after_pool_is_closed() {
    let fixture = Fixture::new("collaboration_zero_db_hot_frames").await;
    let (mut socket, sync) = fixture.socket().await;
    let claim = command(&mut socket, "ClientClaim", json!({"requestId":Uuid::new_v4(),"source":fixture.source(),"purpose":"edit"})).await;
    assert_eq!(claim["payload"]["accepted"], true, "{claim}");
    let token = uuid(&claim["payload"]["leaseToken"]);
    fixture.auth.tenant_a_pool.close().await;
    for index in 0..8 {
        socket.send(Message::Text(json!({"type":"ClientPresenceUpdate", "payload": {
            "context":{"viewMode":"wholeSchool","viewId":null,"dayOfWeek":"MON"},
            "anchor":{"kind":"block","blockId":fixture.block.id,"u":f64::from(index)/8.0,"v":0.5}
        }}).to_string().into())).await.unwrap();
        let presence = event(&mut socket, "DraftPresenceChanged").await;
        assert_eq!(presence["payload"]["connections"][0]["connectionId"], sync["payload"]["connectionId"]);
        let renewed = command(&mut socket, "ClientRenew", json!({"requestId":Uuid::new_v4(),"leaseToken":token})).await;
        assert_eq!(renewed["payload"]["accepted"], true, "renew must not checkout PG");
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    let targeted = command(&mut socket, "ClientTarget", json!({"requestId":Uuid::new_v4(),"leaseToken":token,"target":{"dayOfWeek":"WED","bellSchedulePeriodId":fixture.block.bell_schedule_period_id,"blockId":null}})).await;
    assert_eq!(targeted["payload"]["accepted"], true, "target must not checkout PG");
    let released = command(&mut socket, "ClientRelease", json!({"requestId":Uuid::new_v4(),"leaseToken":token})).await;
    assert_eq!(released["payload"]["accepted"], true);
    let stale = command(&mut socket, "ClientRenew", json!({"requestId":Uuid::new_v4(),"leaseToken":token})).await;
    assert_eq!(stale["payload"]["accepted"], false, "stale private commands do not close transport");
    assert!(stale["payload"]["leaseToken"].is_null());
    socket.close(None).await.unwrap();
}

#[tokio::test]
async fn native_socket_scoped_manager_cannot_claim_school_owned_structural_block() {
    let fixture = Fixture::new("collaboration_scoped_structural_denial").await;
    let actor = fixture.auth.insert_user(&fixture.auth.tenant_a_pool, "scoped-timetable", "active").await;
    grant(&fixture.auth.tenant_a_pool, actor, &["academic_timetable.read.school", "academic_timetable.manage.assigned"]).await;
    let (_, token) = fixture.auth.insert_session(&fixture.auth.tenant_a_pool, actor, 192, false).await;
    let cookie = format!("__Host-schoolorbit_session={}", token.encode().expose_for_cookie());
    let (mut socket, sync) = fixture.socket_with_cookie(&cookie).await;
    assert_eq!(sync["payload"]["workspace"]["blocks"].as_array().unwrap().len(), 2, "read scope includes block");
    let denied = command(&mut socket, "ClientClaim", json!({"requestId":Uuid::new_v4(),"source":fixture.source(),"purpose":"edit"})).await;
    assert_eq!(denied["payload"]["accepted"], false);
    assert_eq!(denied["payload"]["code"], "forbidden");
    assert!(denied["payload"]["source"].is_null());
    socket.close(None).await.unwrap();
}
