//! Central acceptance/presentation and real managed-tenant phase regressions.
use crate::{
    adapters::notification_delivery::APPLICATION_NOTIFICATION_ELIGIBILITY as ELIGIBILITY,
    integration_tests::academic::core::services_tests::prepare_core_fixture_through,
};
use school_errors::AppError;
use school_notifications::{
    dispatch::{
        self, NewNotification, NotificationContent, NotificationSource, NotificationSourceKind,
        Presentation,
    },
    publisher::NotificationType,
};
use sqlx::PgPool;
use uuid::Uuid;
async fn fixture(name: &str) -> (PgPool, Uuid) {
    let latest = sqlx::migrate!("./migrations")
        .iter()
        .map(|m| m.version)
        .max()
        .unwrap();
    let pool = prepare_core_fixture_through(name, latest).await;
    let user = sqlx::query_scalar(
        "SELECT id FROM users WHERE status='active' AND user_type='staff' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    (pool, user)
}
fn item(recipient_id: Uuid) -> NewNotification {
    NewNotification::new(
        recipient_id,
        NotificationSource::Direct,
        format!("synthetic:{}", Uuid::new_v4()),
        NotificationContent {
            title: "Synthetic notice".into(),
            message: "Synthetic body".into(),
            type_: NotificationType::Info,
            link: Some("/staff/calendar".into()),
        },
        Presentation::AlertOnce,
    )
}
async fn enqueue(pool: &PgPool, items: &[NewNotification]) {
    let mut tx = pool.begin().await.unwrap();
    dispatch::create_in_transaction(&mut tx, items)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}
pub(super) async fn drain(
    pool: &PgPool,
    tenant: &str,
    channel: &tokio::sync::broadcast::Sender<school_notifications::events::TenantNotificationEvent>,
) {
    for _ in 0..20 {
        let status = school_notifications::runtime::deliver_tenant_batch(
            pool,
            tenant,
            channel,
            &ELIGIBILITY,
        )
        .await
        .unwrap();
        if !status.due {
            assert!(!status.pending);
            return;
        }
    }
    panic!("synthetic backlog exceeded expected bound")
}
#[tokio::test]
async fn notification_acceptance_is_atomic_idempotent_conflict_checked_and_preserves_read_state() {
    let (pool, user) = fixture("inbox_atomic").await;
    let n = item(user);
    let mut tx = pool.begin().await.unwrap();
    dispatch::create_in_transaction(&mut tx, std::slice::from_ref(&n))
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notifications")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    enqueue(&pool, std::slice::from_ref(&n)).await;
    sqlx::query("UPDATE notifications SET read_at=now() WHERE id=$1")
        .bind(n.notification_id)
        .execute(&pool)
        .await
        .unwrap();
    let before: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(n) FROM notifications n WHERE id=$1")
            .bind(n.notification_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut replay = n.clone();
    replay.id = Uuid::new_v4();
    replay.notification_id = Uuid::new_v4();
    let accepted = dispatch::create(&pool, std::slice::from_ref(&replay))
        .await
        .unwrap();
    assert_eq!(accepted[0].notification_id, n.notification_id);
    assert!(!accepted[0].inserted);
    let after: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(n) FROM notifications n WHERE id=$1")
            .bind(n.notification_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    for mutation in 0..4 {
        let mut bad = replay.clone();
        match mutation {
            0 => bad.message.push('!'),
            1 => bad.presentation = Presentation::InboxOnly,
            2 => {
                bad.source = NotificationSource::Attendance {
                    source_id: Uuid::new_v4(),
                }
            }
            _ => {
                bad.id = n.id;
                bad.idempotency_key.push('!')
            }
        }
        assert!(matches!(
            dispatch::create(&pool, &[bad]).await,
            Err(AppError::Conflict(_))
        ));
    }
    sqlx::query("DELETE FROM notifications WHERE id=$1")
        .bind(n.notification_id)
        .execute(&pool)
        .await
        .unwrap();
    dispatch::create(&pool, &[replay]).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notifications")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn notification_concurrent_consumption_restart_and_uncertain_finish_never_replay() {
    let (pool, user) = fixture("inbox_consume_once").await;
    let n = item(user);
    enqueue(&pool, &[n.clone()]).await;
    let pending = dispatch::pending(&pool).await.unwrap().remove(0);
    let (a, b) = tokio::join!(
        dispatch::consume(&pool, &pending, &ELIGIBILITY),
        dispatch::consume(&pool, &pending, &ELIGIBILITY)
    );
    assert_eq!(
        usize::from(a.unwrap().is_some()) + usize::from(b.unwrap().is_some()),
        1
    );
    // Lost transport and lost post-network cleanup cannot reset a consumed record.
    assert!(dispatch::consume(&pool, &pending, &ELIGIBILITY)
        .await
        .unwrap()
        .is_none());
    assert!(!dispatch::status(&pool).await.unwrap().pending);
    let (channel, mut events) = tokio::sync::broadcast::channel(8);
    drain(&pool, "synthetic", &channel).await;
    assert!(events.try_recv().is_err());
    dispatch::create(&pool, &[n]).await.unwrap();
    assert!(dispatch::pending(&pool).await.unwrap().is_empty());
}
#[tokio::test]
async fn notification_pending_recovers_before_consume_but_expiry_and_missing_inbox_suppress() {
    let (pool, user) = fixture("inbox_suppress").await;
    let first = item(user);
    let second = item(user);
    let mut inbox_only = item(user);
    inbox_only.presentation = Presentation::InboxOnly;
    enqueue(&pool, &[first.clone(), second.clone(), inbox_only]).await;
    assert_eq!(dispatch::pending(&pool).await.unwrap().len(), 2);
    sqlx::query("UPDATE notification_presentations SET expires_at=now()-interval '1 second' WHERE receipt_id=$1").bind(first.id).execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM notifications WHERE id=$1")
        .bind(second.notification_id)
        .execute(&pool)
        .await
        .unwrap();
    let (channel, mut events) = tokio::sync::broadcast::channel(8);
    drain(&pool, "synthetic", &channel).await;
    assert!(events.try_recv().is_err());
    let outcomes: Vec<String> = sqlx::query_scalar(
        "SELECT presentation_outcome FROM notification_receipts ORDER BY presentation_outcome",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(outcomes, vec!["expired", "skipped", "skipped"]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notifications")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
}
#[tokio::test]
async fn notification_source_owned_cancellation_blocks_prefetched_pending_without_resurrection() {
    let (pool, user) = fixture("inbox_cancel").await;
    let n = item(user);
    enqueue(&pool, &[n.clone()]).await;
    let pending = dispatch::pending(&pool).await.unwrap().remove(0);
    let mut tx = pool.begin().await.unwrap();
    dispatch::cancel_identified_in_transaction(
        &mut tx,
        NotificationSourceKind::Direct,
        &[n.notification_id],
        true,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert!(dispatch::consume(&pool, &pending, &ELIGIBILITY)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notifications")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn notification_recipient_snapshot_survives_deactivation_and_batches_are_bounded() {
    let (pool, user) = fixture("inbox_snapshot").await;
    enqueue(&pool, &(0..65).map(|_| item(user)).collect::<Vec<_>>()).await;
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(dispatch::pending(&pool).await.unwrap().len(), 32);
    let (channel, mut events) = tokio::sync::broadcast::channel(128);
    drain(&pool, "synthetic", &channel).await;
    let mut ids = std::collections::HashSet::new();
    while let Ok(event) = events.try_recv() {
        assert_eq!(event.user_id, user);
        assert!(ids.insert(event.notification.id));
    }
    assert_eq!(ids.len(), 65);
}

async fn wait_for_blocker(pool: &PgPool, blocker: i32) {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",
            )
            .bind(blocker)
            .fetch_one(pool)
            .await
            .unwrap()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("third bounded pass must self-wake promptly");
}
#[tokio::test]
async fn notification_dispatch_due_backlog_revisits_other_tenants_before_next_batch() {
    use axum::{extract::Path, http::StatusCode, routing::get, Json, Router};
    use std::{collections::HashMap, sync::Arc};
    struct AbortOnDrop(tokio::task::JoinHandle<()>);
    impl Drop for AbortOnDrop {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let (first, first_recipient) = fixture("dispatch_fair_queue_first").await;
    let (second, second_recipient) = fixture("dispatch_fair_queue_second").await;
    let (recovering, recovering_recipient) = fixture("dispatch_retry_admission").await;
    enqueue(&recovering, &[item(recovering_recipient)]).await;
    let lock_key = i64::from_le_bytes(*Uuid::new_v4().as_bytes().first_chunk::<8>().unwrap());
    let mut blocker = first.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(lock_key)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let mut directory = HashMap::new();
    let mut active = Vec::new();
    let failing_tenant = format!("a-failing-{}", Uuid::new_v4().simple());
    active.push(serde_json::json!({"subdomain":failing_tenant}));
    let failed_admissions = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let admission_observer = failed_admissions.clone();
    let recovery_ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let recovery_control = recovery_ready.clone();
    let recovery_schema: String = sqlx::query_scalar("SELECT current_schema()::text")
        .fetch_one(&recovering)
        .await
        .unwrap();
    let mut recovery_url =
        reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
    recovery_url.query_pairs_mut().append_pair(
        "options",
        &format!("-csearch_path={recovery_schema},public"),
    );
    directory.insert(failing_tenant.clone(),serde_json::json!({"id":Uuid::new_v4(),"name":"fixture","db_connection_string":recovery_url.as_str()}));
    for (f, recipient, name) in [
        (&first, first_recipient, "queue-first"),
        (&second, second_recipient, "queue-second"),
    ] {
        enqueue(f, &(0..97).map(|_| item(recipient)).collect::<Vec<_>>()).await;
        // Two complete passes are permitted; the third must pause. A worker that
        // drains one tenant before visiting the other leaves that other at zero.
        // The formatted advisory key is a generated i64, never caller SQL.
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE FUNCTION block_later_notice_batch() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.presentation_consumed_at IS NOT NULL AND OLD.presentation_consumed_at IS NULL AND (SELECT count(*) FROM notification_receipts WHERE presentation_consumed_at IS NOT NULL)>=64 THEN PERFORM pg_advisory_xact_lock({lock_key}); END IF; RETURN NEW; END $$; CREATE TRIGGER block_later_notice_batch BEFORE UPDATE ON notification_receipts FOR EACH ROW EXECUTE FUNCTION block_later_notice_batch();")))
            .execute(&*f).await.unwrap();
        let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
            .fetch_one(&*f)
            .await
            .unwrap();
        let mut url = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
        url.query_pairs_mut()
            .append_pair("options", &format!("-csearch_path={schema},public"));
        let subdomain = format!("{name}-{}", Uuid::new_v4().simple());
        directory.insert(subdomain.clone(), serde_json::json!({"id":Uuid::new_v4(),"name":"fixture","db_connection_string":url.as_str()}));
        active.push(serde_json::json!({"subdomain":subdomain,"db_connection_string":url.as_str()}));
    }
    let directory = Arc::new(directory);
    let app = Router::new()
        .route(
            "/internal/schools",
            get(move || {
                let schools = active.clone();
                async move { Json(serde_json::json!({"schools":schools})) }
            }),
        )
        .route(
            "/internal/schools/{tenant}",
            get(move |Path(tenant): Path<String>| {
                let directory = directory.clone();
                let failing_tenant = failing_tenant.clone();
                let failed_admissions = failed_admissions.clone();
                let recovery_ready = recovery_ready.clone();
                async move {
                    if tenant == failing_tenant {
                        failed_admissions.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        if !recovery_ready.load(std::sync::atomic::Ordering::SeqCst) {
                            return Err(StatusCode::NOT_FOUND);
                        }
                    }
                    directory
                        .get(&tenant)
                        .cloned()
                        .map(Json)
                        .ok_or(StatusCode::NOT_FOUND)
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut _server = AbortOnDrop(tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    }));
    let (channel, _) = tokio::sync::broadcast::channel(100);
    let state = school_notifications::runtime::NotificationRuntimeState {
        admin_client: Arc::new(school_tenancy::AdminClient::new(
            format!("http://{address}"),
            "fixture-internal".into(),
            school_tenancy::AdminClientConfig::from_env().unwrap(),
        )),
        pool_manager: Arc::new(school_tenancy::PoolManager::new()),
        notification_channel: channel,
        eligibility: &ELIGIBILITY,
    };
    let mut _worker = AbortOnDrop(tokio::spawn(school_notifications::runtime::start(state)));
    // Startup contributes at most one Notify permit and the initial interval tick.
    // Reaching the third batch promptly also proves due-work self-waking.
    wait_for_blocker(&first, blocker_pid).await;
    assert_eq!(
        admission_observer.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "failed admission must defer while other tenants have immediate backlog"
    );
    for f in [&first, &second] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM notification_receipts WHERE presentation_consumed_at IS NOT NULL"
            )
            .fetch_one(&*f)
            .await
            .unwrap(),
            64,
            "each tenant must receive one bounded batch per pass"
        );
    }
    blocker.rollback().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if !dispatch::status(&first).await.unwrap().pending
                && !dispatch::status(&second).await.unwrap().pending
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("due backlog must drain without waiting for the 15-second timer");
    recovery_control.store(true, std::sync::atomic::Ordering::SeqCst);
    // No new wake hint: a failed startup candidate must remain scheduled for a
    // regular retry, independently of the300-second directory refresh.
    tokio::time::timeout(std::time::Duration::from_secs(35), async {
        while dispatch::status(&recovering).await.unwrap().pending {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("failed tenant admission must recover on regular retry");
    _worker.0.abort();
    let _ = (&mut _worker.0).await;
    _server.0.abort();
    let _ = (&mut _server.0).await;
    first.close().await;
    second.close().await;
    recovering.close().await;
}

struct ControlledBatchTransport {
    started: tokio::sync::Notify,
    release: tokio::sync::Notify,
    device: (Uuid, Uuid),
}
#[async_trait::async_trait]
impl school_notifications::runtime::test_support::ControlledTransport for ControlledBatchTransport {
    async fn deliver(
        &self,
        _notification_id: Uuid,
    ) -> Result<school_notifications::runtime::test_support::DeliveryResult, &'static str> {
        self.started.notify_one();
        self.release.notified().await;
        Ok(
            school_notifications::runtime::test_support::DeliveryResult {
                invalid_devices: vec![self.device],
            },
        )
    }
}

async fn managed_batch_phase_case(name: &str, remap: bool) {
    use axum::{routing::get, Json, Router};
    use school_tenancy::{
        background::TenantJobKind, AdminClient, AdminClientConfig, PoolManager, PoolManagerConfig,
    };
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };
    struct Abort(tokio::task::JoinHandle<()>);
    impl Drop for Abort {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let (fixture, recipient) = fixture(name).await;
    let queued = item(recipient);
    enqueue(&fixture, std::slice::from_ref(&queued)).await;
    let endpoint = "https://synthetic.invalid/device";
    sqlx::query("INSERT INTO push_subscriptions(id,version,user_id,endpoint,p256dh_key,auth_key,expires_at) VALUES($3,$4,$1,$2,'synthetic','synthetic',now()+interval '90 days')")
        .bind(recipient).bind(endpoint).bind(queued.id).bind(queued.notification_id).execute(&fixture).await.unwrap();
    let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
        .fetch_one(&fixture)
        .await
        .unwrap();
    let mut url = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
    url.query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema},public"));
    let directory_id = Arc::new(Mutex::new(Uuid::new_v4()));
    let response_id = directory_id.clone();
    let response_url = url.to_string();
    let app = Router::new().route("/internal/schools/{tenant}", get(move || {
        let id = *response_id.lock().unwrap();
        let database_url = response_url.clone();
        async move { Json(serde_json::json!({"id":id,"name":"synthetic","db_connection_string":database_url})) }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let _server = Abort(tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    }));
    let manager = Arc::new(PoolManager::with_config(PoolManagerConfig::for_tests(
        2, 1, 1,
    )));
    let observed = manager.get_pool(url.as_str(), name).await.unwrap();
    let observed_raw = observed.as_ref().clone();
    drop(observed);
    let (channel, _) = tokio::sync::broadcast::channel(8);
    let state = school_notifications::runtime::NotificationRuntimeState {
        admin_client: Arc::new(AdminClient::new(
            format!("http://{address}"),
            "synthetic".into(),
            AdminClientConfig::for_tests(Duration::from_secs(1), 1, Duration::from_millis(1)),
        )),
        pool_manager: manager.clone(),
        notification_channel: channel,
        eligibility: &ELIGIBILITY,
    };
    let transport = Arc::new(ControlledBatchTransport {
        started: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
        device: (queued.id, queued.notification_id),
    });
    let worker_state = state.clone();
    let worker_transport = transport.clone();
    let tenant = name.to_string();
    let worker = tokio::spawn(async move {
        school_notifications::runtime::test_support::deliver_managed_batch(
            &worker_state,
            &tenant,
            worker_transport.as_ref(),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(3), transport.started.notified())
        .await
        .unwrap();
    let turns = manager.background_turns();
    for kind in [
        TenantJobKind::CalendarReminders,
        TenantJobKind::AuthRetention,
        TenantJobKind::AttendanceMaintenance,
        TenantJobKind::FileReconciliation,
    ] {
        assert_eq!(
            tokio::time::timeout(
                Duration::from_millis(200),
                turns.run("healthy", kind, || async { 1 })
            )
            .await
            .unwrap(),
            Some(1)
        );
    }
    assert!(turns
        .begin(name, TenantJobKind::NotificationDispatch)
        .is_none());
    let mut second_url = url.clone();
    second_url
        .query_pairs_mut()
        .append_pair("application_name", "phase-second");
    let second = manager
        .get_pool(second_url.as_str(), "phase-second")
        .await
        .unwrap();
    let mut third_url = url.clone();
    third_url
        .query_pairs_mut()
        .append_pair("application_name", "phase-third");
    let third = tokio::time::timeout(
        Duration::from_secs(1),
        manager.get_pool(third_url.as_str(), "phase-third"),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        observed_raw.is_closed(),
        "actual notification DB lease must end before blocked transport"
    );
    assert!(!second.is_closed() && !third.is_closed());
    drop(second);
    drop(third);
    if remap {
        let token = Uuid::new_v4();
        state
            .admin_client
            .begin_directory_mutation(name, token)
            .unwrap();
        *directory_id.lock().unwrap() = Uuid::new_v4();
        state
            .admin_client
            .finish_directory_mutation(name, token)
            .unwrap();
    }
    transport.release.notify_one();
    let result = tokio::time::timeout(Duration::from_secs(3), worker)
        .await
        .unwrap()
        .unwrap();
    if remap {
        assert!(result.is_err());
    } else {
        assert!(!result.unwrap().pending);
    }
    let present: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM push_subscriptions WHERE endpoint=$1)")
            .bind(endpoint)
            .fetch_one(&fixture)
            .await
            .unwrap();
    let published: bool = sqlx::query_scalar(
        "SELECT presentation_consumed_at IS NOT NULL FROM notification_receipts WHERE id=$1",
    )
    .bind(queued.id)
    .fetch_one(&fixture)
    .await
    .unwrap();
    assert_eq!(
        present, remap,
        "remapped finish must not delete an endpoint"
    );
    assert_eq!(
        published, true,
        "consumption is durable before transport regardless of cleanup outcome"
    );
}

#[tokio::test]
async fn notification_managed_batch_releases_real_pool_lease_and_shared_permit_during_transport() {
    managed_batch_phase_case("dispatch_managed_phase_release", false).await;
}

#[tokio::test]
async fn notification_managed_batch_uuid_remap_refuses_endpoint_cleanup_and_acknowledgment() {
    managed_batch_phase_case("dispatch_managed_uuid_remap", true).await;
}

#[tokio::test]
async fn notification_managed_slow_admin_http_allows_actual_other_kind_sql_and_cancels_without_claim(
) {
    use crate::integration_tests::tenant_directory::{database_url, DirectoryServer};
    use school_tenancy::{background::TenantJobKind, PoolManager, PoolManagerConfig};
    use std::{sync::Arc, time::Duration};
    let (fixture, recipient) = fixture("dispatch_slow_admin").await;
    let queued = item(recipient);
    enqueue(&fixture, std::slice::from_ref(&queued)).await;
    let url = database_url(&fixture).await;
    let directory = DirectoryServer::start(Uuid::new_v4(), url.to_string(), true).await;
    let manager = Arc::new(PoolManager::with_config(PoolManagerConfig::for_tests(
        2, 1, 1,
    )));
    assert_eq!(manager.background_turns().limit(), 1);
    let state = school_notifications::runtime::NotificationRuntimeState {
        admin_client: directory.client(2, Duration::from_secs(30)),
        pool_manager: manager.clone(),
        notification_channel: tokio::sync::broadcast::channel(8).0,
        eligibility: &ELIGIBILITY,
    };
    let transport = Arc::new(ControlledBatchTransport {
        started: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
        device: (queued.id, queued.notification_id),
    });
    let worker_state = state.clone();
    let worker_transport = transport.clone();
    let worker = tokio::spawn(async move {
        school_notifications::runtime::test_support::deliver_managed_batch(
            &worker_state,
            "dispatch_slow_admin",
            worker_transport.as_ref(),
        )
        .await
    });
    directory.wait_for_request().await;
    let turns = manager.background_turns();
    let progress = tokio::time::timeout(
        Duration::from_secs(1),
        turns.run("healthy", TenantJobKind::AttendanceMaintenance, || async {
            let pool = manager
                .get_pool_without_migrations(url.as_str(), "healthy")
                .await
                .unwrap();
            let mut connection = pool
                .checkout("fixture.slow_admin_other_kind")
                .await
                .unwrap();
            school_database::DbSession::new(&mut connection)
                .scalar::<i32>("fixture.slow_admin_other_kind", "SELECT 1")
                .fetch_one()
                .await
                .unwrap()
        }),
    )
    .await
    .unwrap();
    assert_eq!(progress, Some(1));
    assert!(!worker.is_finished());
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    directory.unblock();
    assert!(turns
        .begin("dispatch_slow_admin", TenantJobKind::NotificationDispatch)
        .is_some());
    let consumed: bool = sqlx::query_scalar(
        "SELECT presentation_consumed_at IS NOT NULL FROM notification_receipts WHERE id=$1",
    )
    .bind(queued.id)
    .fetch_one(&fixture)
    .await
    .unwrap();
    assert!(!consumed);
    assert_eq!(directory.calls(), 1);
}

#[derive(Clone, Copy)]
enum QueuedDirectoryChange {
    Fence,
    Rotation,
    Expiry,
    Eviction,
}

async fn notification_managed_queued_finish_case(name: &str, change: QueuedDirectoryChange) {
    use crate::integration_tests::tenant_directory::{database_url, DirectoryServer, PhaseSignals};
    use school_tenancy::{background::TenantJobKind, PoolManager, PoolManagerConfig};
    use std::{sync::Arc, time::Duration};
    use tracing::instrument::WithSubscriber;
    let (fixture, recipient) = fixture(name).await;
    let queued = item(recipient);
    enqueue(&fixture, std::slice::from_ref(&queued)).await;
    let endpoint = "https://synthetic.invalid/queued-directory";
    sqlx::query("INSERT INTO push_subscriptions(id,version,user_id,endpoint,p256dh_key,auth_key,expires_at) VALUES($3,$4,$1,$2,'synthetic','synthetic',now()+interval '90 days')").bind(recipient).bind(endpoint).bind(queued.id).bind(queued.notification_id).execute(&fixture).await.unwrap();
    let url = database_url(&fixture).await;
    let tenant_id = Uuid::new_v4();
    let directory = DirectoryServer::start(tenant_id, url.to_string(), false).await;
    let manager = Arc::new(PoolManager::with_config(PoolManagerConfig::for_tests(
        2, 1, 1,
    )));
    let ttl = if matches!(change, QueuedDirectoryChange::Expiry) {
        Duration::from_secs(1)
    } else {
        Duration::from_secs(30)
    };
    let state = school_notifications::runtime::NotificationRuntimeState {
        admin_client: directory.client(1, ttl),
        pool_manager: manager.clone(),
        notification_channel: tokio::sync::broadcast::channel(8).0,
        eligibility: &ELIGIBILITY,
    };
    let transport = Arc::new(ControlledBatchTransport {
        started: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
        device: (queued.id, queued.notification_id),
    });
    let (signals, dispatcher) = PhaseSignals::for_phase("cleanup");
    let worker_state = state.clone();
    let worker_transport = transport.clone();
    let tenant = name.to_string();
    let worker = tokio::spawn(
        async move {
            school_notifications::runtime::test_support::deliver_managed_batch(
                &worker_state,
                &tenant,
                worker_transport.as_ref(),
            )
            .await
        }
        .with_subscriber(dispatcher),
    );
    tokio::time::timeout(Duration::from_secs(3), transport.started.notified())
        .await
        .unwrap();
    let turns = manager.background_turns();
    assert_eq!(turns.limit(), 1);
    let blocker = turns
        .begin("hold-for-finish", TenantJobKind::AuthRetention)
        .unwrap();
    let mut held = Box::pin(blocker.run(|| std::future::pending::<()>()));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(held.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    transport.release.notify_one();
    tokio::time::timeout(Duration::from_secs(1), signals.resolved.notified())
        .await
        .unwrap();
    let calls = directory.calls();
    let extra_requests = match change {
        QueuedDirectoryChange::Fence => {
            state
                .admin_client
                .begin_directory_mutation(name, Uuid::new_v4())
                .unwrap();
            0
        }
        QueuedDirectoryChange::Rotation => {
            let token = Uuid::new_v4();
            state
                .admin_client
                .begin_directory_mutation(name, token)
                .unwrap();
            let mut rotated = url.clone();
            rotated
                .set_password(Some("synthetic-rotated-credential"))
                .unwrap();
            directory.set_mapping(tenant_id, rotated.to_string());
            state
                .admin_client
                .finish_directory_mutation(name, token)
                .unwrap();
            state
                .admin_client
                .resolve_database_work(name)
                .await
                .unwrap();
            1
        }
        QueuedDirectoryChange::Expiry => {
            tokio::time::sleep(Duration::from_millis(1100)).await;
            1
        }
        QueuedDirectoryChange::Eviction => {
            state
                .admin_client
                .resolve_database_work("evicting-tenant")
                .await
                .unwrap();
            2
        }
    };
    assert!(!worker.is_finished());
    drop(held);
    let result = tokio::time::timeout(Duration::from_secs(3), worker)
        .await
        .unwrap()
        .unwrap();
    if let Ok(status) = result {
        assert!(!status.pending);
    }
    let (endpoint_present, published): (bool, bool) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM push_subscriptions WHERE endpoint=$1),(SELECT presentation_consumed_at IS NOT NULL FROM notification_receipts WHERE id=$2)").bind(endpoint).bind(queued.id).fetch_one(&fixture).await.unwrap();
    assert!(
        endpoint_present,
        "invalidated proof must not delete an endpoint"
    );
    assert!(
        published,
        "consumed attempt remains terminal when cleanup proof expires"
    );
    assert_eq!(directory.calls(), calls + extra_requests, "finish cannot refetch inside admission; only explicit rotation/eviction and later status resolution fetch");
}

#[tokio::test]
async fn notification_managed_queued_finish_fence_refuses_cleanup_and_ack_without_http_retry() {
    notification_managed_queued_finish_case("dispatch_queued_fence", QueuedDirectoryChange::Fence)
        .await;
}
#[tokio::test]
async fn notification_managed_queued_finish_same_uuid_credential_rotation_refuses_cleanup_and_ack()
{
    notification_managed_queued_finish_case(
        "dispatch_queued_rotation",
        QueuedDirectoryChange::Rotation,
    )
    .await;
}
#[tokio::test]
async fn notification_managed_queued_finish_expired_snapshot_refuses_cleanup_and_ack() {
    notification_managed_queued_finish_case(
        "dispatch_queued_expiry",
        QueuedDirectoryChange::Expiry,
    )
    .await;
}
#[tokio::test]
async fn notification_managed_queued_finish_evicted_snapshot_refuses_cleanup_and_ack() {
    notification_managed_queued_finish_case(
        "dispatch_queued_eviction",
        QueuedDirectoryChange::Eviction,
    )
    .await;
}

#[tokio::test]
async fn notification_managed_physical_pool_wait_rechecks_same_uuid_rotated_credentials_before_claim(
) {
    use crate::integration_tests::tenant_directory::{database_url, DirectoryServer, PhaseSignals};
    use school_tenancy::{PoolManager, PoolManagerConfig};
    use std::{sync::Arc, time::Duration};
    use tracing::instrument::WithSubscriber;
    let (fixture, recipient) = fixture("dispatch_physical_rotation").await;
    let queued = item(recipient);
    enqueue(&fixture, std::slice::from_ref(&queued)).await;
    let url = database_url(&fixture).await;
    let tenant_id = Uuid::new_v4();
    let directory = DirectoryServer::start(tenant_id, url.to_string(), false).await;
    let manager = Arc::new(PoolManager::with_config(PoolManagerConfig::for_tests(
        2, 1, 1,
    )));
    let mut first_url = url.clone();
    first_url
        .query_pairs_mut()
        .append_pair("application_name", "occupied-one");
    let first = manager
        .get_pool_without_migrations(first_url.as_str(), "occupied-one")
        .await
        .unwrap();
    let mut second_url = url.clone();
    second_url
        .query_pairs_mut()
        .append_pair("application_name", "occupied-two");
    let second = manager
        .get_pool_without_migrations(second_url.as_str(), "occupied-two")
        .await
        .unwrap();
    let state = school_notifications::runtime::NotificationRuntimeState {
        admin_client: directory.client(2, Duration::from_secs(30)),
        pool_manager: manager,
        notification_channel: tokio::sync::broadcast::channel(8).0,
        eligibility: &ELIGIBILITY,
    };
    let transport = Arc::new(ControlledBatchTransport {
        started: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
        device: (queued.id, queued.notification_id),
    });
    let (signals, dispatcher) = PhaseSignals::for_phase("pending");
    let worker_state = state.clone();
    let worker_transport = transport.clone();
    let worker = tokio::spawn(
        async move {
            school_notifications::runtime::test_support::deliver_managed_batch(
                &worker_state,
                "dispatch_physical_rotation",
                worker_transport.as_ref(),
            )
            .await
        }
        .with_subscriber(dispatcher),
    );
    tokio::time::timeout(Duration::from_secs(1), signals.physical_wait.notified())
        .await
        .unwrap();
    let token = Uuid::new_v4();
    state
        .admin_client
        .begin_directory_mutation("dispatch_physical_rotation", token)
        .unwrap();
    let mut rotated = url.clone();
    rotated
        .set_password(Some("synthetic-new-password"))
        .unwrap();
    directory.set_mapping(tenant_id, rotated.to_string());
    state
        .admin_client
        .finish_directory_mutation("dispatch_physical_rotation", token)
        .unwrap();
    state
        .admin_client
        .resolve_database_work("dispatch_physical_rotation")
        .await
        .unwrap();
    drop(first);
    assert!(tokio::time::timeout(Duration::from_secs(3), worker)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    let (pending, published): (bool, bool) = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM notification_presentations WHERE receipt_id=$1),presentation_consumed_at IS NOT NULL FROM notification_receipts WHERE id=$1",
    )
    .bind(queued.id)
    .fetch_one(&fixture)
    .await
    .unwrap();
    assert_eq!(
        pending, true,
        "second local check must reject before business claim SQL"
    );
    assert!(!published);
    assert_eq!(directory.calls(), 2);
    drop(second);
}
