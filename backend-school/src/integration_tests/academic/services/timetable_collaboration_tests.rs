//! Real session middleware, native socket transport and HTTP share one domain authority.
use std::{sync::Arc, time::Duration};

use axum::{middleware::from_fn_with_state, routing::get, Router};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};
use uuid::Uuid;

use crate::integration_tests::auth::session_http_tests::protected_router::ProtectedFixture;
use school_academic_http::{
    state::{AcademicHttpState, AcademicWebSocketState},
    websockets::{timetable_websocket_handler, WebSocketManager},
};
use school_academic_timetable::{
    collaboration::TimetableMutationContext, models::timetable_block::*,
    services::timetable_block_service,
};

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
    session_id: Uuid,
    version: Uuid,
    term: Uuid,
    block: TimetableBlock,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

impl Fixture {
    async fn new(name: &str) -> Self {
        let pool = migrated_pool(name).await;
        let actor = Uuid::parse_str(ACTOR).unwrap();
        let (version, term, _, bell) = draft_version(&pool).await;
        let period: Uuid = sqlx::query_scalar("SELECT id FROM bell_schedule_periods WHERE bell_schedule_id=$1 AND is_active ORDER BY order_index LIMIT 1").bind(bell).fetch_one(&pool).await.unwrap();
        let blocks = timetable_block_service::create_structural_blocks(
            &pool,
            &TimetableMutationContext::for_tests(&pool, actor),
            CreateStructuralTimetableBlocksRequest {
                timetable_version_id: version,
                academic_term_id: term,
                structural_kind: TimetableStructuralKind::FlagCeremony,
                title: "Shared authority".into(),
                note: None,
                slots: ["MON", "TUE"]
                    .into_iter()
                    .map(|day| TimetableStructuralSlotInput {
                        day_of_week: day.into(),
                        bell_schedule_period_id: period,
                    })
                    .collect(),
                homeroom_ids: vec![],
                teacher_ids: vec![],
                all_homerooms: true,
                all_teachers: false,
                room_id: None,
            },
        )
        .await
        .unwrap();
        let auth = ProtectedFixture::with_prepared_tenant_pool(name, pool).await;
        grant(
            &auth.tenant_a_pool,
            actor,
            &[
                "academic_timetable.read.school",
                "academic_timetable.manage.school",
                "academic_timetable.publish.school",
                "facility.create.all",
                "facility.update.all",
                "facility.delete.all",
                "staff.create.all",
            ],
        )
        .await;
        let (session, token) = auth
            .insert_session(&auth.tenant_a_pool, actor, 191, false)
            .await;
        let cookie = format!(
            "__Host-schoolorbit_session={}",
            token.encode().expose_for_cookie()
        );
        let csrf = auth.csrf(session);
        let manager = Arc::new(WebSocketManager::new());
        manager
            .clone()
            .spawn_authority_invalidation_task(auth.runtime.permission_events.subscribe());
        let http = AcademicHttpState {
            collaboration: manager.collaboration(),
            staff_directory: &crate::adapters::staff_directory::STAFF_DIRECTORY,
            placement_rosters: &crate::http_state::PLACEMENT_ROSTERS,
            auth_runtime: auth.runtime.clone(),
            websocket_manager: manager.clone(),
            result_locks: &crate::adapters::academic_result_locks::RESULT_LOCKS,
            lifecycle_providers: &crate::adapters::academic_lifecycle::LIFECYCLE_PROVIDERS,
        };
        let ws = AcademicWebSocketState {
            collaboration: manager.collaboration(),
            staff_directory: http.staff_directory,
            auth_runtime: auth.runtime.clone(),
            websocket_manager: manager.clone(),
        };
        let files =
            Arc::new(crate::integration_tests::staff::career_integration_tests::file_platform());
        let administration = school_administration::http::state::AdministrationHttpState {
            auth_runtime: auth.runtime.clone(),
            file_platform: files.clone(),
            facility_changes: Arc::new(
                crate::adapters::facility_changes::ApplicationFacilityChanges(manager.clone()),
            ),
        };
        let staff = school_staff::http::state::StaffHttpState {
            auth_runtime: auth.runtime.clone(),
            file_platform: files,
        };
        let app = Router::new()
            .nest(
                "/api/academic",
                school_academic_http::routes::academic_routes().with_state(http),
            )
            .nest(
                "/api/facilities",
                school_administration::facility::http::routes::facility_routes()
                    .with_state(administration),
            )
            .nest(
                "/api",
                school_staff::http::routes::protected_routes().with_state(staff),
            )
            .route_layer(from_fn_with_state(
                auth.runtime.clone(),
                crate::middleware::session::session_middleware,
            ))
            .merge(
                Router::new()
                    .route("/ws/timetable", get(timetable_websocket_handler))
                    .with_state(ws),
            );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            auth,
            manager,
            server,
            address,
            cookie,
            csrf,
            session_id: session,
            version,
            term,
            block: blocks[0].clone(),
        }
    }

    async fn socket(&self) -> (Socket, Value) {
        self.socket_with_cookie(&self.cookie).await
    }

    async fn socket_with_cookie(&self, cookie: &str) -> (Socket, Value) {
        let mut request = format!("ws://{}/ws/timetable?academicTermId={}&timetableVersionId={}&school_subdomain=tenant-a", self.address, self.term, self.version).into_client_request().unwrap();
        request
            .headers_mut()
            .insert("origin", "http://localhost:5173".parse().unwrap());
        request
            .headers_mut()
            .insert("cookie", cookie.parse().unwrap());
        let (mut socket, _) = connect_async(request)
            .await
            .expect("real authenticated socket upgrade");
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
            request = request
                .header("x-timetable-connection-id", connection.to_string())
                .header("x-timetable-lease-token", token.to_string())
                .header("x-timetable-mutation-id", Uuid::new_v4().to_string());
        }
        request
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        reqwest::Client::new()
            .request(method, format!("http://{}{}", self.address, path))
            .header("origin", "http://localhost:5173")
            .header("x-school-subdomain", "tenant-a")
            .header("cookie", &self.cookie)
            .header("x-csrf-token", &self.csrf)
    }

    fn source(&self) -> Value {
        json!({"kind": "existing_block", "blockId": self.block.id, "rowVersion": self.block.row_version})
    }

    fn room(&self) -> Arc<school_academic_timetable::collaboration::DraftRoom> {
        self.manager
            .collaboration()
            .room(school_academic_timetable::collaboration::DraftKey {
                tenant_id: self.auth.tenant_a_id,
                academic_term_id: self.term,
                timetable_version_id: self.version,
            })
            .unwrap()
            .expect("socket installed the shared room")
    }

    async fn mutation(&self, proof: Option<(Uuid, Uuid)>) -> TimetableMutationContext {
        // This fixture operation drives transaction phases directly; the HTTP tests above and
        // queued-generation test exercise real central admission. Crucially it injects exactly
        // the authority already used by the native socket and HTTP router.
        let mut context = TimetableMutationContext::for_tests(
            &self.auth.tenant_a_pool,
            Uuid::parse_str(ACTOR).unwrap(),
        );
        let tenant = school_auth_http::tenant::resolve_tenant_context_by_subdomain(
            &self.auth.runtime,
            "tenant-a",
        )
        .await
        .unwrap();
        context.pool_lease = tenant.pool;
        context.runtime = self.manager.collaboration();
        context.tenant_id = self.auth.tenant_a_id;
        context.session_id = self.session_id;
        context.staff_directory = &crate::adapters::staff_directory::STAFF_DIRECTORY;
        context.identity = proof.map(|(connection_id, lease_token)| {
            school_academic_timetable::models::collaboration::DraftMutationIdentity {
                connection_id,
                lease_token,
                mutation_id: Uuid::new_v4(),
            }
        });
        assert!(Arc::ptr_eq(&context.runtime, &self.manager.collaboration()));
        context
    }
}

async fn grant(pool: &sqlx::PgPool, actor: Uuid, codes: &[&str]) {
    let role: Uuid = sqlx::query_scalar("INSERT INTO roles(code,name,user_type) VALUES($1,'Collaboration test','staff') RETURNING id")
        .bind(format!("COLLAB_{}", Uuid::new_v4().simple())).fetch_one(pool).await.unwrap();
    for code in codes {
        assert_eq!(sqlx::query("INSERT INTO role_permissions(role_id,permission_id) SELECT $1,id FROM permissions WHERE code=$2").bind(role).bind(code).execute(pool).await.unwrap().rows_affected(), 1);
    }
    sqlx::query("INSERT INTO user_roles(user_id,role_id) VALUES($1,$2)")
        .bind(actor)
        .bind(role)
        .execute(pool)
        .await
        .unwrap();
}

async fn event(socket: &mut Socket, kind: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let message = socket
                .next()
                .await
                .expect("socket stays open")
                .expect("socket frame");
            match message {
                Message::Text(wire) => {
                    let value: Value = serde_json::from_str(&wire).unwrap();
                    if kind != "DraftReconcileRequired" {
                        assert_ne!(
                            value["type"], "DraftReconcileRequired",
                            "unexpected reconciliation: {value}"
                        );
                    }
                    if value["type"] == kind {
                        return value;
                    }
                }
                Message::Ping(payload) => socket.send(Message::Pong(payload)).await.unwrap(),
                Message::Close(close) => panic!("socket closed before {kind}: {close:?}"),
                _ => {}
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {kind}"))
}

async fn command(socket: &mut Socket, kind: &str, payload: Value) -> Value {
    socket
        .send(Message::Text(
            json!({"type":kind,"payload":payload}).to_string().into(),
        ))
        .await
        .unwrap();
    event(socket, "DraftCommandResult").await
}

fn uuid(value: &Value) -> Uuid {
    Uuid::parse_str(value.as_str().unwrap()).unwrap()
}

fn reconstructed_lookups(before: &Value, changes: &Value) -> Value {
    let mut result = before.clone();
    for (field, upsert, removed, order, key) in [
        (
            "staff",
            "upsertedStaff",
            "removedStaffIds",
            "staffOrder",
            "id",
        ),
        (
            "homerooms",
            "upsertedHomerooms",
            "removedHomeroomIds",
            "homeroomOrder",
            "id",
        ),
        (
            "rooms",
            "upsertedRooms",
            "removedRoomIds",
            "roomOrder",
            "id",
        ),
        (
            "bellPeriods",
            "upsertedBellPeriods",
            "removedBellPeriodIds",
            "bellPeriodOrder",
            "id",
        ),
        (
            "offeringSubjectGroups",
            "upsertedOfferingSubjectGroups",
            "removedSubjectGroupOfferingIds",
            "subjectGroupOfferingOrder",
            "learningOfferingId",
        ),
    ] {
        if changes[order].is_null() {
            assert!(
                changes[upsert].as_array().unwrap().is_empty()
                    && changes[removed].as_array().unwrap().is_empty()
            );
            continue;
        }
        let mut rows = result[field]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| (row[key].as_str().unwrap().to_owned(), row.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();
        for id in changes[removed].as_array().unwrap() {
            rows.remove(id.as_str().unwrap());
        }
        for row in changes[upsert].as_array().unwrap() {
            rows.insert(row[key].as_str().unwrap().to_owned(), row.clone());
        }
        let ordered = changes[order]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| {
                rows.remove(id.as_str().unwrap())
                    .expect("every AFTER order ID has metadata")
            })
            .collect::<Vec<_>>();
        assert!(
            rows.is_empty(),
            "order must describe the complete authorized AFTER list"
        );
        result[field] = Value::Array(ordered);
    }
    result
}

#[tokio::test]
async fn actual_http_cannot_bypass_tab_lease_and_committed_delta_carries_db_revision() {
    let fixture = Fixture::new("collaboration_http_socket_authority").await;
    sqlx::query("UPDATE auth_sessions SET rotated_at=now()-interval '20 minutes' WHERE id=$1")
        .bind(fixture.session_id)
        .execute(&fixture.auth.tenant_a_pool)
        .await
        .unwrap();
    let session_metadata = || async {
        sqlx::query_scalar::<_, Value>("SELECT jsonb_build_object('idle',idle_expires_at,'absolute',absolute_expires_at,'rotation',rotated_at,'lastSeen',last_seen_at,'pending',pending_token_hash IS NOT NULL) FROM auth_sessions WHERE id=$1")
            .bind(fixture.session_id).fetch_one(&fixture.auth.tenant_a_pool).await.unwrap()
    };
    let before_session = session_metadata().await;
    assert!(
        Arc::strong_count(&fixture.manager.collaboration()) >= 3,
        "HTTP/socket use the same live authority"
    );
    let (mut first, initial) = fixture.socket().await;
    let (mut second, second_initial) = fixture.socket().await;
    let connection = uuid(&initial["payload"]["connectionId"]);
    let other = uuid(&second_initial["payload"]["connectionId"]);
    assert_ne!(
        connection, other,
        "same user's two tabs own different leases"
    );
    assert_eq!(
        initial["payload"]["workspace"]["blocks"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let claim = command(
        &mut first,
        "ClientClaim",
        json!({"requestId":Uuid::new_v4(),"source":fixture.source(),"purpose":"edit"}),
    )
    .await;
    assert_eq!(claim["payload"]["accepted"], true, "{claim}");
    let token = uuid(&claim["payload"]["leaseToken"]);
    for proof in [
        None,
        Some((connection, Uuid::new_v4())),
        Some((other, token)),
    ] {
        let response = fixture
            .update(proof, "must be refused")
            .send()
            .await
            .unwrap();
        assert!(
            [403, 409].contains(&response.status().as_u16()),
            "unguarded HTTP request: {} {}",
            response.status(),
            response.text().await.unwrap()
        );
    }
    let response = fixture
        .update(Some((connection, token)), "accepted own-tab save")
        .send()
        .await
        .unwrap();
    assert!(
        response.headers().get("set-cookie").is_none(),
        "ordinary timetable save never stages rotation"
    );
    assert_eq!(response.status(), 200, "{}", response.text().await.unwrap());
    assert_eq!(
        session_metadata().await,
        before_session,
        "HTTP/socket collaboration traffic does not own human idle or periodic rotation"
    );
    let own = event(&mut first, "DraftPatched").await;
    let peer = event(&mut second, "DraftPatched").await;
    assert_eq!(own["seq"], peer["seq"]);
    let patch = &peer["payload"];
    assert_eq!(
        patch["upsertedBlocks"].as_array().unwrap().len(),
        1,
        "unchanged block is not echoed"
    );
    assert_eq!(
        patch["upsertedBlocks"][0]["id"],
        fixture.block.id.to_string()
    );
    assert_eq!(patch["upsertedBlocks"][0]["note"], "accepted own-tab save");
    let revision: i64 =
        sqlx::query_scalar("SELECT row_version FROM academic_timetable_versions WHERE id=$1")
            .bind(fixture.version)
            .fetch_one(&fixture.auth.tenant_a_pool)
            .await
            .unwrap();
    assert_eq!(patch["version"]["rowVersion"], revision);
    assert!(
        revision
            > initial["payload"]["workspace"]["version"]["rowVersion"]
                .as_i64()
                .unwrap()
    );
    let effective_from: chrono::NaiveDate = sqlx::query_scalar("SELECT effective_from FROM academic_delivery_versions WHERE id=(SELECT delivery_version_id FROM academic_timetable_versions WHERE id=$1)")
        .bind(fixture.version).fetch_one(&fixture.auth.tenant_a_pool).await.unwrap();
    let preview = reqwest::Client::new()
        .post(format!(
            "http://{}/api/academic/timetable-versions/{}/publication-preview",
            fixture.address, fixture.version
        ))
        .header("origin", "http://localhost:5173")
        .header("x-school-subdomain", "tenant-a")
        .header("cookie", &fixture.cookie)
        .header("x-csrf-token", &fixture.csrf)
        .json(&json!({"rowVersion": revision, "effectiveFrom": effective_from}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        preview.status(),
        200,
        "publication uses patch revision without workspace GET: {}",
        preview.text().await.unwrap()
    );
    let (mut joined, latest) = fixture.socket().await;
    assert_eq!(latest["seq"], peer["seq"]);
    assert_eq!(
        latest["payload"]["workspace"]["version"]["rowVersion"],
        revision
    );
    joined.close(None).await.unwrap();
    first.close(None).await.unwrap();
    second.close(None).await.unwrap();
}

#[tokio::test]
async fn warm_native_socket_presence_and_lease_frames_work_after_pool_is_closed() {
    let fixture = Fixture::new("collaboration_zero_db_hot_frames").await;
    let (mut socket, sync) = fixture.socket().await;
    let claim = command(
        &mut socket,
        "ClientClaim",
        json!({"requestId":Uuid::new_v4(),"source":fixture.source(),"purpose":"edit"}),
    )
    .await;
    assert_eq!(claim["payload"]["accepted"], true, "{claim}");
    let token = uuid(&claim["payload"]["leaseToken"]);
    fixture.auth.tenant_a_pool.close().await;
    for index in 0..8 {
        socket.send(Message::Text(json!({"type":"ClientPresenceUpdate", "payload": {
            "context":{"viewMode":"wholeSchool","viewId":null,"dayOfWeek":"MON"},
            "anchor":{"kind":"block","blockId":fixture.block.id,"u":f64::from(index)/8.0,"v":0.5}
        }}).to_string().into())).await.unwrap();
        let presence = event(&mut socket, "DraftPresenceChanged").await;
        assert_eq!(
            presence["payload"]["connections"][0]["connectionId"],
            sync["payload"]["connectionId"]
        );
        let renewed = command(
            &mut socket,
            "ClientRenew",
            json!({"requestId":Uuid::new_v4(),"leaseToken":token}),
        )
        .await;
        assert_eq!(
            renewed["payload"]["accepted"], true,
            "renew must not checkout PG"
        );
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    let targeted = command(&mut socket, "ClientTarget", json!({"requestId":Uuid::new_v4(),"leaseToken":token,"target":{"dayOfWeek":"WED","bellSchedulePeriodId":fixture.block.bell_schedule_period_id,"blockId":null}})).await;
    assert_eq!(
        targeted["payload"]["accepted"], true,
        "target must not checkout PG"
    );
    let released = command(
        &mut socket,
        "ClientRelease",
        json!({"requestId":Uuid::new_v4(),"leaseToken":token}),
    )
    .await;
    assert_eq!(released["payload"]["accepted"], true);
    let stale = command(
        &mut socket,
        "ClientRenew",
        json!({"requestId":Uuid::new_v4(),"leaseToken":token}),
    )
    .await;
    assert_eq!(
        stale["payload"]["accepted"], false,
        "stale private commands do not close transport"
    );
    assert!(stale["payload"]["leaseToken"].is_null());
    let again = command(
        &mut socket,
        "ClientClaim",
        json!({"requestId":Uuid::new_v4(),"source":fixture.source(),"purpose":"edit"}),
    )
    .await;
    assert_eq!(
        again["payload"]["accepted"], true,
        "fresh claim after closing PG is memory-only"
    );
    socket
        .send(Message::Text(
            json!({"type":"ClientReplay","payload":{"epoch":sync["epoch"],"afterSeq":sync["seq"]}})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let after_replay = command(
        &mut socket,
        "ClientRenew",
        json!({"requestId":Uuid::new_v4(),"leaseToken":again["payload"]["leaseToken"]}),
    )
    .await;
    assert_eq!(
        after_replay["payload"]["accepted"], true,
        "replay then renewal work after closing PG"
    );
    socket.close(None).await.unwrap();
}

#[tokio::test]
async fn native_socket_scoped_manager_cannot_claim_school_owned_structural_block() {
    let fixture = Fixture::new("collaboration_scoped_structural_denial").await;
    let actor = fixture
        .auth
        .insert_user(&fixture.auth.tenant_a_pool, "scoped-timetable", "active")
        .await;
    grant(
        &fixture.auth.tenant_a_pool,
        actor,
        &[
            "academic_timetable.read.school",
            "academic_timetable.manage.assigned",
        ],
    )
    .await;
    let (session, token) = fixture
        .auth
        .insert_session(&fixture.auth.tenant_a_pool, actor, 192, false)
        .await;
    let cookie = format!(
        "__Host-schoolorbit_session={}",
        token.encode().expose_for_cookie()
    );
    let (mut socket, sync) = fixture.socket_with_cookie(&cookie).await;
    assert_eq!(
        sync["payload"]["workspace"]["blocks"]
            .as_array()
            .unwrap()
            .len(),
        2,
        "read scope includes block"
    );
    let denied = command(
        &mut socket,
        "ClientClaim",
        json!({"requestId":Uuid::new_v4(),"source":fixture.source(),"purpose":"edit"}),
    )
    .await;
    assert_eq!(denied["payload"]["accepted"], false);
    assert_eq!(denied["payload"]["code"], "forbidden");
    assert!(denied["payload"]["source"].is_null());
    // Existing per-action policy allows explicit own-target creation, while all-target flags
    // and another teacher's resource remain forbidden. Claim/edit semantics are unchanged.
    for (selected_teacher, all_teachers, expected) in [
        (actor, false, 200),
        (Uuid::parse_str(ACTOR).unwrap(), false, 403),
        (actor, true, 403),
    ] {
        let payload = json!({"timetableVersionId":fixture.version,"academicTermId":fixture.term,
            "structuralKind":"teacher_meeting","title":"Own target scoped creation",
            "slots":[{"dayOfWeek":"WED","bellSchedulePeriodId":fixture.block.bell_schedule_period_id}],
            "homeroomIds":[],"teacherIds":[selected_teacher],"allHomerooms":false,"allTeachers":all_teachers,"roomId":null});
        let mut request = fixture
            .request(
                reqwest::Method::POST,
                "/api/academic/timetable-blocks/structural",
            )
            .json(&payload)
            .build()
            .unwrap();
        request
            .headers_mut()
            .insert("cookie", cookie.parse().unwrap());
        request
            .headers_mut()
            .insert("x-csrf-token", fixture.auth.csrf(session).parse().unwrap());
        let response = reqwest::Client::new().execute(request).await.unwrap();
        assert_eq!(
            response.status().as_u16(),
            expected,
            "{}",
            response.text().await.unwrap()
        );
    }
    event(&mut socket, "DraftPatched").await;
    socket.close(None).await.unwrap();
}

/// Real PG version/block locks are held while the accepted owned task is paused before COMMIT.
async fn held_write(
    fixture: &Fixture,
    proof: Option<(Uuid, Uuid)>,
    note: &str,
) -> (
    JoinHandle<Result<(), school_errors::AppError>>,
    Arc<tokio::sync::Notify>,
) {
    let context = fixture.mutation(proof).await;
    let pool = fixture.auth.tenant_a_pool.clone();
    let mut block = fixture.block.clone();
    let version = fixture.version;
    let note = note.to_owned();
    let release = Arc::new(tokio::sync::Notify::new());
    let released = release.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let worker = tokio::spawn(async move {
        context.execute(&pool, version, school_academic_timetable::collaboration::MutationScope::Blocks(vec![block.id]), move |pool, permit| async move {
            let mut tx = school_tenancy::TenantPool::begin_observed(&pool, "test.collaboration.held_write").await?;
            sqlx::query("SELECT id FROM academic_timetable_versions WHERE id=$1 FOR UPDATE").bind(version).execute(&mut **tx).await?;
            block.row_version = sqlx::query_scalar("UPDATE academic_timetable_blocks SET note=$2,row_version=row_version+1 WHERE id=$1 RETURNING row_version")
                .bind(block.id).bind(&note).fetch_one(&mut **tx).await?;
            block.note = Some(note);
            started.send(()).unwrap();
            released.notified().await;
            permit.commit(tx).await?;
            Ok(((), vec![block]))
        }).await
    });
    if tokio::time::timeout(Duration::from_secs(5), ready)
        .await
        .unwrap()
        .is_err()
    {
        panic!(
            "held PG writer failed before pause: {:?}",
            worker.await.unwrap()
        );
    }
    (worker, release)
}

#[tokio::test]
async fn accepted_pg_write_keeps_public_exclusion_after_caller_abort_disconnect_and_ttl() {
    use school_academic_timetable::{
        collaboration::{DraftSignal, MutationScope, LEASE_TTL},
        models::collaboration::DraftServerMessage,
    };
    let fixture = Fixture::new("collaboration_pg_pinned_cancel").await;
    let (mut owner, sync) = fixture.socket().await;
    let (mut peer, peer_sync) = fixture.socket().await;
    let connection = uuid(&sync["payload"]["connectionId"]);
    let peer_connection = uuid(&peer_sync["payload"]["connectionId"]);
    let claim = command(
        &mut owner,
        "ClientClaim",
        json!({"requestId":Uuid::new_v4(),"source":fixture.source(),"purpose":"edit"}),
    )
    .await;
    let token = uuid(&claim["payload"]["leaseToken"]);
    sqlx::raw_sql("CREATE FUNCTION test_collaboration_hold_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.note='survives caller cancellation' THEN PERFORM pg_advisory_xact_lock(813571); END IF; RETURN NEW; END $$; CREATE CONSTRAINT TRIGGER test_collaboration_hold_commit AFTER UPDATE ON academic_timetable_blocks DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION test_collaboration_hold_commit();")
        .execute(&fixture.auth.tenant_a_pool).await.unwrap();
    let mut barrier = fixture.auth.tenant_a_pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(813571)")
        .execute(&mut *barrier)
        .await
        .unwrap();
    let request = fixture.update(Some((connection, token)), "survives caller cancellation");
    let caller = tokio::spawn(async move { request.send().await });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let blocked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE wait_event='advisory' AND query='COMMIT')").fetch_one(&fixture.auth.tenant_a_pool).await.unwrap();
            if blocked { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("real authenticated HTTP writer entered actual PG COMMIT");
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    owner.close(None).await.unwrap();
    // An actual database transaction stays open beyond the interaction lease's real deadline.
    tokio::time::sleep(LEASE_TTL + Duration::from_millis(30)).await;
    fixture.manager.collaboration().expire();
    let room = fixture.room();
    let reservations = room
        .signal_events(peer_connection, DraftSignal::Reservations)
        .unwrap();
    let DraftServerMessage::DraftReservationsChanged { reservations } = &reservations[0].event
    else {
        panic!("reservations")
    };
    assert_eq!(reservations.len(), 1);
    assert_eq!(
        reservations[0].reservation_id.to_string(),
        claim["payload"]["reservationId"]
    );
    assert!(room
        .begin_commit(
            Uuid::parse_str(ACTOR).unwrap(),
            fixture.session_id,
            None,
            MutationScope::Blocks(vec![fixture.block.id])
        )
        .is_err());
    barrier.commit().await.unwrap();
    let patch = event(&mut peer, "DraftPatched").await;
    assert_eq!(
        patch["payload"]["upsertedBlocks"][0]["note"],
        "survives caller cancellation"
    );
    let stored: String =
        sqlx::query_scalar("SELECT note FROM academic_timetable_blocks WHERE id=$1")
            .bind(fixture.block.id)
            .fetch_one(&fixture.auth.tenant_a_pool)
            .await
            .unwrap();
    assert_eq!(stored, "survives caller cancellation");
    peer.close(None).await.unwrap();
}

#[tokio::test]
async fn queued_http_admission_rejects_same_permission_generation_change() {
    let fixture = Fixture::new("collaboration_queued_generation").await;
    let (mut socket, _) = fixture.socket().await;
    let room = fixture.room();
    let gate = room.mutation_gate().await;
    let request = fixture.update(None, "stale queued authorization");
    let worker = tokio::spawn(async move { request.send().await.unwrap() });
    tokio::time::timeout(Duration::from_secs(5), async {
        while fixture.manager.collaboration().accepted_write_count() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("HTTP write reached shared admission behind held gate");
    // The permission strings stay identical; the owner's generation changes because resource
    // assignments may have changed. No notification is sent, so this specifically tests admission.
    fixture
        .auth
        .runtime
        .invalidate_permission_user("tenant-a", Uuid::parse_str(ACTOR).unwrap());
    drop(gate);
    let response = worker.await.unwrap();
    assert_eq!(response.status(), 403, "{}", response.text().await.unwrap());
    let note: Option<String> =
        sqlx::query_scalar("SELECT note FROM academic_timetable_blocks WHERE id=$1")
            .bind(fixture.block.id)
            .fetch_one(&fixture.auth.tenant_a_pool)
            .await
            .unwrap();
    assert!(note.is_none());
    socket.close(None).await.unwrap();
}

#[tokio::test]
async fn gated_native_join_ignores_committed_signal_already_in_state_sync() {
    let fixture = Fixture::new("collaboration_gated_join_watermark").await;
    let (mut existing, _) = fixture.socket().await;
    let room = fixture.room();
    let (worker, release) = held_write(&fixture, None, "committed while joining").await;
    let mut request = format!(
        "ws://{}/ws/timetable?academicTermId={}&timetableVersionId={}&school_subdomain=tenant-a",
        fixture.address, fixture.term, fixture.version
    )
    .into_client_request()
    .unwrap();
    request
        .headers_mut()
        .insert("origin", "http://localhost:5173".parse().unwrap());
    request
        .headers_mut()
        .insert("cookie", fixture.cookie.parse().unwrap());
    let (mut joining, _) = connect_async(request).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while room.signal_subscriber_count() < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    release.notify_one();
    worker.await.unwrap().unwrap();
    let sync = event(&mut joining, "DraftStateSync").await;
    let patch = event(&mut existing, "DraftPatched").await;
    assert_eq!(sync["seq"], patch["seq"]);
    assert!(sync["payload"]["workspace"]["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|block| block["note"] == "committed while joining"));
    // A queued Committed(snapshot.seq) has no projected history for this new tab. It must be
    // skipped, while subsequent frames still work, rather than causing a false journal gap.
    let reply = command(
        &mut joining,
        "ClientRenew",
        json!({"requestId":Uuid::new_v4(),"leaseToken":Uuid::new_v4()}),
    )
    .await;
    assert_eq!(reply["payload"]["accepted"], false);
    existing.close(None).await.unwrap();
    joining.close(None).await.unwrap();
}

#[tokio::test]
async fn upstream_change_invalidates_warm_room_without_members_before_next_join() {
    let fixture = Fixture::new("collaboration_no_member_upstream").await;
    let (mut socket, before) = fixture.socket().await;
    socket.close(None).await.unwrap();
    let room = fixture.room();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !room.members().unwrap().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    sqlx::query("UPDATE academic_timetable_blocks SET note='upstream fixture update' WHERE id=$1")
        .bind(fixture.block.id)
        .execute(&fixture.auth.tenant_a_pool)
        .await
        .unwrap();
    fixture.manager.broadcast_academic_core_changed(
        "tenant-a".into(),
        Uuid::parse_str(ACTOR).unwrap(),
        "resource",
        None,
        None,
        None,
    );
    let (mut joined, after) = fixture.socket().await;
    assert_ne!(before["epoch"], after["epoch"]);
    assert!(after["payload"]["workspace"]["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|block| block["note"] == "upstream fixture update"));
    joined.close(None).await.unwrap();
}

#[tokio::test]
async fn real_pg_commit_followed_by_publication_failure_reconciles_and_cold_read_fences_late_writer(
) {
    use school_academic_timetable::collaboration::MutationScope;
    let fixture = Fixture::new("collaboration_commit_failure_cold_fence").await;
    let (mut socket, original) = fixture.socket().await;
    let old_room = fixture.room();
    let context = fixture.mutation(None).await;
    let block_id = fixture.block.id;
    let version = fixture.version;
    let rollback: Result<(), school_errors::AppError> = context.execute(&fixture.auth.tenant_a_pool, version, MutationScope::Blocks(vec![block_id]), move |pool, _permit| async move {
        let mut tx = school_tenancy::TenantPool::begin_observed(&pool, "test.collaboration.known_rollback").await?;
        sqlx::query("UPDATE academic_timetable_blocks SET note='rolled back before COMMIT' WHERE id=$1").bind(block_id).execute(&mut **tx).await?;
        Err(school_errors::AppError::ValidationError("test rollback before COMMIT".into()))
    }).await;
    assert!(matches!(
        rollback,
        Err(school_errors::AppError::ValidationError(_))
    ));
    let unchanged = command(
        &mut socket,
        "ClientRenew",
        json!({"requestId":Uuid::new_v4(),"leaseToken":Uuid::new_v4()}),
    )
    .await;
    assert_eq!(unchanged["epoch"], original["epoch"]);
    assert_eq!(
        unchanged["seq"], original["seq"],
        "known rollback emits no patch or reset"
    );
    let result: Result<(), school_errors::AppError> = context
        .execute(
            &fixture.auth.tenant_a_pool,
            version,
            MutationScope::Blocks(vec![block_id]),
            move |pool, permit| async move {
                let mut tx = school_tenancy::TenantPool::begin_observed(
                    &pool,
                    "test.collaboration.post_commit_failure",
                )
                .await?;
                sqlx::query("SELECT id FROM academic_timetable_versions WHERE id=$1 FOR UPDATE")
                    .bind(version)
                    .execute(&mut **tx)
                    .await?;
                sqlx::query(
                    "UPDATE academic_timetable_blocks SET note='actually committed' WHERE id=$1",
                )
                .bind(block_id)
                .execute(&mut **tx)
                .await?;
                permit.commit(tx).await?;
                Err(school_errors::AppError::InternalServerError(
                    "test failure after actual PG COMMIT".into(),
                ))
            },
        )
        .await;
    assert!(
        matches!(&result, Err(school_errors::AppError::InternalServerError(message)) if message.contains("after actual PG COMMIT")),
        "unexpected precommit outcome: {result:?}"
    );
    let reconcile = event(&mut socket, "DraftReconcileRequired").await;
    assert_eq!(reconcile["payload"]["reason"], "draft_invalidated");
    let committed: String =
        sqlx::query_scalar("SELECT note FROM academic_timetable_blocks WHERE id=$1")
            .bind(block_id)
            .fetch_one(&fixture.auth.tenant_a_pool)
            .await
            .unwrap();
    assert_eq!(committed, "actually committed");

    // Model the preceding uncertain write's still-resolving transaction with a real version row
    // lock. Cold loading must wait for this transaction, rather than reinstall a stale valid room.
    let mut late = fixture.auth.tenant_a_pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM academic_timetable_versions WHERE id=$1 FOR UPDATE")
        .bind(version)
        .execute(&mut *late)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_timetable_blocks SET note='late writer resolved' WHERE id=$1")
        .bind(block_id)
        .execute(&mut *late)
        .await
        .unwrap();
    let runtime = fixture.manager.collaboration();
    let pool = fixture.auth.tenant_a_pool.clone();
    let tenant = fixture.auth.tenant_a_id;
    let reload = tokio::spawn(async move {
        runtime
            .load_room(
                tenant,
                &pool,
                version,
                &crate::adapters::staff_directory::STAFF_DIRECTORY,
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let blocked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE wait_event_type='Lock' AND query LIKE '%academic_timetable_versions%FOR SHARE%')").fetch_one(&fixture.auth.tenant_a_pool).await.unwrap();
            if blocked { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("canonical cold read is fenced behind the actual version transaction");
    assert!(!reload.is_finished());
    late.commit().await.unwrap();
    let recovered = reload.await.unwrap().unwrap();
    assert!(!Arc::ptr_eq(&old_room, &recovered));
    let (mut joined, latest) = fixture.socket().await;
    assert_ne!(original["epoch"], latest["epoch"]);
    assert!(latest["payload"]["workspace"]["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|block| block["note"] == "late writer resolved"));
    joined.close(None).await.unwrap();
}

#[tokio::test]
async fn actual_pg_deferred_commit_timeout_invalidates_without_emitting_successful_patch() {
    use school_academic_timetable::collaboration::MutationScope;
    let fixture = Fixture::new("collaboration_commit_timeout").await;
    let (mut socket, _) = fixture.socket().await;
    // PostgreSQL runs this deferred trigger during COMMIT, not the UPDATE. Cancel the actual
    // COMMIT response future while PostgreSQL is still resolving the transaction.
    sqlx::raw_sql("CREATE FUNCTION test_collaboration_slow_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_sleep(0.5); RETURN NEW; END $$; CREATE CONSTRAINT TRIGGER test_collaboration_slow_commit AFTER UPDATE ON academic_timetable_blocks DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION test_collaboration_slow_commit();")
        .execute(&fixture.auth.tenant_a_pool).await.unwrap();
    let context = fixture.mutation(None).await;
    let block_id = fixture.block.id;
    let version = fixture.version;
    let result: Result<(), school_errors::AppError> = context.execute(&fixture.auth.tenant_a_pool, version, MutationScope::Blocks(vec![block_id]), move |pool, permit| async move {
        let mut tx = school_tenancy::TenantPool::begin_observed(&pool, "test.collaboration.commit_timeout").await?;
        sqlx::query("SELECT id FROM academic_timetable_versions WHERE id=$1 FOR UPDATE").bind(version).execute(&mut **tx).await?;
        sqlx::query("UPDATE academic_timetable_blocks SET note='must roll back at COMMIT' WHERE id=$1").bind(block_id).execute(&mut **tx).await?;
        match tokio::time::timeout(Duration::from_millis(100), permit.commit(tx)).await {
            Err(_) => Err(school_errors::AppError::ServiceUnavailable("test actual COMMIT response deadline".into())),
            Ok(result) => { result?; panic!("deferred COMMIT must still be pending at deadline"); }
        }
    }).await;
    assert!(
        matches!(&result, Err(school_errors::AppError::ServiceUnavailable(message)) if message.contains("COMMIT response deadline")),
        "{result:?}"
    );
    event(&mut socket, "DraftReconcileRequired").await;
    let (mut joined, snapshot) = fixture.socket().await;
    let note: Option<String> =
        sqlx::query_scalar("SELECT note FROM academic_timetable_blocks WHERE id=$1")
            .bind(block_id)
            .fetch_one(&fixture.auth.tenant_a_pool)
            .await
            .unwrap();
    let recovered = snapshot["payload"]["workspace"]["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|block| block["id"] == block_id.to_string())
        .unwrap();
    assert_eq!(
        recovered["note"],
        json!(note),
        "fenced cold recovery reflects the final uncertain transaction outcome"
    );
    let revision: i64 =
        sqlx::query_scalar("SELECT row_version FROM academic_timetable_versions WHERE id=$1")
            .bind(version)
            .fetch_one(&fixture.auth.tenant_a_pool)
            .await
            .unwrap();
    assert_eq!(
        snapshot["payload"]["workspace"]["version"]["rowVersion"],
        revision
    );
    joined.close(None).await.unwrap();
}

#[tokio::test]
async fn assigned_multi_instructor_block_is_visible_but_both_memory_and_http_deny_unmanageable_teacher(
) {
    let mut fixture = Fixture::new("collaboration_multi_instructor_scope").await;
    let pool = &fixture.auth.tenant_a_pool;
    let teacher = fixture
        .auth
        .insert_user(pool, "collab-assigned-teacher", "active")
        .await;
    let other = fixture
        .auth
        .insert_user(pool, "collab-other-teacher", "active")
        .await;
    let (offering, year, starts): (Uuid, Uuid, chrono::NaiveDate) = sqlx::query_as("SELECT id,academic_year_id,starts_on FROM learning_offerings WHERE academic_term_id=$1 AND kind='course' ORDER BY id LIMIT 1")
        .bind(fixture.term).fetch_one(pool).await.unwrap();
    let group = Uuid::new_v4();
    sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name,status,roster_status) VALUES($1,$2,$3,$4,$5,'Multi instructor collaboration','draft','draft')")
        .bind(group).bind(offering).bind(fixture.term).bind(year).bind(format!("COLLAB-{group}")).execute(pool).await.unwrap();
    for (id, role) in [(teacher, "primary"), (other, "secondary")] {
        sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role,starts_on,created_by,updated_by) VALUES(gen_random_uuid(),$1,$2,$3,$4,$5,$6,$7,$7)")
            .bind(group).bind(fixture.term).bind(year).bind(id).bind(role).bind(starts).bind(Uuid::parse_str(ACTOR).unwrap()).execute(pool).await.unwrap();
    }
    super::timetable_block_service_tests::publish_updated_opening_for_draft(pool, fixture.version)
        .await;
    fixture.block = timetable_block_service::create_ordinary_block(
        pool,
        &TimetableMutationContext::for_tests(pool, Uuid::parse_str(ACTOR).unwrap()),
        CreateOrdinaryTimetableBlockRequest {
            timetable_version_id: fixture.version,
            academic_term_id: fixture.term,
            learning_group_id: group,
            day_of_week: "WED".into(),
            bell_schedule_period_id: fixture.block.bell_schedule_period_id,
            room_id: None,
            instructor_ids: vec![teacher, other],
            note: None,
        },
    )
    .await
    .unwrap();
    grant(
        pool,
        teacher,
        &[
            "academic_timetable.read.assigned",
            "academic_timetable.manage.assigned",
        ],
    )
    .await;
    let (session, token) = fixture.auth.insert_session(pool, teacher, 193, false).await;
    fixture.cookie = format!(
        "__Host-schoolorbit_session={}",
        token.encode().expose_for_cookie()
    );
    fixture.csrf = fixture.auth.csrf(session);
    let (mut socket, sync) = fixture.socket().await;
    assert!(
        sync["payload"]["workspace"]["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|block| block["id"] == fixture.block.id.to_string()),
        "existing whole-block read visibility is preserved"
    );
    let claim = command(
        &mut socket,
        "ClientClaim",
        json!({"requestId":Uuid::new_v4(),"source":fixture.source(),"purpose":"edit"}),
    )
    .await;
    assert_eq!(claim["payload"]["accepted"], false);
    assert_eq!(claim["payload"]["code"], "forbidden");
    let response = fixture
        .update(None, "must not edit other instructor")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403, "{}", response.text().await.unwrap());
    socket.close(None).await.unwrap();
}

#[tokio::test]
async fn roster_portrait_reference_requires_existing_staff_profile_access_for_each_viewer() {
    let fixture = Fixture::new("collaboration_scoped_portraits").await;
    let actor = Uuid::parse_str(ACTOR).unwrap();
    let image = crate::integration_tests::auth::profile_integration_tests::insert_ready_temporary_profile_image(&fixture.auth.tenant_a_pool, actor).await;
    sqlx::query("UPDATE users SET profile_image_file_id=$2 WHERE id=$1")
        .bind(actor)
        .bind(image)
        .execute(&fixture.auth.tenant_a_pool)
        .await
        .unwrap();
    let (mut owner, owner_sync) = fixture.socket().await;
    let own = owner_sync["payload"]["users"]
        .as_array()
        .unwrap()
        .iter()
        .find(|user| user["userId"] == actor.to_string())
        .unwrap();
    assert_eq!(
        own["profileImageFileId"],
        image.to_string(),
        "existing Files ownership permits own portrait"
    );
    let mut sockets = Vec::new();
    for (index, can_read) in [(0_u8, false), (1_u8, true)] {
        let viewer = fixture
            .auth
            .insert_user(
                &fixture.auth.tenant_a_pool,
                &format!("collab-portrait-{index}"),
                "active",
            )
            .await;
        let mut codes = vec!["academic_timetable.read.school"];
        if can_read {
            codes.push("staff_profile.read.school");
        }
        grant(&fixture.auth.tenant_a_pool, viewer, &codes).await;
        let (_, token) = fixture
            .auth
            .insert_session(&fixture.auth.tenant_a_pool, viewer, 194 + index, false)
            .await;
        let cookie = format!(
            "__Host-schoolorbit_session={}",
            token.encode().expose_for_cookie()
        );
        let (socket, sync) = fixture.socket_with_cookie(&cookie).await;
        let roster = sync["payload"]["users"]
            .as_array()
            .unwrap()
            .iter()
            .find(|user| user["userId"] == actor.to_string())
            .unwrap();
        assert_eq!(
            roster["profileImageFileId"],
            if can_read { json!(image) } else { Value::Null }
        );
        sockets.push(socket);
    }
    for mut socket in sockets {
        socket.close(None).await.unwrap();
    }
    owner.close(None).await.unwrap();
}

#[tokio::test]
async fn all_draft_bulk_lifecycle_and_resume_owners_observe_the_same_active_reservation() {
    use school_academic_timetable::{
        models::{
            timetable::{ApplyTemplateRequest, ClearTimetableRequest},
            timetable_publication::PublishTimetableVersionRequest,
            timetable_version::*,
        },
        services::{
            teacher_handoff, timetable_lifecycle, timetable_template_service,
            timetable_version_service,
        },
    };
    let fixture = Fixture::new("collaboration_all_write_owners").await;
    let (mut socket, sync) = fixture.socket().await;
    let claim = command(
        &mut socket,
        "ClientClaim",
        json!({"requestId":Uuid::new_v4(),"source":fixture.source(),"purpose":"edit"}),
    )
    .await;
    assert_eq!(claim["payload"]["accepted"], true);
    let context = fixture.mutation(None).await;
    let pool = &fixture.auth.tenant_a_pool;
    let row = sync["payload"]["workspace"]["version"]["rowVersion"]
        .as_i64()
        .unwrap();
    let delivery = uuid(&sync["payload"]["workspace"]["version"]["deliveryVersionId"]);
    let source = uuid(&sync["payload"]["workspace"]["version"]["sourceVersionId"]);
    let source_row: i64 =
        sqlx::query_scalar("SELECT row_version FROM academic_timetable_versions WHERE id=$1")
            .bind(source)
            .fetch_one(pool)
            .await
            .unwrap();
    fn reserved<T>(result: Result<T, school_errors::AppError>) {
        assert!(
            matches!(result, Err(school_errors::AppError::Conflict(ref message)) if message.contains("reserved by another editor")),
            "every owner must enter the shared guard before its operation-specific database work"
        );
    }
    reserved(
        timetable_template_service::apply_template(
            pool,
            &context,
            Uuid::new_v4(),
            ApplyTemplateRequest {
                timetable_version_id: fixture.version,
                academic_term_id: fixture.term,
            },
        )
        .await,
    );
    reserved(
        timetable_template_service::clear_timetable(
            pool,
            &context,
            ClearTimetableRequest {
                timetable_version_id: fixture.version,
                academic_term_id: fixture.term,
                entry_types: None,
            },
        )
        .await,
    );
    reserved(
        timetable_lifecycle::update_source(
            pool,
            &context,
            fixture.version,
            UpdateTimetableDeliverySourceRequest {
                row_version: row,
                delivery_version_id: delivery,
            },
        )
        .await,
    );
    reserved(
        timetable_lifecycle::delete_draft(
            pool,
            &context,
            fixture.version,
            DeleteTimetableDraftRequest {
                row_version: row,
                expected_block_count: 2,
            },
        )
        .await,
    );
    reserved(
        timetable_lifecycle::publish(
            pool,
            &context,
            fixture.version,
            PublishTimetableVersionRequest {
                row_version: row,
                effective_from: chrono::Utc::now().date_naive(),
                preview_hash: "a".repeat(64),
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await,
    );
    reserved(
        timetable_version_service::clone_draft(
            pool,
            &context,
            source,
            CloneTimetableVersionRequest {
                source_row_version: source_row,
                resume_draft_id: Some(fixture.version),
                draft_row_version: Some(row),
            },
        )
        .await,
    );
    reserved(
        teacher_handoff::apply(
            pool,
            &context,
            Uuid::new_v4(),
            school_academic_delivery::models::ApplyTeacherHandoffRequest {
                timetable_version_id: fixture.version,
                change_set_row_version: 1,
                target_timetable_version_row_version: row,
                teacher_change_item_id: Uuid::new_v4(),
                entries: Vec::new(),
                mode: school_academic_delivery::models::TeacherHandoffMode::Manual,
                instructor_ids: Vec::new(),
                preview_hash: "a".repeat(64),
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await,
    );
    reserved(
        timetable_block_service::deactivate_series(
            pool,
            &context,
            fixture.block.series_id.unwrap(),
            fixture.version,
        )
        .await,
    );
    // Without the outstanding source lease the existing owner still supports no-proof bulk
    // admission; the series removal emits its authoritative changed/deleted block delta.
    command(
        &mut socket,
        "ClientRelease",
        json!({"requestId":Uuid::new_v4(),"leaseToken":claim["payload"]["leaseToken"]}),
    )
    .await;
    timetable_block_service::deactivate_series(
        pool,
        &context,
        fixture.block.series_id.unwrap(),
        fixture.version,
    )
    .await
    .unwrap();
    let patch = event(&mut socket, "DraftPatched").await;
    assert_eq!(
        patch["payload"]["removedBlockIds"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(patch["payload"]["summary"]["blockCount"], 0);
    assert_eq!(
        patch["payload"]["totalDraftBlockCount"], 2,
        "deletion confirmation counts include retained inactive rows"
    );
    socket.close(None).await.unwrap();
}

#[tokio::test]
async fn real_cold_fill_overlapping_upstream_change_cannot_install_stale_valid_projection() {
    use school_academic_timetable::ports::{StaffDirectoryPort, StaffSubjectGroupAssignment};
    struct BarrierDirectory {
        reached: tokio::sync::Notify,
        release: tokio::sync::Notify,
    }
    #[async_trait::async_trait]
    impl StaffDirectoryPort for BarrierDirectory {
        async fn collaboration_portraits(
            &self,
            connection: &mut sqlx::PgConnection,
            actor: &school_authorization::ActorContext,
            ids: &[Uuid],
        ) -> Result<Vec<(Uuid, Uuid)>, school_errors::AppError> {
            crate::adapters::staff_directory::STAFF_DIRECTORY
                .collaboration_portraits(connection, actor, ids)
                .await
        }
        async fn subject_groups_for_users(
            &self,
            connection: &mut sqlx::PgConnection,
            ids: &[Uuid],
        ) -> Result<Vec<StaffSubjectGroupAssignment>, school_errors::AppError> {
            // Canonical room/staff rows have already been read. Pause the real provider boundary
            // so a committed upstream change and its invalidation deterministically overlap fill.
            self.reached.notify_one();
            self.release.notified().await;
            crate::adapters::staff_directory::STAFF_DIRECTORY
                .subject_groups_for_users(connection, ids)
                .await
        }
    }
    let fixture = Fixture::new("collaboration_cold_fill_generation").await;
    let tenant = school_auth_http::tenant::resolve_tenant_context_by_subdomain(
        &fixture.auth.runtime,
        "tenant-a",
    )
    .await
    .unwrap();
    fixture
        .manager
        .register_tenant(&tenant.subdomain, tenant.tenant_id)
        .unwrap();
    let directory = Arc::new(BarrierDirectory {
        reached: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let provider = directory.clone();
    let runtime = fixture.manager.collaboration();
    let version = fixture.version;
    let worker = tokio::spawn(async move {
        let _lease = tenant.pool.clone();
        runtime
            .load_room(tenant.tenant_id, &tenant.pool, version, provider.as_ref())
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), directory.reached.notified())
        .await
        .unwrap();
    let room_id: Uuid =
        sqlx::query_scalar("SELECT id FROM rooms WHERE status='ACTIVE' ORDER BY id LIMIT 1")
            .fetch_one(&fixture.auth.tenant_a_pool)
            .await
            .unwrap();
    sqlx::query("UPDATE rooms SET name_th='Changed during canonical fill' WHERE id=$1")
        .bind(room_id)
        .execute(&fixture.auth.tenant_a_pool)
        .await
        .unwrap();
    fixture.manager.broadcast_academic_core_changed(
        "tenant-a".into(),
        Uuid::parse_str(ACTOR).unwrap(),
        "room",
        Some(room_id),
        None,
        None,
    );
    directory.release.notify_one();
    assert!(
        matches!(
            worker.await.unwrap(),
            Err(school_errors::AppError::ServiceUnavailable(_))
        ),
        "overlapping cache fill must be refused rather than made valid"
    );
    let (mut socket, snapshot) = fixture.socket().await;
    assert!(snapshot["payload"]["workspace"]["rooms"]
        .as_array()
        .unwrap()
        .iter()
        .any(|room| room["id"] == room_id.to_string()
            && room["name"] == "Changed during canonical fill"));
    socket.close(None).await.unwrap();
}

#[tokio::test]
async fn actual_facility_producers_invalidate_active_and_empty_rooms_only_after_success() {
    let fixture = Fixture::new("collaboration_facility_producers").await;
    let (mut socket, initial) = fixture.socket().await;
    let payload =
        json!({"name_th":"Created by facility owner", "room_type":"GENERAL", "status":"ACTIVE"});
    let denied = fixture
        .auth
        .insert_user(
            &fixture.auth.tenant_a_pool,
            "collab-facility-denied",
            "active",
        )
        .await;
    grant(
        &fixture.auth.tenant_a_pool,
        denied,
        &["academic_timetable.read.school"],
    )
    .await;
    let (session, token) = fixture
        .auth
        .insert_session(&fixture.auth.tenant_a_pool, denied, 196, false)
        .await;
    let mut request = fixture
        .request(reqwest::Method::POST, "/api/facilities/rooms")
        .json(&payload)
        .build()
        .unwrap();
    request.headers_mut().insert(
        "cookie",
        format!(
            "__Host-schoolorbit_session={}",
            token.encode().expose_for_cookie()
        )
        .parse()
        .unwrap(),
    );
    request
        .headers_mut()
        .insert("x-csrf-token", fixture.auth.csrf(session).parse().unwrap());
    assert_eq!(
        reqwest::Client::new()
            .execute(request)
            .await
            .unwrap()
            .status(),
        403
    );
    let failed = fixture
        .request(reqwest::Method::POST, "/api/facilities/rooms")
        .json(
            &json!({"name_th":"Invalid room", "room_type":"GENERAL", "building_id":Uuid::new_v4()}),
        )
        .send()
        .await
        .unwrap();
    assert!(!failed.status().is_success());
    let unchanged = command(
        &mut socket,
        "ClientRenew",
        json!({"requestId":Uuid::new_v4(),"leaseToken":Uuid::new_v4()}),
    )
    .await;
    assert_eq!(
        unchanged["epoch"], initial["epoch"],
        "denied/rolled-back producer does not invalidate"
    );

    let response = fixture
        .request(reqwest::Method::POST, "/api/facilities/rooms")
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201, "{}", response.text().await.unwrap());
    let room_id = uuid(&response.json::<Value>().await.unwrap()["data"]["id"]);
    event(&mut socket, "DraftReconcileRequired").await;
    let (mut refreshed, created) = fixture.socket().await;
    assert_ne!(created["epoch"], initial["epoch"]);
    assert!(created["payload"]["workspace"]["rooms"]
        .as_array()
        .unwrap()
        .iter()
        .any(|room| room["id"] == room_id.to_string()));
    refreshed.close(None).await.unwrap();
    let cached = fixture.room();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !cached.members().unwrap().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let response = fixture
        .request(
            reqwest::Method::PUT,
            &format!("/api/facilities/rooms/{room_id}"),
        )
        .json(&json!({"status":"INACTIVE"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "{}", response.text().await.unwrap());
    let (mut joined, inactive) = fixture.socket().await;
    assert_ne!(inactive["epoch"], created["epoch"]);
    assert!(inactive["payload"]["workspace"]["rooms"]
        .as_array()
        .unwrap()
        .iter()
        .all(|room| room["id"] != room_id.to_string()));
    let response = fixture
        .request(
            reqwest::Method::DELETE,
            &format!("/api/facilities/rooms/{room_id}"),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "{}", response.text().await.unwrap());
    event(&mut joined, "DraftReconcileRequired").await;
}

#[tokio::test]
async fn actual_staff_creation_invalidates_empty_canonical_cache_before_private_new_choice() {
    let fixture = Fixture::new("collaboration_staff_create_producer").await;
    let (mut socket, initial) = fixture.socket().await;
    socket.close(None).await.unwrap();
    let cached = fixture.room();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !cached.members().unwrap().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut changes = cached.subscribe();
    let response = fixture.request(reqwest::Method::POST, "/api/staff").json(&json!({
        "username":"collaboration-created-staff", "email":"collaboration-created-staff@example.test",
        "password":"Synthetic-fixture-27!", "title":"นาย", "first_name":"ใหม่", "last_name":"ทดสอบ", "role_ids":[]
    })).send().await.unwrap();
    assert_eq!(response.status(), 201, "{}", response.text().await.unwrap());
    let teacher = uuid(&response.json::<Value>().await.unwrap()["data"]["id"]);
    // Observe the existing domain invalidation signal even while no socket is a member.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if matches!(
                changes.recv().await.unwrap(),
                school_academic_timetable::collaboration::DraftSignal::Invalidated
            ) {
                break;
            }
        }
    })
    .await
    .unwrap();
    let (mut joined, latest) = fixture.socket().await;
    assert_ne!(latest["epoch"], initial["epoch"]);
    // Existing public Staff projection remains source/block scoped. The newly created active
    // teacher is private candidate input, authorized through the cached resource evaluator.
    let candidate = json!({"blockKind":"structural","learningGroupId":null,"learningOfferingId":null,"roomId":null,"homeroomIds":fixture.block.homerooms.iter().map(|row|row.homeroom_id).collect::<Vec<_>>(),"teacherIds":[teacher],"instructorIds":[]});
    let claim = command(&mut joined, "ClientClaim", json!({"requestId":Uuid::new_v4(),"source":fixture.source(),"purpose":"edit","candidate":candidate})).await;
    assert_eq!(
        claim["payload"]["accepted"], true,
        "fresh canonical cache includes the newly created staff resource: {claim}"
    );
    joined.close(None).await.unwrap();
}

#[tokio::test]
async fn scoped_lookup_gain_and_loss_match_fresh_authorized_http_projection() {
    let fixture = Fixture::new("collaboration_scoped_lookup_delta").await;
    let teacher = fixture
        .auth
        .insert_user(
            &fixture.auth.tenant_a_pool,
            "collab-lookup-teacher",
            "active",
        )
        .await;
    grant(
        &fixture.auth.tenant_a_pool,
        teacher,
        &["academic_timetable.read.assigned"],
    )
    .await;
    let unit: Uuid = sqlx::query_scalar("SELECT id FROM organization_units WHERE unit_type='subject_group' AND is_active AND subject_group_id IS NOT NULL ORDER BY id LIMIT 1")
        .fetch_one(&fixture.auth.tenant_a_pool).await.unwrap();
    sqlx::query("INSERT INTO organization_members(user_id,organization_unit_id,position_code,is_primary) VALUES($1,$2,'member',true)")
        .bind(teacher).bind(unit).execute(&fixture.auth.tenant_a_pool).await.unwrap();
    let (session, token) = fixture
        .auth
        .insert_session(&fixture.auth.tenant_a_pool, teacher, 197, false)
        .await;
    let cookie = format!(
        "__Host-schoolorbit_session={}",
        token.encode().expose_for_cookie()
    );
    let csrf = fixture.auth.csrf(session);
    let (mut manager, manager_sync) = fixture.socket().await;
    let (mut scoped, initial) = fixture.socket_with_cookie(&cookie).await;
    assert!(initial["payload"]["workspace"]["blocks"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(
        initial["payload"]["workspace"]["staff"]
            .as_array()
            .unwrap()
            .is_empty(),
        "cold join does not broaden public Staff visibility"
    );
    let connection = uuid(&manager_sync["payload"]["connectionId"]);
    let room_id = uuid(&manager_sync["payload"]["workspace"]["rooms"][0]["id"]);
    let path = format!("http://{}/api/academic/timetable-blocks/workspace?academicYearId={}&academicTermId={}&timetableVersionId={}", fixture.address, fixture.block.academic_year_id, fixture.term, fixture.version);
    let http_projection = || async {
        let response = reqwest::Client::new()
            .get(&path)
            .header("origin", "http://localhost:5173")
            .header("x-school-subdomain", "tenant-a")
            .header("cookie", &cookie)
            .header("x-csrf-token", &csrf)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        response.json::<Value>().await.unwrap()["data"].clone()
    };
    let mut created_block = Value::Null;
    let mut previous = initial["payload"]["workspace"].clone();
    for gain in [true, false] {
        if gain {
            let response = fixture.request(reqwest::Method::POST, "/api/academic/timetable-blocks/structural").json(&json!({
                "timetableVersionId":fixture.version,"academicTermId":fixture.term,"structuralKind":"flag_ceremony","title":"New scoped structural block",
                "slots":[{"dayOfWeek":"WED","bellSchedulePeriodId":fixture.block.bell_schedule_period_id}],
                "homeroomIds":[],"teacherIds":[teacher],"allHomerooms":true,"allTeachers":false,"roomId":room_id
            })).send().await.unwrap();
            assert_eq!(response.status(), 200, "{}", response.text().await.unwrap());
            created_block = response.json::<Value>().await.unwrap()["data"][0].clone();
        } else {
            let source = json!({"kind":"existing_block","blockId":created_block["id"],"rowVersion":created_block["rowVersion"]});
            let claimed = command(
                &mut manager,
                "ClientClaim",
                json!({"requestId":Uuid::new_v4(),"source":source,"purpose":"edit"}),
            )
            .await;
            assert_eq!(claimed["payload"]["accepted"], true, "{claimed}");
            let response = fixture
                .request(
                    reqwest::Method::DELETE,
                    &format!(
                        "/api/academic/timetable-blocks/{}?timetableVersionId={}&rowVersion={}",
                        created_block["id"].as_str().unwrap(),
                        fixture.version,
                        created_block["rowVersion"].as_i64().unwrap()
                    ),
                )
                .header("x-timetable-connection-id", connection.to_string())
                .header(
                    "x-timetable-lease-token",
                    claimed["payload"]["leaseToken"].as_str().unwrap(),
                )
                .header("x-timetable-mutation-id", Uuid::new_v4().to_string())
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200, "{}", response.text().await.unwrap());
        }
        if !gain {
            tokio::time::sleep(Duration::from_millis(20)).await;
            scoped
                .send(Message::Text(
                    json!({"type":"ClientPresenceUpdate","payload":{
                        "context":{"viewMode":"teacher","viewId":teacher,"dayOfWeek":"WED"},
                        "anchor":{"kind":"block","blockId":created_block["id"],"u":0.4,"v":0.6}
                    }})
                    .to_string()
                    .into(),
                ))
                .await
                .unwrap();
        }
        event(&mut manager, "DraftPatched").await;
        let patch = event(&mut scoped, "DraftPatched").await;
        let changes = &patch["payload"]["lookupChanges"];
        let fresh = http_projection().await;
        let reconstructed = reconstructed_lookups(&previous, changes);
        for field in [
            "staff",
            "homerooms",
            "rooms",
            "bellPeriods",
            "offeringSubjectGroups",
        ] {
            assert_eq!(
                reconstructed[field], fresh[field],
                "recipient delta reconstructs exact scoped metadata and producer order for {field}"
            );
        }
        if gain {
            assert_eq!(changes["upsertedStaff"], fresh["staff"]);
            assert!(
                !changes["upsertedStaff"][0]["subjectGroups"]
                    .as_array()
                    .unwrap()
                    .is_empty(),
                "private newly referenced teacher carries full authoritative affiliations"
            );
            assert_eq!(changes["upsertedHomerooms"], fresh["homerooms"]);
            assert_eq!(changes["upsertedRooms"], fresh["rooms"]);
            assert_eq!(
                patch["payload"]["upsertedBlocks"].as_array().unwrap().len(),
                1
            );
        } else {
            assert_eq!(changes["removedStaffIds"], json!([teacher]));
            assert_eq!(changes["removedRoomIds"], json!([room_id]));
            assert_eq!(
                changes["removedHomeroomIds"].as_array().unwrap().len(),
                fixture.block.homerooms.len()
            );
            assert!(changes["upsertedStaff"].as_array().unwrap().is_empty());
            assert_eq!(
                patch["payload"]["removedBlockIds"],
                json!([created_block["id"]])
            );
            assert!(
                patch["payload"]["upsertedBlocks"]
                    .as_array()
                    .unwrap()
                    .is_empty(),
                "lost scope sends no unauthorized block"
            );
            assert!(fresh["staff"].as_array().unwrap().is_empty());
            assert!(fresh["homerooms"].as_array().unwrap().is_empty());
            assert!(fresh["rooms"].as_array().unwrap().is_empty());
        }
        if gain {
            scoped
                .send(Message::Text(
                    json!({"type":"ClientPresenceUpdate","payload":{
                        "context":{"viewMode":"teacher","viewId":teacher,"dayOfWeek":"WED"},
                        "anchor":{"kind":"block","blockId":created_block["id"],"u":0.4,"v":0.6}
                    }})
                    .to_string()
                    .into(),
                ))
                .await
                .unwrap();
            loop {
                let presence = event(&mut manager, "DraftPresenceChanged").await;
                if let Some(connection) = presence["payload"]["connections"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| {
                        row["connectionId"] == initial["payload"]["connectionId"]
                            && row["anchor"]["blockId"] == created_block["id"]
                    })
                {
                    assert_eq!(connection["context"]["viewId"], teacher.to_string());
                    break;
                }
            }
        } else {
            loop {
                let presence = event(&mut manager, "DraftPresenceChanged").await;
                if presence["seq"].as_u64().unwrap() < patch["seq"].as_u64().unwrap() {
                    continue;
                }
                let connection = presence["payload"]["connections"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| row["connectionId"] == initial["payload"]["connectionId"])
                    .unwrap();
                assert!(
                    connection["context"].is_null() && connection["anchor"].is_null(),
                    "delayed once-authorized pointer never rebroadcasts lost-scope identifiers"
                );
                break;
            }
            let alive = command(
                &mut scoped,
                "ClientRenew",
                json!({"requestId":Uuid::new_v4(),"leaseToken":Uuid::new_v4()}),
            )
            .await;
            assert_eq!(
                alive["epoch"], initial["epoch"],
                "normal scope loss keeps the same healthy connection/epoch"
            );
        }
        previous = fresh;
    }
    manager.close(None).await.unwrap();
    scoped.close(None).await.unwrap();
}
